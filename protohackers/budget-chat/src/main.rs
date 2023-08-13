use std::{net::SocketAddr, sync::{Arc, Mutex}, collections::HashMap};
use tokio::{net::{TcpListener, TcpStream}, sync::{mpsc::{UnboundedSender}}, select};
// use budget_chat::{BudgetChatError, connection::Connection, room::Room, staging::Staging, MemberID};
use budget_chat::{MemberID, room::{Room, Message}, staging::Staging, connection::Connection};
use clap::{Parser};
use tracing::{trace, debug, info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::sync::mpsc;


#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    about,
)]
pub struct Args {
    #[clap(short, long)]
    port: u16
}

pub async fn run_server(port: u16) -> budget_chat::Result<()> {

    let (message_received_from_member_tx, message_received_from_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (send_to_member_tx, mut send_to_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_disconnected_tx, client_disconnected_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_connected_with_name_tx, client_connected_with_name_rx) = tokio::sync::mpsc::unbounded_channel();

    let outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>> = Default::default();

    let outbound_peer_map_cp = outbound_peer_map.clone();
    tokio::task::spawn(async move {
        loop {
            match send_to_member_rx.recv().await {
                Some((peer, message)) => {
                    let guard = outbound_peer_map_cp.lock().unwrap();
                    let sender = guard.get(&peer).unwrap();
                    sender.send(message).unwrap();
                },
                None => {
                    debug!("Got None in send_to_member_rx. Dropping sender handle.");
                    break
                }
            }
        }
    });

    let mut room = Room::new(
        message_received_from_member_rx, 
        send_to_member_tx, 
        client_disconnected_rx, 
        client_connected_with_name_rx
    );
    let room_handle = tokio::task::spawn(async move {
        room.run().await;
    });

    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Listening for connections on {}", listener.local_addr()?);

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
    // tokio::select! {
    //     _ = client_loop(
    //         listener, 
    //         message_received_from_member_tx.clone(), 
    //         client_connected_with_name_tx, 
    //         client_disconnected_tx
    //     ) => {
    //         info!("Client loop complete... :sus:");
    //     },
    //     _ = room_handle => {
    //         info!("Room handle complete... :sus:");
    //     }
    // }

    Ok(())
}

pub async fn client_loop(
    listener: TcpListener,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Message)>,
    client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_disconnected_tx: mpsc::UnboundedSender<MemberID>,
    outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>
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
                outbound_peer_map
            ).await {
                error!("Handling client failed: {}", err);
            }
        });
    }
}

#[tracing::instrument(skip_all, fields(addr, peer = %addr))]
pub async fn handle_client(
    socket: TcpStream,
    addr: MemberID,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, Message)>,
    client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_disconnected_tx: mpsc::UnboundedSender<MemberID>,
    outbound_peer_map: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Message>>>>
) -> budget_chat::Result<()> {
    // Set up a connection with channels.
    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();

    let connection = Connection::new(
        socket,
        outbound_rx,
    );

    let (staging_messages_inbound_rx, _disconnect_rx) = connection.subscribe();

    // Whenever a client connects, we initiate the name-giving ceremony.
    let staging = Staging::new(
        addr,
        outbound_tx.clone(),
        staging_messages_inbound_rx,
    );

    trace!("Waiting for connection.run() and staging.run() to complete for client.");
    // Drive the connection and staging together so that we can do a name-giving ceremony.

    // let x = tokio::select! {
    //     connection_result = connection.run() => match connection_result {
    //         Ok(conn) => {
    //             Ok((Some(conn), None))
    //         },
    //         Err(err) => {
    //             Ok((None, None))
    //         }
    //     },
    //     maybe_name = staging.run() => {
    //         let Ok(name) = maybe_name else {
    //             error!("Failed to stage peer.");
    //             return Ok((None, ));
    //         };
    //     }
    // };
    let (mut inbound_message_for_room_rx, on_disconnect_rx) = connection.subscribe();
    let connection_handle = tokio::task::spawn(connection.run());

    let Ok(name) = staging.run().await else {
        return Ok(());
    };

    debug!("Staging complete... Connecting member with Room.");

    {
        let mut guard = outbound_peer_map.lock().unwrap();
        guard.insert(addr, outbound_tx);
        debug!("peer map: {:#?}", guard.keys());
        drop(guard);
    }
    client_connected_with_name_tx.send((addr, name)).unwrap();


    let listen_for_messages = tokio::task::spawn(async move {
        let send_messages = async move {
            loop {
                match inbound_message_for_room_rx.recv().await {
                    Some(message) => {
                        trace!(message = %message, "Message from peer that will be forwarded to Room.");
                        message_recvd_from_member_tx.send((addr, message)).unwrap();
                    },
                    None => {}
                }
            }
        };
        select! {
            _ = send_messages => {
                trace!("No more messages can be received from client.");
            },
            _ = on_disconnect_rx => {
                trace!("Client disconnected while we were waiting on messages.");
                {
                    let mut guard = outbound_peer_map.lock().unwrap();
                    guard.remove(&addr);
                }
                client_disconnected_tx.send(addr).unwrap();
            }
        }
    });

    _ = tokio::join!(listen_for_messages, connection_handle);
    Ok(())
}


#[tokio::main]
async fn main() -> budget_chat::Result<()> {
    tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "budget_chat=trace,tokio=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

    let args = Args::parse();
    run_server(args.port).await.unwrap();
    Ok(())
}
