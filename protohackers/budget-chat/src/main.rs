use std::{net::SocketAddr, sync::{Arc, Mutex}, time::Duration, io::Cursor, collections::HashMap};
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt}, sync::mpsc::{UnboundedSender, UnboundedReceiver}, select};
use budget_chat::{BudgetChatError, connection::Connection, room::Room, staging::Staging, MemberID};
use clap::{Parser, error};
use tracing::{trace, debug, info, warn, error};
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

        // message_received_from_member: mpsc::UnboundedReceiver<(MemberID, Message)>,
        // send_to_member: mpsc::UnboundedSender<(MemberID, Message)>,
        // client_disconnected_rx: mpsc::UnboundedReceiver<MemberID>,
        // client_connected_with_name_rx: mpsc::UnboundedReceiver<(MemberID, String)>
    
    // let room = Room::new();

    let (message_received_from_member_tx, message_received_from_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (send_to_member_tx, mut send_to_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_disconnected_tx, client_disconnected_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_connected_with_name_tx, client_connected_with_name_rx) = tokio::sync::mpsc::unbounded_channel();

    let peer_senders: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<String>>>> = Default::default();

    let peer_senders_cp = peer_senders.clone();
    tokio::task::spawn(async move {
        loop {
            match send_to_member_rx.recv().await {
                Some((peer, message)) => {
                    let guard = peer_senders_cp.lock().unwrap();
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

    // let (register_stream_tx, register_stream_rx) = mpsc::unbounded_channel();
    // let (deregister_stream_tx, deregister_stream_rx) = mpsc::unbounded_channel();

    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Listening for connections on {}", listener.local_addr()?);

    // let room_transport = RoomTransport::new(
    //     client_disconnected_tx.clone(),
    //     message_received_from_member_tx.clone(),
    //     send_to_member_rx,
    //     register_stream_rx,
    //     deregister_stream_rx,
    //     deregister_stream_tx
    // );

    // let room_transport_handle = tokio::task::spawn(async move {
    //     room_transport.run().await.unwrap();
    // });
    tokio::select! {
        _ = client_loop(listener, peer_senders, message_received_from_member_tx.clone(), client_connected_with_name_tx, client_disconnected_tx) => {
            info!("Client loop complete... :sus:");
        },
        _ = room_handle => {
            info!("Room handle complete... :sus:");
        }
    }

    Ok(())
}

pub async fn client_loop(
    listener: TcpListener,
    peer_senders: Arc<Mutex<HashMap<SocketAddr, UnboundedSender<String>>>>,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
    client_disconnected_tx: mpsc::UnboundedSender<MemberID>
) -> budget_chat::Result<()> {
    loop {
        let client_disconnected_tx = client_disconnected_tx.clone();
        let (socket, addr) = listener.accept().await?;

        // Whenever a client connects, we initiate the name-giving ceremony.
        let staging = Staging::new(
            &addr,
            client_connected_with_name_tx.clone(),
            socket,
        );
        let msg_recvd_from_member_tx = message_recvd_from_member_tx.clone();

        let (outbound_msg_tx, outbound_msg_rx) = tokio::sync::mpsc::unbounded_channel();
        {
            let mut guard = peer_senders.lock().unwrap();
            guard.insert(addr, outbound_msg_tx);
        }
        // Initiate staging for this client.
        tokio::task::spawn(async move {
            trace!("Beginning init for potentially connected client.");
            let Ok((stream, (addr, name))) = staging.run().await else {
                error!(peer = %addr, "Couldn't finish name-giving of the client.");
                return;
            };
            // let (send_to_peer_tx, send_to_peer_rx) = tokio::sync::mpsc::unbounded_channel();
            trace!("Finished name-giving for a client.");
            if let Err(err) = handle_connected_client(
                stream, 
                addr, 
                name,
                outbound_msg_rx,
                msg_recvd_from_member_tx
            ).await {
                // something led us to believe the client got lost.
                error!("failed when trying to handle connected client: {}", err);
                client_disconnected_tx.send(addr).unwrap();
            }
        });
    }
}


pub async fn handle_connected_client(
    stream: TcpStream,
    peer_addr: SocketAddr,
    peer_name: String,
    mut outbound_message_rx: UnboundedReceiver<String>,
    message_recvd_from_member_tx: mpsc::UnboundedSender<(MemberID, String)>
) -> budget_chat::Result<()> {
    debug!("Handling connected client: {:#?}", (peer_addr, peer_name));
    let (read_half, mut write_half) = stream.into_split();

    let reader = BufReader::new(read_half);
    let mut lines_from_peer = reader.lines();

    loop {
        select! {
            line = lines_from_peer.next_line() => match line {
                Ok(Some(line)) => {
                    message_recvd_from_member_tx.send((peer_addr, line)).unwrap();
                },
                Ok(None) => {
                    debug!("Received no line from client?");
                },
                Err(err) => {
                    return Err(err.into());
                }
            },
            line = outbound_message_rx.recv() => match line {
                Some(line) => {
                    write_half.write_all(line.as_bytes()).await?;
                    write_half.write_all(b"\n").await?;
                },
                None => {
                    return Ok(());
                }
            }
        }
    }    
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
