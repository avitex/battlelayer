mod codec;
mod error;
mod handler;
mod packet;

use std::future::Future;
use futures_util::future;
use futures_util::stream::{self, StreamExt};
use futures_util::try_stream::TryStreamExt;
use futures_channel::{oneshot, mpsc};
use tokio_io::{split::split, AsyncRead, AsyncWrite};
use tokio_codec::{FramedRead, FramedWrite};
use tokio_executor::{Executor, DefaultExecutor};
use tokio_net::{ToSocketAddrs, tcp::TcpStream};

use self::packet::*;
use self::codec::{PacketCodec, PacketCodecError};

pub use self::error::Error;
pub use self::handler::{Handler, Request, Response};

type OutboundRequest = (Request, oneshot::Sender<Response>);
type RequestReceiver = mpsc::UnboundedReceiver<OutboundRequest>;

#[derive(Debug, Clone)]
pub struct RequestSender {
    req_tx: mpsc::UnboundedSender<OutboundRequest>,
}

impl RequestSender {
    fn new() -> (Self, mpsc::UnboundedReceiver<OutboundRequest>) {
        let (req_tx, req_rx) = mpsc::unbounded();
        (Self { req_tx }, req_rx)
    }

    pub async fn request(&mut self, request: Request) -> Result<Response, Error> {
        let (res_tx, res_rx) = oneshot::channel();
        self.req_tx.unbounded_send((request, res_tx)).unwrap();
        res_rx.await.map_err(|_| Error::RequestCancelled)
    }
}

enum ConnectionEvent {
    Inbound {
        packet: Packet,
    },
    Outbound {
        request: Request,
        response_tx: oneshot::Sender<Response>
    },
}

pub struct Connection<T: AsyncRead + AsyncWrite> {
    req_sender: RequestSender,
    packet_tx: FramedWrite<T, PacketCodec>,
}

impl<T> Connection<T>
where
    T: AsyncRead + AsyncWrite,
{
    fn new(transport: T, mut req_rx: RequestReceiver, origin: PacketOrigin, handler: Handler) -> Result<(), Error> {
        let (transport_read, transport_write) = split(transport);
        let mut framed_read = FramedRead::new(transport_read, PacketCodec);
        let mut framed_write = FramedWrite::new(transport_write, PacketCodec);

        let outbound_stream = req_rx.map(|(request, response_tx)| {
            Ok(ConnectionEvent::Outbound {
                request,
                response_tx
            })
        });

        let inbound_stream = framed_read.map(|packet_res| {
            packet_res
                .map(|packet| ConnectionEvent::Inbound { packet })
                .map_err(|err| Error::Codec(err))
        });

        let fut = stream::select(inbound_stream, outbound_stream).try_for_each(|event| {
            match event {
                ConnectionEvent::Inbound { packet } => {
                    future::ready(Ok(()))
                },
                ConnectionEvent::Outbound { request, response_tx } => {
                    framed_write.send();
                    future::ready(Ok(()))
                },
            }
        });
    }

    pub fn request_sender() -> {}

    pub 
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Default)]
pub struct ConnectionBuilder {
    handler: Handler,
    executor: Option<Box<dyn Executor>>,
}

impl ConnectionBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Define the service for handling incoming requests.
    pub fn handler(mut self, handler: Handler) -> Self {
        self.handler = handler;
        self
    }

    pub fn executor<T>(mut self, exec: T) -> Self
    where
        T: Executor + 'static
    {
        self.executor = Some(Box::new(DefaultExecutor::current()));
        self
    }

    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> Result<Connection<TcpStream>, Error> {
        let transport = TcpStream::connect(addr).await?;
        Ok(Connection::start(transport, PacketOrigin::Client, self.handler))
    }
}

// impl ConnectionBuilder {
//     fn default() -> Self {
//         Self {
//             handler: Default::default(),
//             executor: None,
//         }
//     }
// }