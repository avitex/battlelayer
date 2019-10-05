use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::{mpsc, oneshot};
use futures_util::ready;

use super::{Error, Request, Response};

pub type Receiver = mpsc::UnboundedReceiver<Respondable>;
pub type Responder = oneshot::Sender<Response>;

pub fn channel() -> (Sender, Receiver) {
    let (tx, rx) = mpsc::unbounded();
    (Sender { tx }, rx)
}

#[derive(Debug)]
pub struct Respondable {
    request: Request,
    responder: Responder,
}

impl Respondable {
    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn respond(self, response: Response) -> Result<(), Response> {
        self.responder.send(response)
    }

    pub fn split(self) -> (Request, Responder) {
        (self.request, self.responder)
    }
}

pub struct Sender {
    tx: mpsc::UnboundedSender<Respondable>,
}

impl Sender {
    pub fn send(&mut self, request: Request) -> ResponseFuture {
        let (response_tx, response_rx) = oneshot::channel();
        let responable = Respondable {
            request: request,
            responder: response_tx,
        };
        if self.tx.unbounded_send(responable).is_ok() {
            ResponseFuture {
                rx: Some(response_rx),
            }
        } else {
            ResponseFuture { rx: None }
        }
    }

    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
        self.tx.poll_ready(cx).map_err(|err| Error::Responder(err))
    }
}

pub struct ResponseFuture {
    rx: Option<oneshot::Receiver<Response>>,
}

impl Future for ResponseFuture {
    type Output = Result<Response, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.rx.as_mut() {
            None => Poll::Ready(Err(Error::RequestFailed)),
            Some(mut rx) => {
                let res = ready!(Pin::new(&mut rx).poll(cx));
                Poll::Ready(res.map_err(|_| Error::RequestCancelled))
            }
        }
    }
}
