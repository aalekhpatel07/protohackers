use std::net::{SocketAddr};
use tracing::{debug, error, info, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::{sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, net::{TcpListener, TcpStream}, select, io::{BufReader, AsyncBufReadExt, AsyncWriteExt, stdin, Stdout, stdout}};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref BOGUSCOIN_REGEX: regex::Regex = regex::Regex::new(r#"\b7\w{26,35}\b"#).unwrap();
}


#[derive(Debug)]
struct ProxyBudgetChat {
    outbound_message_rx: UnboundedReceiver<String>,
    inbound_message_tx: UnboundedSender<String>,
    name_ceremony_complete: bool,
}

impl ProxyBudgetChat {

    const BOGUSCOIN_ADDRESS: &'static str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";

    #[tracing::instrument]
    pub fn transform_message(message: &str) -> String {
        debug!("transforming message raw: {:?}", message);
        let skip_chars: isize = message.find("] ").map(|x| x as isize).unwrap_or(-1) + 1;
        let mut msg = message.to_string();

        let (prefix_to_keep, message_to_transform) = msg.split_at_mut(skip_chars as usize);
        trace!("transforming message cleaned: {:?}", message_to_transform);

        let replaced = BOGUSCOIN_REGEX.replace_all(&message_to_transform, Self::BOGUSCOIN_ADDRESS).to_string();
        trace!("transformed cleaned: {}", replaced);

        let joined = format!("{}{}", prefix_to_keep, replaced);
        debug!("transformed: {}", joined);

        joined
    }

    fn is_chat_message(message: &str) -> bool {
        message.starts_with("[")
    }

    pub async fn run(&mut self) -> Result<()> {

        let stream = TcpStream::connect("chat.protohackers.com:16963").await?;
        let (reader_half, mut writer_half) = stream.into_split();

        let reader = BufReader::new(reader_half);
        let mut lines = reader.lines();

        loop {
            select! {
                line = lines.next_line() => match line {
                    Ok(Some(mut line)) => {
                        if self.name_ceremony_complete && Self::is_chat_message(&line) {
                            line = Self::transform_message(&line);
                        }
                        self.inbound_message_tx.send(line).unwrap();
                    },
                    Ok(None) => {
                        break;
                    },
                    Err(err) => {
                        break;
                    }
                },
                message = self.outbound_message_rx.recv() => match message {
                    Some(mut message) => {
                        if self.name_ceremony_complete {
                            message = Self::transform_message(&message);
                        }
                        writer_half.write_all(message.as_bytes()).await?;
                        writer_half.write_all(b"\n").await?;
                        self.name_ceremony_complete = true;
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
        let (socket, _) = listener.accept().await.unwrap();
        tokio::task::spawn(async move { handle_client(socket).await });
    }

}

pub async fn handle_client(
    socket: TcpStream,
) -> Result<()> {

    let (messages_from_client_tx, messages_from_client_rx) = tokio::sync::mpsc::unbounded_channel();
    let (messages_to_client_tx, mut messages_to_client_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let (reader_half, mut writer_half) = socket.into_split();
    let reader = BufReader::new(reader_half);
    let mut lines_from_client = reader.lines();

    let t1 = tokio::task::spawn(async move {
        loop {
            select! {
                line = lines_from_client.next_line() => match line {
                    Ok(Some(line)) => {
                        messages_from_client_tx.send(line).unwrap();
                    },
                    Ok(None) => {
                        break;
                    },
                    Err(err) => {
                        break;
                    }
                },
                message = messages_to_client_rx.recv() => match message {
                    Some(message) => {
                        let Ok(_) = writer_half.write_all(message.as_bytes()).await else {
                            break;
                        };
                        let Ok(_) = writer_half.write_all(b"\n").await else {
                            break;
                        };
                    },
                    None => {
                        break;
                    }
                }
            }
        }
    });

    let mut proxy = ProxyBudgetChat { 
        outbound_message_rx: messages_from_client_rx, 
        inbound_message_tx: messages_to_client_tx,
        name_ceremony_complete: false,
    };

    _ = tokio::join!(proxy.run(), t1);

    Ok(())
}



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mob_in_the_middle=trace,tokio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    run_server(12003).await.unwrap();
}
