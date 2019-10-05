use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::{mpsc, oneshot};
use futures_util::ready;

use super::{Error, Request, Response};

#[derive(Debug)]
pub struct OutboundRequest {
    req: Request,
    res: oneshot::Sender<Response>,
}

pub type RequestReceiver = mpsc::UnboundedReceiver<OutboundRequest>;

pub struct RequestSender {
    tx: mpsc::UnboundedSender<OutboundRequest>,
}

impl RequestSender {
    pub fn new() -> (Self, RequestReceiver) {
        let (tx, rx) = mpsc::unbounded();
        (Self { tx }, rx)
    }

    pub fn send(&mut self, request: Request) -> ResponseFuture {
        let (response_tx, response_rx) = oneshot::channel();
        let outbound_request = OutboundRequest {
            req: request,
            res: response_tx,
        };
        if self.tx.unbounded_send(outbound_request).is_ok() {
            ResponseFuture {
                rx: Some(response_rx),
            }
        } else {
            ResponseFuture { rx: None }
        }
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
