use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::capnp::TlsListen;
use futures_rustls::TlsAcceptor;
use miette::Diagnostic;
use rustls::version::{TLS12, TLS13};
use rustls::{Certificate, PrivateKey, ServerConfig, SupportedCipherSuite};
use thiserror::Error;
use tracing::Level;

use crate::keylog::KeyLogFile;
use crate::tls::Error::KeyLogOpen;

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

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("failed to open certificate file at path {0}")]
    OpenCertFile(PathBuf, #[source] io::Error),
    #[error("failed to open private key file at path {0}")]
    OpenKeyFile(PathBuf, #[source] io::Error),
    #[error("failed to read system certs")]
    SystemCertsFile(#[source] io::Error),
    #[error("failed to read from key file")]
    ReadKeyFile(#[source] io::Error),
    #[error("private key file must contain a single PEM-encoded private key")]
    KeyFileFormat,
    #[error("invalid TLS version {0}")]
    TlsVersion(String),
    #[error("Initializing TLS context failed")]
    Builder(
        #[from]
        #[source]
        rustls::Error,
    ),
    #[error("failed to initialize key log")]
    KeyLogOpen(#[source] io::Error),
}

impl TlsConfig {
    pub fn new(keylogfile: Option<impl AsRef<Path>>, warn: bool) -> Result<Self, Error> {
        let span = tracing::span!(Level::INFO, "tls");
        let _guard = span.enter();

        if warn {
            Self::warn_logging_secrets(keylogfile.as_ref());
        }

        if let Some(path) = keylogfile {
            let keylog = Some(
                KeyLogFile::new(path)
                    .map(|ok| Arc::new(ok))
                    .map_err(KeyLogOpen)?,
            );
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

    pub fn make_tls_acceptor(&self, config: &TlsListen) -> Result<TlsAcceptor, Error> {
        let span = tracing::debug_span!("tls");
        let _guard = span.enter();

        let path = config.certfile.as_path();
        tracing::debug!(path = %path.display(), "reading certificates");
        let mut certfp =
            BufReader::new(File::open(path).map_err(|e| Error::OpenCertFile(path.into(), e))?);
        let certs = rustls_pemfile::certs(&mut certfp)
            .map_err(Error::SystemCertsFile)?
            .into_iter()
            .map(Certificate)
            .collect();

        let path = config.keyfile.as_path();
        tracing::debug!(path = %path.display(), "reading private key");
        let mut keyfp =
            BufReader::new(File::open(path).map_err(|err| Error::OpenKeyFile(path.into(), err))?);
        let key = match rustls_pemfile::read_one(&mut keyfp).map_err(Error::ReadKeyFile)? {
            Some(rustls_pemfile::Item::PKCS8Key(key) | rustls_pemfile::Item::RSAKey(key)) => {
                PrivateKey(key)
            }
            _ => {
                tracing::error!("private key file invalid");
                return Err(Error::KeyFileFormat);
            }
        };

        let tls_builder = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups();

        let tls_builder = if let Some(ref min) = config.tls_min_version {
            let v = min.to_lowercase();
            match v.as_str() {
                "tls12" => tls_builder.with_protocol_versions(&[&TLS12]),
                "tls13" => tls_builder.with_protocol_versions(&[&TLS13]),
                _ => return Err(Error::TlsVersion(v)),
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
