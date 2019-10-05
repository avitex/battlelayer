use std::io;

use super::SocketError;
use tokio_executor::SpawnError;

#[derive(Debug)]
pub enum Error {
    Spawn(SpawnError),
    Socket(SocketError),
    InvalidSequence,
    OriginMismatch,
    RequestFailed,
    RequestCancelled,
}

impl From<SocketError> for Error {
    fn from(err: SocketError) -> Self {
        Self::Socket(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        SocketError::Io(err).into()
    }
}
