use std::io::{stdin, Cursor};

use tokio::{net::{TcpStream, TcpSocket}, io::{AsyncReadExt, AsyncWriteExt}};
use clap::Parser;
use tracing::{info, debug, warn, trace, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};


#[derive(Debug, Parser)]
pub struct Args {
    #[clap(short = 'p', long, default_value_t = 12003)]
    server_port: u16,
    #[clap(short = 'u', long, default_value_t = String::from("localhost"))]
    server_url: String
}


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "client=trace,budget_chat=trace,tokio=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();
    let args = Args::parse();

    let stream = TcpStream::connect((args.server_url, args.server_port)).await?;
    let (mut read_half, write_half) = stream.into_split();


    let read_handle = tokio::task::spawn(async move {
        loop {
            let mut dest = String::new();
            let Ok(bytes_read) = read_half.read_to_string(&mut dest).await else {
                error!("Failed to read bytes from stream. Disconnecting.");
                return;
            };
            if bytes_read == 0 {
                trace!("Received EOF from server. Not sure what to do about it..");
                break;
            }
            trace!("Read bytes {}", bytes_read);
            info!("{}", dest);
        }
    });

    let write_handle = tokio::task::spawn(async move {
        let mut write_half = write_half;
        loop {
            let mut contents = String::new();
            if let Ok(bytes_read) = stdin().read_line(&mut contents) {
                if bytes_read == 0 {
                    trace!("Received EOF from stdin. Disconnecting.");
                    break;
                }
                let mut cursor = Cursor::new(contents);
                if let Err(err) = write_half.write_all_buf(&mut cursor).await { 
                    trace!("Connection reset by server... Disconnecting. {}", err);
                    break;
                }
            }

        }
    });

    _ = tokio::join!(read_handle, write_handle);

    Ok(())
}