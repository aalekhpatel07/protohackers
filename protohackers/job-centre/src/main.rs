use job_centre::connection::Connection;
use job_centre::message::Request;
use job_centre::message::Response;
use job_centre::supervisor::Message;
use job_centre::supervisor::Supervisor;

use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};



pub async fn run_server(
    port: u16,
    supervisor_request_tx: &UnboundedSender<Message<Request>>,
    peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Response>>>>
) {

    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("Listening on {}", addr);

    loop {
        let (stream, peer) = listener.accept().await.unwrap();

        let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel();
        let (response_tx, response_rx) = tokio::sync::mpsc::unbounded_channel();

        peer_map.lock().unwrap().insert(peer, response_tx);
        info!("Ready to send responses to {}", peer);
        let supervisor_request_tx = supervisor_request_tx.clone();

        // These are inbound requests from client.
        // request_rx
        tokio::task::spawn(async move {
            while let Some(request) = request_rx.recv().await {
                supervisor_request_tx.send((peer, request)).unwrap();
            }
        });


        let connection = Connection::new(stream, request_tx, response_rx);
        tokio::task::spawn(async move {
            connection.handle().await;
        });
    }
}



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "job_center=debug,tokio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    let (supervisor_request_tx, supervisor_request_rx) = tokio::sync::mpsc::unbounded_channel();
    let (supervisor_response_tx, mut supervisor_response_rx) = tokio::sync::mpsc::unbounded_channel();

    let supervisor = Supervisor::new(supervisor_request_rx, supervisor_response_tx);

    let peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Response>>>> = Default::default();
    let peer_map_cp = peer_map.clone();

    tokio::task::spawn(async move {
        while let Some((peer_id, response)) = supervisor_response_rx.recv().await {
            if let Some(peer) = peer_map.lock().unwrap().get(&peer_id) {
                peer.send(response).unwrap();
            } else {
                tracing::error!("Failed to find peer for sending response.");
            }
        }
    });

    tokio::task::spawn(async move {
        supervisor.run().await;
    });
    run_server(20019, &supervisor_request_tx, peer_map_cp).await;
}
