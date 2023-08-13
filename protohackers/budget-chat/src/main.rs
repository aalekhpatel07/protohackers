use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::UnboundedSender, io::{BufReader, AsyncBufReadExt, AsyncWriteExt},
};
use budget_chat::{
    connection::Connection,
    room::{Message, Room},
    MemberID,
};
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[clap(short, long)]
    port: u16,
}

#[tokio::main]
async fn main() -> budget_chat::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "budget_chat=info,tokio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    run_server(args.port).await.unwrap();
    Ok(())
}

/// 
pub async fn run_server(port: u16) -> budget_chat::Result<()> {
    let (message_received_from_member_tx, message_received_from_member_rx) =
        tokio::sync::mpsc::unbounded_channel();
    let (send_to_member_tx, mut send_to_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_disconnected_tx, client_disconnected_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_connected_with_name_tx, client_connected_with_name_rx) =
        tokio::sync::mpsc::unbounded_channel();

    let outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>> =
        Default::default();

    let outbound_peer_map_cp = outbound_peer_map.clone();
    // Any outbound messages from the chat room need to be sent to the corresponding
    // transport handle, so let's do that in background.
    tokio::task::spawn(async move {
        loop {
            match send_to_member_rx.recv().await {
                Some((peer, message)) => {
                    let guard = outbound_peer_map_cp.lock().unwrap();
                    let sender = guard.get(&peer).unwrap();
                    sender.send(message).unwrap();
                }
                None => {
                    debug!("Got None in send_to_member_rx. Dropping sender handle.");
                    break;
                }
            }
        }
    });

    // Setup our Chat Room with the appropriate channels.
    let mut room = Room::new(
        message_received_from_member_rx,
        send_to_member_tx,
        client_disconnected_rx,
        client_connected_with_name_rx,
    );
    // Drive our chat room in the background, so it only needs to wait for the members
    // and messages to be thrown around in the channels.
    let room_handle = tokio::task::spawn(async move {
        room.run().await;
    });

    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Listening for connections on {}", listener.local_addr()?);

    // Wait until the room as well as the main client loop is complete.
    let (room_result, client_loop_result) = tokio::join!(
        room_handle,
        client_loop(
            listener,
            message_received_from_member_tx.clone(),
            client_connected_with_name_tx,
            client_disconnected_tx,
            outbound_peer_map.clone()
        )
    );

    room_result.unwrap();
    client_loop_result.unwrap();

    Ok(())
}

/// Start listening for connections, and spawn individual
/// client handlers.
pub async fn client_loop(
    listener: TcpListener,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Message)>,
    client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_disconnected_tx: mpsc::UnboundedSender<MemberID>,
    outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>,
) -> budget_chat::Result<()> {
    debug!("Entering client_loop...");
    loop {
        let (socket, addr) = listener.accept().await?;
        debug!(peer = %addr, "Accepted connection");

        let message_recvd_from_member_tx = message_recvd_from_member_tx.clone();
        let client_disconnected_tx = client_disconnected_tx.clone();
        let client_connected_with_name_tx = client_connected_with_name_tx.clone();
        let outbound_peer_map = outbound_peer_map.clone();

        tokio::task::spawn(async move {
            if let Err(err) = handle_client(
                socket,
                addr,
                message_recvd_from_member_tx,
                client_connected_with_name_tx,
                client_disconnected_tx,
                outbound_peer_map,
            )
            .await
            {
                error!("Handling client failed: {}", err);
            }
        });
    }
}


/// Try to complete the name-giving ceremony (aka "staging")
/// for a TCP-connected peer and if everything goes okay, return the stream
/// back as is, along with the name the peer chose.
#[tracing::instrument(fields(kind = "staging"))]
pub async fn stage_client(
    socket: TcpStream,
    addr: MemberID
) -> budget_chat::Result<(String, TcpStream)> {
    let (read_half, mut write_half) = socket.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();

    // Try writing. If we fail, drop the client.
    trace!("Requesting client for a name.");
    write_half.write_all(b"Welcome to budgetchat! What shall I call you?\n").await?;

    let Some(ref name) = lines.next_line().await? else {
        warn!("Could not read line from client when we were expecting a name. Disconnecting.");
        return Err(budget_chat::ClientInitializationError::ConnectionResetByClient.into());
    };

    trace!("Just read line: {}", name);
    let Some(cleaned) = is_name_valid(name) else {
        warn!(raw = %name, "Got invalid name. Disconnecting.");
        return Err(budget_chat::ClientInitializationError::InvalidName(name.to_string()).into());
    };

    debug!(name = %cleaned, "Client name is valid.");

    let read_half = lines.into_inner().into_inner();
    let socket = read_half.reunite(write_half)?;

    Ok((cleaned.to_string(), socket))
}

/// Check if the name is a non-empty ascii alphanumeric string.
pub fn is_name_valid(name: &str) -> Option<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.chars().all(|char| char.is_ascii_alphanumeric()) {
        true => Some(trimmed),
        false => None,
    }
}

/// Given a raw socket, try to finish the name-giving of the peer,
/// and if it goes fine, make a long running [Connection] out of it.
#[tracing::instrument(skip_all, fields(addr, peer = %addr, name))]
pub async fn handle_client(
    socket: TcpStream,
    addr: MemberID,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Message)>,
    client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_disconnected_tx: mpsc::UnboundedSender<MemberID>,
    outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>,
) -> budget_chat::Result<()> {

    let Ok((name, socket)) = stage_client(socket, addr).await else {
        error!("Staging failed for peer");
        return Ok(());
    };
    
    tracing::Span::current()
    .record("name", &name);

    info!("Staging complete... Connecting member with Room.");

    // We'll only call it a Connection after its been through staging.
    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
    let connection = Connection::new(socket, outbound_rx);

    let (mut inbound_message_for_room_rx, on_disconnect_rx) = connection.subscribe();
    let connection_handle = tokio::task::spawn(connection.run());

    // Once a named connection to the room is established,
    // register this peer into a shared state.
    {
        let mut guard = outbound_peer_map.lock().unwrap();
        guard.insert(addr, outbound_tx);
        debug!("peer map: {:#?}", guard.keys());
        drop(guard);
    }
    client_connected_with_name_tx.send((addr, name)).unwrap();

    let listen_for_messages = tokio::task::spawn(async move {
        // Propagate the individual messages received into the sink keyed by the
        // peer addr.
        let send_messages = async move {
            loop {
                if let Some(message) = inbound_message_for_room_rx.recv().await {
                    trace!(message = %message, "Message from peer that will be forwarded to Room.");
                    message_recvd_from_member_tx.send((addr, message)).unwrap();
                }
            }
        };
        select! {
            _ = send_messages => {
                trace!("No more messages can be received from client.");
            },
            // The connection just terminated for whatever reason.
            _ = on_disconnect_rx => {
                trace!("Client disconnected while we were waiting on messages.");
                {
                    let mut guard = outbound_peer_map.lock().unwrap();
                    guard.remove(&addr);
                }
                // Tell the chat room about it.
                client_disconnected_tx.send(addr).unwrap();
            }
        }
    });

    _ = tokio::join!(listen_for_messages, connection_handle);
    Ok(())
}
