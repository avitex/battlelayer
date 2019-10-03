mod codec;
mod error;

use std::future::Future;
use std::task::{Context, Poll};
use std::pin::Pin;

use futures_util::future;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_net::{tcp::TcpStream, ToSocketAddrs};

use tower_service::Service;
use tower_util::BoxService;

use self::error::Error;
use crate::packet::*;

pub use self::codec::PacketCodec;
pub use self::error::Error as ClientError;

#[derive(Debug, Default)]
pub struct DefaultService {
    pub response: Response,
}

impl Service<Request> for DefaultService {
    type Response = Response;
    type Error = Error;
    type Future = Pin<Box<future::Ready<Result<Response, Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        Pin::new(Box::new(future::ok(self.response.clone())))
    }
}

#[derive(Debug)]
pub struct Request {
    words: Vec<PacketWord>,
}

#[derive(Debug, Clone)]
pub struct Response {
    words: Vec<PacketWord>,
}

impl Default for Response {
    fn default() -> Self {
        let ok = PacketWord::new("OK").unwrap();
        Self { words: vec![ok] }
    }
}

impl Default for ConnectionBuilder {
    fn default() -> Self {
        let service = BoxService::new(DefaultService::default());
        Self { service }
    }
}

#[derive(Debug)]
pub struct ConnectionBuilder {
    service: BoxService<Request, Response, Error>,
}

impl ConnectionBuilder {
    //pub new() -> Self {
    //     Default::default()
    //}

    /// Define the service for handling incoming requests.
    pub fn service<S>(mut self, service: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Error> + Send + 'static,
        S::Future: Send + 'static,
    {
        self.service = BoxService::new(service);
        self
    }

    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Connection<TcpStream>, Error> {
        let transport = TcpStream::connect(addr).await?;
        Ok(Connection {
            transport,
            origin: PacketOrigin::Client,
        })
    }
}

pub struct Connection<T> {
    origin: PacketOrigin,
    transport: T,
}

impl<T> Connection<T> where T: AsyncRead + AsyncWrite {}
