use bytes::BytesMut;
use std::io;
use tokio_codec::{Decoder, Encoder};

use crate::packet::*;

pub struct PacketCodec;

pub enum PacketCodecError {
    Io(io::Error),
    Packet(PacketError),
}

impl From<io::Error> for PacketCodecError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<PacketError> for PacketCodecError {
    fn from(err: PacketError) -> Self {
        Self::Packet(err)
    }
}

impl Encoder for PacketCodec {
    type Item = Packet;
    type Error = PacketCodecError;

    fn encode(&mut self, packet: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        Ok(write_packet(buf, packet)?)
    }
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = PacketCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(read_packet(buf)?)
    }
}
