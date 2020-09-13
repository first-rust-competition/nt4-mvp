use crate::server::NTServer;
use log::LevelFilter;

mod client;
mod entry;
mod error;
mod net;
mod server;

fn main() {
    env_logger::init();
    let srv = NTServer::new();
    println!("Server started on ws://localhost:5810");

    loop {}
}
