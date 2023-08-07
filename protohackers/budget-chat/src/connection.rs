use std::{net::SocketAddr, sync::{Arc, Mutex}};
use tracing::{debug, info, error};
use tokio::{net::TcpStream, sync::oneshot};

use crate::room::{Room, Message};


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
    pub async fn run_to_completion(
        self, 
        room: Arc<Room>,
    ) -> crate::Result<()> {

        debug!("Accepted connection from {}", self.remote_address);
        // if let Err(err) = room.acknowledge_member(
        //     self.stream
        // ).await {
        //     error!("Failed to acknowledge member: {}", err);
        //     return Ok(());
        // }


        Ok(())
    }
    pub fn get_stream<'conn>(&'conn self) -> &'conn TcpStream {
        &self.stream
    }
}