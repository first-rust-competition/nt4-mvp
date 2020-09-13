//! Strongly-typed message bodies for messages related to metadata about values
//! The messages here can be used to query, and subscribe to changes in metadata for values stored in the server.

use super::MessageBody;
use crate::text::DataType;
use serde::{Deserialize, Serialize};

/// Key Announcement Message
///
/// Sent asynchronously from the server to a subscribed client to notify it of a new key in a directory.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Announce {
    /// The full name of the key
    pub name: String,
    /// The ID used by the server when publishing CBOR updates for this key. This can be used with [`GetValues`] and [`Subscribe`]
    /// to receive the value associated with this key.
    ///
    /// [`GetValues`]: ../subscription/struct.GetValues.html
    /// [`Subscribe`]: ../subscription/struct.Subscribe.html
    pub id: u32,
    /// The type of the data associated with this key
    #[serde(rename = "type")]
    pub _type: DataType,
    /// Any flags associated with this entry
    pub flags: Vec<String>,
}

/// Key Removed Message
///
/// Sent asynchronously from the server to indicate to a subscribed client that a value that was previously shared by [`Announce`] has been deleted.
///
/// [`Announce`]: ./struct.Announce.html
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Unannounce {
    /// The name of the value
    pub name: String,
    /// The ID that was used when publishing value updates through CBOR messages.
    pub id: u32,
}

impl_message!(Announce, Unannounce);
