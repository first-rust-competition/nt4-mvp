use crate::server::NTServer;
use chrono::Local;
use log::LevelFilter;
use std::io;
use fern::colors::{ColoredLevelConfig, Color};

mod client;
mod entry;
mod error;
mod net;
mod persist;
mod server;
mod util;

fn init_logger() {
    let colors = ColoredLevelConfig::new()
        .info(Color::Green)
        .error(Color::Red)
        .warn(Color::Yellow)
        .debug(Color::White);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} {} {}  {}",
                Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::Info)
        .chain(io::stdout())
        .apply()
        .unwrap();
}

fn main() {
    init_logger();
    let _srv = NTServer::new();
    log::info!("Server started on ws://localhost:5810");

    loop {}
}
