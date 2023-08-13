use std::net::{SocketAddr};
use tracing::{debug, error, info, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::{sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, net::{TcpListener, TcpStream}, select, io::{BufReader, AsyncBufReadExt, AsyncWriteExt, stdin}};



struct ProxyBudgetChat {
    outbound_message_rx: UnboundedReceiver<String>,
    inbound_message_tx: UnboundedSender<String>
}

impl ProxyBudgetChat {

    const BOGUSCOIN_ADDRESS: &'static str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";

    pub fn transform_message(message: &str) -> String {
        message.into()
    }

    pub async fn run(&mut self) -> Result<()> {

        let stream = TcpStream::connect("chat.protohackers.com:16963").await?;
        let (reader_half, mut writer_half) = stream.into_split();

        let reader = BufReader::new(reader_half);
        let mut lines = reader.lines();

        loop {
            select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {

                    },
                    Ok(None) => {

                    },
                    Err(err) => {

                    }
                },
                message = self.outbound_message_rx.recv() => match message {
                    Some(message) => {
                        writer_half.write_all(message.as_bytes()).await?;
                        writer_half.write_all(b"\n").await?;
                    },
                    None => {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}


pub type Result<T, E = Box<dyn std::error::Error + Send + Sync + 'static>> = core::result::Result<T, E>;


pub async fn run_server(port: u16) -> Result<()> {
    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await.unwrap();
    loop {
        let (socket, peer) = listener.accept().await.unwrap();
        tokio::task::spawn(async move { handle_client(socket, peer).await });
    }

}

pub async fn handle_client(
    socket: TcpStream,
    peer: SocketAddr
) -> Result<()> {

    let (inbound_tx, inbound_rx) = tokio::sync::mpsc::unbounded_channel();
    let (outbound_tx, outbound_rx) = tokio::sync::mpsc::unbounded_channel();

    let stdin = stdin();
    let stdin_reader = BufReader::new(stdin);
    let mut lines_from_stdin = stdin_reader.lines();

    tokio::task::spawn(async move {
        while let Ok(Some(line)) = lines_from_stdin.next_line().await {
            outbound_tx.send(line);
        }
    });


    let proxy = ProxyBudgetChat { outbound_message_rx: outbound_rx, inbound_message_tx: inbound_tx };

    Ok(())
}



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mob_in_the_middle=info,tokio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

}
