use std::sync::MutexGuard;

use thiserror::Error;


#[derive(Debug, Error)]
pub enum BudgetChatError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Initialization(#[from] ClientInitializationError),
    #[error(transparent)]
    Reunite(#[from] tokio::net::tcp::ReuniteError),
    #[error("Member (id: {0}) is not known")]
    UnknownMember(String),
    #[error("No messages to read...")]
    NoMsgsToRead
}

/// Any error at initialization warrants a disconnection.
#[derive(Debug, Error)]
pub enum ClientInitializationError {
    #[error("Client provided bad name: {0}")]
    InvalidName(String),
    #[error("Connection was reset by the client...")]
    ConnectionResetByClient
}


pub type Result<T, E=BudgetChatError> = core::result::Result<T, E>;