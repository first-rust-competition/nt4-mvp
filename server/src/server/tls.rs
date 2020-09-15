use rustls::internal::pemfile::{pkcs8_private_keys, certs};
use rustls::{PrivateKey, Certificate, ServerConfig, NoClientAuth};
use std::io::BufReader;
use std::fs::File;
use async_tls::TlsAcceptor;

fn load_key() -> Vec<PrivateKey> {
    pkcs8_private_keys(&mut BufReader::new(File::open("./NT4.key").unwrap()))
        .unwrap()
}

fn load_cert() -> Vec<Certificate> {
    certs(&mut BufReader::new(File::open("./NT4.crt.sgn").unwrap()))
        .unwrap()
}

pub fn generate_acceptor() -> TlsAcceptor {
    let mut keys = load_key();
    let cert = load_cert();

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(cert, keys.remove(0))
        .unwrap();

    TlsAcceptor::from(config)
}