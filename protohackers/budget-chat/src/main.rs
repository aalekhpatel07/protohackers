use std::{net::SocketAddr, sync::{Arc, Mutex}, time::Duration, io::Cursor};
use bytes::Buf;
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};
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
    let (send_to_member_tx, send_to_member_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_disconnected_tx, client_disconnected_rx) = tokio::sync::mpsc::unbounded_channel();
    let (client_connected_with_name_tx, client_connected_with_name_rx) = tokio::sync::mpsc::unbounded_channel();

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
        _ = client_loop(listener, client_connected_with_name_tx, client_disconnected_tx) => {

        },
        _ = room_handle => {

        }
    }

    Ok(())
}

pub async fn client_loop(
    listener: TcpListener,
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

        // Initiate staging for this client.
        tokio::task::spawn(async move {
            trace!("Beginning init for potentially connected client.");
            let Ok((stream, (addr, name))) = staging.run().await else {
                error!(peer = %addr, "Couldn't finish name-giving of the client.");
                return;
            };
            trace!("Finished name-giving for a client.");
            if let Err(err) = handle_connected_client(stream, addr, name).await {
                // something led us to believe the client got lost.
                error!("failed when trying to handle connected client: {}", err);
                client_disconnected_tx.send(addr).unwrap();
            }
        });
    }
}


pub async fn handle_connected_client(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    peer_name: String,
    // outbound_message_rx: mpsc::UnboundedReceiver<String>,
    // message_recvd_from_tx: mpsc::UnboundedSender<(MemberID, String)>
) -> budget_chat::Result<()> {
    let (mut read_half, mut write_half) = stream.into_split();




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
