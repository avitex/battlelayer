use std::collections::HashMap;
use std::task::{Context, Poll};

use futures_util::select;
use futures_util::future::{FutureExt, RemoteHandle};
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use tokio_executor::{DefaultExecutor, Executor};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_net::{tcp::TcpStream, ToSocketAddrs};
use tower_service::Service;

use super::{
    respondable, Error, Handler, Packet, PacketKind, PacketSequence, Request, Response,
    Respondable, Role, Socket, SocketError,
};

pub struct Connection {
    sender: respondable::Sender,
    process_handle: RemoteHandle<Result<(), Error>>,
}

impl Connection {
    pub fn send_request(&mut self, request: Request) -> respondable::ResponseFuture {
        self.sender.send(request)
    }

    pub fn finish(self) -> RemoteHandle<Result<(), Error>> {
        self.process_handle
    }
}

impl Service<Request> for Connection {
    type Response = Response;
    type Error = Error;
    type Future = respondable::ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.send_request(request)
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct ConnectionBuilder {
    handler: Handler,
}

impl ConnectionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handler<T: Into<Handler>>(mut self, handler: T) -> Self {
        self.handler = handler.into();
        self
    }

    pub fn with_transport_and_exec<T, E>(
        self,
        transport: T,
        role: Role,
        exec: E,
    ) -> Result<Connection, Error>
    where
        E: Executor,
        T: Send + AsyncRead + AsyncWrite + Unpin + 'static,
    {
        ConnectionProcess::new(transport, self.handler, role).start(exec)
    }

    pub fn with_transport<T>(self, transport: T, role: Role) -> Result<Connection, Error>
    where
        T: Send + AsyncRead + AsyncWrite + Unpin + 'static,
    {
        self.with_transport_and_exec(transport, role, DefaultExecutor::current())
    }

    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Connection, Error> {
        self.with_transport(TcpStream::connect(addr).await?, Role::Client)
    }
}

impl Default for ConnectionBuilder {
    fn default() -> Self {
        Self {
            handler: Default::default(),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

struct ConnectionProcess<T>
where
    T: AsyncRead + AsyncWrite,
{
    next_seq: u32,
    role: Role,
    sock: Socket<T>,
    handler: Handler,
    request_tx: Option<respondable::Sender>,
    request_rx: respondable::Receiver,
    pending_requests: HashMap<u32, respondable::Responder>,
}

impl<T> ConnectionProcess<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(transport: T, handler: Handler, role: Role) -> Self {
        let (request_tx, request_rx) = respondable::channel();
        Self {
            role,
            handler,
            request_rx,
            next_seq: 0,
            request_tx: Some(request_tx),
            sock: Socket::new(transport),
            pending_requests: HashMap::new(),
        }
    }

    pub fn start<E>(mut self, mut exec: E) -> Result<Connection, Error>
    where
        E: Executor,
    {
        let request_tx = self
            .request_tx
            .take()
            .expect("connection process started more than once");
        let (process_fut, process_handle) = async move { self.run().await }.remote_handle();
        match exec.spawn(Box::pin(process_fut)) {
            Ok(()) => Ok(Connection { process_handle, sender: request_tx }),
            Err(err) => Err(Error::Spawn(err)),
        }
    }

    async fn handle_incoming_packet(&mut self, packet: Packet) -> Result<(), Error> {
        if packet.seq.kind() == PacketKind::Request {
            // TODO: Are BF4 servers compliant with their own standard?
            // if packet.seq.origin() == self.role {
            //     return Err(Error::OriginMismatch);
            // }
            // Build the request for the handler.
            let request = Request { body: packet.words };
            // Get the response built by handler.
            let response = self.handler.handle(request).await?;
            // Build the response packet.
            let response_seq = PacketSequence::new(PacketKind::Response, packet.seq.origin(), packet.seq.number())
                .map_err(|_| Error::InvalidSequence)?;
            let response_packet = Packet::new(response_seq, response.body);
            // Send it braz
            self.sock.send(response_packet).await?;
            Ok(())
        } else {
            if packet.seq.origin() != self.role {
                return Err(Error::OriginMismatch);
            }
            let responder = self
                .pending_requests
                .remove(&packet.seq.number())
                .ok_or(Error::InvalidSequence)?;
            let response = Response { body: packet.words };
            // Ignore errors here.
            let _ = responder.send(response);
            Ok(())
        }
    }

    async fn handle_outgoing_request(
        &mut self,
        outbound_request: Respondable,
    ) -> Result<(), Error> {
        let (request, reponder) = outbound_request.split();
        // Get next sequence number
        let seq_num = self.next_seq;
        self.next_seq += 1;
        // Build the packet
        let seq = PacketSequence::new(PacketKind::Request, self.role, seq_num)
            .map_err(|_| Error::InvalidSequence)?;
        let packet = Packet::new(seq, request.body);
        // Send it braz
        self.sock.send(packet).await?;
        // Add the responder to the queue
        self.pending_requests.insert(seq_num, reponder);
        Ok(())
    }

    async fn run(&mut self) -> Result<(), Error> {
        loop {
            select! {
                sock_res = self.sock.next() => {
                    let packet = sock_res.unwrap_or(Err(SocketError::Closed))?;
                    self.handle_incoming_packet(packet).await?;
                },
                outbound_request_opt = self.request_rx.next() => {
                    // TODO: better error..
                    let outbound_request = outbound_request_opt.ok_or(SocketError::Closed)?;
                    self.handle_outgoing_request(outbound_request).await?;
                },
            }
        }
    }
}