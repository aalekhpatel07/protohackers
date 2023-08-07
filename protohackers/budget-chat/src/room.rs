use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::mpsc::{self, unbounded_channel};
use tracing::{trace, error, warn, span, debug_span, debug};

use crate::MemberID;
use crate::{BudgetChatError, ClientInitializationError};



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
    pub async fn notify_others_of_disconnection(&mut self, disconnected_member: MemberID) {
        let disconnected_member_name = self.get_name(&disconnected_member).unwrap();

        self
        .members
        .keys()
        .filter(|&member_id| *member_id != disconnected_member)
        .for_each(|member_id| {
            _ = self.send_to_member.send(
                (
                    member_id.clone(), 
                    format!("* {} has left the room", disconnected_member_name)
                )
            );
        });
    }

    /// Given the member_id just connected (after finding a right name), send messages to everyone else
    /// that this member has connected.
    pub fn notify_others_of_new_member(&mut self, connected_member: MemberID) {
        let conncted_member_name = self.get_name(&connected_member).unwrap();

        self
        .members
        .keys()
        .filter(|&member_id| *member_id != connected_member)
        .for_each(|member_id| {
            _ = self.send_to_member.send(
                (
                    member_id.clone(), 
                    format!("* {} has entered the room", conncted_member_name)
                )
            );
        });
    }

    pub async fn run(&mut self) -> crate::Result<()> {

        loop {
            tokio::select! {
                Some(disconnected_member) = self.client_disconnected_rx.recv() => {
                    self.notify_others_of_disconnection(disconnected_member).await;
                },
                Some((new_member, new_member_name)) = self.client_connected_with_name_rx.recv() => {
                    self.add_member(new_member, &new_member_name)?;
                    self.notify_others_of_new_member(new_member);
                },
                Some((sender, msg)) = self.message_received_from_member.recv() => {
                    self
                    .members
                    .keys()
                    .filter(|&member_id| *member_id != sender)
                    .for_each(|member_id| {
                        _ = self.send_to_member.send(
                            (
                                member_id.clone(), 
                                self.create_message_from_member(member_id, &msg)
                            )
                        );
                    });
                },
            }
        }
    }

    pub fn create_message_from_member(&self, member_id: &MemberID, message: &str) -> String {
        self
        .members
        .get(member_id)
        .map(|member_name| {
            format!("[{}] {}", member_name, message)
        })
        .unwrap()
    }


    pub fn add_member(&mut self, member_id: MemberID, member_name: &str) -> crate::Result<()> {
        self.members.insert(member_id, member_name.to_string());
        Ok(())
    }

    pub fn get_name(&self, member_id: &MemberID) -> Option<String> {
        self.members.get(member_id).cloned()
    }

}