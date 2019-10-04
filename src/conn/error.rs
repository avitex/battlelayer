use std::io;
use super::codec::PacketCodecError;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Codec(PacketCodecError),
    RequestCancelled,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
