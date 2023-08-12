use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{warn, info};
use crate::MemberID;


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
    pub async fn notify_others_of_disconnection(&mut self, disconnected_member: MemberID) {
        let disconnected_member_name = self.get_name(&disconnected_member).unwrap();

        self
        .members
        .keys()
        .filter(|&member_id| *member_id != disconnected_member)
        .for_each(|member_id| {
            self.send_to_member.send(
                (
                    member_id.clone(), 
                    format!("* {} has left the room", disconnected_member_name)
                )
            )
            .unwrap();
        });
    }

    /// Given the member_id just connected (after finding a right name), send messages to everyone else
    /// that this member has connected.
    #[tracing::instrument(skip(self))]
    pub fn notify_others_of_new_member(&mut self, connected_member: MemberID) {
        let conncted_member_name = self.get_name(&connected_member).unwrap();

        self
        .members
        .keys()
        .filter(|&member_id| *member_id != connected_member)
        .for_each(|member_id| {
            self.send_to_member.send(
                (
                    member_id.clone(), 
                    format!("* {} has entered the room", conncted_member_name)
                )
            )
            .unwrap();
        });
    }

    #[tracing::instrument(skip(self), fields(self.members = ?self.members))]
    pub async fn run(&mut self) {
        info!("Starting Room...");
        loop {
            tokio::select! {
                Some(disconnected_member) = self.client_disconnected_rx.recv() => {
                    warn!("Recvd client_disconnected");
                    self.notify_others_of_disconnection(disconnected_member).await;
                    self.remove_member(disconnected_member);
                },
                Some((new_member, new_member_name)) = self.client_connected_with_name_rx.recv() => {
                    info!("Client {} connected with name: {}", new_member, new_member_name);
                    self.add_member(new_member, &new_member_name);
                    self.notify_others_of_new_member(new_member);
                    info!("After adding member and notifying: {:#?}", self.members);
                },
                Some((sender, msg)) = self.message_received_from_member.recv() => {
                    self.broadcast_message_to_other_members_except(&sender, &msg);
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn broadcast_message_to_other_members_except(
        &self, 
        except_member_id: &MemberID, message: &str) {
        self
        .members
        .keys()
        .filter(|&member_id| member_id != except_member_id)
        .for_each(|member_id| {
            self.send_to_member.send(
                (
                    member_id.clone(), 
                    self.create_message_from_member(member_id, &message)
                )
            ).unwrap();
        });
    }

    #[tracing::instrument(skip(self))]
    pub fn create_message_from_member(&self, member_id: &MemberID, message: &str) -> String {
        self
        .members
        .get(member_id)
        .map(|member_name| {
            format!("[{}] {}\n", member_name, message.trim())
        })
        .unwrap()
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