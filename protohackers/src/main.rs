use clap::Parser;
use protohackers::solutions::echo_server;
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



#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    info!("Args: {:#?}", args);
    match args.solution {
        None => {
            tokio::join!(
                start_echo_server()
            );
        },
        Some(x) => {
            match x {
                0 => {
                    _ = start_echo_server().await;
                },
                _ => {
                    unimplemented!("unknown solution id")
                }
            }
        },
    }
}