use tokio_net::{tcp::TcpStream, ToSocketAddrs};

use crate::conn::{self, Connection, ConnectionBuilder};

pub struct Client {
    conn: Connection<TcpStream>,
}

impl Client {
    fn from_conn(conn: Connection<TcpStream>) -> Self {
        Self { conn }
    }
}

///////////////////////////////////////////////////////////////////////////////

pub struct ClientBuilder {
    builder: ConnectionBuilder,
}

// impl ClientBuilder {
//     pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client, conn::Error> {
//         let conn = ConnectionBuilder::new().connect(addr).await?;
//         Ok(Client::from_conn(conn))
//     }
// }
