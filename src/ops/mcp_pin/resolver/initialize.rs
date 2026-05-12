//! MCP `initialize` JSON-RPC capture.
//!
//! Sends the canonical `initialize` request, reads the response, and
//! returns a SHA-256 of the canonicalized response body.
//!
//! Stdio transport is fully implemented. Remote SSE/HTTP transports
//! return `UnsupportedTransport` in v1 — wire-up deferred to a future
//! follow-up (tracked in unit notes; not blocking v0.8 GA core path).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

use super::ResolverError;
use crate::ops::mcp_pin::hasher::CanonicalJsonHasher;
use crate::ops::mcp_pin::types::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializeResponseDigest(pub [u8; 32]);

impl InitializeResponseDigest {
    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

#[derive(Debug, Clone)]
pub enum TransportAddr {
    Stdio { command: String, args: Vec<String> },
    Sse { url: String },
    Http { url: String },
}

pub struct InitializeResponseCapturer;

impl InitializeResponseCapturer {
    pub fn capture(
        transport: Transport,
        addr: TransportAddr,
        timeout: Duration,
    ) -> Result<InitializeResponseDigest, ResolverError> {
        match (transport, addr) {
            (Transport::Stdio, TransportAddr::Stdio { command, args }) => {
                capture_stdio(&command, &args, timeout)
            }
            (Transport::Sse, TransportAddr::Sse { url })
            | (Transport::Http, TransportAddr::Http { url }) => {
                Err(ResolverError::UnsupportedTransport {
                    transport: format!("{transport:?}"),
                    url,
                })
            }
            _ => Err(ResolverError::UnsupportedTransport {
                transport: format!("{transport:?}"),
                url: String::new(),
            }),
        }
    }
}

fn capture_stdio(
    command: &str,
    args: &[String],
    timeout: Duration,
) -> Result<InitializeResponseDigest, ResolverError> {
    let started = Instant::now();
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ResolverError::Io {
            context: format!("spawn '{command}'"),
            source: e,
        })?;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "envforge", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    let payload = format!("{}\n", request);

    {
        let mut stdin = child.stdin.take().ok_or_else(|| ResolverError::Io {
            context: format!("stdin '{command}'"),
            source: std::io::Error::other("no stdin"),
        })?;
        stdin
            .write_all(payload.as_bytes())
            .map_err(|e| ResolverError::Io {
                context: format!("write stdin '{command}'"),
                source: e,
            })?;
        stdin.flush().map_err(|e| ResolverError::Io {
            context: format!("flush stdin '{command}'"),
            source: e,
        })?;
    }

    let stdout = child.stdout.take().ok_or_else(|| ResolverError::Io {
        context: format!("stdout '{command}'"),
        source: std::io::Error::other("no stdout"),
    })?;
    let mut reader = BufReader::new(stdout);
    // Read a single JSON-RPC response line (MCP stdio is line-delimited).
    if started.elapsed() >= timeout {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ResolverError::Timeout {
            operation: format!("initialize stdio '{command}'"),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }
    let mut response_line = String::new();
    match reader.read_line(&mut response_line) {
        Ok(_) => {}
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ResolverError::Io {
                context: format!("read stdout '{command}'"),
                source: e,
            });
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    if response_line.trim().is_empty() {
        return Err(ResolverError::Timeout {
            operation: format!("initialize stdio '{command}': empty response"),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }

    let hash =
        CanonicalJsonHasher::canonicalize_and_hash(response_line.as_bytes()).map_err(|e| {
            ResolverError::Io {
                context: format!("canonicalize response '{command}'"),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            }
        })?;
    Ok(InitializeResponseDigest(hash))
}
