//! # The main loop of the NT4 server
//! j
//! This module contains most of the top-level logic for handling client events in the function [`channel_loop`]
//!
//! The task spawned from [`channel_loop`] runs until the server is shut down, and responds to messages from all clients connected
//! to the server. These can range from disconnection events, published values, or text control messages
//!
//! [`channel_loop`]: ./fn.channel_loop.html
use std::sync::Arc;
use async_std::sync::{Mutex, Receiver, channel};
use crate::server::{NTServer, ServerMessage, MAX_BATCHING_SIZE};
use proto::prelude::{NTMessage, MessageBody, MessageValue, NTBinaryMessage};
use std::ops::DerefMut;
use crate::util::batch_messages;
use async_std::task;
use crate::server::broadcast::broadcast_with_period;
use chrono::Local;
use crate::client::TopicSnapshot;

/// The main loop of the NT4 server
///
/// This task is spawned when the server is started and runs indefinitely until the runtime is shut down.
/// It handles messages communicated by clients encapsulated in a [`ServerMessage`]. There are three
/// types of [`ServerMessage`] that must be handled:
///
/// [`ServerMessage::ClientDisconnected`] : Be it because of a WS CLOSE frame, or because the socket was closed without a proper handshake,
/// a client that was being served by the server has now disconnected. The handling of this cleans up any published values from this client, and closes
/// any custom loop tasks associated with its subscriptions.
///
/// [`ServerMessage::ValuePublish`] : Sent by a client handle when it receives new values in the form of a WS BIN frame. The updates in this message
/// update the internal state of the server, are pushed to clients with immediate subscriptions, and are queued to be dispatched by a subscription task otherwise.
/// When the server receives a Timestamp Synchronization message (ID -1) it will immediately respond to the client with its local time, to minimize drift caused by a delayed response.
///
/// [`ServerMessage::ControlMessage`] : Sent by a client handle when it receives a WS TEXT frame that contains valid NT4 messages. These values update internal state as appropriate,
/// and may result in messages being dispatched to connected clients (e.g. an Announce message in response to a new published value)
///
/// [`ServerMessage`]: ../enum.ServerMessage.html
/// [`ServerMessage::ClientDisconnected`]: ../enum.ServerMessage.html#variant.ClientDisconnected
/// [`ServerMessage::ValuePublish`]: ../enum.ServerMessage.html#variant.ValuePublish
/// [`ServerMessage::ControlMessage`]: ../enum.ServerMessage.html#variant.ControlMessage
pub async fn channel_loop(
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
