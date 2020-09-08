use serde_cbor::Value;

macro_rules! gen_funcs {
    ($($func_name:ident => ($value:ident,$prim:ident)),+) => {
    $(
    fn $func_name(&self) -> Option<$prim> {
        match self {
            Self::$value(v) => Some(v.clone()),
            _ => None
        }
    }
    )+
    }
}

pub trait ValueExt: Sized {
    fn as_integer(&self) -> Option<i64>;
    fn as_float(&self) -> Option<f64>;
    fn as_bool(&self) -> Option<bool>;
    fn as_bytes(&self) -> Option<Vec<u8>>;
    fn as_text(&self) -> Option<String>;
    fn as_array(&self) -> Option<Vec<Value>>;
}

impl ValueExt for Value {
    gen_funcs!(
        as_float => (Float,f64),
        as_bool => (Bool,bool),
        as_text => (Text,String)
    );

    fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i as i64),
            _ => None,
        }
    }

    fn as_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Value::Bytes(v) => Some(v.clone()),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<Vec<Value>> {
        match self {
            Value::Array(v) => Some(v.clone()),
            _ => None,
        }
    }
}
