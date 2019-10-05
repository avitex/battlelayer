use battlelayer::conn::{ConnectionBuilder, PacketWord, Request};
use std::convert::TryFrom;

#[tokio::main]
async fn main() {
    let mut conn = ConnectionBuilder::new()
        .connect("109.200.214.230:25515")
        .await
        .unwrap();

    let req = Request {
        body: vec![PacketWord::try_from("serverInfo").unwrap()],
    };

    let res = conn.send_request(req).await;

    println!("Hello, world!");
}
