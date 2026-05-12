//! TLS handshake + SubjectPublicKeyInfo SHA-256 extraction.
//!
//! Pins the SPKI digest (not the full cert) — survives Let's Encrypt
//! 90-day cert rotation as long as the server's keypair stays stable.

use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use sha2::{Digest, Sha256};
use x509_cert::der::{Decode, Encode};
use x509_cert::Certificate;

use super::ResolverError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpkiDigest(pub [u8; 32]);

impl SpkiDigest {
    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

pub struct SpkiExtractor {
    config: Arc<ClientConfig>,
}

impl Default for SpkiExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl SpkiExtractor {
    pub fn new() -> Self {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Self {
            config: Arc::new(config),
        }
    }

    pub fn extract_spki(&self, url: &str, timeout: Duration) -> Result<SpkiDigest, ResolverError> {
        let (host, port) = parse_https_url(url)?;
        let addr = format!("{host}:{port}")
            .to_socket_addrs()
            .map_err(|e| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("dns: {e}"),
            })?
            .next()
            .ok_or_else(|| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: "dns: no address".into(),
            })?;

        let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
            ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("connect: {e}"),
            }
        })?;
        tcp.set_read_timeout(Some(timeout))
            .map_err(|e| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("set_read_timeout: {e}"),
            })?;
        tcp.set_write_timeout(Some(timeout))
            .map_err(|e| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("set_write_timeout: {e}"),
            })?;

        let server_name = ServerName::try_from(host).map_err(|e| ResolverError::TlsHandshake {
            url: url.to_string(),
            cause: format!("invalid server name: {e}"),
        })?;
        let conn = ClientConnection::new(self.config.clone(), server_name).map_err(|e| {
            ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("client init: {e}"),
            }
        })?;
        let mut stream = StreamOwned::new(conn, tcp);

        // Trigger handshake by flushing.
        stream.flush().map_err(|e| ResolverError::TlsHandshake {
            url: url.to_string(),
            cause: format!("handshake: {e}"),
        })?;

        let certs = stream
            .conn
            .peer_certificates()
            .ok_or_else(|| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: "no peer certificates".into(),
            })?;
        let leaf = certs.first().ok_or_else(|| ResolverError::TlsHandshake {
            url: url.to_string(),
            cause: "empty cert chain".into(),
        })?;

        let cert =
            Certificate::from_der(leaf.as_ref()).map_err(|e| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("cert parse: {e}"),
            })?;
        let spki = cert
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|e| ResolverError::TlsHandshake {
                url: url.to_string(),
                cause: format!("spki encode: {e}"),
            })?;

        let mut hasher = Sha256::new();
        hasher.update(&spki);
        let digest = hasher.finalize();
        Ok(SpkiDigest(digest.into()))
    }
}

fn parse_https_url(url: &str) -> Result<(String, u16), ResolverError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| ResolverError::InvalidUrl {
            url: url.to_string(),
        })?;
    let path_split = rest.split_once('/').map_or(rest, |(host, _)| host);
    if let Some((host, port)) = path_split.rsplit_once(':') {
        let port: u16 = port.parse().map_err(|_| ResolverError::InvalidUrl {
            url: url.to_string(),
        })?;
        Ok((host.to_string(), port))
    } else {
        Ok((path_split.to_string(), 443))
    }
}
