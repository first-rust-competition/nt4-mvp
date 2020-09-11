use proto::prelude::{NTValue, DataType};

pub struct Topic {
    pub name: String,
    pub value: NTValue,
    pub flags: Vec<String>
}

impl Topic {
    pub fn new(name: String, _type: DataType) -> Topic {
        Topic {
            name,
            value: _type.default_value(),
            flags: vec![]
        }
    }

    pub fn entry_type(&self) -> DataType {
        self.value.data_type()
    }
}