use crate::MemberID;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub type Message = String;


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
        client_connected_with_name_rx: mpsc::UnboundedReceiver<(MemberID, String)>,
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

        self.members
            .keys()
            .filter(|&member_id| *member_id != disconnected_member)
            .for_each(|member_id| {
                self.send_to_member
                    .send((
                        *member_id,
                        format!("* {} has left the room", disconnected_member_name).into(),
                    ))
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
        let others = self
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

        others.into_iter().for_each(|member_id| {
            self.send_to_member
                .send((
                    member_id,
                    format!("* {} has entered the room", connected_member_name).into(),
                ))
                .unwrap();
        });
    }

    #[tracing::instrument(skip(self))]
    pub fn notify_member_of_other_members(&self, newly_connected_member: MemberID) {
        let existing_member_names: Vec<_> = self
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
        self.send_to_member
            .send((newly_connected_member, message.into()))
            .unwrap();
    }

    #[tracing::instrument(skip(self), fields(self.members = ?self.members))]
    pub async fn run(&mut self) {
        info!("Starting Room...");
        loop {
            tokio::select! {
                Some((new_member, new_member_name)) = self.client_connected_with_name_rx.recv() => {
                    debug!("Client {} connected with name: {}", new_member, new_member_name);
                    info!("* {} has entered the room.", new_member_name);
                    self.add_member(new_member, &new_member_name);
                    info!(
                        "* The room contains: {}, {}",
                        new_member_name,
                        self.member_names_except(&new_member).join(", ")
                    );
                    self.notify_others_of_new_member(new_member);
                    self.notify_member_of_other_members(new_member);
                    debug!("After adding member and notifying: {:#?}", self.members);
                },
                Some(disconnected_member) = self.client_disconnected_rx.recv() => {
                    self.notify_others_of_disconnection(disconnected_member).await;
                    self.remove_member(disconnected_member);
                    debug!("Recvd client_disconnected");
                    info!(
                        "* {:?} has left the room",
                        self.get_name(&disconnected_member)
                    );
                },
                Some((sender, msg)) = self.message_received_from_member.recv() => {
                    info!(
                        "[{:?}] {}",
                        self.get_name(&sender),
                        msg
                    );
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
        message: &Message,
    ) {
        let others: Vec<_> = self
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

        others.into_iter().for_each(|member_id| {
            if let Some(message_prefixed) =
                self.create_message_from_member(except_member_id, message)
            {
                self.send_to_member
                    .send((member_id, message_prefixed))
                    .unwrap();
            }
        });
    }

    #[tracing::instrument(skip(self))]
    pub fn create_message_from_member(
        &self,
        member_id: &MemberID,
        message: &Message,
    ) -> Option<Message> {
        self.get_name(member_id)
            .map(|member_name| format!("[{}] {}", member_name, message.as_str().trim()).into())
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

    fn members_except(&self, member_id: &MemberID) -> Vec<(MemberID, String)> {
        self
        .members
        .iter()
        .filter(|(stored_member_id, _)| {
            *stored_member_id != member_id
        })
        .map(|(id, name)| (*id, name.clone()))
        .collect()
    }

    fn member_names_except(&self, member_id: &MemberID) -> Vec<String> {
        self.members_except(member_id)
        .into_iter()
        .map(|(_, name)| name.clone())
        .collect()
    }

}
