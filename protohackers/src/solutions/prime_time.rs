use std::net::SocketAddr;
use tracing::{info, error, warn, trace};

use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};
use std::io::BufReader;


#[derive(Debug)]
pub struct PrimeTime {
    pub port: u16,
}

pub mod data {

    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    pub struct IsPrimeRequest {
        pub method: String,
        pub number: f64
    }

    #[derive(Debug, serde::Serialize, Clone)]
    pub struct IsPrimeResponse {
        pub method: String,
        pub prime: bool
    }

}




impl Default for PrimeTime {
    fn default() -> Self {
        Self {
            port: 12001
        }
    }
}

pub const TOLERANCE: f64 = 1e-16;

impl PrimeTime {
    pub fn loopback_addr(&self) -> SocketAddr {
        ([0; 8], self.port).into()
    }

    pub fn is_prime_f64(number: f64) -> bool {
        if (number.floor() - number).abs() > TOLERANCE {
            return false;
        }
        Self::is_prime(number.floor() as u64)
    }

    pub fn is_prime(number: u64) -> bool {
        let start = 2;
        let end = ((number as f64).sqrt().ceil() + 1.0) as u64;
        (start..=end)
        .all(|divisor| number % divisor != 0)
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        
        let listener = TcpListener::bind(self.loopback_addr()).await?;
        info!("PrimeTime listening on {}", self.loopback_addr());

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

        'conn: loop {
            let Ok(bytes_read) = read_half.read(&mut buf).await else {
                error!("Failed to read from read half");
                break;
            };

            if bytes_read == 0 {
                trace!("Received EOF from client {}. Terminating connection.", client_addr);
                break;
            }

            let mut current_offset = 0;
            loop {
                if current_offset >= bytes_read {
                    break;
                }
                let mut buf_reader = BufReader::new(&buf[current_offset..bytes_read]);
                match serde_json::from_reader::<_, data::IsPrimeRequest>(&mut buf_reader) {
                    Ok(request) => {

                        if &request.method != "isPrime" {
                            warn!("Method is not `isPrime`. Treating this as a malformed request...");
                            if let Err(write_err) = write_half.write(&[1, 2, b'\n']).await {
                                error!("failed to send malformed response: {}", write_err);
                            }
                            break 'conn;
                        }

                        let size = serde_json::to_vec(&request).unwrap().len();
                        current_offset += size;
                        if current_offset < bytes_read {
                            if buf[current_offset] == b'\n' {
                                current_offset += 1;
                            }
                            else {
                                warn!("Not newline terminated. Treating this as a malformed request... :sus:");
                                if let Err(write_err) = write_half.write(&[1, 2, b'\n']).await {
                                    error!("failed to send malformed response: {}", write_err);
                                }
                                break 'conn;
                            }
                        }

                        let is_prime = Self::is_prime_f64(request.number);
                        let response = data::IsPrimeResponse {
                            prime: is_prime,
                            method: "isPrime".to_string()
                        };

                        let response_bytes = serde_json::to_vec(&response).unwrap();
                        if let Err(err) = write_half.write_all(&response_bytes).await {
                            error!("Failed to write conforming response: {}", err);
                        }
                        if let Err(err) = write_half.write_all(&[b'\n']).await {
                            error!("Failed to write newline: {}", err);
                        }
                    },
                    Err(err) => {
                        error!("Received unparseable data from client. Sending malformed data and terminating connection... {}", err);
                        if bytes_read == buf.len() {
                            warn!("We filled the entire buffer up. Might be an incomplete frame?");
                        }
                        // Some random data, who cares, its malformed anyway.
                        if let Err(write_err) = write_half.write(&[1, 2, b'\n']).await {
                            error!("failed to send malformed response: {}", write_err);
                        }
                        break 'conn;
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