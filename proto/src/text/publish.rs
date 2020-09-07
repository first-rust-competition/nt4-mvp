use super::{DataType, MessageBody};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishReq {
    pub name: String,
    #[serde(rename = "type")]
    pub _type: DataType,
    pub options: Option<PublishRequestOptions>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
pub struct PublishRequestOptions {
    persistent: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishAck {
    pub name: String,
    #[serde(rename = "type")]
    pub _type: DataType,
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishRel {
    id: u32,
    delete: bool,
}

impl_message!(PublishReq, PublishAck, PublishRel);
