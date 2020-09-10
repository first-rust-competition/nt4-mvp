use std::io::{Read, ErrorKind};
use rmpv::Value;
use rmpv::decode::read_value;
use crate::bin::DecodeError;

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
    fn as_f32(&self) -> Option<f32>;
    fn as_f64(&self) -> Option<f64>;
    fn as_bool(&self) -> Option<bool>;
    fn as_bytes(&self) -> Option<Vec<u8>>;
    fn as_text(&self) -> Option<String>;
}

impl ValueExt for Value {
    gen_funcs!(
        as_f64 => (F64,f64),
        as_f32 => (F32, f32),
        as_bool => (Boolean,bool)
    );

    fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => i.as_i64().or_else(|| i.as_u64().map(|i| i as i64)),
            _ => None
        }
    }

    fn as_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Value::Binary(v) => Some(v.clone()),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<String> {
        match self {
            Value::String(s) => s.as_str().map(|s| s.to_string()),
            _ => None
        }
    }
}

pub struct MsgpackStreamIterator<R: Read> {
    rd: R,
}

impl<R: Read> MsgpackStreamIterator<R> {
    pub fn new(rd: R) -> MsgpackStreamIterator<R> {
        MsgpackStreamIterator {
            rd
        }
    }
}

impl<R: Read> Iterator for MsgpackStreamIterator<R> {
    type Item = Result<Value, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = read_value(&mut self.rd);

        match result {
            Ok(v) => Some(Ok(v)),
            Err(e) => if e.kind() == ErrorKind::UnexpectedEof {
                None
            } else {
                Some(Err(e.into()))
            }
        }
    }
}