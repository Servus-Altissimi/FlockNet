use tokio::net::TcpStream;
use std::net::SocketAddr;

pub struct Connection {
    stream: TcpStream,
    addr: SocketAddr,
}

impl Connection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self { stream, addr }
    }
}
