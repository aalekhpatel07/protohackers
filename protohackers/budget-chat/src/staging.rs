
// use tokio::{sync::{mpsc, oneshot}, net::tcp::{OwnedWriteHalf, OwnedReadHalf}, io::{AsyncWriteExt, AsyncReadExt, BufReader, AsyncBufReadExt}};
// use tracing::{debug_span, debug, error, trace};
// use tokio::net::{TcpStream};
// use crate::MemberID;

use futures::channel::mpsc::UnboundedReceiver;
use tokio::sync::mpsc;
use tracing::{trace, warn};

use crate::{MemberID, ClientInitializationError};




#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Membership {
    #[default]
    Candidate,
    NameRequested,
    Member(String)
}

/// We'll have a staging area for every member.
#[derive(Debug)]
pub struct Staging {
    pub potential_peer_id: MemberID,
    pub send_to_member_tx: mpsc::UnboundedSender<String>,
    pub recvd_from_member_rx: mpsc::UnboundedReceiver<String>,
    pub membership: Membership,
}


impl Staging {

    pub fn new(
        potential_peer_id: MemberID, 
        send_to_member_tx: mpsc::UnboundedSender<String>, 
        recvd_from_member_rx: mpsc::UnboundedReceiver<String>, 
    ) -> Self {
        Self {
            potential_peer_id,
            send_to_member_tx,
            recvd_from_member_rx,
            membership: Default::default(),
        }
    }

    #[tracing::instrument(
        skip(self), 
        fields(
            peer_addr = %self.potential_peer_id,
            membership = ?self.membership,
        )
    )]
    pub async fn run(mut self) -> crate::Result<String> {
        loop {
            match self.membership {
                Membership::Candidate => {
                    trace!("Requesting name to potential member.");
                    self.request_name();
                    self.membership = Membership::NameRequested;
                },
                Membership::NameRequested => {
                    trace!("Waiting to receive name from member");
                    let Some(message) = self.recvd_from_member_rx.recv().await else {
                        warn!("We were waiting to receive name from member but it went away without telling us.");
                        return Err(ClientInitializationError::ConnectionResetByClient.into());
                    };
                    trace!(raw = %message, "Received name from member. Validating...");
                    let Some(cleaned) = Self::is_name_valid(&message) else {
                        warn!("The member sent us a bad name. We'll terminate the connection.");
                        self.send_bad_name_message(&message);
                        return Err(ClientInitializationError::InvalidName(message).into());
                    };
                    trace!(cleaned = %cleaned, "Name validation complete for member...");
                    self.membership = Membership::Member(cleaned.to_string());
                },
                Membership::Member(name) => {
                    trace!("Member sign up successful. Notifying staging is complete.");
                    return Ok(name);
                }
            }
        }
    }

    pub fn request_name(&self) {
        self.send_to_member_tx.send("Welcome to budgetchat! What shall I call you?".to_string()).unwrap();
    }

    pub fn send_bad_name_message(&self, bad_name: &str) {
        let message = format!("You provided an invalid name so byeee!: {}", bad_name);
        self.send_to_member_tx.send(message).unwrap();
    }

    /// Return None if the name cannot be cleaned into a valid name, Some(cleaned) otherwise.
    pub fn is_name_valid<'a>(name: &'a str) -> Option<&'a str> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        match trimmed
        .chars()
        .all(|char| {
            char.is_ascii_alphanumeric()
        }) {
            true => {
                Some(trimmed)
            },
            false => None
        }
    }
}