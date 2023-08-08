pub mod room;
pub mod connection;
pub mod member;
mod errors;

pub use errors::*;
pub mod staging;


pub type MemberID = std::net::SocketAddr;