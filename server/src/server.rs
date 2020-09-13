use crate::client::ConnectedClient;
use crate::entry::Topic;
use crate::net::NTSocket;
use async_std::net::TcpListener;
use async_std::sync::{channel, Arc, Mutex, Sender};
use async_std::task;
use async_tungstenite::tungstenite::handshake::server::{Request, Response};
use async_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use itertools::Itertools;
use proto::prelude::{
    DataType, MessageBody, NTBinaryMessage, NTMessage, NTTextMessage,
};
use std::collections::HashMap;
use std::ops::DerefMut;

mod broadcast;
use broadcast::*;

mod loop_;
use loop_::*;

pub static MAX_BATCHING_SIZE: usize = 5;

/// The main state of the NT4 server
/// Contains all connected clients, the most up-to-date copy of topics on this server,
/// and a count of publishers to a topic (Used to determine when a topic should be deleted).
pub struct NTServer {
    clients: HashMap<u32, ConnectedClient>,
    entries: HashMap<String, Topic>,
    pub_count: HashMap<String, usize>,
}

/// The main network loop of the server
///
/// This task is started when the server is created, and runs indefinitely until the runtime is shut down
/// It accepts connections at the standard NT4 IP, ensures that they are valid NT4 clients (It will respond with HTTP 400 Bad Request if a client does not correctly specify the protocol),
/// and stores new clients in the state of the server.
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

/// This task updates a new client about all topics that currently exist on the server
/// For each value that the server recognizes, it will send an Announce message to inform the client of the values that exist
///
/// This task is spawned by [`tcp_loop`] each time a new client connects.
///
/// [`tcp_loop`]: ./fn.tcp_loop.html
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

impl NTServer {
    /// Creates a new instance of an NT4 server, and spawns all the tasks that
    /// run indefinitely that are associated with it
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

    /// Creates a new entry in response to a PublishReq message
    ///
    /// If an entry already exists, the publisher count for that entry is incremented instead.
    pub fn create_entry(&mut self, name: String, _type: DataType) {
        if self.entries.contains_key(&name) {
            let entry = &self.entries[&name];
            if entry.entry_type() == _type {
                let cnt = self.pub_count.get_mut(&name).unwrap();
                *cnt += 1;
                return;
            }
        }
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
