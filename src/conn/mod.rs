mod connection;
mod error;
mod handler;
mod packet;
pub mod respondable;
mod socket;

pub use self::connection::{Connection, ConnectionBuilder};
pub use self::error::Error;
pub use self::handler::{DefaultHandler, Handler, RespondableHandler};
pub use self::packet::*;
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
    pub body: Vec<PacketWord>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub body: Vec<PacketWord>,
}

impl Default for Response {
    fn default() -> Self {
        let ok = PacketWord::new("OK").unwrap();
        Self { body: vec![ok] }
    }
}
