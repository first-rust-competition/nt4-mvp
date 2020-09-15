use rustls::internal::pemfile::{pkcs8_private_keys, certs};
use rustls::{PrivateKey, Certificate, ServerConfig, NoClientAuth};
use std::io::BufReader;
use std::fs::File;
use async_tls::TlsAcceptor;
use anyhow::Result;

fn load_key() -> Result<Vec<PrivateKey>> {
    pkcs8_private_keys(&mut BufReader::new(File::open("./NT4.key")?))
        .map_err(|_| anyhow::anyhow!("Unable to decode PKCS8 private key"))
}

fn load_cert() -> Result<Vec<Certificate>> {
    certs(&mut BufReader::new(File::open("./NT4.crt.sgn")?))
        .map_err(|_| anyhow::anyhow!("Unable to decode certificate"))
}

pub fn generate_acceptor() -> Result<TlsAcceptor> {
    let mut keys = load_key()?;
    let cert = load_cert()?;

    let mut config = ServerConfig::new(NoClientAuth::new());
    config.set_single_cert(cert, keys.remove(0))?;

    Ok(TlsAcceptor::from(config))
}
