use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use crate::capnp::TlsListen;
use futures_rustls::TlsAcceptor;
use rustls::version::{TLS12, TLS13};
use rustls::{Certificate, PrivateKey, ServerConfig, SupportedCipherSuite};
use tracing::Level;

use crate::keylog::KeyLogFile;

fn lookup_cipher_suite(name: &str) -> Option<SupportedCipherSuite> {
    match name {
        "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256" => {
            Some(rustls::cipher_suite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256)
        }
        "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384" => {
            Some(rustls::cipher_suite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384)
        }
        "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256" => {
            Some(rustls::cipher_suite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256)
        }
        "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256" => {
            Some(rustls::cipher_suite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256)
        }
        "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384" => {
            Some(rustls::cipher_suite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384)
        }
        "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256" => {
            Some(rustls::cipher_suite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256)
        }
        "TLS13_AES_128_GCM_SHA256" => Some(rustls::cipher_suite::TLS13_AES_128_GCM_SHA256),
        "TLS13_AES_256_GCM_SHA384" => Some(rustls::cipher_suite::TLS13_AES_256_GCM_SHA384),
        "TLS13_CHACHA20_POLY1305_SHA256" => {
            Some(rustls::cipher_suite::TLS13_CHACHA20_POLY1305_SHA256)
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    keylog: Option<Arc<KeyLogFile>>,
}

impl TlsConfig {
    pub fn new(keylogfile: Option<impl AsRef<Path>>, warn: bool) -> io::Result<Self> {
        let span = tracing::span!(Level::INFO, "tls");
        let _guard = span.enter();

        if warn {
            Self::warn_logging_secrets(keylogfile.as_ref());
        }

        if let Some(path) = keylogfile {
            let keylog = Some(KeyLogFile::new(path).map(|ok| Arc::new(ok))?);
            Ok(Self { keylog })
        } else {
            Ok(Self { keylog: None })
        }
    }

    fn warn_logging_secrets(path: Option<impl AsRef<Path>>) {
        if let Some(path) = path {
            let path = path.as_ref().display();
            tracing::warn!(keylog = true, path = %path,
                "TLS secret logging is ENABLED! TLS secrets and keys will be written to {}",
                path);
        } else {
            tracing::debug!(keylog = false, "TLS secret logging is disabled.");
        }
    }

    pub fn make_tls_acceptor(&self, config: &TlsListen) -> anyhow::Result<TlsAcceptor> {
        let span = tracing::debug_span!("tls");
        let _guard = span.enter();

        tracing::debug!(path = %config.certfile.as_path().display(), "reading certificates");
        let mut certfp = BufReader::new(File::open(config.certfile.as_path())?);
        let certs = rustls_pemfile::certs(&mut certfp)?
            .into_iter()
            .map(Certificate)
            .collect();

        tracing::debug!(path = %config.keyfile.as_path().display(), "reading private key");
        let mut keyfp = BufReader::new(File::open(config.keyfile.as_path())?);
        let key = match rustls_pemfile::read_one(&mut keyfp)? {
            Some(rustls_pemfile::Item::PKCS8Key(key) | rustls_pemfile::Item::RSAKey(key)) => {
                PrivateKey(key)
            }
            _ => {
                tracing::error!("private key file invalid");
                anyhow::bail!("private key file must contain a PEM-encoded private key")
            }
        };

        let tls_builder = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups();

        let tls_builder = if let Some(ref min) = config.tls_min_version {
            match min.as_str() {
                "tls12" => tls_builder.with_protocol_versions(&[&TLS12]),
                "tls13" => tls_builder.with_protocol_versions(&[&TLS13]),
                x => anyhow::bail!("TLS version {} is invalid", x),
            }
        } else {
            tls_builder.with_safe_default_protocol_versions()
        }?;

        let mut tls_config = tls_builder
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        if let Some(keylog) = &self.keylog {
            tls_config.key_log = keylog.clone();
        }

        Ok(Arc::new(tls_config).into())
    }
}
