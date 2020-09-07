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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    #[serde(rename = "publish")]
    PublishReq,
    #[serde(rename = "puback")]
    PublishAck,
    #[serde(rename = "pubrel")]
    PublishRel,
    List,
    Directory,
    Listen,
    Unlisten,
    Announce,
    Unannounce,
    GetValues,
    Subscribe,
    Unsubscribe,
}

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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Boolean,
    Raw,
    String,
    Integer,
    Float,
    Double,
    #[serde(rename = "boolean[]")]
    BooleanArray,
    #[serde(rename = "string[]")]
    StringArray,
    #[serde(rename = "integer[]")]
    IntegerArray,
    #[serde(rename = "float[]")]
    FloatArray,
    #[serde(rename = "double[]")]
    DoubleArray,
}

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
    use crate::text::{NTMessage, MessageType, MessageValue, DataType, MessageBody};
    use crate::text::publish::{PublishReq, PublishAck};

    #[test]
    fn test_de() {
        let msg = r#"{"type":"publish", "data": {"name": "/foo", "type": "integer"}}"#;
        let msg = serde_json::from_str::<NTMessage>(msg).unwrap();
        assert_eq!(msg._type, MessageType::PublishReq);
        assert_eq!(msg.data(), MessageValue::PublishReq(PublishReq { name: "/foo".to_string(), _type: DataType::Integer, options: None }));
    }

    #[test]
    fn test_ser() {
        let msg = PublishAck { name: "/foo".to_string(), _type: DataType::Integer, id: 42 };

        assert_eq!(serde_json::to_string(&msg.into_message()).unwrap(), r#"{"type":"puback","data":{"id":42,"name":"/foo","type":"integer"}}"#)
    }
}
