use std::io;

use super::SocketError;
use tokio_executor::SpawnError;

#[derive(Debug)]
pub enum Error {
    Spawn(SpawnError),
    Socket(SocketError),
    RequestFailed,
    RequestCancelled,
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Socket(SocketError::Io(err))
    }
}
