use std::{net::SocketAddr, sync::{Arc, Mutex, atomic::AtomicBool}};
use tracing::{debug, info, error, warn, trace};
use tokio::{net::TcpStream, sync::{oneshot::{self, Receiver}, mpsc::UnboundedReceiver, mpsc::UnboundedSender}, select, io::{BufReader, AsyncBufReadExt, AsyncWriteExt}};

use crate::room::{Room, Message};


#[derive(Debug)]
pub struct Connection {
    pub stream: TcpStream,
    pub remote_address: SocketAddr,
    inbound_message_subscribers: Arc<Mutex<Vec<(UnboundedSender<Message>, oneshot::Sender<()>)>>>,
    outbound_message_rx: UnboundedReceiver<Message>,
    staging_message_sent: AtomicBool,
    staging_message_received: AtomicBool,
}


impl Connection {
    pub fn new(
        stream: TcpStream, 
        outbound_message_rx: UnboundedReceiver<Message>,
    ) -> Self {
        let remote_address = stream.peer_addr().unwrap();
        Self {
            stream,
            remote_address,
            inbound_message_subscribers: Default::default(),
            outbound_message_rx,
            staging_message_sent: AtomicBool::new(false),
            staging_message_received: AtomicBool::new(false)
        }
    }

    pub fn subscribe(&self) -> (UnboundedReceiver<Message>, Receiver<()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (on_disconnect_tx, on_disconnect_rx) = tokio::sync::oneshot::channel();
        {
            let mut guard = self.inbound_message_subscribers.lock().unwrap();
            guard.push((tx, on_disconnect_tx));
        }
        (rx, on_disconnect_rx)
    }

    #[tracing::instrument(skip_all, fields(message))]
    fn notify_subscribers(
        inbound_message_subscribers: Arc<Mutex<Vec<(UnboundedSender<Message>, oneshot::Sender<()>)>>>, 
        message: &Message
    ) {
        let mut guard = inbound_message_subscribers.lock().unwrap();
        let mut stale_subscribers = vec![];
        
        for (index, (subscriber, _)) in guard.iter().enumerate() {
            if let Err(err) = subscriber.send(message.clone()) {
                error!("Found a subscriber that we couldn't send a message to. Dropping the sender: {}", err);
                stale_subscribers.push(index);
            } else {
                trace!("Notified subscriber ({}) of a new inbound message.", index);
            }
        }
        if stale_subscribers.is_empty() {
            trace!("All subscribers of this connection are healthy...");
            return;
        }

        for index in stale_subscribers.into_iter().rev() {
            let (removed, on_disconnect) = guard.remove(index);
            trace!(removed = ?removed, "Removed stream subscriber.");
            trace!("Notifying of disconnection.");
            _ = on_disconnect.send(());
        }
    }

    fn notify_subscribers_of_failure(
        inbound_message_subscribers: Arc<Mutex<Vec<(UnboundedSender<Message>, oneshot::Sender<()>)>>>
    ) {
        let mut guard = inbound_message_subscribers.lock().unwrap();
        for index in (0..guard.len()).rev() {
            let (removed, on_disconnect) = guard.remove(index);
            trace!(removed = ?removed, "Removed stream subscriber.");
            trace!("Notifying of disconnection.");
            _ = on_disconnect.send(());
        }
    }

    pub async fn run(self) -> crate::Result<()> {

        let (read_half, mut write_half) = self.stream.into_split();
        let subscribers = self.inbound_message_subscribers.clone();
        let mut outbound_message_rx = self.outbound_message_rx;
        let staging_message_sent = self.staging_message_sent;
        let staging_message_received = self.staging_message_received;

        let reader = BufReader::new(read_half);
        let mut lines_from_reader = reader.lines();

        loop {
            select! {
                message = outbound_message_rx.recv() => match message {
                    Some(msg) => {
                        if msg.is_staging() && staging_message_sent.load(std::sync::atomic::Ordering::Relaxed) {
                            // Should only ever have to emit 1 staging message for a client.
                            // kinda hacky but it works so ¯\_(ツ)_/¯
                            continue;
                        }
                        else if msg.is_staging() {
                            // Set the flag that now we've sent a staging message.
                            staging_message_sent.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        write_half.write_all(msg.as_str().as_bytes()).await?;
                        write_half.write_all(b"\n").await?;
                    },
                    None => {
                        // error!("No outbound message senders remaining for the client. Dropping the connection.");
                        // Self::notify_subscribers_of_failure(Arc::clone(&subscribers));
                        // break;
                    }
                },
                maybe_line = lines_from_reader.next_line() => match maybe_line {
                    Ok(Some(line)) => {
                        trace!("Read line from peer: {}", line);
                        // We may only read one staging message.
                        let message = {
                            if staging_message_received.load(std::sync::atomic::Ordering::Relaxed) {
                                Message::Chat(line)
                            } else {
                                staging_message_received.store(true, std::sync::atomic::Ordering::Relaxed);
                                Message::Staging(line)
                            }
                        };

                        Self::notify_subscribers(Arc::clone(&subscribers), &message);
                    },
                    Ok(None) => {
                        warn!("No more lines to read from peer. ");
                        Self::notify_subscribers_of_failure(Arc::clone(&subscribers));
                        break;
                    },
                    Err(err) => {
                        error!("Failed to read a line from peer. {}", err);
                        Self::notify_subscribers_of_failure(Arc::clone(&subscribers));
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}