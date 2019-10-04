use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future;
use tower_service::Service;
use tower_util::BoxService;

use super::error::Error;
use super::packet::*;

#[derive(Debug, Clone)]
pub struct Response {
    pub words: Vec<PacketWord>,
}

#[derive(Debug)]
pub struct Request {
    pub words: Vec<PacketWord>,
}

impl Default for Response {
    fn default() -> Self {
        let ok = PacketWord::new("OK").unwrap();
        Self { words: vec![ok] }
    }
}

#[derive(Debug)]
pub struct Handler {
    inner: BoxService<Request, Response, Error>,
}

impl Handler {
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<Request, Response = Response, Error = Error> + Send + 'static,
        S::Future: Send + 'static,
    {
        Self {
            inner: BoxService::new(inner),
        }
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self::new(DefaultHandlerService::default())
    }
}

#[derive(Default)]
pub struct DefaultHandlerService {
    pub response: Response,
}

impl Service<Request> for DefaultHandlerService {
    type Error = Error;
    type Response = Response;

    type Future = Pin<Box<future::Ready<Result<Response, Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: Request) -> Self::Future {
        Box::pin(future::ok(self.response.clone()))
    }
}
