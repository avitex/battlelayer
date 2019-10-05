use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;
use futures_util::ready;
use futures_util::sink::Sink;
use futures_util::stream::{FusedStream, Stream};
use tokio_codec::{Decoder, Encoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};

use super::packet::{read_packet, write_packet, Packet, PacketError};

pub struct Socket<T: AsyncRead + AsyncWrite> {
    inner: Framed<T, PacketCodec>,
    broken: bool,
}

impl<T> Socket<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner: Framed::new(inner, PacketCodec),
            broken: false,
        }
    }

    fn get_pinned_inner(&mut self) -> Result<Pin<&mut Framed<T, PacketCodec>>, SocketError> {
        if self.is_terminated() {
            Err(SocketError::Broken)
        } else {
            Ok(Pin::new(&mut self.inner))
        }
    }
}

impl<T> Stream for Socket<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Item = Result<Packet, SocketError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<Packet, SocketError>>> {
        let res = match self.get_pinned_inner() {
            Ok(inner) => ready!(inner.poll_next(cx)),
            Err(_) => return Poll::Ready(None),
        };
        if res.as_ref().map_or(false, Result::is_err) {
            self.broken = true;
        }
        Poll::Ready(res)
    }
}

impl<T> FusedStream for Socket<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.broken
    }
}

impl<T> Sink<Packet> for Socket<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    type Error = SocketError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        match self.get_pinned_inner() {
            Ok(inner) => inner.poll_ready(cx),
            Err(err) => Poll::Ready(Err(err)),
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.get_pinned_inner()
            .and_then(|inner| inner.start_send(item))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        match self.get_pinned_inner() {
            Ok(inner) => inner.poll_flush(cx),
            Err(err) => Poll::Ready(Err(err)),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        match self.get_pinned_inner() {
            Ok(inner) => inner.poll_close(cx),
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum SocketError {
    Broken,
    Closed,
    Io(io::Error),
    Packet(PacketError),
}

impl From<io::Error> for SocketError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<PacketError> for SocketError {
    fn from(err: PacketError) -> Self {
        Self::Packet(err)
    }
}

///////////////////////////////////////////////////////////////////////////////

struct PacketCodec;

impl Encoder for PacketCodec {
    type Item = Packet;
    type Error = SocketError;

    fn encode(&mut self, packet: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        Ok(write_packet(buf, packet)?)
    }
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = SocketError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(read_packet(buf)?)
    }
}
