use super::MessageBody;
use crate::text::DataType;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct List {
    prefix: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Directory {
    items: Vec<DirectoryItem>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DirectoryItem {
    name: String,
    id: u32,
    #[serde(rename = "type")]
    _type: DataType,
    persistent: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Listen {
    prefix: String,
    listenuid: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Unlisten {
    listenuid: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Announce {
    name: String,
    id: u32,
    #[serde(rename = "type")]
    _type: DataType,
    persistent: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Unannounce {
    name: String,
    id: u32,
}

impl_message!(List, Directory, Listen, Unlisten, Announce, Unannounce);
