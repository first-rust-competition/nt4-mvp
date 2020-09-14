use crate::server::NTServer;

mod client;
mod entry;
mod error;
mod net;
mod server;
mod util;
mod persist;

fn main() {
    env_logger::init();
    let _srv = NTServer::new();
    log::info!("Server started on ws://localhost:5810");

    loop {}
}
