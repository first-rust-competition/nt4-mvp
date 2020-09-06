use serde::{Serialize, Deserialize};
use super::MessageBody;

#[derive(Serialize, Deserialize)]
pub struct GetValues {
    ids: Vec<u32>,
    options: Option<GetValuesOptions>
}

#[derive(Serialize, Deserialize)]
pub struct GetValuesOptions {
    timestamped: bool
}

#[derive(Serialize, Deserialize)]
pub struct Subscribe {
    ids: Vec<u32>,
    subuid: u32,
    options: Option<SubscribeOptions>
}

#[derive(Serialize, Deserialize)]
pub struct SubscribeOptions {
    immediate: bool,
    periodic: f64,
    logging: bool,
    timestamped: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Unsubscribe {
    subuid: u32,
}

impl_message!(GetValues, Subscribe, Unsubscribe);