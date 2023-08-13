use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    select,
    sync::{
        mpsc::UnboundedReceiver,
        mpsc::UnboundedSender,
        oneshot::{self, Receiver},
    },
};
use tracing::{error, trace, warn};

use crate::{room::Message, Shared};

/// Keep the actual sender handle around, as well as a disconnect signal sender,
/// in case any of the underlying IO failed when we didn't expect it to.
type SubscriberHandle = (UnboundedSender<Message>, oneshot::Sender<()>);

/// A thin wrapper around a [TcpStream], a collection of send handles
/// for broadcast inbound messages to any interested subscribers, and a
/// source of outbound messages to write to the stream.
#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    remote_address: SocketAddr,
    inbound_message_subscribers: crate::Shared<Vec<SubscriberHandle>>,
    outbound_message_rx: UnboundedReceiver<Message>,
}

impl Connection {
    /// Thinly wrap a stream, and a channel to write outgoing messages to this client.
    pub fn new(stream: TcpStream, outbound_message_rx: UnboundedReceiver<Message>) -> Self {
        let remote_address = stream.peer_addr().unwrap();
        Self {
            stream,
            remote_address,
            inbound_message_subscribers: Default::default(),
            outbound_message_rx,
        }
    }

    /// Get a receiver handle to get notified of any messages received on this connection,
    /// as well as tracking if the peer went away.
    pub fn subscribe(&self) -> (UnboundedReceiver<Message>, Receiver<()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (on_disconnect_tx, on_disconnect_rx) = tokio::sync::oneshot::channel();
        {
            let mut guard = self.inbound_message_subscribers.lock().unwrap();
            guard.push((tx, on_disconnect_tx));
        }
        (rx, on_disconnect_rx)
    }

    /// We made some progress because we just read a message, notify all our subscribers
    /// about this message.
    #[tracing::instrument(skip_all, fields(message))]
    fn notify_subscribers(
        inbound_message_subscribers: crate::Shared<Vec<SubscriberHandle>>,
        message: &Message,
    ) {
        let mut guard = inbound_message_subscribers.lock().unwrap();
        let mut stale_subscribers = vec![];

        for (index, (subscriber, _)) in guard.iter().enumerate() {
            // Stale handles could be collected and dropped so we don't keep retrying.
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

        // Remove in reverse order to not break the ordering of the elements since we're popping them
        // based on indices.
        for index in stale_subscribers.into_iter().rev() {
            let (removed, on_disconnect) = guard.remove(index);
            trace!(removed = ?removed, "Removed stream subscriber.");
            trace!("Notifying of disconnection.");
            // Notifying of disconnection could fail but only if the subscriber itself
            // didn't care to be notified (i.e. dropped the receiver before receiving the signal).
            _ = on_disconnect.send(());
        }
    }

    /// We failed on some IO, so tell all our subscribers
    /// about it, and drop all the sender handles we carried
    /// with us. This way, if the subscriber was only listening to
    /// us, then it'll stop receiving messages, thus closing it.
    fn notify_subscribers_of_failure(inbound_message_subscribers: Shared<Vec<SubscriberHandle>>) {
        let mut guard = inbound_message_subscribers.lock().unwrap();
        for index in (0..guard.len()).rev() {
            let (removed, on_disconnect) = guard.remove(index);
            trace!(removed = ?removed, "Removed stream subscriber.");
            trace!("Notifying of disconnection.");
            _ = on_disconnect.send(());
        }
    }

    /// Drive this connection by writing any outbound messages to the client,
    /// and when incoming messages are received on the underlying stream,
    /// the contents are broadcasted to all our subscribers.
    #[tracing::instrument(skip(self), fields(peer))]
    pub async fn run(self) -> crate::Result<()> {
        let (read_half, mut write_half) = self.stream.into_split();
        let peer = self.remote_address;

        tracing::Span::current().record("peer", peer.to_string());

        let subscribers = self.inbound_message_subscribers.clone();
        let mut outbound_message_rx = self.outbound_message_rx;

        let reader = BufReader::new(read_half);
        let mut lines_from_reader = reader.lines();

        loop {
            select! {
                message = outbound_message_rx.recv() => if let Some(msg) = message {
                    write_half.write_all(msg.as_bytes()).await.unwrap();
                    write_half.write_all(b"\n").await.unwrap();
                },
                maybe_line = lines_from_reader.next_line() => match maybe_line {
                    Ok(Some(line)) => {
                        trace!("Read line from peer: {}", line);
                        Self::notify_subscribers(subscribers.clone(), &line);
                    },
                    Ok(None) => {
                        trace!("No more lines to read from peer. ");
                        Self::notify_subscribers_of_failure(subscribers.clone());
                        break;
                    },
                    Err(err) => {
                        error!("Failed to read a line from peer. {}", err);
                        Self::notify_subscribers_of_failure(subscribers.clone());
                        break;
                    }
                },
            }
        }
        Ok(())
    }
}
