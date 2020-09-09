//! Strongly typed message bodies for messages related to publishing new entries
//!
//! These messages are sent for the creation and deletion of new entries, and contain the metadata necessary to begin publishing
//! values over CBOR.
use super::{DataType, MessageBody};
use serde::{Deserialize, Serialize};

/// Publish Request Message
///
/// Sent from a client to a server to indicate the client wishes to start publishing values at the given NetworkTables key
///
/// The server will respond to this with the [`PublishAck`] message. Once that is received, a client can start publishing values
/// over CBOR with the given ID.
///
/// When the client no longer wishes to publish to this key, it can send the [`PublishRel`] message to indicate that to the server.
/// This also allows the client to delete the value, if it wishes.
///
/// [`PublishAck`]: ./struct.PublishAck.html
/// [`PublishRel`]: ./struct.PublishRel.html
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishReq {
    /// The NetworkTables key
    pub name: String,
    /// The type of the data that the client wishes to start publishing
    #[serde(rename = "type")]
    pub _type: DataType,
    /// Additional options
    pub options: Option<PublishRequestOptions>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
pub struct PublishRequestOptions {
    /// Whether the client wishes for this value to be persisted.
    ///
    /// When this is set to true, the server will periodically flush the value to disk, and restore it when it restarts.
    /// Otherwise the value will be forgotten when the server restarts.
    persistent: bool,
}

/// Publish Acknowledge Message
///
/// Sent by the server in response to a [`PublishReq`] message. This message contains the ID that must be used in CBOR messages
/// to publish messages to the given key
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishAck {
    /// The NetworkTables key
    pub name: String,
    /// The type of the data to be published at this key
    #[serde(rename = "type")]
    pub _type: DataType,
    /// The ID associated with this key. This ID is used in CBOR messages to push value updates.
    pub id: u32,
}

/// Publish Release Message
///
/// Sent by a client to indicate that it no longer wishes to publish values at the given ID.
/// The client can also request that the associated key can be deleted.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PublishRel {
    /// The ID of the NetworkTables entry
    pub id: u32,
    /// Set when the client wishes for the server to delete this entry when this message is received.
    pub delete: bool,
}

impl_message!(PublishReq, PublishAck, PublishRel);
