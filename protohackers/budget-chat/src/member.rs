// use std::net::SocketAddr;

// use tokio::{net::tcp::{OwnedReadHalf, OwnedWriteHalf}, sync::mpsc};
// use tracing::{trace, error, debug_span, warn};

// use crate::room::{Message};
// use tokio::io::{AsyncReadExt, AsyncWriteExt};




// #[derive(Debug)]
// pub struct Member {
//     read_half: OwnedReadHalf,
//     write_half: OwnedWriteHalf,
//     membership: Membership,
//     id: MemberID,
//     disconnected_tx: mpsc::UnboundedSender<MemberID>,
//     send_to_others_tx: mpsc::UnboundedSender<(MemberID, Message)>
// }

// impl core::fmt::Display for Member {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let peer_addr = self.read_half.peer_addr().unwrap().to_string();
//         f.debug_struct("Member")
//         .field("name", &self.name().unwrap_or_else(|| "unknown"))
//         .field("peer", &peer_addr)
//         .finish()
//     }
// }

// impl Member {
//     pub fn new(
//         read_half: OwnedReadHalf, 
//         write_half: OwnedWriteHalf,
//         disconnected_tx: mpsc::UnboundedSender<MemberID>,
//         send_to_others_tx: mpsc::UnboundedSender<(MemberID, Message)>
//     ) -> Self {
//         Self {
//             id: read_half.peer_addr().unwrap(),
//             read_half,
//             write_half,
//             membership: Membership::None,
//             disconnected_tx,
//             send_to_others_tx
//         }
//     }

//     pub fn is_name_valid<'a>(name: &'a str) -> Option<&'a str> {
//         let trimmed = name.trim();
//         if trimmed.is_empty() {
//             return None;
//         }
//         match trimmed
//         .chars()
//         .all(|char| {
//             char.is_ascii_alphanumeric()
//         }) {
//             true => {
//                 Some(trimmed)
//             },
//             false => None
//         }
//     }


//     pub fn name(&self) -> Option<&str> {
//         match self.membership {
//             Membership::Member(ref member) => Some(member),
//             _ => None
//         }
//     }
//     pub fn set_name(&mut self, name: &str) {
//         self.membership = Membership::Member(name.to_string());
//     }

//     pub async fn send_message(&mut self, message: &str) -> crate::Result<()> {
//         let mut full_message = message.to_string();
//         full_message.push('\n');

//         let msg = full_message.as_bytes();
//         let msg_len = msg.len();
//         match self.write_half.write_all(msg).await {
//             Ok(_) => {
//                 trace!("Sent {} bytes (+ EOF) to member {:#}", msg_len, self);
//             },
//             Err(err) => {
//                 error!("Failed to send_message to {:#}: {}", self, err);
//                 return Err(err.into());
//             }
//         }
//         Ok(())
//     }

//     pub async fn recv_message(&mut self) -> crate::Result<Option<String>> {
//         let mut message = String::new();
//         match self.read_half.read_to_string(&mut message).await {
//             Ok(bytes_read) => {
//                 if bytes_read == 0 {
//                     trace!("Received EOF..");
//                     return Ok(None);
//                 }
//                 trace!("Received a message of ({}) bytes", bytes_read);
//                 return Ok(Some(message));
//             },
//             Err(err) => {
//                 error!("Failed to recv_message: {}", err);
//                 return Err(err.into());
//             }
//         }
//     }

//     pub async fn listen_for_messages(mut self) -> crate::Result<()> {

//         let span = debug_span!("Listening for messages", user = %self);
//         loop {
//             match self.recv_message().await {
//                 Ok(Some(msg)) => {
//                     if let Err(err) = self.send_to_others_tx.send((self.id, msg)) {
//                         span.in_scope(|| {
//                             error!(error = %err, "Received message from client but failed to let the Room know about it. Message will be lost.");
//                         });
//                     }
//                 },
//                 Ok(None) => {
//                     span.in_scope(|| {
//                         warn!("Client sent EOF so assume disconnection...");
//                     });
//                     _ = self.disconnected_tx.send(self.id);

//                     return Ok(());
//                 },
//                 Err(err) => {
//                     span.in_scope(|| {
//                         error!(error = %err, "Connection reset by client while we were listening for messages. Treating this as disconnection");
//                     });
//                     _ = self.disconnected_tx.send(self.id);
//                     return Ok(());
//                 }
//             }
//         }
//     }

// }

// pub type MemberID = SocketAddr;