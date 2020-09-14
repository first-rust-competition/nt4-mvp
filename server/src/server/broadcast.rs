//! A module containing functions broadcasting topic updates to clients
//!
//! There are two functions in this module, [`broadcast_loop`] and  [`broadcast_with_period`].
//! The only difference between these is that [`broadcast_loop`] is a singlet task, it is spawned when
//! the server starts and broadcasts message updates at the **default** period of 100ms to clients who
//! don't specify a different period, or immediate notification.
//!
//! [`broadcast_with_period`] is spawned when the server receives a subscription with a non-default period.
//! This functions identically to [`broadcast_loop`], albeit with the specified loop period. It also has a channel
//! which is used by the server to stop the loop when the client unsubscribes, be it because it disconnected or because it sent the unsubscribe message.
//!
//! [`broadcast_loop`]: ./fn.broadcast_loop.html
//! [`broadcast_with_period`]: ./fn.broadcast_with_period.html
use crate::client::Subscription;
use crate::server::{NTServer, MAX_BATCHING_SIZE};
use crate::util::batch_messages;
use async_std::sync::{Mutex, Receiver};
use futures::future::{select, Either};
use futures::StreamExt;
use itertools::Itertools;
use proto::prelude::NTBinaryMessage;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

/// Broadcasts topic updates associated with a specific subscription on a custom loop period
///
/// This task will send updates to the TCP stream associated with the given `cid` that are related to the specified [`Subscription`].
/// This task loops indefinitely until the client sends an Unsubscribe message, or until the client disconnects
/// and the send half of `rx` is dropped.
///
/// [`Subscription`]: ../../client/struct.Subscription.html
pub async fn broadcast_with_period(
    state: Arc<Mutex<NTServer>>,
    cid: u32,
    sub: Subscription,
    mut rx: Receiver<()>,
) {
    let mut interval = async_std::stream::interval(Duration::from_secs_f64(sub.periodic));

    let mut end = rx.next();

    loop {
        match select(interval.next(), end).await {
            Either::Left((Some(()), end_cnt)) => {
                end = end_cnt;
                let mut state = state.lock().await;
                let state = state.deref_mut();

                let client = state.clients.get_mut(&cid).unwrap();

                let mut updates = Vec::new();
                let mut names = Vec::new();
                for (name, group) in &client
                    .queued_updates
                    .iter()
                    .filter(|snapshot| {
                        sub.prefixes
                            .iter()
                            .any(|prefix| snapshot.name.starts_with(prefix))
                    })
                    .group_by(|snapshot| &snapshot.name)
                {
                    if sub.logging {
                        // Subscribers with this option need to receive every
                        for update in group {
                            updates.push(NTBinaryMessage {
                                id: client.lookup_id(name).unwrap(),
                                timestamp: update.timestamp,
                                value: update.value.clone(),
                            });
                        }
                    } else {
                        let snapshot = group
                            .max_by(|s1, s2| s1.timestamp.cmp(&s2.timestamp))
                            .unwrap();
                        updates.push(NTBinaryMessage {
                            id: client.lookup_id(name).unwrap(),
                            timestamp: snapshot.timestamp,
                            value: snapshot.value.clone(),
                        })
                    }

                    names.push(name.clone());
                }

                for msg in batch_messages(updates, MAX_BATCHING_SIZE) {
                    client.send_message(msg).await;
                }

                for name in names {
                    while let Some((idx, _)) = client
                        .queued_updates
                        .iter()
                        .enumerate()
                        .find(|(_, topic)| topic.name == name)
                    {
                        client.queued_updates.remove(idx);
                    }
                }
            }
            _ => break,
        }
    }

    log::info!(
        "Terminating custom loop for CID {}. (Had period {})",
        cid,
        sub.periodic
    );
}

/// Broadcast topic updates associated with all subscriptions on the default update period
///
/// This loop runs indefinitely until the server is shut down, and propagates pending updates related to subscriptions
/// that are not immediate, and that did not specify a custom loop time. It updates all clients with default period subscriptions at once.
pub async fn broadcast_loop(state: Arc<Mutex<NTServer>>) {
    let mut interval = async_std::stream::interval(Duration::from_millis(100));

    while let Some(()) = interval.next().await {
        let mut state = state.lock().await;
        let state = state.deref_mut();

        for client in state.clients.values_mut() {
            let mut updates = Vec::new();
            for (_, sub) in &client.subs {
                if sub.periodic != 0.1 {
                    continue;
                }
                if sub.logging {
                    // Subscribers with this option need to receive every
                    for (name, group) in &client
                        .queued_updates
                        .iter()
                        .group_by(|snapshot| &snapshot.name)
                    {
                        for update in group {
                            updates.push(NTBinaryMessage {
                                id: client.lookup_id(name).unwrap(),
                                timestamp: update.timestamp,
                                value: update.value.clone(),
                            });
                        }
                    }
                } else {
                    for (name, group) in &client
                        .queued_updates
                        .iter()
                        .group_by(|snapshot| &snapshot.name)
                    {
                        let snapshot = group
                            .max_by(|s1, s2| s1.timestamp.cmp(&s2.timestamp))
                            .unwrap();
                        updates.push(NTBinaryMessage {
                            id: client.lookup_id(name).unwrap(),
                            timestamp: snapshot.timestamp,
                            value: snapshot.value.clone(),
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
