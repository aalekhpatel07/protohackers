use thiserror::Error;


#[derive(Debug, Error)]
pub enum BudgetChatError {
    #[error(transparent)]
    IO(#[from] std::io::Error),

}


pub type Result<T, E=BudgetChatError> = core::result::Result<T, E>;