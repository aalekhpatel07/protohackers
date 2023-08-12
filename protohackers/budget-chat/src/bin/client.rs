use tokio::{net::TcpStream, io::{AsyncWriteExt, AsyncBufReadExt, BufReader}, select};
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
    let (reader, mut writer) = stream.into_split();
    let reader = BufReader::new(reader);
    let mut lines_from_server = reader.lines();

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines_from_stdin = stdin.lines();

    loop {
        select! {
            line = lines_from_server.next_line() => match line {
                Ok(Some(line)) => {
                    println!("{}", line);
                },
                Ok(None) => break,
                Err(err) => {
                    error!("err when reading line from server: {}", err);
                    break;
                }
            },
            line = lines_from_stdin.next_line() => match line {
                Ok(Some(line)) => {
                    // eprintln!("Will send: {}", line);
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                },
                Ok(None) => break,
                Err(err) => {
                    error!("err when reading line from stdin: {}", err);
                    break;
                }
            }
        }
    }
    Ok(())
}