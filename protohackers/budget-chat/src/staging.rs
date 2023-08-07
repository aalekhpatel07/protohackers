use std::net::TcpStream;

use tokio::{sync::{mpsc, oneshot}, net::tcp::{OwnedWriteHalf, OwnedReadHalf}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{debug_span, debug, error, trace};

use crate::MemberID;




#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Membership {
    #[default]
    None,
    Joining,
    Member(String)
}


/// Kind of a staging area before a user actually joins the room.
#[derive(Debug)]
pub struct Staging {
    pub potential_member_id: MemberID,
    /// Let the receiver know when one client successfully connects to the room.
    pub client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,

    /// Someone will tell us that this client failed to complete the initialization
    /// at the transport layer. So drop this client.
    pub anonymous_client_disconnected_rx: mpsc::UnboundedReceiver<()>,

    /// Tell the transport to send a message to a potential client.
    pub send_to_potential_member_tx: mpsc::UnboundedSender<String>,

    /// Someone will send us messages from potential members.
    pub recv_from_potential_member_rx: mpsc::UnboundedReceiver<Option<String>>
}


impl Staging {

    pub fn new(
        potential_member_id: &MemberID,
        client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
        anonymous_client_disconnected_rx: mpsc::UnboundedReceiver<()>,
        send_to_potential_member_tx: mpsc::UnboundedSender<String>,
        recv_from_potential_member_rx: mpsc::UnboundedReceiver<Option<String>>
    ) -> Self {

        Self {
            potential_member_id: *potential_member_id,
            client_connected_with_name_tx,
            anonymous_client_disconnected_rx,
            send_to_potential_member_tx,
            recv_from_potential_member_rx
        }
    }

    fn is_name_valid<'a>(name: &'a str) -> Option<&'a str> {
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

    pub async fn run(self) -> crate::Result<()> {
        let mut anon_disconnected_rx = self.anonymous_client_disconnected_rx;

        let send_to_potential_member_tx = self.send_to_potential_member_tx;
        let mut recv_from_potential_member_rx = self.recv_from_potential_member_rx;
        let client_connected_with_name_tx = self.client_connected_with_name_tx;
        let potential_member_id = self.potential_member_id;

        tokio::select! {
            Some(_) = anon_disconnected_rx.recv() => {
                error!(
                    "An initialization error occurred at transport layer that led to failure of name-giving for this client. Dropping connection."
                );
                return Ok(());
            },
            res = Self::initiate_membership(
                potential_member_id,
                &send_to_potential_member_tx,
                &mut recv_from_potential_member_rx,
                &client_connected_with_name_tx
            ) => {
                if let Err(err) = res {
                    error!(
                        error = %err,
                        "Couldn't complete the name-giving of the client. Dropping connection."
                    );
                    return Ok(());
                }
            }
        }
        Ok(())
    }


    pub async fn initiate_membership(
        potential_member_id: MemberID,
        send_to_potential_member_tx: &mpsc::UnboundedSender<String>,
        recv_from_potential_member_rx: &mut mpsc::UnboundedReceiver<Option<String>>,
        client_connected_with_name_tx: &mpsc::UnboundedSender<(MemberID, String)>,
    ) -> crate::Result<()> {
        // let (mut read_half, mut write_half) = stream.into_split();

        let span = debug_span!("membership initiation", member_id = %potential_member_id);

        span.in_scope(|| {
            debug!("Sending init message to member and waiting for the name in response...");
        });

        Self::
        send_message(
            send_to_potential_member_tx,
            "Welcome to budgetchat! What shall I call you?"
        )
        .await
        .map_err(|err| {
            span.in_scope(|| {
                error!(
                    err = %err,
                    "Client established connection but went away before we could request a name... Bad client!",
                );
            });
            crate::ClientInitializationError::ConnectionResetByClient
        })?;

        let maybe_message =
            Self
            ::recv_message(
                recv_from_potential_member_rx
            )
            .await
            .map_err(|err| {
                span.in_scope(|| {
                    error!(
                        err = %err,
                        "Client established connection but went away before we got a name... Bad client!",
                    );
                });
                crate::ClientInitializationError::ConnectionResetByClient
            })?;

        match maybe_message {
            None => {
                span.in_scope(|| {
                    error!("Expected to receive a name but received EOF instead.. Bad client! Dropping connection.");
                });
                return Err(crate::ClientInitializationError::ConnectionResetByClient.into());
            },
            Some(message) => {
                span.in_scope(|| {
                    debug!("Received the name message which will be checked for validity: {}", message);
                });
                match Self::is_name_valid(&message) {
                    Some(name) => {
                        span.in_scope(|| {
                            debug!(clean_name = name, "Name is valid: {}", message);
                        });
                        client_connected_with_name_tx
                        .send((
                            potential_member_id, 
                            name.to_string()
                        ))
                        .unwrap();
                        return Ok(());
                    },
                    None => {
                        let bad_name_message = format!("Received bad name: \"{}\". Bye!", &message.trim());
                        span.in_scope(|| {
                            error!(bad_name_message);
                        });
                        // We may fail to send if the client reset the connection,
                        // but we don't care about it so just ignore any errors.
                        return Err(crate::ClientInitializationError::InvalidName(message).into());
                    }
                }
            }
        }
    }


    pub async fn send_message(
        send_to_potential_member_tx: &mpsc::UnboundedSender<String>,
        message: &str,
    ) -> crate::Result<()> {

        let mut full_message = message.to_string();
        full_message.push('\n');
        // let msg = full_message.as_bytes();
        // let msg_len = msg.len();
        send_to_potential_member_tx.send(full_message).unwrap();
        // match write_half.write_all(msg).await {
        //     Ok(_) => {
        //         trace!("Sent {} bytes (+ EOF) to member {:#}", msg_len, potential_member_id);
        //     },
        //     Err(err) => {
        //         error!("Failed to send_message to {:#}: {}", potential_member_id, err);
        //         return Err(err.into());
        //     }
        // }
        Ok(())
    }

    pub async fn recv_message(recv_from_potential_member_rx: &mut mpsc::UnboundedReceiver<Option<String>>) -> crate::Result<Option<String>> {
        match recv_from_potential_member_rx.recv().await {
            Some(msg) => {
                Ok(msg)   
            },
            None => {
                Err(crate::BudgetChatError::Initialization(crate::ClientInitializationError::ConnectionResetByClient))
            }
        }

        // match read_half.read_to_string(&mut message).await {
        //     Ok(bytes_read) => {
        //         if bytes_read == 0 {
        //             trace!("Received EOF..");
        //             return Ok(None);
        //         }
        //         trace!("Received a message of ({}) bytes", bytes_read);
        //         return Ok(Some(message));
        //     },
        //     Err(err) => {
        //         error!("Failed to recv_message: {}", err);
        //         return Err(err.into());
        //     }
        // }
    }
}