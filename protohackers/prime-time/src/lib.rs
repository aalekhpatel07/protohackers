use std::{net::SocketAddr, io::Cursor};
use tracing::{info, error, warn, trace, debug};

use tokio::{net::{TcpStream, TcpListener}, io::{AsyncReadExt, AsyncWriteExt}};
use bytes::{BytesMut, Buf};

use self::data::IsPrimeResponse;



#[derive(Debug)]
struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
    received_eof: bool,
}


impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(4096),
            received_eof: false,
        }
    }

    pub async fn read_frame(&mut self) -> Result<Option<data::IsPrimeRequest>, PrimeTimeError> {
        let span = tracing::trace_span!("read_frame", ?self.buffer);
        loop {
            match self.parse_frame() {
                Ok(Some(frame)) => {
                    return Ok(Some(frame));
                },
                Ok(None) => {
                    span.in_scope(|| {
                        trace!("Not enough data to parse. Waiting for more...");
                    });
                },
                Err(PrimeTimeError::Serde(serde_err)) => {
                    span.in_scope(|| {
                        error!(category = ?serde_err.classify(), "Serde error: {}", serde_err);
                    });
                    if serde_err.is_data() || serde_err.is_syntax() {
                        return Err(PrimeTimeError::Serde(serde_err));
                    }
                    if serde_err.is_eof() && !self.buffer.is_empty() && self.received_eof {
                        return Err(PrimeTimeError::Serde(serde_err));
                    }
                },
                _ => {}
            }
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?; 
            trace!("Bytes read: {}, buffer: {:#?}", bytes_read, self.buffer);

            if bytes_read == 0 {
                self.received_eof = true;
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                // If there is still data buffered and we got an EOF, it probably means we couldn't convert it to 
                // valid request. Just treat it as malformed request.
                if let Err(err) = self.parse_frame() {
                    warn!("We still had something buffered when we got an EOF and we couldn't parse it: {}", err);
                    self.write_frame(None).await?;
                    return Ok(None);
                }

                return Err(
                    std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset, 
                        "client sent some partial bytes that we couldn't make sense of and then reset the connection before sending a complete frame."
                    ).into()
                );
            }
        }
    }

    pub fn parse_frame(&mut self) -> Result<Option<data::IsPrimeRequest>, PrimeTimeError> {
        let mut buf = Cursor::new(&self.buffer[..]);
        if self.buffer.is_empty() {
            return Ok(None);
        }

        match data::IsPrimeRequest::check(&mut buf) {
            Ok(_) => {
                let len = buf.position() as usize;
                buf.set_position(0);
                let frame = data::IsPrimeRequest::parse(&mut buf)?;
                self.buffer.advance(len);
                self.buffer.advance(1); // Skip the newline.
                Ok(Some(frame))
            },
            Err(err) => {
                Err(err)
            }
        }
    }

    pub async fn write_frame(&mut self, frame: Option<data::IsPrimeResponse>) -> std::io::Result<()> {
        match frame {
            Some(response) => {
                let as_bytes = serde_json::to_vec(&response)?;
                self.stream.write_all(&as_bytes).await?;
            },
            None => {
                // Write buncha random corrupt data.
                self.stream.write_all(&[1, 2]).await?;
            }
        }
        self.stream.write(&[b'\n']).await?;
        Ok(())
    }
}

pub mod math {
    pub const TOLERANCE: f64 = 1e-6;

    pub fn is_prime_f64(number: f64) -> bool {
        if (number.floor() - number).abs() > TOLERANCE {
            return false;
        }
        is_prime(number.floor() as u64)
    }

    pub fn is_prime(number: u64) -> bool {
        let _start_time = std::time::Instant::now();
        if matches!(number, 2 | 3 | 5 | 7 | 11) {
            return true;
        }
        if matches!(number, 0 | 1 | 4 | 6 | 8 | 9 | 10) {
            return false;
        }
        let start = 2;
        let end = ((number as f64).sqrt().ceil() + 1.0) as u64;
        

        // tracing::trace!("Checked primality of {} (prime: {}) in {:#?}", number, result, start_time.elapsed());

        (start..=end)
        .all(|divisor| number % divisor != 0)
    }
}


#[derive(Debug)]
pub struct PrimeTime {
    pub listener: TcpListener
}

#[derive(Debug)]
struct Handler {
    connection: Connection,
    remote_addr: SocketAddr,
}

impl Handler {
    pub async fn run(&mut self) -> Result<(), PrimeTimeError> {
        let span = tracing::trace_span!("Connection", remote_addr=self.remote_addr.to_string());
        loop {
            match self.connection.read_frame().await {
                Ok(Some(frame)) => {
                    span.in_scope(|| {
                        trace!(malformed = frame.is_malformed(), frame = ?frame);
                    });
                    if frame.is_malformed() {
                        self.connection.write_frame(None).await?;
                        self.connection.stream.flush().await?;
                        self.connection.stream.shutdown().await?;
                        break;
                    }
                    let prime_response = IsPrimeResponse {
                        prime: math::is_prime_f64(frame.number),
                        method: "isPrime".to_string()
                    };
                    span.in_scope(|| {
                        trace!(request = ?frame, response = ?prime_response);
                    });
                    self.connection.write_frame(Some(prime_response)).await?;
                },
                Ok(None) => {
                    span.in_scope(|| {
                        debug!("No frame found... EOF?");
                    });
                    break;
                },
                Err(err) => {
                    match err {
                        PrimeTimeError::Serde(serde_err) => {
                            span.in_scope(|| {
                                debug!(
                                    err_category = ?serde_err.classify(),
                                    serde_err = ?serde_err,
                                    "Will treat this as a malformed request. Found serde error: {}", 
                                    serde_err
                                );
                            });

                            if serde_err.is_data() || serde_err.is_syntax() {
                                self.connection.write_frame(None).await?;
                                break;
                            }
                            if serde_err.is_eof() && !self.connection.buffer.is_empty() && self.connection.received_eof {
                                self.connection.write_frame(None).await?;
                                break;
                            }
                        },
                        _ => {
                            return Err(err);
                        }
                    }
                }
            }
        }
        debug!("Finished handling connection from {}", self.remote_addr);
        Ok(())
    }
}

impl PrimeTime {
    pub fn new(listener: TcpListener) -> Self {
        Self {
            listener
        }
    }

    pub async fn run(&mut self) -> Result<(), PrimeTimeError> {
        info!("Accepting inbound connections at {:#?}.", self.listener.local_addr()?);
        loop {
            let (socket, remote_addr) = self.listener.accept().await?;
            debug!("Accepted connection from {}", remote_addr);
            let mut handler = Handler {
                connection: Connection::new(socket),
                remote_addr
            };
            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!(cause =? err, "connection error");
                }
            });
        }
    }

}


#[derive(Debug, thiserror::Error)]
pub enum PrimeTimeError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("Received Malformed data: {0:#?}")]
    Malformed(Vec<u8>),
    #[error(transparent)]
    Serde(#[from] serde_json::Error)
}

pub mod data {
    use std::io::Cursor;
    use serde::Deserialize;

    use super::{PrimeTimeError, math::TOLERANCE};


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

    impl IsPrimeRequest {
        pub fn check(buffer: &mut Cursor<&[u8]>) -> Result<(), PrimeTimeError> {
            // trace!("Checking buffer: {:#?}", buffer);
            let mut de = serde_json::Deserializer::from_reader(buffer);
            Self::deserialize(&mut de)?;
            Ok(())
        }
        pub fn parse(buffer: &mut Cursor<&[u8]>) -> Result<Self, PrimeTimeError> {
            let mut de = serde_json::Deserializer::from_reader(buffer);
            let request = Self::deserialize(&mut de)?;
            Ok(request)
        }
        pub fn is_malformed(&self) -> bool {
            self.method != "isPrime" || (self.number.floor() - self.number).abs() > TOLERANCE
        }
    }

}
