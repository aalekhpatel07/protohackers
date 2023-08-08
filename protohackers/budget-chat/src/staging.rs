
use tokio::{sync::{mpsc, oneshot}, net::tcp::{OwnedWriteHalf, OwnedReadHalf}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{debug_span, debug, error, trace};
use tokio::net::{TcpStream};
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
    pub stream: TcpStream,
    /// Let the receiver know when one client successfully connects to the room.
    pub client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
}


impl Staging {

    pub fn new(
        potential_member_id: &MemberID,
        client_connected_with_name_tx: mpsc::UnboundedSender<(MemberID, String)>,
        stream: TcpStream
    ) -> Self {

        Self {
            potential_member_id: *potential_member_id,
            client_connected_with_name_tx,
            stream
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

    pub async fn run(self) -> crate::Result<(TcpStream, (MemberID, String))> {

        let client_connected_with_name_tx = self.client_connected_with_name_tx;
        let potential_member_id = self.potential_member_id;
        let stream = self.stream;

        let (mut read_half, mut write_half) = stream.into_split();
        Self::initiate_membership(&mut read_half, &mut write_half)
        .await
        .map(|(addr, clean_name)| {
            let stream = read_half.reunite(write_half).unwrap();
            (stream, (addr, clean_name))
        })
    }


    pub async fn initiate_membership(
        read_half: &mut OwnedReadHalf,
        write_half: &mut OwnedWriteHalf,
    ) -> crate::Result<(MemberID, String)> {

        let addr = read_half.peer_addr()?;
        let span = debug_span!("membership initiation", member_id = %addr);

        span.in_scope(|| {
            debug!("Sending init message to member and waiting for the name in response...");
        });

        Self::
        send_message(
            &addr,
            write_half,
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
                read_half,
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
                        return Ok((addr, name.trim().to_string()));
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
        potential_member_id: &MemberID,
        write_half: &mut OwnedWriteHalf,
        message: &str,
    ) -> crate::Result<()> {

        let mut full_message = message.to_string();
        full_message.push('\n');
        let msg = full_message.as_bytes();
        let msg_len = msg.len();
        // send_to_potential_member_tx.send(full_message).unwrap();
        match write_half.write_all(msg).await {
            Ok(_) => {
                trace!("Sent {} bytes (+ EOF) to member {:#}", msg_len, potential_member_id);
            },
            Err(err) => {
                error!("Failed to send_message to {:#}: {}", potential_member_id, err);
                return Err(err.into());
            }
        }
        Ok(())
    }

    pub async fn recv_message(
        read_half: &mut OwnedReadHalf
    ) -> crate::Result<Option<String>> {
        let mut message = String::new();
        match read_half.read_to_string(&mut message).await {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    trace!("Received EOF..");
                    return Ok(None);
                }
                trace!("Received a message of ({}) bytes", bytes_read);
                return Ok(Some(message));
            },
            Err(err) => {
                error!("Failed to recv_message: {}", err);
                return Err(err.into());
            }
        }
    }
}