use crate::server::NTServer;
use log::LevelFilter;

mod net;
mod error;
mod client;
mod server;
mod entry;

fn main() {
    env_logger::init();
    let srv = NTServer::new();
    println!("Server started on ws://localhost:5810");

    loop {}
}
