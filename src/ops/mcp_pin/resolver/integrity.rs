//! Package-manager integrity extraction.
//!
//! Strategy: lockfile-first (deterministic + airgap-safe); subprocess
//! fallback (`npm view`) only when no lockfile entry available.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value as JsonValue;

use super::subprocess::{truncate_stderr, SubprocessExecutor};
use super::ResolverError;

pub trait IntegrityResolver: Send + Sync {
    fn resolve_integrity(
        &self,
        pkg: &str,
        version: Option<&str>,
        project_root: Option<&Path>,
    ) -> Result<Option<String>, ResolverError>;
}

/// Cache key: (package name, optional version constraint).
type CacheKey = (String, Option<String>);

/// Cache value: (insertion time, integrity string).
type CacheEntry = (Instant, String);

/// In-memory TTL cache for `npm view` subprocess fallback. Scoped to one
/// `Resolver` instance.
pub struct NpmViewCache {
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
    ttl: Duration,
}

impl Default for NpmViewCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl NpmViewCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    fn get(&self, pkg: &str, ver: Option<&str>) -> Option<String> {
        let map = self.entries.lock().ok()?;
        let key = (pkg.to_string(), ver.map(String::from));
        let (inserted, value) = map.get(&key)?;
        if inserted.elapsed() <= self.ttl {
            Some(value.clone())
        } else {
            None
        }
    }

    fn put(&self, pkg: &str, ver: Option<&str>, value: String) {
        if let Ok(mut map) = self.entries.lock() {
            let key = (pkg.to_string(), ver.map(String::from));
            map.insert(key, (Instant::now(), value));
        }
    }
}

pub struct NpmIntegrityResolver<E: SubprocessExecutor> {
    executor: E,
    cache: NpmViewCache,
    subprocess_timeout: Duration,
    allow_network: bool,
}

impl<E: SubprocessExecutor> NpmIntegrityResolver<E> {
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            cache: NpmViewCache::default(),
            subprocess_timeout: Duration::from_secs(5),
            allow_network: true,
        }
    }

    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.subprocess_timeout = timeout;
        self
    }
}

impl<E: SubprocessExecutor> IntegrityResolver for NpmIntegrityResolver<E> {
    fn resolve_integrity(
        &self,
        pkg: &str,
        version: Option<&str>,
        project_root: Option<&Path>,
    ) -> Result<Option<String>, ResolverError> {
        // Step 1: package-lock.json in project_root.
        if let Some(root) = project_root {
            if let Some(integrity) = scan_npm_lockfile(root, pkg, version)? {
                return Ok(Some(integrity));
            }
        }

        // Step 2: subprocess fallback `npm view <pkg>@<ver> dist.integrity --json`.
        if !self.allow_network {
            return Ok(None);
        }
        if let Some(cached) = self.cache.get(pkg, version) {
            return Ok(Some(cached));
        }
        let spec = match version {
            Some(v) => format!("{pkg}@{v}"),
            None => pkg.to_string(),
        };
        let outcome = self
            .executor
            .execute(
                "npm",
                &["view", &spec, "dist.integrity", "--json"],
                self.subprocess_timeout,
            )
            .map_err(|e| match e {
                ResolverError::SubprocessTimeout { .. } => e,
                ResolverError::Io { ref context, .. } if context.contains("spawn 'npm'") => {
                    ResolverError::NoNetwork {
                        cmd: "npm".to_string(),
                    }
                }
                other => other,
            })?;

        if outcome.exit_code != 0 {
            return Err(ResolverError::SubprocessFailed {
                cmd: format!("npm view {spec}"),
                exit_code: outcome.exit_code,
                stderr_excerpt: truncate_stderr(&outcome.stderr),
            });
        }

        // `npm view dist.integrity --json` returns either `"sha512-..."` or `[...]`.
        let trimmed = String::from_utf8_lossy(&outcome.stdout).trim().to_string();
        if trimmed.is_empty() || trimmed == "null" {
            return Err(ResolverError::PackageNotFound {
                pkg: pkg.to_string(),
                ver: version.map(String::from),
            });
        }
        let value: JsonValue =
            serde_json::from_str(&trimmed).map_err(|e| ResolverError::SubprocessFailed {
                cmd: format!("npm view {spec}"),
                exit_code: 0,
                stderr_excerpt: format!("parse error: {e}"),
            })?;
        let integrity = match value {
            JsonValue::String(s) => s,
            JsonValue::Array(arr) => arr
                .into_iter()
                .find_map(|v| v.as_str().map(String::from))
                .ok_or_else(|| ResolverError::PackageNotFound {
                    pkg: pkg.to_string(),
                    ver: version.map(String::from),
                })?,
            _ => {
                return Err(ResolverError::PackageNotFound {
                    pkg: pkg.to_string(),
                    ver: version.map(String::from),
                });
            }
        };

        self.cache.put(pkg, version, integrity.clone());
        Ok(Some(integrity))
    }
}

/// Scan a project's `package-lock.json` for the matching `pkg@ver` integrity.
fn scan_npm_lockfile(
    project_root: &Path,
    pkg: &str,
    version: Option<&str>,
) -> Result<Option<String>, ResolverError> {
    let lockfile_path = project_root.join("package-lock.json");
    if !lockfile_path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&lockfile_path).map_err(|e| ResolverError::Io {
        context: format!("read {}", lockfile_path.display()),
        source: e,
    })?;
    let parsed: JsonValue = serde_json::from_slice(&bytes).map_err(|e| ResolverError::Io {
        context: format!("parse {}", lockfile_path.display()),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;

    // npm v3 lockfile shape: `{"packages": {"node_modules/<pkg>": {"version":"...","integrity":"..."}}}`
    if let Some(packages) = parsed.get("packages").and_then(JsonValue::as_object) {
        let target = format!("node_modules/{pkg}");
        if let Some(entry) = packages.get(&target) {
            if let Some(ver) = version {
                if entry.get("version").and_then(JsonValue::as_str) != Some(ver) {
                    return Ok(None);
                }
            }
            if let Some(integrity) = entry.get("integrity").and_then(JsonValue::as_str) {
                return Ok(Some(integrity.to_string()));
            }
        }
    }
    // Legacy v1 shape: `{"dependencies": {"<pkg>": {"version":"...","integrity":"..."}}}`
    if let Some(deps) = parsed.get("dependencies").and_then(JsonValue::as_object) {
        if let Some(entry) = deps.get(pkg) {
            if let Some(ver) = version {
                if entry.get("version").and_then(JsonValue::as_str) != Some(ver) {
                    return Ok(None);
                }
            }
            if let Some(integrity) = entry.get("integrity").and_then(JsonValue::as_str) {
                return Ok(Some(integrity.to_string()));
            }
        }
    }
    Ok(None)
}

/// Pip integrity resolver. Scans `requirements.txt` and `poetry.lock`
/// for the matching `pkg==ver` hash entries.
pub struct PipHashResolver;

impl IntegrityResolver for PipHashResolver {
    fn resolve_integrity(
        &self,
        pkg: &str,
        version: Option<&str>,
        project_root: Option<&Path>,
    ) -> Result<Option<String>, ResolverError> {
        let Some(root) = project_root else {
            return Ok(None);
        };

        // requirements.txt: lines like `pkg==1.2.3 \` followed by `    --hash=sha256:...`.
        let req = root.join("requirements.txt");
        if req.exists() {
            let bytes = std::fs::read(&req).map_err(|e| ResolverError::Io {
                context: format!("read {}", req.display()),
                source: e,
            })?;
            if let Some(h) = scan_requirements_txt(&bytes, pkg, version) {
                return Ok(Some(h));
            }
        }

        let poetry = root.join("poetry.lock");
        if poetry.exists() {
            let bytes = std::fs::read(&poetry).map_err(|e| ResolverError::Io {
                context: format!("read {}", poetry.display()),
                source: e,
            })?;
            if let Some(h) = scan_poetry_lock(&bytes, pkg, version) {
                return Ok(Some(h));
            }
        }

        Ok(None)
    }
}

fn scan_requirements_txt(bytes: &[u8], pkg: &str, version: Option<&str>) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut hashes = Vec::new();
    let mut current_matches = false;
    let pkg_lower = pkg.to_ascii_lowercase();

    for raw in text.lines() {
        let line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("--hash=") {
            if current_matches {
                hashes.push(rest.trim().to_string());
            }
            continue;
        }
        // New package line.
        current_matches = false;
        let head = line.split_whitespace().next().unwrap_or("");
        let (name, op_ver) = head
            .split_once("==")
            .or_else(|| head.split_once(">="))
            .unwrap_or((head, ""));
        if name.to_ascii_lowercase() == pkg_lower {
            current_matches = match version {
                None => true,
                Some(v) => op_ver == v,
            };
        }
    }

    if hashes.is_empty() {
        None
    } else {
        Some(hashes.join(","))
    }
}

fn scan_poetry_lock(bytes: &[u8], pkg: &str, version: Option<&str>) -> Option<String> {
    // Best-effort substring match within `[[package]]` sections. A full
    // TOML parse is heavier than necessary for hash extraction; document
    // limitations in module rustdoc.
    let text = std::str::from_utf8(bytes).ok()?;
    let mut in_pkg = false;
    let mut pkg_matches = false;
    let mut hashes = Vec::new();
    let mut in_files = false;
    let pkg_lower = pkg.to_ascii_lowercase();

    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("[[package]]") {
            in_pkg = true;
            pkg_matches = false;
            in_files = false;
            continue;
        }
        if line.starts_with('[') && !line.starts_with("[[package]]") {
            in_pkg = false;
            in_files = false;
            continue;
        }
        if !in_pkg {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name = ") {
            let name = rest.trim_matches('"').to_ascii_lowercase();
            pkg_matches = name == pkg_lower;
            continue;
        }
        if let Some(rest) = line.strip_prefix("version = ") {
            if let Some(v) = version {
                if rest.trim_matches('"') != v {
                    pkg_matches = false;
                }
            }
            continue;
        }
        if pkg_matches && line.starts_with("files = [") {
            in_files = true;
            continue;
        }
        if in_files {
            if line.starts_with(']') {
                in_files = false;
                continue;
            }
            if let Some(idx) = line.find("hash = \"") {
                let after = &line[idx + "hash = \"".len()..];
                if let Some(end) = after.find('"') {
                    hashes.push(after[..end].to_string());
                }
            }
        }
    }

    if hashes.is_empty() {
        None
    } else {
        Some(hashes.join(","))
    }
}

/// Uvx wraps an npm package effectively; delegate to npm integrity.
/// Stub for now — returns `None`. Real impl would inspect uvx cache or
/// the underlying registry. Tracked for future enhancement.
pub struct UvxIntegrityResolver;

impl IntegrityResolver for UvxIntegrityResolver {
    fn resolve_integrity(
        &self,
        _pkg: &str,
        _version: Option<&str>,
        _project_root: Option<&Path>,
    ) -> Result<Option<String>, ResolverError> {
        Ok(None)
    }
}
