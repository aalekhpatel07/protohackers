


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
    
}

impl Staging {

    fn is_name_valid<'a>(&self, name: &'a str) -> Option<&'a str> {
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



    // async fn initiate_membership(&self, stream: TcpStream) -> crate::Result<(String, MemberID), ClientInitializationError> {
    //     let (read_half, write_half) = stream.into_split();
    //     let member_id = read_half.peer_addr().unwrap();
    //     // let mut member = Member::new(
    //     //     read_half,
    //     //     write_half,
    //     // );

    //     let span = debug_span!("membership initiation", member = %member);
    //     span.in_scope(|| {
    //         debug!("Sending init message to member and waiting for the name in response...");
    //     });

    //     member
    //     .send_message("Welcome to budgetchat! What shall I call you?")
    //     .await
    //     .map_err(|err| {
    //         span.in_scope(|| {
    //             error!(
    //                 err = %err,
    //                 "Client established connection but went away before we could request a name... Bad client!",
    //             );
    //         });
    //         crate::ClientInitializationError::ConnectionResetByClient
    //     })?;

    //     let maybe_message =
    //         member
    //         .recv_message()
    //         .await
    //         .map_err(|err| {
    //             span.in_scope(|| {
    //                 error!(
    //                     err = %err,
    //                     "Client established connection but went away before we got a name... Bad client!",
    //                 );
    //             });
    //             crate::ClientInitializationError::ConnectionResetByClient
    //         })?;

    //     match maybe_message {
    //         None => {
    //             span.in_scope(|| {
    //                 error!("Expected to receive a name but received EOF instead.. Bad client! Dropping connection.");
    //             });
    //             Err(crate::ClientInitializationError::ConnectionResetByClient)
    //         },
    //         Some(message) => {
    //             span.in_scope(|| {
    //                 debug!("Received the name message which will be checked for validity: {}", message);
    //             });
    //             match Member::is_name_valid(&message) {
    //                 Some(name) => {
    //                     span.in_scope(|| {
    //                         debug!(clean_name = name, "Name is valid: {}", message);
    //                     });
    //                     Ok((name.to_string(), member_id))
    //                 },
    //                 None => {
    //                     let bad_name_message = format!("Received bad name: \"{}\". Bye!", &message.trim());
    //                     span.in_scope(|| {
    //                         error!(bad_name_message);
    //                     });
    //                     // We may fail to send if the client reset the connection,
    //                     // but we don't care about it so just ignore any errors.
    //                     _ = member.send_message(&bad_name_message).await;
    //                     Err(crate::ClientInitializationError::InvalidName(message))
    //                 }
    //             }
    //         }
    //     }
    // }