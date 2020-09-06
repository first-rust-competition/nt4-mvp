use serde::{Serialize, Deserialize};
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

mod publish;
mod directory;
mod subscription;

pub trait MessageBody {
    fn into_message(self) -> NTMessage;
}

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct NTMessage {
    #[serde(rename = "type")]
    _type: MessageType,
    data: Value,
}
