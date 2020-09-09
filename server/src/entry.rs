use proto::prelude::{NTValue, DataType};

pub struct Entry {
    name: String,
    value: NTValue,
}

impl Entry {
    pub fn new(name: String, _type: DataType) -> Entry {
        Entry {
            name,
            value: _type.default_value(),
        }
    }
}