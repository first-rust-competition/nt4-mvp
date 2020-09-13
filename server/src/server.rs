use crate::client::ConnectedClient;
use proto::prelude::{NTTextMessage, NTBinaryMessage, NTMessage, MessageValue, DataType, MessageBody, NTValue};
use std::collections::HashMap;
use async_std::sync::{Arc, Mutex, Sender, channel, Receiver};
use async_std::task;
use async_std::net::TcpListener;
use crate::net::NTSocket;
use rand::Rng;
use crate::entry::Topic;
use proto::prelude::directory::Announce;
use std::ops::DerefMut;
use itertools::Itertools;
use std::time::Duration;
use futures::{StreamExt, SinkExt};
use async_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
use async_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use async_tungstenite::tungstenite::protocol::CloseFrame;
use async_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use std::borrow::Cow;
use async_tungstenite::tungstenite::{Message, Error};

pub static MAX_BATCHING_SIZE: usize = 5;

pub struct NTServer {
    clients: HashMap<u32, ConnectedClient>,
    entries: HashMap<String, Topic>,
    pub_count: HashMap<String, usize>,
}

async fn tcp_loop(state: Arc<Mutex<NTServer>>, tx: Sender<ServerMessage>) -> anyhow::Result<()> {
    let mut listener = TcpListener::bind("0.0.0.0:5810").await?;

    while let Ok((sock, addr)) = listener.accept().await {
        log::info!("TCP connection at {}", addr);
        let cid = rand::random::<u32>();
        let sock = async_tungstenite::accept_hdr_async(sock, |req: &Request, mut res: Response| {
            let ws_proto = req.headers().iter().find(|(hdr, _)| **hdr == "Sec-WebSocket-Protocol");

            match ws_proto.map(|(_, s)| s.to_str().unwrap()) {
                Some("networktables.first.wpi.edu") => {
                    res.headers_mut().insert("Sec-WebSocket-Protocol", HeaderValue::from_static("networktables.first.wpi.edu"));
                    Ok(res)
                }
                _ => {
                    log::error!("Rejecting client that did not specify correct subprotocol");
                    Err(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Some("Protocol 'networktables.first.wpi.edu' required to communicate with this server".to_string()))
                        .unwrap())
                }
            }
        }).await;

        if let Ok(sock) = sock {
            log::info!("Client assigned CID {}", cid);
            let client = ConnectedClient::new(NTSocket::new(sock), tx.clone(), cid);
            state.lock().await.clients.insert(cid, client);
            task::spawn(update_new_client(cid, state.clone()));
        }
    }
    Ok(())
}

async fn update_new_client(id: u32, state: Arc<Mutex<NTServer>>) {
    let mut state = state.lock().await;
    // thanks borrowck :ha:
    let state = state.deref_mut();
    let client = state.clients.get_mut(&id).unwrap();

    let mut batch = Vec::with_capacity(MAX_BATCHING_SIZE);
    for entry in state.entries.values() {
        batch.push(client.announce(entry).into_message());

        if batch.len() == MAX_BATCHING_SIZE {
            // Im pretty sure split_off effectively does the same thing as cloning and clearing but it hides it away
            client.send_message(NTMessage::Text(batch.split_off(0))).await;
        }
    }

    if batch.len() > 0 && batch.len() < MAX_BATCHING_SIZE {
        client.send_message(NTMessage::Text(batch)).await;
    }
}


async fn channel_loop(state: Arc<Mutex<NTServer>>, rx: Receiver<ServerMessage>) -> anyhow::Result<()> {
    while let Ok(msg) = rx.recv().await {
        match msg {
            ServerMessage::ClientDisconnected(cid) => {
                log::info!("Received disconnect from CID {}", cid);
                state.lock().await.clients.remove(&cid);
            }
            ServerMessage::ControlMessage(msg, cid) => {
                let mut state = state.lock().await;
                let state = state.deref_mut();

                for msg in msg {
                    match msg.data() {
                        MessageValue::PublishReq(req) => {
                            log::info!("Received publish request");
                            state.create_entry(req.name.clone(), req._type);
                            let entry = &state.entries[&req.name];

                            for client in state.clients.values_mut() {
                                let msg = client.announce(entry).into_message();
                                client.send_message(NTMessage::single_text(msg)).await;
                            }
                        }
                        MessageValue::PublishRel(rel) => {
                            let publishers = state.pub_count.get_mut(&rel.name).unwrap();
                            *publishers -= 1;

                            if *publishers == 0 {
                                let entry = state.entries.remove(&rel.name).unwrap();
                                for client in state.clients.values_mut() {
                                    let msg = client.unannounce(&entry).into_message();
                                    client.send_message(NTMessage::single_text(msg)).await;
                                }
                                state.pub_count.remove(&rel.name);
                            }
                        }
                        MessageValue::GetValues(gv) => {
                            let client = state.clients.get(&cid).unwrap();

                            let packets = gv.ids.into_iter()
                                .map(|id| (id, client.id_to_name(id).unwrap()))
                                .map(|(id, name)| NTBinaryMessage {
                                    id,
                                    timestamp: 0,
                                    value: state.entries[name].value.clone(),
                                })
                                .chunks(MAX_BATCHING_SIZE)
                                .into_iter()
                                .map(|batch| NTMessage::Binary(batch.collect()))
                                .collect::<Vec<NTMessage>>();

                            let client = state.clients.get_mut(&cid).unwrap();
                            for packet in packets {
                                client.send_message(packet).await;
                            }
                        }
                        MessageValue::Subscribe(sub) => {
                            let client = state.clients.get_mut(&cid).unwrap();
                            client.subscribe(sub);
                        }
                        MessageValue::Unsubscribe(unsub) => {
                            let client = state.clients.get_mut(&cid).unwrap();
                            client.unsubscribe(unsub);
                        }
                        //TODO
                        MessageValue::SetFlags(set) => {}
                        _ => {}
                    }
                }
            }
            ServerMessage::ValuePublish(values, cid) => {
                let mut state = state.lock().await;
                let state = state.deref_mut();

                let client = &state.clients[&cid];

                for msg in values {
                    let name = client.id_to_name(msg.id).unwrap();
                    log::info!("Received update message for name {}. New value {:?}", name, msg.value);

                    let entry = state.entries.get_mut(name).unwrap();
                    entry.set_value(msg.value);
                }

                // for (_, client) in state.clients.iter_mut().filter(|(id, _)| **id != cid) {
                //     let values = values.clone().into_iter()
                //         .filter(|update| client.subscribed_to(update.id))
                //         .collect::<Vec<NTBinaryMessage>>();
                //
                //     client.send_message(NTMessage::Binary(values)).await;
                // }
            }
        }
    }

    Ok(())
}

async fn broadcast_loop(state: Arc<Mutex<NTServer>>) {
    let mut interval = async_std::stream::interval(Duration::from_millis(100));

    while let Some(()) = interval.next().await {
        let mut state = state.lock().await;
        let state = state.deref_mut();

        for (cid, client) in state.clients.iter_mut() {
            let sub_prefixes = client.subs.clone().into_iter()
                .map(|(_, sub)| sub)
                .flat_map(|sub| sub.prefixes)
                .collect::<Vec<String>>();

            for prefix in sub_prefixes {
                let messages = state.entries.iter()
                    .filter(move |(name, _)| name.starts_with(&prefix))
                    .filter(|(_, topic)| topic.is_dirty())
                    .map(|(key, topic)| (client.lookup_id(key).unwrap(), topic.value.clone()))
                    .map(|(id, value)| NTBinaryMessage {
                        id,
                        timestamp: 0,
                        value,
                    })
                    .chunks(MAX_BATCHING_SIZE)
                    .into_iter()
                    .map(|batch| NTMessage::Binary(batch.collect()))
                    .collect::<Vec<NTMessage>>();

                for msg in messages {
                    client.send_message(msg).await;
                }
            }
        }

        state.mark_clean();
    }
}

impl NTServer {
    pub fn new() -> Arc<Mutex<NTServer>> {
        let _self = Arc::new(Mutex::new(NTServer { clients: HashMap::new(), entries: HashMap::new(), pub_count: HashMap::new() }));

        let (tx, rx) = channel(32);

        task::spawn(tcp_loop(_self.clone(), tx));
        task::spawn(channel_loop(_self.clone(), rx));
        task::spawn(broadcast_loop(_self.clone()));

        _self
    }

    //TODO: Doesn't handle duplicate names
    pub fn create_entry(&mut self, name: String, _type: DataType) {
        let entry = Topic::new(name.clone(), _type);
        self.pub_count.insert(name.clone(), 1);
        self.entries.insert(name, entry);
    }

    pub fn mark_clean(&mut self) {
        for entry in self.entries.values_mut() {
            entry.clear_dirty();
        }
    }
}

pub enum ServerMessage {
    ControlMessage(Vec<NTTextMessage>, u32),
    ValuePublish(Vec<NTBinaryMessage>, u32),
    ClientDisconnected(u32),
}
