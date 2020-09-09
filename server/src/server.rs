use crate::client::ConnectedClient;
use proto::prelude::{NTTextMessage, CborMessage, NTMessage, MessageValue, DataType, MessageBody};
use std::collections::HashMap;
use async_std::sync::{Arc, Mutex, Sender, channel, Receiver};
use async_std::task;
use async_std::net::TcpListener;
use crate::net::NTSocket;
use rand::Rng;
use crate::entry::Entry;
use proto::prelude::publish::PublishAck;

pub struct NTServer {
    clients: HashMap<u32, ConnectedClient>,
    next_pub_id: u32,
    entries: HashMap<u32, Entry>,
}

async fn tcp_loop(state: Arc<Mutex<NTServer>>, tx: Sender<ServerMessage>) -> anyhow::Result<()> {
    let mut listener = TcpListener::bind("0.0.0.0:5810").await?;

    while let Ok((sock, _)) = listener.accept().await {
        let cid = rand::random::<u32>();
        let sock = NTSocket::from_socket(async_tungstenite::accept_async(sock).await?);
        let client = ConnectedClient::new(sock, tx.clone(), cid);
        state.lock().await.clients.insert(cid, client);
    }
    Ok(())
}

async fn channel_loop(state: Arc<Mutex<NTServer>>, rx: Receiver<ServerMessage>) -> anyhow::Result<()> {
    while let Ok(msg) = rx.recv().await {
        match msg {
            ServerMessage::ClientDisconnected(cid) => {
                state.lock().await.clients.remove(&cid);
            }
            ServerMessage::ControlMessage(msg, cid) => {
                let mut state = state.lock().await;
                let mut client = state.clients.get_mut(&cid).unwrap();

                match msg.data() {
                    MessageValue::PublishReq(req) => {
                        let id = state.create_entry(req.name.clone(), req._type);
                        client.send_message(NTMessage::Text(PublishAck { name: req.name, _type: req._type, id }.into_message())).await;
                    }
                    MessageValue::PublishAck(_) => {}
                    MessageValue::PublishRel(rel) => {
                        if rel.delete {
                            state.entries.remove(&rel.id);
                        }
                    }
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
            ServerMessage::ValuePublish(values, cid) => {
                let mut state = state.lock().await;

                for (_, client) in state.clients.iter_mut().filter(|(id, _)| **id != cid) {
                    let values = values.clone().into_iter()
                        .filter(|update| client.subscribed_to(update.id))
                        .collect::<Vec<CborMessage>>();

                    client.send_message(NTMessage::Binary(values)).await;
                }
            }
        }
    }

    Ok(())
}

impl NTServer {
    pub fn new() -> Arc<Mutex<NTServer>> {
        let _self = Arc::new(Mutex::new(NTServer { clients: HashMap::new(), next_pub_id: 1 }));

        let (tx, rx) = channel(32);

        task::spawn(tcp_loop(_self.clone(), tx));
        task::spawn(channel_loop(_self.clone(), rx));

        _self
    }

    pub fn create_entry(&mut self, name: String, _type: DataType) -> u32 {
        let entry = Entry::new(name, _type);
        let id = self.next_pub_id;
        self.next_pub_id += 1;
        self.entries.insert(id, entry);
        id
    }
}

pub enum ServerMessage {
    ControlMessage(NTTextMessage, u32),
    ValuePublish(Vec<CborMessage>, u32),
    ClientDisconnected(u32),
}