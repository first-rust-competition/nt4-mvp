use proto::prelude::{DataType, NTValue};

#[derive(PartialEq, Debug, Clone)]
pub struct Topic {
    pub name: String,
    pub value: NTValue,
    pub flags: Vec<String>,
    pub dirty: bool,
}

impl Topic {
    pub fn new(name: String, _type: DataType) -> Topic {
        Topic {
            name,
            value: _type.default_value(),
            flags: vec![],
            dirty: false,
        }
    }

    pub fn set_value(&mut self, value: NTValue) {
        self.value = value;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn entry_type(&self) -> DataType {
        self.value.data_type()
    }
}
