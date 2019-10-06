mod body;
mod connection;
mod error;
mod handler;
mod socket;

pub mod packet;
pub mod respondable;

pub use self::body::{Body, BodyError, Word};
pub use self::connection::{Connection, ConnectionBuilder};
pub use self::error::Error;
pub use self::handler::{DefaultHandler, Handler, RespondableHandler};
pub use self::packet::{Packet, PacketKind, PacketSequence};
pub use self::respondable::Respondable;
pub use self::socket::{Socket, SocketError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Role {
    /// Indicates a server role.
    Server,
    /// Indicates a client role.
    Client,
}

#[derive(Debug)]
pub struct Request {
    pub body: Body,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub body: Body,
}

impl Default for Response {
    fn default() -> Self {
        let body = Body::new(vec!["OK"]).unwrap();
        Self { body }
    }
}
