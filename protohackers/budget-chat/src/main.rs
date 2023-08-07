use std::{net::SocketAddr, sync::{Arc, Mutex}};
use tokio::net::{TcpListener, TcpStream};
use budget_chat::{BudgetChatError, connection::Connection, room::Room};
use clap::Parser;
use tracing::{trace, debug, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};


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

    // let room = Room::new();

    // let addr: SocketAddr = ([0; 8], port).into();
    // let listener = TcpListener::bind(addr).await?;
    // info!("Listening for connections on {}", listener.local_addr()?);

    // loop {
    //     let (socket, addr) = listener.accept().await?;
    //     let connection = Connection::new(socket, addr);
    //     let room = room.clone();
    //     tokio::task::spawn(async move {
    //         match connection.run_to_completion(room).await {
    //             Ok(_) => {
    //                 debug!("Handled connection successfully from {}", addr);
    //             },
    //             Err(err) => {
    //                 warn!("Handled connection from {} with an error: {}", addr, err);
    //             }
    //         }
    //     });
    // }
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
