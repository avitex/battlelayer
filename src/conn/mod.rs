mod connection;
mod error;
mod handler;
mod outbound;
mod packet;
mod socket;

pub use self::connection::{Connection, ConnectionBuilder};
pub use self::error::Error;
pub use self::handler::Handler;
pub use self::outbound::ResponseFuture;
pub use self::packet::*;
pub use self::socket::{Socket, SocketError};

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
