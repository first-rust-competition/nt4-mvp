use serde::{Serialize, Deserialize};
use super::MessageBody;
use crate::text::DataType;

#[derive(Serialize, Deserialize)]
pub struct List {
    prefix: String,
}

#[derive(Serialize, Deserialize)]
pub struct Directory {
    items: Vec<DirectoryItem>,
}

#[derive(Serialize, Deserialize)]
pub struct DirectoryItem {
    name: String,
    id: u32,
    #[serde(rename = "type")]
    _type: DataType,
    persistent: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Listen {
    prefix: String,
    listenuid: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Unlisten {
    listenuid: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Announce {
    name: String,
    id: u32,
    #[serde(rename = "type")]
    _type: DataType,
    persistent: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Unannounce {
    name: String,
    id: u32,
}

impl_message!(List, Directory, Listen, Unlisten, Announce, Unannounce);