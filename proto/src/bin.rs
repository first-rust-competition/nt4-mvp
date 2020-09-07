use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use serde_cbor::tags::Tagged;
use serde_cbor::{Deserializer, Serializer, Value};

#[derive(PartialEq, Debug)]
pub enum NTValue {
    Integer(i64),
    Double(f64),
    Boolean(bool),
    Raw(Vec<u8>),
    String(String),
    BooleanArray(Vec<bool>),
    IntegerArray(Vec<i64>),
    DoubleArray(Vec<f64>),
    StringArray(Vec<String>),
}

impl Serialize for NTValue {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            NTValue::Integer(i) => i.serialize(s),
            NTValue::Double(f) => f.serialize(s),
            NTValue::Boolean(b) => b.serialize(s),
            NTValue::Raw(bytes) => s.serialize_bytes(&bytes[..]),
            NTValue::String(st) => st.serialize(s),
            NTValue::BooleanArray(bs) => {
                if bs.len() == 0 {
                    let tagged = Tagged::new(Some(6), bs);
                    tagged.serialize(s)
                } else {
                    bs.serialize(s)
                }
            }
            NTValue::IntegerArray(is) => {
                if is.len() == 0 {
                    let tagged = Tagged::new(Some(7), is);
                    tagged.serialize(s)
                } else {
                    is.serialize(s)
                }
            }
            NTValue::DoubleArray(fs) => {
                if fs.len() == 0 {
                    let tagged = Tagged::new(Some(9), fs);
                    tagged.serialize(s)
                } else {
                    fs.serialize(s)
                }
            }
            NTValue::StringArray(ss) => {
                if ss.len() == 0 {
                    let tagged = Tagged::new(Some(10), ss);
                    tagged.serialize(s)
                } else {
                    ss.serialize(s)
                }
            }
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct CborMessage {
    id: u32,
    timestamp: Option<u64>, // TODO: support FP timestamp
    value: NTValue,
}

impl Serialize for CborMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(ts) = self.timestamp {
            let mut seq = serializer.serialize_seq(Some(3))?;
            seq.serialize_element(&self.id)?;
            seq.serialize_element(&ts)?;
            seq.serialize_element(&self.value)?;
            seq.end()
        } else {
            let mut seq = serializer.serialize_seq(Some(2))?;
            seq.serialize_element(&self.id)?;
            seq.serialize_element(&self.value)?;
            seq.end()
        }
    }
}

impl CborMessage {
    fn from_slice(slice: &[u8]) -> Vec<Self> {
        use std::io::Read;
        let de = Deserializer::from_slice(slice.clone()).into_iter::<Value>();

        let mut messages = Vec::new();
        for value in de {
            match value {
                Ok(Value::Array(values)) => {
                    println!("{:?}", values);
                    let id = match &values[0] {
                        Value::Integer(id) => *id as u32,
                        val => panic!("Invalid id type {:?}", val),
                    };

                    if values.len() == 2 {
                        let value = match &values[1] {
                            Value::Integer(i) => NTValue::Integer(*i as i64),
                            Value::Float(f) => NTValue::Double(*f),
                            Value::Bool(b) => NTValue::Boolean(*b),
                            Value::Bytes(b) => NTValue::Raw(b.clone()),
                            Value::Text(s) => NTValue::String(s.clone()),
                            Value::Array(v) => match &v[0] {
                                Value::Float(_) => NTValue::DoubleArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Float(f) = value {
                                                f
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Integer(_) => NTValue::IntegerArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Integer(i) = value {
                                                i as i64
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Bool(_) => NTValue::BooleanArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Bool(b) = value {
                                                b
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Text(_) => NTValue::StringArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Text(s) = value {
                                                s
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                _ => panic!("Invalid array type"),
                            },
                            Value::Tag(tag, v) => {
                                if let Value::Array(_) = **v {
                                    match *tag {
                                        6 => NTValue::BooleanArray(vec![]),
                                        7 => NTValue::IntegerArray(vec![]),
                                        8 | 9 => NTValue::DoubleArray(vec![]),
                                        10 => NTValue::StringArray(vec![]),
                                        _ => panic!("Invalid tag"),
                                    }
                                } else {
                                    panic!("Invalid tagged value")
                                }
                            }
                            _ => panic!("Invalid value"),
                        };

                        messages.push(Self {
                            id,
                            timestamp: None,
                            value,
                        });
                    } else {
                        let timestamp = match values[1] {
                            Value::Integer(ts) => ts as u64,
                            _ => panic!("Invalid timestamp type"),
                        };

                        let value = match &values[2] {
                            Value::Integer(i) => NTValue::Integer(*i as i64),
                            Value::Float(f) => NTValue::Double(*f),
                            Value::Bool(b) => NTValue::Boolean(*b),
                            Value::Bytes(b) => NTValue::Raw(b.clone()),
                            Value::Text(s) => NTValue::String(s.clone()),
                            Value::Array(v) => match &v[0] {
                                Value::Float(_) => NTValue::DoubleArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Float(f) = value {
                                                f
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Integer(_) => NTValue::IntegerArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Integer(i) = value {
                                                i as i64
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Bool(_) => NTValue::BooleanArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Bool(b) = value {
                                                b
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                Value::Text(_) => NTValue::StringArray(
                                    v.clone()
                                        .into_iter()
                                        .map(|value| {
                                            if let Value::Text(s) = value {
                                                s
                                            } else {
                                                panic!("Arrays must be of uniform type");
                                            }
                                        })
                                        .collect(),
                                ),
                                _ => panic!("Invalid array type"),
                            },
                            Value::Tag(tag, v) => {
                                if let Value::Array(_) = **v {
                                    match *tag {
                                        6 => NTValue::BooleanArray(vec![]),
                                        7 => NTValue::IntegerArray(vec![]),
                                        8 | 9 => NTValue::DoubleArray(vec![]),
                                        10 => NTValue::StringArray(vec![]),
                                        _ => panic!("Invalid tag"),
                                    }
                                } else {
                                    panic!("Invalid tagged value")
                                }
                            }
                            _ => panic!("Invalid value"),
                        };

                        messages.push(Self {
                            id,
                            timestamp: Some(timestamp),
                            value,
                        });
                    }
                }
                _ => panic!("No incomplete data"),
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
            0x83, // array(3)
            0x18, 0x2A, // unsigned(42)
            0x1A, 0x49, 0x96, 0x02, 0xD2, // unsigned(1234567890)
            0x83, // array(3)
            0xF5, // true
            0xF4, // false
            0xF5, // true
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0],
            CborMessage {
                id: 42,
                timestamp: Some(1234567890),
                value: NTValue::BooleanArray(vec![true, false, true])
            }
        )
    }

    #[test]
    fn test_multi_message_stream() {
        let data = vec![
            // ITEM 1
            0x83, // array(3)
            0x18, 0x2A, // unsigned(42)
            0x1A, 0x49, 0x96, 0x02, 0xD2, // unsigned(1234567890)
            0x83, // array(3)
            0xF5, // true
            0xF4, // false
            0xF5, // true
            // ITEM 2
            0x82, // array(2)
            0x18, 0x45, // unsigned(69)
            0x65, // text
            0x48, 0x65, 0x6C, 0x6C, 0x6F, // Hello
            // ITEM 3
            0x83, // array(3)
            0x19, 0x01, 0xA4, // unsigned(420)
            0x19, 0x16, 0x2E, // unsigned(5678)
            0x84, // array(4)
            0x01, // unsigned(1)
            0x02, // unsigned(2)
            0x03, // unsigned(3)
            0x04, // unsigned(4)
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 3);

        assert_eq!(
            messages[0],
            CborMessage {
                id: 42,
                timestamp: Some(1234567890),
                value: NTValue::BooleanArray(vec![true, false, true])
            }
        );

        assert_eq!(
            messages[1],
            CborMessage {
                id: 69,
                timestamp: None,
                value: NTValue::String("Hello".to_string())
            }
        );

        assert_eq!(
            messages[2],
            CborMessage {
                id: 420,
                timestamp: Some(5678),
                value: NTValue::IntegerArray(vec![1, 2, 3, 4])
            }
        );
    }

    #[test]
    fn test_empty_array() {
        let data = vec![
            0x82, // array(2)
            0x01, // unsigned(1)
            0xCA, 0x80, // tagged(string[])
        ];

        let messages = CborMessage::from_slice(&data[..]);

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0],
            CborMessage {
                id: 1,
                timestamp: None,
                value: NTValue::StringArray(vec![])
            }
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

        assert_eq!(&v[..], &[0x82, 0x01, 0xC9, 0x80]);

        let msg = CborMessage {
            id: 42,
            timestamp: Some(1234),
            value: NTValue::Double(1.5),
        };

        let v = serde_cbor::to_vec(&msg).unwrap();

        assert_eq!(
            &v[..],
            &[0x83, 0x18, 0x2A, 0x19, 0x04, 0xD2, 0xF9, 0x3E, 0x00]
        );
    }
}
