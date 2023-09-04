use std::{net::SocketAddr, sync::{Mutex, Arc}};

use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tracing::debug;

use crate::message::{Request, Response};


pub type Message<T> = (SocketAddr, T);


#[derive(Debug)]
pub struct Supervisor {
    /// An inbound message from some peer.
    pub request_rx: UnboundedReceiver<Message<Request>>,
    /// An outbound message to some peer.
    pub response_tx: UnboundedSender<Message<Response>>,
    pub app_state: Arc<Mutex<crate::state::AppState>>
}

impl Supervisor {
    pub fn new(request_rx: UnboundedReceiver<Message<Request>>, response_tx: UnboundedSender<Message<Response>>) -> Self {
        Self {
            request_rx,
            response_tx,
            app_state: Default::default()
        }
    }

    pub async fn run(mut self) {
        while let Some((peer, request)) = self.request_rx.recv().await {
            debug!("Received request from peer ({}): {:?}", peer, request);
            let response = self.handle_request(&peer, &request);
            self.response_tx.send((peer, response)).unwrap();
        }
    }

    pub fn handle_request(&self, peer: &SocketAddr, request: &Request) -> Response {
        self.app_state.lock().unwrap().process_request(peer, request)
    }


}