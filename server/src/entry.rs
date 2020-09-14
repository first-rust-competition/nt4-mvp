use crate::client::TopicSnapshot;
use proto::prelude::{DataType, NTValue};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub value: NTValue,
    pub timestamp: u64,
    pub flags: Vec<String>,
}

impl Topic {
    pub fn new(name: String, _type: DataType) -> Topic {
        Topic {
            name,
            value: _type.default_value(),
            timestamp: 0,
            flags: vec![],
        }
    }

    pub fn set_value(&mut self, value: NTValue, timestamp: u64) {
        self.value = value;
        self.timestamp = timestamp;
    }

    pub fn snapshot(&self) -> TopicSnapshot {
        TopicSnapshot {
            name: self.name.clone(),
            value: self.value.clone(),
            timestamp: self.timestamp,
        }
    }

    pub fn entry_type(&self) -> DataType {
        self.value.data_type()
    }
}
