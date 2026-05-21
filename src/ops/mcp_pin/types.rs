//! Value objects for the MCP pin / lockfile domain.
//!
//! See `bolts/075-lockfile-hasher/ddd-01-domain-model.md` for the static model
//! and `bolts/075-lockfile-hasher/adr-014-format-version-migration-pattern.md`
//! for the cross-cutting versioning policy.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How a server pin was created. See `PinMethod` in the static model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinMethod {
    #[default]
    Auto,
    Manual,
    Strict,
}

impl fmt::Display for PinMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinMethod::Auto => f.write_str("auto"),
            PinMethod::Manual => f.write_str("manual"),
            PinMethod::Strict => f.write_str("strict"),
        }
    }
}

/// Package-manager dispatch tag for a `ServerPin`.
///
/// Serialized as a tagged TOML table: `{ kind = "npm", pkg = "...", ver = "..." }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PackageManager {
    Npm {
        pkg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ver: Option<String>,
    },
    Pip {
        pkg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ver: Option<String>,
    },
    Uvx {
        pkg: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ver: Option<String>,
    },
    #[serde(rename = "python-module")]
    PythonModule {
        module: String,
    },
    Bare {
        path: PathBuf,
    },
    #[serde(rename = "remote-sse")]
    RemoteSse {
        url: String,
    },
    #[serde(rename = "remote-http")]
    RemoteHttp {
        url: String,
    },
}

/// Wire-protocol transport for an MCP server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    #[default]
    Stdio,
    Sse,
    Http,
}

/// `{os}-{arch}` platform identifier.
///
/// Constructed via `Platform::current()` at pin/verify time and serialized as a
/// plain lowercase string. Equality is byte equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Platform(String);

impl Platform {
    /// Platform of the currently running binary.
    pub fn current() -> Self {
        Platform(format!(
            "{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    }

    pub fn new<S: Into<String>>(s: S) -> Self {
        Platform(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
