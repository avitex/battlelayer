use std::collections::HashMap;
use std::convert::TryInto;
use std::task::{Context, Poll};

use futures_util::future::{BoxFuture, FutureExt, RemoteHandle};
use futures_util::select;
use futures_util::sink::SinkExt;
use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio_executor::{DefaultExecutor, Executor};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_net::{tcp::TcpStream, ToSocketAddrs};
use tower_service::Service;

use super::{
    respondable, Body, BodyError, Error, Handler, Packet, PacketKind, PacketSequence, Request,
    Respondable, Response, Role, Socket, SocketError, Word,
};

pub struct Connection {
    sender: respondable::Sender,
    process_handle: RemoteHandle<Result<(), Error>>,
}

impl Connection {
    pub async fn exec<C>(&mut self, command: C) -> Result<Vec<Word>, Error>
    where
        C: TryInto<Body, Error = BodyError>,
    {
        let body = command.try_into()?;
        let request = Request { body };
        let response = self.send_request(request).await?;
        Ok(response.body.to_vec())
    }

    pub fn finish(self) -> RemoteHandle<Result<(), Error>> {
        self.process_handle
    }

    fn send_request(&mut self, request: Request) -> respondable::ResponseFuture {
        self.sender.send(request)
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

type PendingResponseResult = Result<(PacketSequence, Response), Error>;

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
    pending_responses: FuturesUnordered<BoxFuture<'static, PendingResponseResult>>,
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
            pending_responses: FuturesUnordered::new(),
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
            Ok(()) => Ok(Connection {
                process_handle,
                sender: request_tx,
            }),
            Err(err) => Err(Error::Spawn(err)),
        }
    }

    async fn handle_incoming_packet(&mut self, packet: Packet) -> Result<(), Error> {
        if packet.seq.kind() == PacketKind::Request {
            let packet_seq = packet.seq;
            let packet_words = packet.words;
            // TODO: Are BF4 servers compliant with their own standard?
            // if packet.seq.origin() == self.role {
            //     return Err(Error::OriginMismatch);
            // }
            // Build the request for the handler.
            let request = Request {
                body: packet_words.into(),
            };
            // Get the response built by handler.
            let response_fut = self.handler.handle(request);
            let response_fut = async move { Ok((packet_seq, response_fut.await?)) };
            let boxed_response_fut = Box::pin(response_fut);
            // Push to the queue.
            self.pending_responses.push(boxed_response_fut);
            Ok(())
        } else {
            if packet.seq.origin() != self.role {
                return Err(Error::OriginMismatch);
            }
            let responder = self
                .pending_requests
                .remove(&packet.seq.number())
                .ok_or(Error::InvalidSequence)?;
            let response = Response {
                body: packet.words.into(),
            };
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
        let packet = Packet::new(seq, request.body.to_vec());
        // Send it braz
        self.sock.send(packet).await?;
        // Add the responder to the queue
        self.pending_requests.insert(seq_num, reponder);
        Ok(())
    }

    async fn handle_outgoing_response(
        &mut self,
        outbound_response: (PacketSequence, Response),
    ) -> Result<(), Error> {
        let (request_seq, response) = outbound_response;
        // Build the response packet.
        let response_seq = PacketSequence::new(
            PacketKind::Response,
            request_seq.origin(),
            request_seq.number(),
        )
        .map_err(|_| Error::InvalidSequence)?;
        let response_packet = Packet::new(response_seq, response.body.to_vec());
        // Send it braz
        Ok(self.sock.send(response_packet).await?)
    }

    async fn run(&mut self) -> Result<(), Error> {
        loop {
            select! {
                sock_res = self.sock.next() => {
                    let packet = sock_res.unwrap_or(Err(SocketError::Closed))?;
                    self.handle_incoming_packet(packet).await?;
                },
                outbound_request_opt = self.request_rx.next() => {
                    let outbound_request = outbound_request_opt.ok_or(SocketError::Closed)?;
                    self.handle_outgoing_request(outbound_request).await?;
                },
                outbound_response_opt = self.pending_responses.next() => {
                    if let Some(outbound_response_res) = outbound_response_opt {
                        self.handle_outgoing_response(outbound_response_res?).await?
                    }
                },
            }
        }
    }
}
