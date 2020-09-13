use crate::client::{ConnectedClient, TopicSnapshot, Subscription};
use crate::entry::Topic;
use crate::net::NTSocket;
use async_std::net::TcpListener;
use async_std::sync::{channel, Arc, Mutex, Receiver, Sender};
use async_std::task;
use async_tungstenite::tungstenite::handshake::server::{Request, Response};
use async_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use chrono::offset::Local;
use futures::StreamExt;
use futures::future::{select, Either};
use itertools::Itertools;
use proto::prelude::{
    DataType, MessageBody, MessageValue, NTBinaryMessage, NTMessage, NTTextMessage,
};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::time::Duration;
use crate::util::batch_messages;

pub static MAX_BATCHING_SIZE: usize = 5;

pub struct NTServer {
    clients: HashMap<u32, ConnectedClient>,
    entries: HashMap<String, Topic>,
    pub_count: HashMap<String, usize>,
}

async fn tcp_loop(state: Arc<Mutex<NTServer>>, tx: Sender<ServerMessage>) -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:5810").await?;

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

    let batches = state
        .entries
        .values()
        .map(|value| client.announce(value).into_message())
        .chunks(MAX_BATCHING_SIZE)
        .into_iter()
        .map(|batch| NTMessage::Text(batch.collect()))
        .collect::<Vec<NTMessage>>();

    for msg in batches {
        client.send_message(msg).await;
    }
}

async fn channel_loop(
    state_lock: Arc<Mutex<NTServer>>,
    rx: Receiver<ServerMessage>,
) -> anyhow::Result<()> {
    while let Ok(msg) = rx.recv().await {
        match msg {
            ServerMessage::ClientDisconnected(cid) => {
                log::info!("Received disconnect from CID {}", cid);
                let mut state = state_lock.lock().await;
                if let Some(client) = state.clients.remove(&cid) {
                    for key in client.pubs.into_iter() {
                        let delete = match state.pub_count.get_mut(&key) {
                            Some(cnt) => {
                                *cnt -= 1;
                                *cnt == 0
                            }
                            None => false
                        };

                        if delete {
                            log::info!("Deleting topic {}. No remaining publishers after client disconnection.", key);
                            let topic = state.entries.remove(&key).unwrap();
                            for client in state.clients.values_mut() {
                                let msg = NTMessage::single_text(client.unannounce(&topic).into_message());
                                client.send_message(msg).await;
                            }
                            state.pub_count.remove(&key);
                        }
                    }
                }

            }
            ServerMessage::ControlMessage(msg, cid) => {
                let mut state = state_lock.lock().await;
                let state = state.deref_mut();

                for msg in msg {
                    match msg.data() {
                        MessageValue::PublishReq(req) => {
                            log::info!("Received publish request");
                            state.create_entry(req.name.clone(), req._type);
                            let entry = &state.entries[&req.name];

                            let client = state.clients.get_mut(&cid).unwrap();
                            client.pubs.push(req.name);

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
                                log::info!("Deleting topic {}, received pubrel from last publisher", entry.name);
                                for client in state.clients.values_mut() {
                                    let msg = client.unannounce(&entry).into_message();
                                    client.send_message(NTMessage::single_text(msg)).await;
                                }
                                state.pub_count.remove(&rel.name);
                            }
                        }
                        MessageValue::GetValues(gv) => {
                            let client = state.clients.get(&cid).unwrap();

                            let mut updates = Vec::new();
                            for id in gv.ids {
                                let name = client.id_to_name(id).unwrap();
                                let entry = &state.entries[name];
                                updates.push(NTBinaryMessage {
                                    id,
                                    timestamp: entry.timestamp,
                                    value: entry.value.clone()
                                });
                            }
                            let client = state.clients.get_mut(&cid).unwrap();
                            for msg in batch_messages(updates, MAX_BATCHING_SIZE) {
                                client.send_message(msg).await;
                            }
                        }
                        MessageValue::Subscribe(sub) => {
                            let client = state.clients.get_mut(&cid).unwrap();

                            let mut updates = Vec::new();
                            for prefix in &sub.prefixes {
                                for (_, topic)in state.entries.iter().filter(|(key, _)| key.starts_with(prefix)) {
                                    updates.push(NTBinaryMessage {
                                        id: client.lookup_id(&topic.name).unwrap(),
                                        timestamp: topic.timestamp,
                                        value: topic.value.clone()
                                    });
                                }
                            }

                            for msg in batch_messages(updates, MAX_BATCHING_SIZE) {
                                client.send_message(msg).await;
                            }

                            let subuid = sub.subuid;
                            let sub = client.subscribe(sub);
                            if sub.periodic != 0.1 {
                                let (tx, rx) = channel(1);
                                client.subscribe_channel(subuid, tx);
                                task::spawn(broadcast_with_period(state_lock.clone(), cid, sub, rx));
                            }
                        }
                        MessageValue::Unsubscribe(unsub) => {
                            let client = state.clients.get_mut(&cid).unwrap();
                            client.unsubscribe(unsub).await;
                        }
                        //TODO
                        MessageValue::SetFlags(_set) => {}
                        _ => {}
                    }
                }
            }
            ServerMessage::ValuePublish(values, cid) => {
                let mut state = state_lock.lock().await;
                let state = state.deref_mut();

                let client = state.clients.get_mut(&cid).unwrap();

                let mut updates = Vec::new();
                for mut msg in values {
                    if msg.id == -1 {
                        // Timestamp communication
                        let now = Local::now();
                        let timestamp = now.timestamp_nanos() as u64 / 1000; // Timestamp in us since Unix Epoch
                        // Update the message timestamp and send it back
                        msg.timestamp = timestamp;
                        client.send_message(NTMessage::single_bin(msg)).await;
                        log::info!("Immediately sending timestamp to client");
                        continue;
                    }

                    log::info!("Performing lookup for ID {}", msg.id);
                    let name = client.id_to_name(msg.id).unwrap();
                    log::info!(
                        "Received update message for name {}. New value {:?}",
                        name,
                        msg.value
                    );

                    let entry = state.entries.get_mut(name).unwrap();
                    entry.set_value(msg.value, msg.timestamp);
                    updates.push(entry.snapshot());
                }

                for client in state.clients.values_mut() {
                    let iter = updates.iter().cloned().filter(|snapshot| client.subscribed_to(&snapshot.name))
                        .collect::<Vec<TopicSnapshot>>();

                    for topic in iter {
                        let sub = client.subscription(&topic.name).unwrap();
                        if sub.immediate {
                            client.send_message(NTMessage::single_bin(NTBinaryMessage {
                                id: client.lookup_id(&topic.name).unwrap(),
                                timestamp: topic.timestamp,
                                value: topic.value.clone()
                            })).await;
                        } else {
                            client.queued_updates.push(topic);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn broadcast_with_period(state: Arc<Mutex<NTServer>>, cid: u32, sub: Subscription, mut rx: Receiver<()>) {
    let mut interval = async_std::stream::interval(Duration::from_secs_f64(sub.periodic));

    let mut end = rx.next();

    loop {
        match select(interval.next(), end).await {
            Either::Left((Some(()), end_cnt)) => {
                log::info!("Custom broadcast interval triggered");
                end = end_cnt;
                let mut state = state.lock().await;
                let state = state.deref_mut();

                let client = state.clients.get_mut(&cid).unwrap();

                let mut updates = Vec::new();
                let mut names = Vec::new();
                log::info!("Queued updates: {:?}", client.queued_updates);
                for (name, group) in &client.queued_updates.iter()
                    .filter(|snapshot| sub.prefixes.iter().any(|prefix| snapshot.name.starts_with(prefix)))
                    .group_by(|snapshot| &snapshot.name) {

                    if sub.logging {
                        // Subscribers with this option need to receive every
                        for update in group {
                            updates.push(NTBinaryMessage {
                                id: client.lookup_id(name).unwrap(),
                                timestamp: update.timestamp,
                                value: update.value.clone()
                            });
                        }
                    } else {
                        let snapshot = group.max_by(|s1, s2| s1.timestamp.cmp(&s2.timestamp)).unwrap();
                        updates.push(NTBinaryMessage {
                            id: client.lookup_id(name).unwrap(),
                            timestamp: snapshot.timestamp,
                            value: snapshot.value.clone()
                        })
                    }

                    names.push(name.clone());
                }

                for msg in batch_messages(updates, MAX_BATCHING_SIZE) {
                    client.send_message(msg).await;
                }

                for name in names {
                    while let Some((idx, _)) = client.queued_updates.iter().enumerate().find(|(_, topic)| topic.name == name) {
                        client.queued_updates.remove(idx);
                    }
                }
            }
            _ => break,
        }
    }

    log::info!("Terminating custom loop for CID {}. (Had period {})", cid, sub.periodic);
}

async fn broadcast_loop(state: Arc<Mutex<NTServer>>) {
    let mut interval = async_std::stream::interval(Duration::from_millis(100));

    while let Some(()) = interval.next().await {
        let mut state = state.lock().await;
        let state = state.deref_mut();

        for client in state.clients.values_mut() {
            let mut updates = Vec::new();
            for (_, sub)in &client.subs {
                if sub.periodic != 0.1 {
                    continue;
                }
                if sub.logging {
                    // Subscribers with this option need to receive every
                    for (name, group) in &client.queued_updates.iter()
                        .group_by(|snapshot| &snapshot.name)
                    {
                        for update in group {
                            updates.push(NTBinaryMessage {
                                id: client.lookup_id(name).unwrap(),
                                timestamp: update.timestamp,
                                value: update.value.clone()
                            });
                        }
                    }
                } else {
                    for (name, group) in &client.queued_updates.iter()
                        .group_by(|snapshot| &snapshot.name)
                    {
                        let snapshot = group.max_by(|s1, s2| s1.timestamp.cmp(&s2.timestamp)).unwrap();
                        updates.push(NTBinaryMessage {
                            id: client.lookup_id(name).unwrap(),
                            timestamp: snapshot.timestamp,
                            value: snapshot.value.clone()
                        })
                    }
                }

                client.queued_updates.clear();
            }

            for msg in batch_messages(updates, MAX_BATCHING_SIZE) {
                client.send_message(msg).await;
            }
        }
    }
}

impl NTServer {
    pub fn new() -> Arc<Mutex<NTServer>> {
        let _self = Arc::new(Mutex::new(NTServer {
            clients: HashMap::new(),
            entries: HashMap::new(),
            pub_count: HashMap::new(),
        }));

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
}

pub enum ServerMessage {
    ControlMessage(Vec<NTTextMessage>, u32),
    ValuePublish(Vec<NTBinaryMessage>, u32),
    ClientDisconnected(u32),
}
