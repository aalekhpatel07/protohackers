use std::{collections::HashMap};
use tokio::sync::mpsc;
use tracing::{warn, info, debug, error};
use crate::MemberID;

#[derive(Debug, Clone)]
pub enum Message {
    Staging(String),
    Chat(String)
}

impl<T> From<T> for Message 
where
    T: AsRef<str>
{
    fn from(value: T) -> Self {
        Message::Chat(value.as_ref().to_string())
    }
}

impl From<Message> for String {
    fn from(value: Message) -> Self {
        match value {
            Message::Staging(message) => message,
            Message::Chat(message) => message
        }
    }
}

impl Message {
    pub fn as_str(&self) -> &str {
        match self {
            Message::Staging(message) => message.as_str(),
            Message::Chat(message) => message.as_str()
        }
    }
    pub fn is_staging(&self) -> bool {
        matches!(self, Message::Staging(_))
    }

    pub fn new_of_kind<S: AsRef<str>>(text: S, other: &Message) -> Self {
        match other {
            Message::Staging(_) => Message::Staging(text.as_ref().to_string()),
            Message::Chat(_) => Message::Chat(text.as_ref().to_string())
        }
    }

}

impl core::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let props = match self {
            Message::Staging(message) => format!("(kind=staging, message={})", message),
            Message::Chat(message) => format!("(kind=chat, message={})", message)
        };
        write!(f, "{}", props)
    }
}


#[derive(Debug)]
pub struct Room {
    /// A source of truth for currently active members.
    members: HashMap<MemberID, String>,

    /// Someone will let us know when we receive a message from a member.
    message_received_from_member: mpsc::UnboundedReceiver<(MemberID, Message)>,

    /// We'll tell transport what message to send and to which member.
    send_to_member: mpsc::UnboundedSender<(MemberID, Message)>,

    /// Someone will let us know when a client disconnects.
    client_disconnected_rx: mpsc::UnboundedReceiver<MemberID>,

    /// Someone will let us know when a client connects with a given name.
    client_connected_with_name_rx: mpsc::UnboundedReceiver<(MemberID, String)>,
}


impl Room {
    #[tracing::instrument(skip_all)]
    pub fn new(
        message_received_from_member: mpsc::UnboundedReceiver<(MemberID, Message)>,
        send_to_member: mpsc::UnboundedSender<(MemberID, Message)>,
        client_disconnected_rx: mpsc::UnboundedReceiver<MemberID>,
        client_connected_with_name_rx: mpsc::UnboundedReceiver<(MemberID, String)>
    ) -> Self {
        Self {
            message_received_from_member,
            send_to_member,
            client_disconnected_rx,
            client_connected_with_name_rx,
            members: Default::default(),
        }
    }

    /// Given the member_id just disconnected, send messages to others about it.
    #[tracing::instrument(skip(self))]
    pub async fn notify_others_of_disconnection(&self, disconnected_member: MemberID) {
        let Some(disconnected_member_name) = self.get_name(&disconnected_member) else {
            error!("Don't know the name of the member who just disconnected");
            return;
        };

        self
        .members
        .keys()
        .filter(|&member_id| *member_id != disconnected_member)
        .for_each(|member_id| {
            self.send_to_member.send(
                (
                    member_id.clone(), 
                    format!("* {} has left the room", disconnected_member_name).into()
                )
            )
            .unwrap();
        });
    }

    /// Given the member_id just connected (after finding a right name), send messages to everyone else
    /// that this member has connected.
    #[tracing::instrument(skip(self))]
    pub fn notify_others_of_new_member(&self, connected_member: MemberID) {
        let Some(connected_member_name) = self.get_name(&connected_member) else {
            error!("Don't know who to notify about...");
            return;
        };
        let others = 
            self
            .members
            .keys()
            .filter(|&member_id| *member_id != connected_member)
            .cloned()
            .collect::<Vec<_>>();

        debug!(
            others = ?others,
            connected_member_name = %connected_member_name,
            connected_member_id = %connected_member,
            "Notifying others of new member",
        );

        others
        .into_iter()
        .for_each(|member_id| {
            self.send_to_member.send(
                (
                    member_id.clone(),
                    format!("* {} has entered the room", connected_member_name).into()
                )
            )
            .unwrap();
        });
    }

    #[tracing::instrument(skip(self))]
    pub fn notify_member_of_other_members(&self, newly_connected_member: MemberID) {


        let existing_member_names: Vec<_> = 
        self
        .members
        .keys()
        .filter(|&member_id| *member_id != newly_connected_member)
        .filter_map(|member_id| self.get_name(member_id))
        .collect();
        let message = format!("* The room contains: {}", existing_member_names.join(", "));

        debug!(
            existing_member_names = ?existing_member_names,
            newly_connected_member = %newly_connected_member,
            newly_connected_member_name = ?(self.get_name(&newly_connected_member)),
            message = %message,
            "Notifying connected member of the existing members",
        );
        self.send_to_member.send((newly_connected_member, message.into())).unwrap();
    }

    #[tracing::instrument(skip(self), fields(self.members = ?self.members))]
    pub async fn run(&mut self) {
        info!("Starting Room...");
        loop {
            tokio::select! {
                Some((new_member, new_member_name)) = self.client_connected_with_name_rx.recv() => {
                    info!("Client {} connected with name: {}", new_member, new_member_name);
                    self.add_member(new_member, &new_member_name);
                    self.notify_others_of_new_member(new_member);
                    self.notify_member_of_other_members(new_member);
                    info!("After adding member and notifying: {:#?}", self.members);
                },
                Some(disconnected_member) = self.client_disconnected_rx.recv() => {
                    warn!("Recvd client_disconnected");
                    self.notify_others_of_disconnection(disconnected_member).await;
                    self.remove_member(disconnected_member);
                },
                Some((sender, msg)) = self.message_received_from_member.recv() => {
                    debug!("Received message from sender that will be broadcasted to others.");
                    self.broadcast_message_to_other_members_except(&sender, &msg);
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn broadcast_message_to_other_members_except(
        &self, 
        except_member_id: &MemberID, 
        message: &Message
    ) {

        let others: Vec<_> = 
            self
            .members
            .keys()
            .filter(|&member_id| member_id != except_member_id)
            .cloned()
            .collect();

        debug!(
            others = ?others,
            sender_id = %except_member_id,
            sender_name = ?self.get_name(except_member_id),
            "Broadcasting message to other members except",
        );


        others
        .into_iter()
        .for_each(|member_id| {
            if let Some(message_prefixed) = self.create_message_from_member(&except_member_id, message) {
                self.send_to_member.send(
                    (
                        member_id.clone(),
                        message_prefixed
                    )
                ).unwrap();
            }
        });
    }

    #[tracing::instrument(skip(self))]
    pub fn create_message_from_member(&self, member_id: &MemberID, message: &Message) -> Option<Message> {
        self
        .get_name(member_id)
        .map(|member_name| {
            format!("[{}] {}", member_name, message.as_str().trim()).into()
        })
    }


    #[tracing::instrument(skip(self))]
    pub fn add_member(&mut self, member_id: MemberID, member_name: &str) {
        self.members.insert(member_id, member_name.to_string());
    }

    #[tracing::instrument(skip(self))]
    pub fn remove_member(&mut self, member_id: MemberID) {
        self.members.remove(&member_id);
    }

    #[tracing::instrument(skip(self))]
    pub fn get_name(&self, member_id: &MemberID) -> Option<String> {
        self.members.get(member_id).cloned()
    }

}