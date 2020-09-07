use super::MessageBody;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct GetValues {
    ids: Vec<u32>,
    options: Option<GetValuesOptions>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
pub struct GetValuesOptions {
    timestamped: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Subscribe {
    ids: Vec<u32>,
    subuid: u32,
    options: Option<SubscribeOptions>,
}

fn default_periodic() -> f64 {
    0.1
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
pub struct SubscribeOptions {
    immediate: bool,
    #[serde(default = "default_periodic")]
    periodic: f64,
    logging: bool,
    timestamped: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Unsubscribe {
    subuid: u32,
}

impl_message!(GetValues, Subscribe, Unsubscribe);
