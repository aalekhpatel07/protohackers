use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::io::{
    AsyncBufReadExt,
    AsyncWriteExt,
    BufReader
};
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tracing::{error, debug};
use crate::message::Request;
use crate::message::Response;


#[derive(Debug)]
pub struct Connection {
    pub read_half: OwnedReadHalf,
    pub write_half: OwnedWriteHalf,
    pub request_tx: UnboundedSender<Request>,
    pub response_rx: UnboundedReceiver<Response>
}

impl Connection {
    pub fn new(stream: TcpStream, request_tx: UnboundedSender<Request>, response_rx: UnboundedReceiver<Response>) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            read_half,
            write_half,
            request_tx,
            response_rx
        }
    }

    async fn write<T>(write_half: &mut OwnedWriteHalf, data: T) 
    where
        T: serde::Serialize
    {
        let serialized = serde_json::to_vec(&data).unwrap();
        let bytes_to_write = serialized.len();
        let mut bytes_written_so_far = 0;

        loop {
            match write_half.write(&serialized).await {
                Ok(bytes_written) => {
                    bytes_written_so_far += bytes_written;
                },
                Err(err) => {
                    error!("Failed to write bytes to connection: {}", err);
                    break;
                }
            }

            if bytes_written_so_far == bytes_to_write {
                break;
            }
        }
    }

    async fn read(mut read_half: OwnedReadHalf, request_tx: &UnboundedSender<Request>) {
        let reader = BufReader::new(&mut read_half);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let request: Request = 
                serde_json::from_str(&line)
                .unwrap_or(Request::Invalid);
            request_tx.send(request).unwrap();
        }
    }

    pub async fn handle(self) {
        let mut response_rx = self.response_rx;
        let mut write_half = self.write_half;

        let write_handle = tokio::task::spawn(async move {
            while let Some(response) = response_rx.recv().await {
                Self::write(&mut write_half, response).await;
            }
        });

        let read_half = self.read_half;
        let request_tx = self.request_tx;

        let read_handle = tokio::task::spawn(async move {
            Self::read(read_half, &request_tx).await;
        });

        let (write_res, read_res) = tokio::join!(write_handle, read_handle);

        if let Err(err) = write_res {
            error!("Failed to write to connection: {}", err);
        }

        if let Err(err) = read_res {
            error!("Failed to read from connection: {}", err);
        }

        debug!("Connection closed");
    }
}