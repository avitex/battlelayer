use std::collections::HashMap;
use std::task::{Context, Poll};

use futures_util::select;
use futures_util::stream::StreamExt;
use tokio_executor::{DefaultExecutor, Executor};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_net::{tcp::TcpStream, ToSocketAddrs};
use tower_service::Service;

use super::{outbound, Error, Handler, PacketOrigin, Request, Response, ResponseFuture, Socket};

pub struct Connection {
    sender: outbound::RequestSender,
}

impl Connection {
    fn from_sender(sender: outbound::RequestSender) -> Self {
        Self { sender }
    }

    pub fn send_request(&mut self, request: Request) -> ResponseFuture {
        self.sender.send(request)
    }
}

impl Service<Request> for Connection {
    type Response = Response;
    type Error = Error;
    type Future = ResponseFuture;

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
    origin: PacketOrigin,
    handler: Handler,
}

impl ConnectionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn origin(mut self, origin: PacketOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn handler(mut self, handler: Handler) -> Self {
        self.handler = handler;
        self
    }

    pub fn with_transport_and_exec<T, E>(self, transport: T, exec: E) -> Result<Connection, Error>
    where
        E: Executor,
        T: Send + AsyncRead + AsyncWrite + Unpin + 'static,
    {
        ConnectionProcess::new(transport).start(exec)
    }

    pub fn with_transport<T>(self, transport: T) -> Result<Connection, Error>
    where
        T: Send + AsyncRead + AsyncWrite + Unpin + 'static,
    {
        self.with_transport_and_exec(transport, DefaultExecutor::current())
    }

    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Connection, Error> {
        self.with_transport(TcpStream::connect(addr).await?)
    }
}

impl Default for ConnectionBuilder {
    fn default() -> Self {
        Self {
            handler: Default::default(),
            origin: PacketOrigin::Client,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

struct ConnectionProcess<T>
where
    T: AsyncRead + AsyncWrite,
{
    sock: Socket<T>,
    request_tx: Option<outbound::RequestSender>,
    request_rx: outbound::RequestReceiver,
    request_registry: HashMap<u32, ()>,
}

impl<T> ConnectionProcess<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(transport: T) -> Self {
        let (request_tx, request_rx) = outbound::RequestSender::new();
        Self {
            request_rx,
            request_tx: Some(request_tx),
            sock: Socket::new(transport),
            request_registry: HashMap::new(),
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
        let fut = async move { self.run().await };
        match exec.spawn(Box::pin(fut)) {
            Ok(_) => Ok(Connection::from_sender(request_tx)),
            Err(err) => Err(Error::Spawn(err)),
        }
    }

    async fn run(&mut self) {
        loop {
            // TODO
            select! {
                packet = self.sock.next() => {
                    dbg!(packet);
                },
                request = self.request_rx.next() => {
                    dbg!(request);
                },
            }
        }
    }
}
