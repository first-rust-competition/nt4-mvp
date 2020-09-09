use crate::net::NTSocket;
use std::collections::HashMap;
use async_std::sync::{Sender, MutexGuard};
use async_std::task;
use crate::server::{ServerMessage, NTServer};
use proto::prelude::{NTMessage, MessageValue, NTTextMessage};
use futures::prelude::*;
use futures::stream::{SplitStream, SplitSink};
use log::*;

pub struct Subscription {
    ids: Vec<u32>,
    immediate: bool,
    periodic: f64,
    logging: bool,
    timestamped: bool,
}

pub struct ConnectedClient {
    net_tx: SplitSink<NTSocket, NTMessage>,
    subs: HashMap<u32, Subscription>,
}

async fn client_loop(mut rx: SplitStream<NTSocket>, tx: Sender<ServerMessage>, cid: u32) -> anyhow::Result<()> {

    while let Some(packet) = rx.next().await {
        match packet {
            Ok(msg) => match msg {
                NTMessage::Text(msg) => tx.send(ServerMessage::ControlMessage(msg, cid)).await,
                NTMessage::Binary(msgs) => tx.send(ServerMessage::ValuePublish(msgs, cid)).await,
                NTMessage::Close => {
                    tx.send(ServerMessage::ClientDisconnected(cid)).await;
                    return Ok(());
                }
            }
            Err(e) => error!("Encountered error decoding frame from client: {}", e),
        }
    }

    Ok(())
}

impl ConnectedClient {
    pub fn new(sock: NTSocket, server_tx: Sender<ServerMessage>, cid: u32) -> ConnectedClient {
        let (net_tx, net_rx) = sock.split();

        task::spawn(client_loop(net_rx, server_tx, cid));

        ConnectedClient {
            net_tx,
            subs: HashMap::new(),
        }
    }

    pub fn handle_control_message(&mut self, msg: NTTextMessage) {
        match msg.data() {
            MessageValue::PublishReq(_) => {}
            MessageValue::PublishAck(_) => {}
            MessageValue::PublishRel(_) => {}
            MessageValue::List(_) => {}
            MessageValue::Directory(_) => {}
            MessageValue::Listen(_) => {}
            MessageValue::Unlisten(_) => {}
            MessageValue::Announce(_) => {}
            MessageValue::Unannounce(_) => {}
            MessageValue::GetValues(_) => {}
            MessageValue::Subscribe(_) => {}
            MessageValue::Unsubscribe(_) => {}
        }
    }

    pub fn subscribed_to(&self, id: u32) -> bool {
        self.subs.values().any(|sub| sub.ids.iter().any(|it| *it == id))
    }

    pub async fn send_message(&mut self, msg: NTMessage) {
        self.net_tx.send(msg).await;
    }
}