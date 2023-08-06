use clap::Parser;
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
    #[clap(default_value_t = 12001, short, long)]
    port: u16
}

pub async fn start_prime_time() {
}



#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "prime-time=trace,tokio=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

    let args = Args::parse();
    info!("Args: {:#?}", args);

    let addr: std::net::SocketAddr = ([0; 8], args.port).into();
    let listener = TcpListener::bind(&addr).await.unwrap();
    prime_time::PrimeTime::new(listener).run().await.unwrap();
}
