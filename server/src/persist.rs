use crate::entry::Topic;
use std::collections::HashMap;
use async_std::fs;

pub async fn flush_persistent(entries: &HashMap<String, Topic>) {

    let mut ser_entries = HashMap::new();
    for (name, topic) in entries.iter().filter(|(_, topic)| topic.flags.contains(&"persistent".to_string())) {
        ser_entries.insert(name.clone(), topic.clone());
    }

    fs::write("./persisted_entries.bin", rmp_serde::to_vec(&ser_entries).unwrap()).await;
}

pub fn restore_persistent() -> anyhow::Result<HashMap<String, Topic>> {
    let data = std::fs::read("./persisted_entries.bin")?;

    rmp_serde::from_read(&data[..]).map_err(Into::into)
}