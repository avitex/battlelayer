use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::{self, BoxFuture};
use tower_service::Service;
use tower_util::BoxService;

use super::{respondable, Error, Request, Response};

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

    pub(crate) fn handle(
        &mut self,
        request: Request,
    ) -> BoxFuture<'static, Result<Response, Error>> {
        Box::pin(self.inner.call(request))
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self::new(DefaultHandler::default())
    }
}

impl<S> From<S> for Handler
where
    S: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    fn from(service: S) -> Self {
        Handler::new(service)
    }
}

#[derive(Default)]
pub struct DefaultHandler {
    pub response: Response,
}

impl Service<Request> for DefaultHandler {
    type Error = Error;
    type Response = Response;

    type Future = Pin<Box<future::Ready<Result<Response, Error>>>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _request: Request) -> Self::Future {
        Box::pin(future::ok(self.response.clone()))
    }
}

pub struct RespondableHandler {
    sender: respondable::Sender,
}

impl RespondableHandler {
    pub fn new() -> (Self, respondable::Receiver) {
        let (sender, receiver) = respondable::channel();
        (Self { sender }, receiver)
    }
}

impl Service<Request> for RespondableHandler {
    type Error = Error;
    type Response = Response;

    type Future = respondable::ResponseFuture;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.sender.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.sender.send(request)
    }
}
