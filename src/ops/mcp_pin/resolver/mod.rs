//! MCP server resolution: input fragment → concrete pin-able artifacts.
//!
//! Outputs are transient `ResolvedArtifact` values.

pub mod binary;
pub mod detector;
pub mod fragment;
pub mod initialize;
pub mod integrity;
pub mod spki;
pub mod subprocess;
pub mod volatile;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::ops::mcp_pin::hasher::{HashedBinary, HasherError};
use crate::ops::mcp_pin::lockfile::LockfileError;
use crate::ops::mcp_pin::types::{PackageManager, Transport};

pub use binary::BinaryPathResolver;
pub use detector::PackageManagerDetector;
pub use fragment::McpConfigFragment;
pub use initialize::{InitializeResponseCapturer, InitializeResponseDigest, TransportAddr};
pub use integrity::{
    IntegrityResolver, NpmIntegrityResolver, PipHashResolver, UvxIntegrityResolver,
};
pub use spki::{SpkiDigest, SpkiExtractor};
pub use subprocess::{StdSubprocessExecutor, SubprocessExecutor, SubprocessOutcome};
pub use volatile::{ReputationLookup, StubReputationLookup, Tier, VolatileChecker};

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("server '{name}' has ambiguous config: {reason}")]
    AmbiguousConfig { name: String, reason: String },

    #[error("server '{name}' has empty config (no command or url)")]
    EmptyConfig { name: String },

    #[error("unknown package manager dispatch: {command}")]
    UnknownPackageManager { command: String },

    #[error("command not found: '{cmd}'")]
    CommandNotFound { cmd: String },

    #[error("file is not executable: {path}")]
    NotExecutable { path: PathBuf },

    #[error("subprocess timeout: '{cmd}' ran for {elapsed_ms} ms")]
    SubprocessTimeout { cmd: String, elapsed_ms: u128 },

    #[error("subprocess failed: '{cmd}' (exit {exit_code}); stderr: {stderr_excerpt}")]
    SubprocessFailed {
        cmd: String,
        exit_code: i32,
        stderr_excerpt: String,
    },

    #[error("network unreachable for '{cmd}'")]
    NoNetwork { cmd: String },

    #[error("package not found: {pkg}{}", ver.as_ref().map(|v| format!("@{v}")).unwrap_or_default())]
    PackageNotFound { pkg: String, ver: Option<String> },

    #[error("TLS handshake failed for '{url}': {cause}")]
    TlsHandshake { url: String, cause: String },

    #[error("timeout: {operation} after {elapsed_ms} ms")]
    Timeout { operation: String, elapsed_ms: u128 },

    #[error("invalid URL: '{url}'")]
    InvalidUrl { url: String },

    #[error("unsupported transport: {transport} for '{url}'")]
    UnsupportedTransport { transport: String, url: String },

    #[error("hasher error: {0}")]
    Hasher(#[from] HasherError),

    #[error("lockfile error: {0}")]
    Lockfile(#[from] LockfileError),

    #[error("I/O: {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

/// Output value object. Consumed downstream to construct a `ServerPin`.
#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub name: String,
    pub package_manager: PackageManager,
    pub transport: Transport,
    pub package_integrity: Option<String>,
    pub binary_hash: Option<HashedBinary>,
    pub spki_sha256: Option<SpkiDigest>,
    pub initialize_response_hash: Option<InitializeResponseDigest>,
    pub volatile: bool,
    pub resolved_at: DateTime<Utc>,
}

/// Options governing a `Resolver::resolve` call. `Default::default()`
/// wires the `StubReputationLookup` so this unit can compile + test
/// without `mcp_reputation`.
pub struct ResolveOpts {
    pub reputation: Arc<dyn ReputationLookup>,
    pub project_root: Option<PathBuf>,
    pub inspect: bool,
    pub allow_self_signed: bool,
    pub subprocess_timeout: Duration,
    pub tls_timeout: Duration,
}

impl Default for ResolveOpts {
    fn default() -> Self {
        Self {
            reputation: Arc::new(StubReputationLookup),
            project_root: None,
            inspect: false,
            allow_self_signed: false,
            subprocess_timeout: Duration::from_secs(5),
            tls_timeout: Duration::from_secs(10),
        }
    }
}

/// Façade orchestrating detector → integrity → binary/SPKI → optional
/// initialize-capture into a `ResolvedArtifact`.
pub struct Resolver;

impl Resolver {
    pub fn resolve(
        fragment: &McpConfigFragment,
        opts: &ResolveOpts,
    ) -> Result<ResolvedArtifact, ResolverError> {
        let package_manager = PackageManagerDetector::detect(fragment)?;
        let transport = fragment.effective_transport();
        let volatile_checker = VolatileChecker::new(opts.reputation.clone());
        let volatile = volatile_checker.is_volatile(&fragment.name);

        let mut package_integrity: Option<String> = None;
        let mut binary_hash: Option<HashedBinary> = None;
        let mut spki_sha256: Option<SpkiDigest> = None;
        let mut initialize_response_hash: Option<InitializeResponseDigest> = None;

        let executor = StdSubprocessExecutor;

        match &package_manager {
            PackageManager::Npm { pkg, ver } => {
                let resolver = NpmIntegrityResolver::new(StdSubprocessExecutor)
                    .with_timeout(opts.subprocess_timeout);
                package_integrity = resolver.resolve_integrity(
                    pkg,
                    ver.as_deref(),
                    opts.project_root.as_deref(),
                )?;
                if !volatile {
                    if let Some(cmd) = fragment.command.as_deref() {
                        binary_hash = Some(BinaryPathResolver::hash_binary_command(cmd)?);
                    }
                }
            }
            PackageManager::Pip { pkg, ver } => {
                let resolver = PipHashResolver;
                package_integrity = resolver.resolve_integrity(
                    pkg,
                    ver.as_deref(),
                    opts.project_root.as_deref(),
                )?;
                if !volatile {
                    if let Some(cmd) = fragment.command.as_deref() {
                        binary_hash = Some(BinaryPathResolver::hash_binary_command(cmd)?);
                    }
                }
            }
            PackageManager::Uvx { pkg, ver } => {
                let resolver = UvxIntegrityResolver;
                package_integrity = resolver.resolve_integrity(
                    pkg,
                    ver.as_deref(),
                    opts.project_root.as_deref(),
                )?;
                if !volatile {
                    if let Some(cmd) = fragment.command.as_deref() {
                        binary_hash = Some(BinaryPathResolver::hash_binary_command(cmd)?);
                    }
                }
            }
            PackageManager::PythonModule { .. } | PackageManager::Bare { .. } => {
                if !volatile {
                    if let Some(cmd) = fragment.command.as_deref() {
                        binary_hash = Some(BinaryPathResolver::hash_binary_command(cmd)?);
                    }
                }
            }
            PackageManager::RemoteSse { url } | PackageManager::RemoteHttp { url } => {
                let extractor = SpkiExtractor::new();
                spki_sha256 = Some(extractor.extract_spki(url, opts.tls_timeout)?);
            }
        }

        if opts.inspect && matches!(transport, Transport::Stdio) {
            if let Some(cmd) = fragment.command.as_deref() {
                let addr = TransportAddr::Stdio {
                    command: cmd.to_string(),
                    args: fragment.args.clone().unwrap_or_default(),
                };
                initialize_response_hash = match InitializeResponseCapturer::capture(
                    transport,
                    addr,
                    opts.subprocess_timeout * 6,
                ) {
                    Ok(d) => Some(d),
                    Err(ResolverError::UnsupportedTransport { .. }) => None,
                    Err(e) => return Err(e),
                };
            }
        }

        // Sanity hook: keep clippy happy that executor is used in
        // multi-arm dispatch when the poisoning scan reuses the trait.
        let _ = &executor;

        Ok(ResolvedArtifact {
            name: fragment.name.clone(),
            package_manager,
            transport,
            package_integrity,
            binary_hash,
            spki_sha256,
            initialize_response_hash,
            volatile,
            resolved_at: Utc::now(),
        })
    }
}
