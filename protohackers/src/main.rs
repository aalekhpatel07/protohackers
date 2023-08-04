use clap::Parser;
use protohackers::solutions::echo_server;
use protohackers::solutions::prime_time;
use tracing::info;



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
    prime_time::PrimeTime { port: 12001 }.run().await.unwrap();
}



#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
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