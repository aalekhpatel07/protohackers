use std::{net::SocketAddr, sync::{Arc, Mutex}, io::Cursor, collections::HashMap};

use bytes::Buf;
use futures::{stream::{futures_unordered, FuturesUnordered}, StreamExt};
use tokio::{net::{TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}}, sync::mpsc, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{error, trace, warn};

use crate::MemberID;



#[derive(Debug)]
pub struct InitialTransport {
    read_half: OwnedReadHalf,
    write_half: OwnedWriteHalf,
    addr: SocketAddr,
    send_to_potential_member_rx: mpsc::UnboundedReceiver<String>,
    recvd_from_potential_member_tx: mpsc::UnboundedSender<Option<String>>,
    anonymous_client_disconnected_signal_tx: mpsc::UnboundedSender<()>,
    staging_failed_rx: mpsc::UnboundedReceiver<()>
}

impl InitialTransport {
    pub fn new(
        stream: TcpStream, 
        addr: SocketAddr,
        send_to_potential_member_rx: mpsc::UnboundedReceiver<String>,
        recvd_from_potential_member_tx: mpsc::UnboundedSender<Option<String>>,
        anonymous_client_disconnected_signal_tx: mpsc::UnboundedSender<()>,
        staging_failed_rx: mpsc::UnboundedReceiver<()>
    ) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            read_half,
            write_half,
            addr,
            send_to_potential_member_rx,
            recvd_from_potential_member_tx,
            anonymous_client_disconnected_signal_tx,
            staging_failed_rx
        }
    }

    pub async fn run(self) -> crate::Result<TcpStream> {

        let mut send_to_potential_member_rx = self.send_to_potential_member_rx;
        let recvd_from_potential_member_tx = self.recvd_from_potential_member_tx;
        let anonymous_client_disconnected_signal_tx = self.anonymous_client_disconnected_signal_tx;
        let mut read_half = self.read_half;
        let mut write_half = self.write_half;
        let addr = self.addr;
        let mut staging_failed_rx = self.staging_failed_rx;

        loop {
            tokio::select! {
                Some(_) = staging_failed_rx.recv() => {
                    error!("Recvd staging_failed.");
                    break;
                },
                res = Self::read_message(
                    addr,
                    &mut read_half,
                    anonymous_client_disconnected_signal_tx.clone(),
                    recvd_from_potential_member_tx.clone()
                ) => {
                    if let Err(err) = res {
                        error!("Read message err in init phase: {}", err);
                        break;
                    }
                },
                res = Self::write_message(
                    addr,
                    &mut write_half,
                    anonymous_client_disconnected_signal_tx.clone(),
                    &mut send_to_potential_member_rx
                ) => {
                    if let Err(err) = res {
                        error!("Write message err in init phase: {}", err);
                        break;
                    }
                }
            }
        }
        
        Ok(read_half.reunite(write_half)?)
    }

    pub async fn write_message(
        addr: SocketAddr,
        write_half: &mut OwnedWriteHalf,
        anonymous_client_disconnected_signal_tx: mpsc::UnboundedSender<()>,
        send_to_potential_member_rx: &mut mpsc::UnboundedReceiver<String>
    ) -> crate::Result<()> {

        loop {
            if let Some(msg) = send_to_potential_member_rx.recv().await {

                let contents = msg.as_bytes();
                let len = contents.len();
                let mut cursor = Cursor::new(contents);

                // For cancellation-safety, we'll keep trying until we can't write anymore.

                loop {
                    if !cursor.has_remaining() {
                        break;
                    }
                    match write_half.write_buf(&mut cursor).await {
                        Ok(bytes_written) => {
                            trace!("Wrote {}/{} bytes to client: {}", bytes_written, len, addr);
                        },
                        Err(err) => {
                            warn!("Failed to write bytes in init phase. Dropping connection.");
                            anonymous_client_disconnected_signal_tx.send(()).unwrap();
                            return Err(err.into());
                        }
                    }
                }
            }
        }
    }

    pub async fn read_message(
        addr: SocketAddr,
        read_half: &mut OwnedReadHalf,
        anonymous_client_disconnected_signal_tx: mpsc::UnboundedSender<()>,
        recvd_from_potential_member_tx: mpsc::UnboundedSender<Option<String>>,
    ) -> crate::Result<()> {
        
        let mut dest = String::new();
        match read_half.read_to_string(&mut dest).await {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    // warn!(client = %addr.to_string(), "Read EOF.. Sending None");
                    // recvd_from_potential_member_tx.send(None).unwrap();
                } else {
                    trace!(client = %addr.to_string(), "Read {} bytes from client.", bytes_read);
                    recvd_from_potential_member_tx.send(Some(dest)).unwrap();
                }
            },
            Err(err) => {
                error!("Failed to read message in initialization phase. We'll silently drop connection: {}", err);
                anonymous_client_disconnected_signal_tx.send(()).unwrap();
                return Err(crate::ClientInitializationError::ConnectionResetByClient.into());
            }
        }

        Ok(())
    }


}


#[derive(Debug)]
pub struct RoomTransport {
    read_halves: Arc<tokio::sync::Mutex<HashMap<MemberID, OwnedReadHalf>>>,
    write_halves: Arc<tokio::sync::Mutex<HashMap<MemberID, OwnedWriteHalf>>>,
    client_disconnected_signal_tx: mpsc::UnboundedSender<MemberID>,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Option<String>)>,
    send_to_member_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<(MemberID, String)>>>,
    register_stream_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<TcpStream>>>,
    deregister_stream_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<MemberID>>>,
    deregister_stream_tx: mpsc::UnboundedSender<MemberID>,
}


impl RoomTransport {

    pub fn new(
        client_disconnected_signal_tx: mpsc::UnboundedSender<MemberID>,
        message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Option<String>)>,
        send_to_member_rx: mpsc::UnboundedReceiver<(MemberID, String)>,
        register_stream_rx: mpsc::UnboundedReceiver<TcpStream>,
        deregister_stream_rx: mpsc::UnboundedReceiver<MemberID>,
        deregister_stream_tx: mpsc::UnboundedSender<MemberID>,
    ) -> Self {
        Self {
            client_disconnected_signal_tx,
            message_recvd_from_member_tx,
            send_to_member_rx: Arc::new(tokio::sync::Mutex::new(send_to_member_rx)),
            register_stream_rx: Arc::new(tokio::sync::Mutex::new(register_stream_rx)),
            deregister_stream_rx: Arc::new(tokio::sync::Mutex::new(deregister_stream_rx)),
            deregister_stream_tx,
            read_halves: Default::default(),
            write_halves: Default::default()
        }
    }

    pub async fn register_stream(&self) -> crate::Result<()> {
        loop {
            if let Some(stream) = self.register_stream_rx.lock().await.recv().await {
                let (read_half, write_half) = stream.into_split();
                let addr = read_half.peer_addr()?;
                
                self.read_halves.lock().await.insert(addr, read_half);
                self.write_halves.lock().await.insert(addr, write_half);
            }
        }
    }
    pub async fn deregister_stream(&self) -> crate::Result<()> {
        loop {
            if let Some(member_id) = self.deregister_stream_rx.lock().await.recv().await {
                self.read_halves.lock().await.remove(&member_id);
                self.write_halves.lock().await.remove(&member_id);
            }
        }
    }

    pub async fn run(&self) -> crate::Result<()> {

        let message_recvd_from_member_tx = &self.message_recvd_from_member_tx;
        let client_disconnected_tx = &self.client_disconnected_signal_tx;
        trace!("About to lock send_to_member_rx");
        let mut send_to_member_rx = self.send_to_member_rx.lock().await;
        let deregister_stream_tx = &self.deregister_stream_tx;
        // We'll lock/unlock a bit too much
        // but how else should we let new clients subscribe
        // for polling?
        loop {
            trace!("Transport running...");

            tokio::select! {
                Ok(_) = Self::read_message(
                    self.read_halves.clone(),
                    message_recvd_from_member_tx,
                    client_disconnected_tx,
                    deregister_stream_tx
                ) => {
                    trace!("Read message in transport.");
                },
                _ = Self::write_message(
                    self.write_halves.clone(),
                    &mut send_to_member_rx,
                    client_disconnected_tx,
                    deregister_stream_tx
                ) => {
                    trace!("Wrote message in transport.");
                }
            }
        }
    }

    pub async fn write_message(
        write_halves: Arc<tokio::sync::Mutex<HashMap<MemberID, OwnedWriteHalf>>>,
        send_to_member_rx: &mut mpsc::UnboundedReceiver<(MemberID, String)>,
        client_disconnected_signal_tx: &mpsc::UnboundedSender<MemberID>,
        deregister_stream_tx: &mpsc::UnboundedSender<MemberID>,
    ) -> crate::Result<()> {
        
        loop {
            if let Some((recipient, message)) = send_to_member_rx.recv().await {
                let mut guard = write_halves.lock().await;
                let Some(write_half) = guard.get_mut(&recipient) else {
                    return Err::<(), crate::BudgetChatError>(crate::BudgetChatError::UnknownMember(recipient.to_string()));
                };

                let contents = message.as_bytes();
                trace!(recipient = %recipient, "Want to send message {:#?}", message);
                let len = contents.len();
                let mut cursor = Cursor::new(contents);

                // For cancellation-safety, we'll keep trying until we can't write anymore.
                loop {
                    if !cursor.has_remaining() {
                        break;
                    }
                    match write_half.write_buf(&mut cursor).await {
                        Ok(bytes_written) => {
                            trace!("Wrote {}/{} bytes to client: {}", bytes_written, len, recipient);
                        },
                        Err(err) => {
                            warn!("Failed to write bytes in init phase. Dropping connection.");
                            client_disconnected_signal_tx.send(recipient).unwrap();
                            deregister_stream_tx.send(recipient).unwrap();

                            return Err(err.into());
                        }
                    }
                }
            }
        }
    }

    pub async fn read_message(
        read_halves: Arc<tokio::sync::Mutex<HashMap<MemberID, OwnedReadHalf>>>,
        message_recvd_from_member_tx: &mpsc::UnboundedSender<(MemberID, Option<String>)>,
        client_disconnected_signal_tx: &mpsc::UnboundedSender<MemberID>,
        deregister_stream_tx: &mpsc::UnboundedSender<MemberID>,
    ) -> crate::Result<()> {

        let mut read_all_messages = FuturesUnordered::default();
        let mut guard = read_halves.lock().await;
        for mut read_half in guard.values_mut() {
            read_all_messages.push(
                Self::read_message_single(
                    &mut read_half, 
                    message_recvd_from_member_tx, 
                    client_disconnected_signal_tx,
                    deregister_stream_tx
                )
            );
        }


        if let Some(Ok(member_id)) = read_all_messages.next().await {
            trace!(peer = %member_id, "Recvd a message from a client.");
            return Ok(())
        } else {
            return Err(crate::BudgetChatError::NoMsgsToRead)
        }

    }

    async fn read_message_single(
        read_half: &mut OwnedReadHalf,
        message_recvd_from_member_tx: &mpsc::UnboundedSender<(MemberID, Option<String>)>,
        client_disconnected_signal_tx: &mpsc::UnboundedSender<MemberID>,
        deregister_stream_tx: &mpsc::UnboundedSender<MemberID>,
    ) -> crate::Result<MemberID> {
        let mut dest = String::new();
        let addr = read_half.peer_addr()?;
        match read_half.read_to_string(&mut dest).await {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    trace!(client = %addr.to_string(), "Read EOF.. Sending None");
                    message_recvd_from_member_tx.send((addr, None)).unwrap();
                } else {
                    trace!(client = %addr.to_string(), "Read {} bytes from client.", bytes_read);
                    message_recvd_from_member_tx.send((addr, Some(dest))).unwrap();
                }
            },
            Err(err) => {
                error!("Failed to read message of an established client. Disconnecting: {}", err);
                client_disconnected_signal_tx.send(addr).unwrap();
                deregister_stream_tx.send(addr).unwrap();
            }
        }
        Ok(addr)
    }


}