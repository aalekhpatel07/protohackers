use clap::Parser;
use protohackers::solutions::echo_server;
use protohackers::solutions::prime_time;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};



#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    about,
)]
pub struct Args {
    #[clap(default_value = None)]
    solution: Option<u8>
}


pub async fn start_echo_server() {
    echo_server::EchoServer { port: 12000 }.run().await.unwrap();
}
pub async fn start_prime_time() {
    let addr: std::net::SocketAddr = ([0; 8], 12001).into();
    let listener = TcpListener::bind(&addr).await.unwrap();
    prime_time::PrimeTime::new(listener).run().await.unwrap();
}



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "protohackers=trace,tokio=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

    let args = Args::parse();
    info!("Args: {:#?}", args);
    match args.solution {
        None => {
            tokio::join!(
                start_echo_server(),
                start_prime_time()
            );
        },
        Some(x) => {
            match x {
                0 => {
                    _ = start_echo_server().await;
                },
                1 => {
                    _ = start_prime_time().await;
                }
                _ => {
                    unimplemented!("unknown solution id")
                }
            }
        },
    }
}