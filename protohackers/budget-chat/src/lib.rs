pub mod connection;
mod errors;
pub mod room;

pub use errors::*;

pub type MemberID = std::net::SocketAddr;
