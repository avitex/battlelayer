use std::io;

use super::SocketError;
use futures_channel::mpsc;
use tokio_executor::SpawnError;

#[derive(Debug)]
pub enum Error {
    Spawn(SpawnError),
    Socket(SocketError),
    Responder(mpsc::SendError),
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
