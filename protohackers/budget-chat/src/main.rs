use std::{net::SocketAddr, sync::{Arc, Mutex}, time::Duration, io::Cursor};
use bytes::Buf;
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};
use budget_chat::{BudgetChatError, connection::Connection, room::Room, staging::Staging, transport::RoomTransport};
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

    let (register_stream_tx, register_stream_rx) = mpsc::unbounded_channel();
    let (deregister_stream_tx, deregister_stream_rx) = mpsc::unbounded_channel();

    let addr: SocketAddr = ([0; 8], port).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Listening for connections on {}", listener.local_addr()?);

    let room_transport = RoomTransport::new(
        client_disconnected_tx.clone(),
        message_received_from_member_tx.clone(),
        send_to_member_rx,
        register_stream_rx,
        deregister_stream_rx,
        deregister_stream_tx
    );

    let room_transport_handle = tokio::task::spawn(async move {
        room_transport.run().await.unwrap();
    });

    loop {

        let (socket, addr) = listener.accept().await?;

        let (anon_disconnected_tx, anon_disconnected_rx) = tokio::sync::mpsc::unbounded_channel();
        let (send_to_potential_member_tx, send_to_potential_member_rx) = tokio::sync::mpsc::unbounded_channel();
        let (recvd_from_potential_member_tx, recvd_from_potential_member_rx) = tokio::sync::mpsc::unbounded_channel();

        let (staging_failed_tx, staging_failed_rx) = tokio::sync::mpsc::unbounded_channel();

        // Whenever a client connects, we initiate the name-giving ceremony.
        let staging = Staging::new(
            &addr,
            client_connected_with_name_tx.clone(),
            anon_disconnected_rx,
            send_to_potential_member_tx,
            recvd_from_potential_member_rx
        );

        let initial_transport_handle = budget_chat::transport::InitialTransport::new(
            socket,
            addr,
            send_to_potential_member_rx,
            recvd_from_potential_member_tx,
            anon_disconnected_tx,
            staging_failed_rx
        );

        let register_stream_tx_cp = register_stream_tx.clone();
        tokio::task::spawn(async move {

            let staging_handle = tokio::task::spawn(async move {
                if let Err(err) = staging.run().await {
                    error!("staging.run() failed: {}", err);
                    staging_failed_tx.send(()).unwrap();
                    return Err(budget_chat::ClientInitializationError::ConnectionResetByClient)
                }
                return Ok(())
            });

            if let (Ok(_), Ok(stream)) = tokio::join!(
                staging_handle,
                initial_transport_handle.run(),
            ){
                // If we got here, means the name-giving ceremony was complete.
                info!("Name-giving ceremony complete for client {:#?}", stream);
                register_stream_tx_cp.clone().send(stream).unwrap();

            } else {
                error!("Name-giving error");
            }
        });
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
