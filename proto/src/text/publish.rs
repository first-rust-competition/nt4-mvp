use serde::{Serialize, Deserialize};
use super::{MessageBody, DataType};


#[derive(Serialize, Deserialize)]
pub struct PublishReq {
    name: String,
    #[serde(rename = "type")]
    _type: DataType,
    options: Option<PublishRequestOptions>
}

#[derive(Serialize, Deserialize)]
pub struct PublishRequestOptions {
    persistent: bool,
}

#[derive(Serialize, Deserialize)]
pub struct PublishAck {
    name: String,
    #[serde(rename = "type")]
    _type: String,
    id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct PublishRel {
    id: u32,
    delete: bool,
}

impl_message!(PublishReq, PublishAck, PublishRel);