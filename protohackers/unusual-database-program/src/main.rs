use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{channel, Sender};
use tracing::{debug, error, info, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Default, Clone)]
pub struct KeyValueStore {
    store: std::collections::HashMap<String, String>,
}

impl KeyValueStore {
    pub fn insert(&mut self, key: &str, value: &str) -> Option<String> {
        self.store.insert(key.to_string(), value.to_string());
        None
    }
    pub fn retrieve(&self, key: &str) -> Option<String> {
        self.store
            .get(key)
            .map(|value| format!("{}={}", key, value))
    }
}

#[derive(Debug)]
pub enum Operation {
    Insert { key: String, value: String },
    Retrieve { key: String },
}

impl Operation {
    pub fn is_forbidden(&self) -> bool {
        match self {
            Self::Insert { key, .. } => key == "version",
            _ => false,
        }
    }
}

impl From<String> for Operation {
    fn from(v: String) -> Self {
        match v.find('=') {
            Some(index) => {
                let (key, value) = v.split_at(index);
                Operation::Insert {
                    key: key.to_string(),
                    value: value.chars().skip(1).collect(),
                }
            }
            None => Operation::Retrieve { key: v },
        }
    }
}

pub type Response = Option<String>;
pub type Request = (Operation, Sender<Response>);

async fn handle_operations(
    store: std::sync::Arc<std::sync::Mutex<KeyValueStore>>,
    mut operations_rx: UnboundedReceiver<Request>,
) {
    loop {
        if let Some((operation, response_tx)) = operations_rx.recv().await {
            let mut guard = store.lock().unwrap();
            let op_debug = format!("{:#?}", operation);
            let response = match operation {
                Operation::Insert { key, value } => guard.insert(key.as_str(), value.as_str()),
                Operation::Retrieve { key } => guard.retrieve(&key),
            };
            info!("Sending result of processing {}: {:?}", op_debug, response);
            response_tx.send(response).unwrap();
        }
    }
}

async fn run_server(
    operations_tx: UnboundedSender<Request>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = ([0; 8], port).into();
    let socket = std::sync::Arc::new(UdpSocket::bind(addr).await?);

    let mut buf = [0; 1024];
    let socket_recv = socket.clone();

    info!("Listening for datagrams on: {}", addr);
    loop {
        if let Ok((bytes_read, remote_addr)) = socket_recv.recv_from(&mut buf).await {
            let mut contents = String::from_utf8_lossy(&buf[0..bytes_read]).to_string();
            if let Some(contents_trimmed) = contents.strip_suffix('\n') {
                contents = contents_trimmed.to_string();
            }
            trace!(bytes_read = %bytes_read, contents = %contents, "Received data from peer.");

            let operation: Operation = contents.into();
            debug!(peer = %remote_addr, "Received Operation: {:#?}", operation);

            if operation.is_forbidden() {
                error!(operation = ?operation, "Forbidden operation. Ignoring...");
                continue;
            }

            let socket_send = socket.clone();
            let (tx, rx) = channel();

            operations_tx.send((operation, tx)).unwrap();

            tokio::task::spawn(async move {
                match rx.await {
                    Ok(Some(maybe_response)) => {
                        debug!(peer = %remote_addr, "Received result of processing an operation for {}", remote_addr);
                        socket_send
                            .send_to(maybe_response.as_bytes(), remote_addr)
                            .await
                            .unwrap();
                    }
                    Ok(None) => {
                        debug!(peer = %remote_addr, "Request was processed but we chose to not send a response back.");
                    }
                    Err(err) => {
                        error!(peer = %remote_addr, "Error waiting for a response for the request of {}\n {}", remote_addr, err);
                    }
                }
            });
        }
    }
}

#[tokio::main]

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "unusual_database_program=info,tokio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let store: std::sync::Arc<std::sync::Mutex<KeyValueStore>> = Default::default();

    {
        let mut guard = store.lock().unwrap();
        guard.insert("version", "A rusty key-value store 1.0");
    }

    let (operations_tx, operations_rx) = tokio::sync::mpsc::unbounded_channel::<Request>();

    let operations_handle = tokio::task::spawn(handle_operations(store, operations_rx));

    _ = tokio::join!(operations_handle, run_server(operations_tx, 12002));

    Ok(())
}
