use std::net::SocketAddr;
use tracing::{info, error, warn, trace};

use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};


#[derive(Debug)]
pub struct EchoServer {
    pub port: u16,
}

impl Default for EchoServer {
    fn default() -> Self {
        Self {
            port: 12000
        }
    }
}

impl EchoServer {
    pub fn loopback_addr(&self) -> SocketAddr {
        ([0; 8], self.port).into()
    }


    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        
        let listener = TcpListener::bind(self.loopback_addr()).await?;
        info!("EchoServer listening on {}", self.loopback_addr());

        loop {
            let (stream, addr) = listener.accept().await?;
            tokio::task::spawn(async move {
                Self::handle_connection(stream, addr).await;
            });
        }
    }

    pub async fn handle_connection(
        stream: TcpStream,
        client_addr: SocketAddr,
    ) {
        let (mut read_half, mut write_half) = stream.into_split();
        info!("Accepted connection from {}", client_addr);

        let mut buf = [0u8; 1024];

        loop {
            let Ok(bytes_read) = read_half.read(&mut buf).await else {
                error!("Failed to read from read half");
                break;
            };
            trace!("Read {} bytes from client.", bytes_read);

            if bytes_read == 0 {
                trace!("Received EOF from client {}. Terminating connection.", client_addr);
                break;
            }

            let mut current_offset = 0;
            loop {
                match write_half.write(&mut buf[current_offset..bytes_read]).await {
                    Ok(bytes_written) => {
                        if bytes_written == (bytes_read - current_offset) {
                            trace!("Wrote all bytes ({}) to client successfully.", bytes_read);
                            break;
                        } else {
                            trace!("Wrote some bytes ({}) to client successfully.", bytes_written);
                            current_offset += bytes_written;
                        }
                    },
                    Err(err) => {
                        warn!("Failed to write bytes to client. Will keep retrying... {}", err);
                    }
                }
            }
        }

        let mut stream = read_half.reunite(write_half).expect("failed to reunite.");
        if let Err(err) = stream.flush().await {
            error!("failed to flush stream for {}: {}", client_addr, err);
        }
        if let Err(err) = stream.shutdown().await {
            error!("failed to shutdown stream: {}", err);
        } else {
            info!("Closed connection from {}", client_addr);
        }
    }

}
