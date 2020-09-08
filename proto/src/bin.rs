//! This module handles the binary half of the NTv4 protocol.
//!
//! **Value** updates in NTv4 are sent as WS Binary messages, and are encoded with the Compact Binary Object Representation (CBOR)
//! as defined in RFC 7049.

use crate::ext::*;
use crate::text::DataType;
use serde::ser::SerializeSeq;
use serde::Serialize;
use serde_cbor::{Deserializer, Value};
use thiserror::Error;

macro_rules! impl_conversion {
    ($self:ident, $($inst:ident),+) => {
        match $self {
            $(
            NTValue::$inst(_) => DataType::$inst,
            )+
        }
    }
}

/// A NetworkTables value
#[derive(PartialEq, Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum NTValue {
    /// An integer value. This value stores both signed and unsigned integers in an `i64`
    Integer(i64),
    Float(f32),
    /// A floating-point value. This value represents both single and double precision floats, and is stored in an `f64`
    Double(f64),
    /// A boolean value
    Boolean(bool),
    /// A Raw value, stored as a Vec of the raw bytes
    #[serde(serialize_with = "raw_serializer")]
    Raw(Vec<u8>),
    #[serde(serialize_with = "raw_serializer")]
    RPC(Vec<u8>),
    /// A String value
    String(String),
    /// An Array of booleans
    BooleanArray(Vec<bool>),
    /// An Array of integers
    IntegerArray(Vec<i64>),
    FloatArray(Vec<f32>),
    /// An Array of floating-point numbers
    DoubleArray(Vec<f64>),
    /// An Array of strings
    StringArray(Vec<String>),
}

fn raw_serializer<S: serde::Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_bytes(&v[..])
}

impl NTValue {
    fn data_type(&self) -> DataType {
        impl_conversion!(
            self,
            Boolean,
            Double,
            Integer,
            Float,
            String,
            Raw,
            RPC,
            BooleanArray,
            DoubleArray,
            IntegerArray,
            FloatArray,
            StringArray
        )
    }
}

// An error encountered when attempting to deserialize NT4 messages from CBOR
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Error decoding ID from CBOR message. Expected an integer. Found: `{0:?}`")]
    InvalidId(Value),
    #[error("Error decoding timestamp from CBOR message. Expected an integer. Found: `{0:?}`")]
    InvalidTimestamp(Value),
    #[error("Error decoding NT type from CBOR message. Expected an integer. Found: `{0:?}`")]
    InvalidTypeFieldCBOR(Value),
    #[error("Error decoding NT type from CBOR message. Invalid NT type `{0}`.")]
    InvalidTypeFieldValue(u8),
    #[error("Error decoding NT value from CBOR message. Expected `{0:?}`, found an array.")]
    InvalidArrayType(DataType),
    #[error("Error decoding NT value from CBOR message. Arrays must be of a uniform type. Expected `{0:?}`, found element `{1:?}`.")]
    NonUniformArray(DataType, Value),
    #[error("Error decoding NT value from CBOR message. Found unexpected CBOR value `{0:?}`")]
    InvalidCBORValue(Value),
    #[error("Error decoding NT value from CBOR message. Data tagged as type `{0:?}` but decoded as type `{1:?}`")]
    TypeMismatch(DataType, DataType),
    #[error("Error decoding CBOR message. Codec Error: {0}")]
    CBOR(#[from] serde_cbor::Error),
    #[error("Error decoding CBOR message. Expected top-level array, found `{0:?}`")]
    InvalidTopLevelValue(Value),
    #[error("Error decoding CBOR message. Invalid length of top-level array: `{0}`.")]
    InvalidTopLevelArrayLength(usize),
}

/// A message sent or recieved over CBOR
#[derive(PartialEq, Debug, Clone)]
pub struct CborMessage {
    /// The ID associated with the given value
    ///
    /// This value is received from the textual half of the protocol, where its relation with a NetworkTables key is specified.
    id: u32,
    /// An optional timestamp associated with this change
    ///
    /// This timestamp is represented in microseconds, though implementations can also choose to represent it with seconds using
    /// a double-precision float.
    ///
    /// A timestamp is only sent with a value change when requested by the client, by default this value is not sent by the server
    timestamp: Option<u64>, // TODO: support FP timestamp
    /// The value associated with this change
    value: NTValue,
}

impl Serialize for CborMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(ts) = self.timestamp {
            let mut seq = serializer.serialize_seq(Some(4))?;
            seq.serialize_element(&self.id)?;
            seq.serialize_element(&ts)?;
            seq.serialize_element::<u8>(&self.value.data_type().into())?;
            seq.serialize_element(&self.value)?;
            seq.end()
        } else {
            let mut seq = serializer.serialize_seq(Some(3))?;
            seq.serialize_element(&self.id)?;
            seq.serialize_element::<u8>(&self.value.data_type().into())?;
            seq.serialize_element(&self.value)?;
            seq.end()
        }
    }
}

macro_rules! unpack_value {
    ($messages:expr => $value:expr, $func:ident, $nt_name:ident) => {
        match $value.$func() {
            Some(v) => NTValue::$nt_name(v),
            None => {
                $messages.push(Err(DecodeError::InvalidCBORValue($value.clone())));
                continue;
            }
        }
    };
    ($messages:expr => $value:expr, $func:ident) => {
        match $value.$func() {
            Some(v) => v,
            None => {
                $messages.push(Err(DecodeError::InvalidCBORValue($value.clone())));
                continue;
            }
        }
    };
}

macro_rules! unpack_array {
    (($messages:expr, $ty:expr, $label:tt) => $value:expr, $conv_func:ident, $nt_name:ident) => {{
        let v = unpack_value!($messages => $value, as_array);
        let mut arr = Vec::with_capacity(v.len());

        for value in v {
            match value.$conv_func() {
                Some(v) => arr.push(v),
                None => {
                    $messages.push(Err(DecodeError::NonUniformArray($ty, value.clone())));
                    continue $label;
                }
            }
        }

        NTValue::$nt_name(arr)
    }}
}

impl CborMessage {
    fn from_slice(slice: &[u8]) -> Vec<Result<Self, DecodeError>> {
        let de = Deserializer::from_slice(slice.clone()).into_iter::<Value>();

        let mut messages = Vec::new();
        'outer: for value in de {
            match value {
                Ok(Value::Array(values)) => {
                    let id = match &values[0] {
                        Value::Integer(id) => *id as u32,
                        val => {
                            messages.push(Err(DecodeError::InvalidId(val.clone())));
                            continue;
                        }
                    };

                    let (ty_idx, timestamp) = match values.len() {
                        3 => (1, None),
                        4 => {
                            let timestamp = match &values[1] {
                                Value::Integer(ts) => *ts as u64,
                                value => {
                                    messages
                                        .push(Err(DecodeError::InvalidTimestamp(value.clone())));
                                    continue;
                                }
                            };
                            (2, Some(timestamp))
                        }
                        len => {
                            messages.push(Err(DecodeError::InvalidTopLevelArrayLength(len)));
                            continue;
                        }
                    };

                    let ty = match &values[ty_idx] {
                        Value::Integer(i) => match i {
                            0 => DataType::Boolean,
                            1 => DataType::Double,
                            2 => DataType::Integer,
                            3 => DataType::Float,
                            4 => DataType::String,
                            5 => DataType::Raw,
                            6 => DataType::RPC,
                            16 => DataType::BooleanArray,
                            17 => DataType::DoubleArray,
                            18 => DataType::IntegerArray,
                            19 => DataType::FloatArray,
                            20 => DataType::StringArray,
                            ty => {
                                messages.push(Err(DecodeError::InvalidTypeFieldValue(*ty as u8)));
                                continue;
                            }
                        },
                        val => {
                            messages.push(Err(DecodeError::InvalidTypeFieldCBOR(val.clone())));
                            continue;
                        }
                    };

                    let cbor_value = &values[ty_idx + 1];
                    let value = match ty {
                        DataType::Integer => {
                            unpack_value!(messages => cbor_value, as_integer, Integer)
                        }
                        DataType::Boolean => {
                            unpack_value!(messages => cbor_value, as_bool, Boolean)
                        }
                        DataType::Raw => unpack_value!(messages => cbor_value, as_bytes, Raw),
                        DataType::RPC => unpack_value!(messages => cbor_value, as_bytes, RPC),
                        DataType::String => unpack_value!(messages => cbor_value, as_text, String),
                        // Special case because CBOR doesn't distinguish between single/double precision floats
                        DataType::Float => match cbor_value.as_float() {
                            Some(f) => NTValue::Float(f as f32),
                            None => {
                                messages
                                    .push(Err(DecodeError::InvalidCBORValue(cbor_value.clone())));
                                continue;
                            }
                        },
                        DataType::Double => unpack_value!(messages => cbor_value, as_float, Double),
                        DataType::BooleanArray => {
                            unpack_array!((messages, ty, 'outer) => cbor_value, as_bool, BooleanArray)
                        }
                        DataType::StringArray => {
                            unpack_array!((messages, ty, 'outer) => cbor_value, as_text, StringArray)
                        }
                        DataType::IntegerArray => {
                            unpack_array!((messages, ty, 'outer) => cbor_value, as_integer, IntegerArray)
                        }
                        // Special case because CBOR doesn't distinguish between single/double precision floats
                        DataType::FloatArray => {
                            let v = unpack_value!(messages => cbor_value, as_array);
                            let mut arr = Vec::with_capacity(v.len());

                            for value in v {
                                if let Value::Float(v) = value {
                                    arr.push(v as f32);
                                } else {
                                    messages
                                        .push(Err(DecodeError::NonUniformArray(ty, value.clone())));
                                    continue 'outer;
                                }
                            }

                            NTValue::FloatArray(arr)
                        }
                        DataType::DoubleArray => {
                            unpack_array!((messages, ty, 'outer) => cbor_value, as_float, DoubleArray)
                        }
                    };

                    messages.push(Ok(Self {
                        id,
                        timestamp,
                        value,
                    }));
                }
                Ok(value) => messages.push(Err(DecodeError::InvalidTopLevelValue(value))),
                Err(e) => messages.push(Err(DecodeError::CBOR(e))),
            }
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::CborMessage;
    use crate::bin::NTValue;

    #[test]
    fn test_single_message_stream() {
        let data = vec![
            0x84, // array(4)
            0x18, 0x2A, // unsigned(42)
            0x1A, 0x49, 0x96, 0x02, 0xD2, // unsigned(1234567890)
            0x10, // type: boolean[]
            0x83, // array(3)
            0xF5, // true
            0xF4, // false
            0xF5, // true
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages.into_iter().next().unwrap().ok(),
            Some(CborMessage {
                id: 42,
                timestamp: Some(1234567890),
                value: NTValue::BooleanArray(vec![true, false, true])
            })
        )
    }

    #[test]
    fn test_multi_message_stream() {
        let data = vec![
            // ITEM 1
            0x84, // array(4)
            0x18, 0x2A, // unsigned(42)
            0x1A, 0x49, 0x96, 0x02, 0xD2, // unsigned(1234567890)
            0x10, // Type: bool[]
            0x83, // array(3)
            0xF5, // true
            0xF4, // false
            0xF5, // true
            // ITEM 2
            0x83, // array(3)
            0x18, 0x45, // unsigned(69)
            0x04, // type: string
            0x65, // text
            0x48, 0x65, 0x6C, 0x6C, 0x6F, // Hello
            // ITEM 3
            0x84, // array(4)
            0x19, 0x01, 0xA4, // unsigned(420)
            0x19, 0x16, 0x2E, // unsigned(5678)
            0x12, // type: int[]
            0x84, // array(4)
            0x01, // unsigned(1)
            0x02, // unsigned(2)
            0x03, // unsigned(3)
            0x04, // unsigned(4)
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 3);

        let mut messages = messages.into_iter();

        assert_eq!(
            messages.next().unwrap().ok(),
            Some(CborMessage {
                id: 42,
                timestamp: Some(1234567890),
                value: NTValue::BooleanArray(vec![true, false, true])
            })
        );

        assert_eq!(
            messages.next().unwrap().ok(),
            Some(CborMessage {
                id: 69,
                timestamp: None,
                value: NTValue::String("Hello".to_string())
            })
        );

        assert_eq!(
            messages.next().unwrap().ok(),
            Some(CborMessage {
                id: 420,
                timestamp: Some(5678),
                value: NTValue::IntegerArray(vec![1, 2, 3, 4])
            })
        );
    }

    #[test]
    fn test_empty_array() {
        let data = vec![
            0x83, // array(3)
            0x01, // unsigned(1)
            0x14, // type: string[]
            0x80, // tagged(string[])
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages.into_iter().next().unwrap().ok(),
            Some(CborMessage {
                id: 1,
                timestamp: None,
                value: NTValue::StringArray(vec![])
            })
        )
    }

    #[test]
    fn test_serialize() {
        let msg = CborMessage {
            id: 1,
            timestamp: None,
            value: NTValue::DoubleArray(vec![]),
        };

        let v = serde_cbor::to_vec(&msg).unwrap();

        assert_eq!(&v[..], &[0x83, 0x01, 0x11, 0x80]);

        let msg = CborMessage {
            id: 42,
            timestamp: Some(1234),
            value: NTValue::Double(1.5),
        };

        let v = serde_cbor::to_vec(&msg).unwrap();

        assert_eq!(
            &v[..],
            &[0x84, 0x18, 0x2A, 0x19, 0x04, 0xD2, 0x01, 0xF9, 0x3E, 0x00]
        );
    }

    #[test]
    fn test_serialize_raw() {
        let msg = CborMessage {
            id: 5,
            timestamp: None,
            value: NTValue::Raw(vec![0x42, 0x69, 0x2, 0xa]),
        };

        let v = serde_cbor::to_vec(&msg).unwrap();
        assert_eq!(
            &v[..],
            &[0x83, 0x05, 0x05, 0x44, 0x42, 0x69, 0x02, 0x0A],
            "Vector {:x?}",
            v
        );
    }

    #[test]
    fn test_identity() {
        let msg = CborMessage {
            id: 5,
            timestamp: Some(12345),
            value: NTValue::Double(4.2),
        };

        let v = serde_cbor::to_vec(&msg).unwrap();
        let mut msg2 = CborMessage::from_slice(&v[..]).into_iter();

        assert_eq!(Some(msg), msg2.next().unwrap().ok());
    }

    #[test]
    fn test_invalid_message() {
        let v = vec![
            0x83, // array(3)
            0x01, // unsigned(1)
            0x63, 0x66, 0x6F, 0x6F, // text(foo)
            0x05, // unsigned(5)
        ];

        let messages = CborMessage::from_slice(&v[..]);

        assert_eq!(messages.len(), 1);

        assert!(messages[0].is_err());
    }

    #[test]
    fn test_mixed_messages() {
        //FB 3F BF 97 24 74 53 8E F3
        let v = vec![
            // ITEM 1
            0x83, // array(3)
            0x01, // unsigned(1)
            0x02, // unsigned(2)
            0x05, // unsigned(5)
            // ITEM 2
            0x83, // array(3)
            0x01, // unsigned(1)
            0x02, // unsigned(2)
            // Data tagged as integer, but with FP value
            0xFB, 0x3F, 0xBF, 0x97, 0x24, 0x74, 0x53, 0x8E, 0xF3, // float(0.1234)
        ];

        let messages = CborMessage::from_slice(&v[..]);
        assert_eq!(messages.len(), 2);

        let mut messages = messages.into_iter();

        assert_eq!(
            messages.next().unwrap().ok(),
            Some(CborMessage {
                id: 1,
                timestamp: None,
                value: NTValue::Integer(5),
            })
        );

        assert!(messages.next().unwrap().is_err());
    }

    #[test]
    fn test_value_serializer() {
        // Booleans
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Boolean(false)).unwrap()[..],
            &[0xF4]
        );
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Boolean(true)).unwrap()[..],
            &[0xF5]
        );

        // Doubles
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Double(0.25)).unwrap()[..],
            &[0xF9, 0x34, 0x00]
        );

        // Integers
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Integer(42)).unwrap()[..],
            &[0x18, 0x2A]
        );
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Integer(-42)).unwrap()[..],
            &[0x38, 0x29]
        );

        // Byte strings (Raw/RPC)
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::Raw(vec![0xa, 0x42, 0x13, 0x20])).unwrap()[..],
            &[0x44, 0x0A, 0x42, 0x13, 0x20]
        );

        // Text strings
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::String("abcd".to_string())).unwrap()[..],
            &[0x64, 0x61, 0x62, 0x63, 0x64]
        );

        // Arrays
        assert_eq!(
            &serde_cbor::to_vec(&NTValue::IntegerArray(vec![1, -2, 3, -4])).unwrap()[..],
            &[0x84, 0x01, 0x21, 0x03, 0x23]
        );
    }
}
