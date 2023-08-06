use std::net::SocketAddr;
use tracing::{debug, info};
use tokio::net::TcpStream;


#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    remote_address: SocketAddr,
}


impl Connection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self {
            stream,
            remote_address: addr
        }
    }
    pub async fn run_to_completion(&self) -> crate::Result<()> {
        debug!("Accepted connection from {} {:#?}", self.remote_address, self.stream);
        Ok(())
    }
}