use crate::text::directory::{Announce, Directory, List, Listen, Unannounce, Unlisten};
use crate::text::publish::{PublishAck, PublishRel, PublishReq};
use crate::text::subscription::{GetValues, Subscribe, Unsubscribe};
use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! impl_message {
    ($($name:ident),+) => {
        $(
        impl MessageBody for $name {
            fn into_message(self) -> $crate::text::NTMessage {
                $crate::text::NTMessage {
                    _type: $crate::text::MessageType::$name,
                    data: serde_json::to_value(self).unwrap()
                }
            }
        }
        )+
    }
}

mod directory;
mod publish;
mod subscription;

pub trait MessageBody {
    fn into_message(self) -> NTMessage;
}

/// The type of the message that is being sent or received
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    /// Publish Request Message
    /// Direction: Client to Server
    /// Response: Publish Acknowledge
    ///
    /// Sent from a client to the server to indicate the client wants to start publishing values at the given NetworkTables key.
    /// The server will respond with a “puback” message.
    /// Once the client receives the “puback” message it can start publishing data value updates via binary CBOR messages.
    #[serde(rename = "publish")]
    PublishReq,
    /// Publish Acknowledge Message
    /// Direction: Server to Client
    ///
    /// Sent from the server to a client in response to the client’s “publish” message.
    #[serde(rename = "puback")]
    PublishAck,
    /// Publish Release Message
    /// Direction: Client to Server
    ///
    /// Sent from a client to the server to indicate the client wants to stop publishing values at the given NetworkTables key.
    /// The client may also request the key be deleted.
    /// The client **must** stop publishing data value updates via binary CBOR messages prior to sending this message.
    #[serde(rename = "pubrel")]
    PublishRel,
    /// Get Directory Message
    /// Direction: Client to Server
    /// Response: Directory Response
    ///
    /// Sent from a client to the server to indicate the client wants to get information about what keys are currently published.
    /// The server shall send a “directory” message in response.
    List,
    /// Directory Response Message
    /// Direction: Server to Client
    ///
    /// Sent from the server to a client in response to the client’s “list” message.
    Directory,
    /// Start Announcements Message
    /// Direction: Client to Server
    /// Response: Key Announcement, Key Removed
    ///
    /// Sent from a client to the server to indicate the client wants to get information about what keys are currently published and listen for future published keys.
    /// The server shall send “announce” and “unannounce” messages as keys are added or removed (respectively) with the specified prefix.
    /// Any keys currently present will immediately result in “announce” messages being sent.
    Listen,
    /// Stop Announcements Message
    /// Direction: Client to Server
    ///
    /// Sent from a client to the server to indicate the client wants to stop getting “announce” and “unannounce” messages for a particular prefix.
    Unlisten,
    /// Key Announcement Message
    /// Direction: Server to Client
    ///
    /// Sent from the server to a client with an announcement listener covering the key.
    /// The server shall send this message either initially after receiving Start Announcements from a client,
    /// or when new keys are created with the prefix specified.
    Announce,
    /// Key Removed Message
    /// Direction: Server to Client
    ///
    /// Sent from the server to a client with an announcement listener covering the key.
    /// The server shall send this message when a previously announced (via an “announce” message) key is deleted.
    Unannounce,
    /// Get Values Message
    /// Direction: Client to Server
    /// Response: Values over CBOR
    ///
    /// Sent from a client to the server to indicate the client wants to get the current values for the specified keys (identifiers).
    /// The server shall send CBOR messages containing the current values immediately upon receipt.
    /// While this message could theoretically be used to poll for value updates, it is much better to use the “subscribe” message to request periodic push updates.
    GetValues,
    /// Subscribe Message
    /// Direction: Client to Server
    /// Response: Values over CBOR
    ///
    /// Sent from a client to the server to indicate the client wants to subscribe to value changes for the specified keys (identifiers).
    /// The server shall send CBOR messages containing the current values upon receipt, and continue sending CBOR messages for future value changes.
    /// Subscriptions may overlap; only one CBOR message is sent per value change regardless of the number of subscriptions.
    /// Sending a “subscribe” message with the same subscription UID as a previous “subscribe” message results in updating the subscription (replacing the array of identifiers and updating any specified options).
    Subscribe,
    /// Unsubscribe Message
    /// Direction: Client to Server
    ///
    /// Sent from a client to the server to indicate the client wants to stop subscribing to value changes for the given subscription.
    Unsubscribe,
}

/// An enum containing the structs representing each text message, the explanation of each message can be found in the documentation for [`MessageType`]
///
/// [`MessageType`]: ./enum.MessageType.html
#[derive(Debug, PartialEq)]
pub enum MessageValue {
    PublishReq(PublishReq),
    PublishAck(PublishAck),
    PublishRel(PublishRel),
    List(List),
    Directory(Directory),
    Listen(Listen),
    Unlisten(Unlisten),
    Announce(Announce),
    Unannounce(Unannounce),
    GetValues(GetValues),
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
}

/// An enum representation of the acceptable data types in NTv4
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    /// Represents a boolean, true or false
    Boolean,
    /// Represents a sequence of raw bytes
    Raw,
    /// Represents a sequence of bytes representing a String
    String,
    /// Represents a signed 64-bit integer
    Integer,
    /// Represents an IEEE754 single-precision floating-point number
    Float,
    /// Represents an IEEE754 double-precision floating-point number
    Double,
    /// Represents an array of Booleans
    #[serde(rename = "boolean[]")]
    BooleanArray,
    /// Represents an array of Strings
    #[serde(rename = "string[]")]
    StringArray,
    /// Represents an array of Integers
    #[serde(rename = "integer[]")]
    IntegerArray,
    /// Represents an array of Floats
    #[serde(rename = "float[]")]
    FloatArray,
    /// Represents an array of Doubles
    #[serde(rename = "double[]")]
    DoubleArray,
}

/// The most generic struct representing a textual message transmitted in NT4
///
/// This struct should probably not be used directly, and instead can be constructed from the implementors of [`MessageBody`], found in submodules
/// These implementors are strongly typed equivalents to the `data` field on this type, and contain more information about how they should be used.
///
/// [`MessageBody`]: ./trait.MessageBody.html
#[derive(Serialize, Deserialize, PartialEq)]
pub struct NTMessage {
    #[serde(rename = "type")]
    _type: MessageType,
    data: Value,
}

macro_rules! to_data_body {
    ($self:ident, $($ty:ident),+) => {
        match $self._type {
            $(
            MessageType::$ty => MessageValue::$ty(serde_json::from_value::<$ty>($self.data).unwrap()),
            )+
        }
    }
}

impl NTMessage {
    /// Decodes the `Value` stored in `self` as a strongly typed struct depending on the value of `self._type`
    ///
    /// Returns the value wrapped inside the [`MessageValue`] enum.
    ///
    /// [`MessageValue`]: ./enum.MessageValue.html
    pub fn data(self) -> MessageValue {
        use self::directory::*;
        use self::publish::*;
        use self::subscription::*;
        to_data_body!(
            self,
            PublishReq,
            PublishAck,
            PublishRel,
            List,
            Directory,
            Listen,
            Unlisten,
            Announce,
            Unannounce,
            GetValues,
            Subscribe,
            Unsubscribe
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::text::publish::{PublishAck, PublishReq};
    use crate::text::{DataType, MessageBody, MessageType, MessageValue, NTMessage};

    #[test]
    fn test_de() {
        let msg = r#"{"type":"publish", "data": {"name": "/foo", "type": "integer"}}"#;
        let msg = serde_json::from_str::<NTMessage>(msg).unwrap();
        assert_eq!(msg._type, MessageType::PublishReq);
        assert_eq!(
            msg.data(),
            MessageValue::PublishReq(PublishReq {
                name: "/foo".to_string(),
                _type: DataType::Integer,
                options: None
            })
        );
    }

    #[test]
    fn test_ser() {
        let msg = PublishAck {
            name: "/foo".to_string(),
            _type: DataType::Integer,
            id: 42,
        };

        assert_eq!(
            serde_json::to_string(&msg.into_message()).unwrap(),
            r#"{"type":"puback","data":{"id":42,"name":"/foo","type":"integer"}}"#
        )
    }
}
