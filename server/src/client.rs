use crate::entry::Topic;
use crate::net::NTSocket;
use crate::server::ServerMessage;
use async_std::sync::Sender;
use async_std::task;
use futures::prelude::*;
use futures::stream::{SplitSink, SplitStream};
use log::*;
use proto::prelude::directory::{Announce, Unannounce};
use proto::prelude::subscription::{Subscribe, Unsubscribe};
use proto::prelude::{NTMessage, NTValue};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Subscription {
    pub prefixes: Vec<String>,
    pub immediate: bool,
    pub periodic: f64,
    pub logging: bool,
}

#[derive(PartialEq, Clone, Debug)]
pub struct TopicSnapshot {
    pub name: String,
    pub value: NTValue,
    pub timestamp: u64,
}

pub struct ConnectedClient {
    net_tx: SplitSink<NTSocket, NTMessage>,
    pub subs: HashMap<u32, Subscription>,
    pub sub_loop_channels: HashMap<u32, Sender<()>>,
    pub pub_ids: HashMap<String, i32>,
    pub pubs: Vec<String>,
    pub queued_updates: Vec<TopicSnapshot>,
    next_pub_id: i32,
}

/// A loop over the read-half of a connected peer
///
/// This loop continuously reads from the stream, forwarding applicable messages and reporting errors where applicable
///
/// When the stream is closed it notifies the server with [`ServerMessage::ClientDisconnected`] and is terminated.
///
/// [`ServerMessage::ClientDisconnected`]: ../server/enum.ServerMessage.html#variant.ClientDisconnected
async fn client_loop(
    mut rx: SplitStream<NTSocket>,
    tx: Sender<ServerMessage>,
    cid: u32,
) -> anyhow::Result<()> {
    while let Some(packet) = rx.next().await {
        match packet {
            Ok(msg) => match msg {
                NTMessage::Text(msg) => tx.send(ServerMessage::ControlMessage(msg, cid)).await,
                NTMessage::Binary(msgs) => tx.send(ServerMessage::ValuePublish(msgs, cid)).await,
                NTMessage::Close => {
                    tx.send(ServerMessage::ClientDisconnected(cid)).await;
                    return Ok(());
                }
            },
            Err(e) => error!("Encountered error decoding frame from client: {}", e),
        }
    }

    log::info!("Client loop for CID {} terminated", cid);
    tx.send(ServerMessage::ClientDisconnected(cid)).await;

    Ok(())
}

impl ConnectedClient {
    pub fn new(sock: NTSocket, server_tx: Sender<ServerMessage>, cid: u32) -> ConnectedClient {
        let (net_tx, net_rx) = sock.split();

        task::spawn(client_loop(net_rx, server_tx, cid));

        ConnectedClient {
            net_tx,
            subs: HashMap::new(),
            sub_loop_channels: HashMap::new(),
            pub_ids: HashMap::new(),
            queued_updates: Vec::new(),
            next_pub_id: 1,
            pubs: Vec::new()
        }
    }

    pub fn subscribed_to(&self, name: &str) -> bool {
        self.subs
            .values()
            .any(|sub| sub.prefixes.iter().any(|prefix| name.starts_with(prefix)))
    }

    pub fn subscription(&self, name: &str) -> Option<&Subscription> {
        self.subs.values()
            .find(|sub| sub.prefixes.iter().any(|prefix| name.starts_with(prefix)))
    }

    pub fn announce(&mut self, entry: &Topic) -> Announce {
        let id = self.next_pub_id;
        self.next_pub_id += 1;
        self.pub_ids.insert(entry.name.clone(), id);
        Announce {
            name: entry.name.clone(),
            id,
            _type: entry.entry_type(),
            flags: entry.flags.clone(),
        }
    }

    pub fn unannounce(&mut self, entry: &Topic) -> Unannounce {
        let id = self.pub_ids.remove(&entry.name).unwrap();
        Unannounce {
            name: entry.name.clone(),
            id,
        }
    }

    pub fn id_to_name(&self, id: i32) -> Option<&String> {
        self.pub_ids
            .iter()
            .find(|(_, pub_id)| **pub_id == id)
            .map(|(name, _)| name)
    }

    pub fn lookup_id(&self, name: &str) -> Option<i32> {
        self.pub_ids.get(name).map(|id| *id)
    }

    pub fn subscribe(&mut self, packet: Subscribe) -> Subscription {
        let mut sub = Subscription {
            prefixes: packet.prefixes,
            immediate: false,
            periodic: 0.0,
            logging: false,
        };

        if let Some(opts) = packet.options {
            sub.immediate = opts.immediate;
            sub.logging = opts.logging;
            sub.periodic = opts.periodic;
        }

        self.subs.insert(packet.subuid, sub.clone());
        sub
    }

    pub fn subscribe_channel(&mut self, subuid: u32, ch: Sender<()>) {
        self.sub_loop_channels.insert(subuid, ch);
    }

    pub async fn unsubscribe(&mut self, packet: Unsubscribe) {
        self.subs.remove(&packet.subuid);
        if let Some(ch) = self.sub_loop_channels.remove(&packet.subuid) {
            ch.send(()).await;
        }
    }

    pub async fn send_message(&mut self, msg: NTMessage) {
        let _ = self.net_tx.send(msg).await;
    }
}
