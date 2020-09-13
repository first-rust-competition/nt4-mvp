use crate::entry::Topic;
use crate::net::NTSocket;
use crate::server::{NTServer, ServerMessage};
use async_std::sync::{MutexGuard, Sender};
use async_std::task;
use futures::prelude::*;
use futures::stream::{SplitSink, SplitStream};
use log::*;
use proto::prelude::directory::{Announce, Unannounce};
use proto::prelude::subscription::{Subscribe, Unsubscribe};
use proto::prelude::{MessageValue, NTMessage, NTTextMessage};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Subscription {
    pub prefixes: Vec<String>,
    pub immediate: bool,
    pub periodic: f64,
    pub logging: bool,
}

pub struct ConnectedClient {
    net_tx: SplitSink<NTSocket, NTMessage>,
    pub subs: HashMap<u32, Subscription>,
    pub pub_ids: HashMap<String, i32>,
    next_pub_id: i32,
}

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
            pub_ids: HashMap::new(),
            next_pub_id: 1,
        }
    }

    pub fn subscribed_to(&self, prefix: &str) -> bool {
        self.subs
            .values()
            .any(|sub| sub.prefixes.iter().any(|it| it == prefix))
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

    pub fn ids_matching_prefix(&self, prefix: &str) -> Vec<i32> {
        self.pub_ids
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(_, id)| *id)
            .collect()
    }

    pub fn subscribe(&mut self, packet: Subscribe) {
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

        self.subs.insert(packet.subuid, sub);
    }

    pub fn unsubscribe(&mut self, packet: Unsubscribe) {
        self.subs.remove(&packet.subuid);
    }

    pub async fn send_message(&mut self, msg: NTMessage) {
        self.net_tx.send(msg).await;
    }
}
