pub mod connection;
mod errors;
pub mod member;
pub mod room;

pub use errors::*;
pub mod staging;

pub type MemberID = std::net::SocketAddr;
