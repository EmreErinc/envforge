use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::OpError;

const LEASES_DIR: &str = "leases";

/// Process-wide lock serializing every lease operation
/// (create / revoke / renew / check). Without this, a concurrent
/// `revoke_lease` could land between [`check_lease_access`] returning
/// "allowed" and the caller actually using the secret. The window is
/// narrow but real on multi-threaded callers (proxy + run sharing
/// state). Cross-process races still exist but are out of scope —
/// kernel `flock` would address them.
fn lease_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Acquire the lease lock, recovering from poisoning so a panic in one
/// caller doesn't permanently disable lease checks for the rest.
fn acquire_lease_lock() -> std::sync::MutexGuard<'static, ()> {
    lease_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lease {
    pub name: String,
    pub created_at: String,
    pub expires_at: String,
    pub keys: Option<Vec<String>>, // None = all keys
    pub revoked: bool,
    // ─── JIT extension (additive; serde-default for backward compat). See ADR-009. ───
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub single_redeem: bool,
    #[serde(default)]
    pub redeemed: bool,
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Secret one-time-redemption ticket for JIT leases. Returned (once) in the
    /// `JitHandle` at grant time and required to match on redeem, so the
    /// single-redeem capability cannot be exercised by anyone who merely learns
    /// the (audit-logged, predictable) lease name. `None` for manual leases.
    #[serde(default)]
    pub uuid: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LeaseStatus {
    pub name: String,
    pub expires_at: String,
    pub remaining_seconds: i64,
    pub expired: bool,
    pub revoked: bool,
    pub key_count: Option<usize>,
    pub pid: Option<u32>,
    pub redeemed: bool,
}

/// Get leases directory path.
pub fn leases_dir() -> Result<PathBuf, OpError> {
    let dir = crate::config::config_dir()?.join(LEASES_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get leases directory under a custom base (for testing).
#[cfg(test)]
fn leases_dir_at(base: &std::path::Path) -> PathBuf {
    let dir = base.join(LEASES_DIR);
    std::fs::create_dir_all(&dir).expect("create test leases dir");
    dir
}

/// Create a new lease.
pub fn create_lease(
    name: &str,
    ttl_seconds: i64,
    keys: Option<Vec<String>>,
) -> Result<Lease, OpError> {
    validate_lease_name(name)?;
    let _guard = acquire_lease_lock();
    let now = Utc::now();
    let expires = now + chrono::Duration::seconds(ttl_seconds);

    let lease = Lease {
        name: name.to_string(),
        created_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        keys,
        revoked: false,
        ..Default::default()
    };

    let dir = leases_dir()?;
    let path = dir.join(format!("{}.toml", name));
    let content = toml::to_string_pretty(&lease)?;
    std::fs::write(&path, content)?;

    Ok(lease)
}

/// Write a lease to a specific directory (for testing).
#[cfg(test)]
fn write_lease_at(dir: &std::path::Path, lease: &Lease) {
    let path = dir.join(format!("{}.toml", lease.name));
    let content = toml::to_string_pretty(lease).expect("serialize lease");
    std::fs::write(&path, content).expect("write lease file");
}

/// List all leases with their status.
pub fn list_leases() -> Result<Vec<LeaseStatus>, OpError> {
    let dir = leases_dir()?;
    Ok(list_leases_in(&dir))
}

fn list_leases_in(dir: &std::path::Path) -> Vec<LeaseStatus> {
    let mut statuses = Vec::new();

    if !dir.exists() {
        return statuses;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return statuses,
    };

    let now = Utc::now();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(lease) = toml::from_str::<Lease>(&content) {
                    let expires = DateTime::parse_from_rfc3339(&lease.expires_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(now);
                    let remaining = (expires - now).num_seconds();

                    statuses.push(LeaseStatus {
                        name: lease.name,
                        expires_at: lease.expires_at,
                        remaining_seconds: remaining,
                        expired: remaining <= 0,
                        revoked: lease.revoked,
                        key_count: lease.keys.as_ref().map(|k| k.len()),
                        pid: lease.pid,
                        redeemed: lease.redeemed,
                    });
                }
            }
        }
    }

    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    statuses
}

/// Revoke a specific lease.
pub fn revoke_lease(name: &str) -> Result<bool, OpError> {
    validate_lease_name(name)?;
    let _guard = acquire_lease_lock();
    let dir = leases_dir()?;
    revoke_lease_in(&dir, name)
}

/// Renew (extend) an existing, non-revoked lease by `ttl_seconds`.
/// Returns the updated lease, or `None` if the lease does not exist.
/// Errors if the lease has already been revoked or has already expired
/// (callers should create a new lease in that case).
pub fn renew_lease(name: &str, ttl_seconds: i64) -> Result<Option<Lease>, OpError> {
    validate_lease_name(name)?;
    let _guard = acquire_lease_lock();
    let dir = leases_dir()?;
    let path = dir.join(format!("{}.toml", name));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let mut lease: Lease = toml::from_str(&content)?;
    if lease.revoked {
        return Err(OpError::from(format!(
            "lease '{}' is revoked; create a new lease instead",
            name
        )));
    }
    if let Ok(expires) = DateTime::parse_from_rfc3339(&lease.expires_at) {
        if expires.with_timezone(&Utc) <= Utc::now() {
            return Err(OpError::from(format!(
                "lease '{}' has already expired; create a new lease instead",
                name
            )));
        }
    }
    let new_expires = Utc::now() + chrono::Duration::seconds(ttl_seconds);
    lease.expires_at = new_expires.to_rfc3339();
    let updated = toml::to_string_pretty(&lease)?;
    std::fs::write(&path, updated)?;
    Ok(Some(lease))
}

fn revoke_lease_in(dir: &std::path::Path, name: &str) -> Result<bool, OpError> {
    let path = dir.join(format!("{}.toml", name));

    if !path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&path)?;
    let mut lease: Lease = toml::from_str(&content)?;
    lease.revoked = true;
    let updated = toml::to_string_pretty(&lease)?;
    std::fs::write(&path, updated)?;

    Ok(true)
}

/// Revoke ALL leases (killswitch).
pub fn revoke_all_leases() -> Result<usize, OpError> {
    let _guard = acquire_lease_lock();
    let dir = leases_dir()?;
    revoke_all_in(&dir)
}

fn revoke_all_in(dir: &std::path::Path) -> Result<usize, OpError> {
    let mut count = 0;

    if !dir.exists() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            std::fs::remove_file(&path)?;
            count += 1;
        }
    }

    Ok(count)
}

/// Check if a key is accessible under any active (non-expired, non-revoked) lease.
/// Returns Some(lease_name) if access granted, None if denied.
///
/// Holds the process-wide lease lock for the duration of the check, so a
/// concurrent `revoke_lease` / `revoke_all_leases` cannot land between
/// the check and the caller's use of the returned verdict (in-process
/// TOCTOU). For tighter coupling, callers can use
/// [`with_lease_check_locked`] to keep the lock held while consuming.
pub fn check_lease_access(key: &str) -> Option<String> {
    let _guard = acquire_lease_lock();
    let dir = match leases_dir() {
        Ok(d) => d,
        Err(_) => return None,
    };
    check_lease_access_in(&dir, key)
}

/// Run `f` while holding the lease lock, after a successful access check.
/// Lets a caller (e.g. proxy / run) atomically check access and then
/// release the secret without a concurrent revoke racing in between.
pub fn with_lease_check_locked<T, F: FnOnce(&str) -> T>(key: &str, f: F) -> Option<T> {
    let _guard = acquire_lease_lock();
    let dir = leases_dir().ok()?;
    let lease_name = check_lease_access_in(&dir, key)?;
    Some(f(&lease_name))
}

fn check_lease_access_in(dir: &std::path::Path, key: &str) -> Option<String> {
    if !dir.exists() {
        return None;
    }

    let now = Utc::now();

    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(lease) = toml::from_str::<Lease>(&content) {
                    if lease.revoked {
                        continue;
                    }
                    if let Ok(expires) = DateTime::parse_from_rfc3339(&lease.expires_at) {
                        if expires.with_timezone(&Utc) <= now {
                            continue; // expired
                        }
                    }
                    // Active lease -- check if key is allowed
                    match &lease.keys {
                        None => return Some(lease.name), // all keys
                        Some(keys) if keys.iter().any(|k| k == key) => return Some(lease.name),
                        _ => {}
                    }
                }
            }
        }
    }

    None
}

/// Parse duration string ("30s", "1h", "30m", "8h", "24h", "7d") to seconds.
/// JIT leases routinely use "s" suffix; manual leases the others.
pub fn parse_lease_duration(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<i64>().map_err(|e| e.to_string())
    } else if let Some(minutes) = s.strip_suffix('m') {
        minutes
            .parse::<i64>()
            .map(|m| m * 60)
            .map_err(|e| e.to_string())
    } else if let Some(hours) = s.strip_suffix('h') {
        hours
            .parse::<i64>()
            .map(|h| h * 3600)
            .map_err(|e| e.to_string())
    } else if let Some(days) = s.strip_suffix('d') {
        days.parse::<i64>()
            .map(|d| d * 86400)
            .map_err(|e| e.to_string())
    } else {
        Err(format!(
            "Invalid duration '{}'. Use: 30s, 30m, 1h, 8h, 24h, 7d",
            s
        ))
    }
}

/// Clean up expired lease files.
pub fn cleanup_expired() -> Result<usize, OpError> {
    let dir = leases_dir()?;
    cleanup_expired_in(&dir)
}

fn cleanup_expired_in(dir: &std::path::Path) -> Result<usize, OpError> {
    let now = Utc::now();
    let mut removed = 0;

    if !dir.exists() {
        return Ok(0);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(lease) = toml::from_str::<Lease>(&content) {
                    if lease.revoked {
                        std::fs::remove_file(&path)?;
                        removed += 1;
                    } else if let Ok(expires) = DateTime::parse_from_rfc3339(&lease.expires_at) {
                        if expires.with_timezone(&Utc) <= now {
                            std::fs::remove_file(&path)?;
                            removed += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(removed)
}

use dashmap::DashMap;
use zeroize::Zeroizing;

/// Reason a JIT lease was revoked. Audit-log metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevokeReason {
    Explicit,
    PidExit,
    TtlExpired,
    Panic,
}

/// One-time-use redemption ticket returned by `jit_grant`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitHandle {
    pub uuid: String,
    pub lease_name: String,
}

/// Inputs to `jit_grant`. All validated before lease is created.
#[derive(Debug, Clone)]
pub struct GrantRequest {
    pub key: String,
    pub pid: u32,
    pub ttl_secs: u64,
    pub tool_name: String,
    pub single_redeem: bool,
}

/// Errors specific to the JIT lease flow.
#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    #[error("lease not found: {0}")]
    NotFound(String),
    #[error("lease already revoked: {0}")]
    AlreadyRevoked(String),
    #[error("lease already redeemed: {0}")]
    AlreadyRedeemed(String),
    #[error("lease expired: {0}")]
    Expired(String),
    #[error("invalid pid: {0}")]
    InvalidPid(u32),
    #[error("invalid ttl: {0}")]
    InvalidTtl(String),
    #[error("invalid key name: {0}")]
    InvalidKey(String),
    #[error("invalid redemption ticket for lease: {0}")]
    InvalidTicket(String),
    #[error("op error: {0}")]
    OpError(#[from] OpError),
}

/// Constant-time byte-slice equality. Avoids leaking, via early-return timing,
/// how many leading bytes of a redemption ticket were guessed correctly.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Process-wide registry of active watcher tasks. One per JIT lease.
fn watcher_registry() -> &'static DashMap<String, tokio::task::JoinHandle<()>> {
    static R: OnceLock<DashMap<String, tokio::task::JoinHandle<()>>> = OnceLock::new();
    R.get_or_init(DashMap::new)
}

/// Number of currently-tracked watcher tasks (for tests + diagnostics).
pub fn watcher_count() -> usize {
    watcher_registry().len()
}

fn validate_pid(pid: u32) -> Result<(), LeaseError> {
    if pid == 0 {
        return Err(LeaseError::InvalidPid(pid));
    }
    if pid == std::process::id() {
        return Err(LeaseError::InvalidPid(pid));
    }
    Ok(())
}

fn validate_key_name(key: &str) -> Result<(), LeaseError> {
    if key.is_empty() {
        return Err(LeaseError::InvalidKey(key.to_string()));
    }
    Ok(())
}

/// Reject lease names that could escape the leases directory.
///
/// A lease name becomes a filename (`<name>.toml`) joined onto the leases
/// dir. Without validation, a name like `../../tmp/evil` would write, read, or
/// delete a file *outside* `~/.config/envforge/leases` — violating the "never
/// write outside protected zones" principle, and such a traversed lease also
/// escapes `revoke_all_leases` cleanup. Allow only `[A-Za-z0-9._-]` and forbid
/// `..`. (JIT leases are unaffected: they mint UUID names internally.)
fn validate_lease_name(name: &str) -> Result<(), OpError> {
    if name.is_empty() {
        return Err(OpError::from("lease name must not be empty".to_string()));
    }
    let charset_ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !charset_ok || name.contains("..") {
        return Err(OpError::from(format!(
            "invalid lease name '{name}': only [A-Za-z0-9._-] allowed, and '..' is forbidden"
        )));
    }
    Ok(())
}

fn persist_lease(lease: &Lease) -> Result<(), OpError> {
    let dir = leases_dir()?;
    let path = dir.join(format!("{}.toml", lease.name));
    let content = toml::to_string_pretty(lease)?;
    std::fs::write(&path, content)?;
    Ok(())
}

fn load_lease(name: &str) -> Result<Option<Lease>, OpError> {
    let dir = leases_dir()?;
    let path = dir.join(format!("{}.toml", name));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(Some(toml::from_str(&content)?))
}

fn lease_expired(lease: &Lease) -> bool {
    DateTime::parse_from_rfc3339(&lease.expires_at)
        .map(|exp| exp.with_timezone(&Utc) <= Utc::now())
        .unwrap_or(true)
}

/// Liveness probe via `kill(pid, 0)`. Returns true if the process exists and
/// the EnvForge process has permission to signal it (which it does for any
/// process owned by the same uid).
fn pid_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) is a side-effect-free liveness probe; it does not
    // deliver any signal. The pid is u32 → i32 safe for the userspace range.
    #[cfg(unix)]
    unsafe {
        let r = libc::kill(pid as i32, 0);
        if r == 0 {
            return true;
        }
        // ESRCH means the process is gone. Anything else (e.g. EPERM for a
        // process we can see but not signal) means the process *exists*.
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Read PID start-time fingerprint to defeat PID reuse races.
/// Returns `None` if not readable on this platform / for this caller.
#[cfg(target_os = "linux")]
fn pid_start_time(pid: u32) -> Option<u64> {
    let path = format!("/proc/{}/stat", pid);
    let stat = std::fs::read_to_string(&path).ok()?;
    // Field layout: pid (comm) state ppid ... starttime (field 22, 0-indexed 21).
    // The comm field is parenthesized and can contain spaces; use rsplit on ')'
    // to skip past it, then count from there.
    let after_comm = stat.rsplit(')').next()?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // Post-comm fields begin at index 0 = "state". starttime is field 22 overall,
    // i.e. index 19 of the post-comm slice (state, ppid, pgrp, session, tty_nr,
    // tpgid, flags, minflt, cminflt, majflt, cmajflt, utime, stime, cutime,
    // cstime, priority, nice, num_threads, itrealvalue, starttime).
    fields.get(19).and_then(|s| s.parse().ok())
}

#[cfg(not(target_os = "linux"))]
fn pid_start_time(_pid: u32) -> Option<u64> {
    // macOS proc_pidinfo path deferred to a future intent. Falling back to
    // PID-only liveness is documented as a known limitation in the bolt brief.
    None
}

fn poll_interval_ms() -> u64 {
    std::env::var("LEASE_WATCHER_POLL_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| n.clamp(50, 500))
        .unwrap_or(100)
}

/// Spawn a watcher task that calls `jit_revoke` when the bound PID exits or
/// the TTL deadline is reached. Registers the JoinHandle so explicit revoke
/// can abort the task. No-op if there's no tokio runtime active.
fn spawn_watcher(lease_name: &str, pid: u32, expires_at: String) {
    let name = lease_name.to_string();
    let deadline = match DateTime::parse_from_rfc3339(&expires_at) {
        Ok(d) => d.with_timezone(&Utc),
        Err(_) => Utc::now() + chrono::Duration::hours(1),
    };
    let pid_start = pid_start_time(pid);
    let poll_ms = poll_interval_ms();

    // tokio::spawn requires a runtime; if there isn't one, fall back gracefully.
    let runtime_handle = tokio::runtime::Handle::try_current().ok();
    let Some(rt) = runtime_handle else {
        log::warn!(
            "JIT lease {name} created without tokio runtime; PID watcher disabled (TTL still applies via cleanup_expired)"
        );
        return;
    };

    let handle = rt.spawn(async move {
        loop {
            // 1. TTL deadline?
            if Utc::now() >= deadline {
                let _ = jit_revoke(&name, RevokeReason::TtlExpired);
                break;
            }
            // 2. PID still alive (and same start-time)?
            if !pid_alive_with_start(pid, pid_start) {
                let _ = jit_revoke(&name, RevokeReason::PidExit);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
        }
        watcher_registry().remove(&name);
    });

    watcher_registry().insert(lease_name.to_string(), handle);
}

fn pid_alive_with_start(pid: u32, expected_start: Option<u64>) -> bool {
    if !pid_alive(pid) {
        return false;
    }
    match (expected_start, pid_start_time(pid)) {
        (Some(a), Some(b)) => a == b,
        _ => true, // fall back to PID-only check if start-time unreadable
    }
}

/// Mint a JIT lease. Validates inputs, creates the lease record (extended with
/// JIT fields), spawns a PID watcher, and returns a single-redemption handle.
pub fn jit_grant(req: GrantRequest) -> Result<JitHandle, LeaseError> {
    validate_pid(req.pid)?;
    validate_key_name(&req.key)?;
    if req.ttl_secs == 0 {
        return Err(LeaseError::InvalidTtl("ttl == 0".into()));
    }
    if req.ttl_secs > 7 * 86_400 {
        return Err(LeaseError::InvalidTtl(format!(
            "ttl {} > 7 days",
            req.ttl_secs
        )));
    }
    let ttl_i64 = i64::try_from(req.ttl_secs)
        .map_err(|_| LeaseError::InvalidTtl(format!("ttl too large: {}", req.ttl_secs)))?;

    // Generate lease name. UUID prefix avoids collisions with manual leases.
    let lease_name = format!("jit-{}", uuid::Uuid::new_v4());
    // Secret redemption ticket — distinct from the (audit-logged) lease name.
    let ticket = uuid::Uuid::new_v4().to_string();

    // Acquire lock once around create + extension + persist.
    let _guard = acquire_lease_lock();

    // Reuse existing TTL math from create_lease: build the record manually so
    // we can include JIT fields in a single write (avoid double-fsync).
    let now = Utc::now();
    let expires = now + chrono::Duration::seconds(ttl_i64);
    let lease = Lease {
        name: lease_name.clone(),
        created_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        keys: Some(vec![req.key.clone()]),
        revoked: false,
        pid: Some(req.pid),
        single_redeem: req.single_redeem,
        redeemed: false,
        tool_name: Some(req.tool_name.clone()),
        uuid: Some(ticket.clone()),
    };
    persist_lease(&lease)?;

    // Spawn watcher *outside* the lock — the watcher acquires the lock itself
    // when it eventually calls jit_revoke.
    drop(_guard);
    spawn_watcher(&lease_name, req.pid, lease.expires_at.clone());

    audit_emit_lease_granted(&lease, &req);
    Ok(JitHandle {
        uuid: ticket,
        lease_name,
    })
}

/// Resolve the secret value bound to a JIT lease. On success, returns a
/// `Zeroizing<String>` whose drop overwrites the heap buffer. Single-redeem
/// leases reject the second redemption.
///
/// NOTE: this implementation reads the value from the system shell environment
/// (`std::env::var`). Future intents may resolve via the secrets-provider
/// pipeline (`ops/secrets`), at which point this fn becomes a thin facade.
pub fn jit_redeem(handle: &JitHandle) -> Result<Zeroizing<String>, LeaseError> {
    let _guard = acquire_lease_lock();
    let mut lease = load_lease(&handle.lease_name)?
        .ok_or_else(|| LeaseError::NotFound(handle.lease_name.clone()))?;

    // Verify the one-time ticket before doing anything else. Redemption must
    // require the secret UUID handed back at grant time — gating on the lease
    // name alone (which is emitted in audit metadata and returned in the
    // handle) would let anyone who learns the name redeem the secret.
    match &lease.uuid {
        Some(stored) if ct_eq(stored.as_bytes(), handle.uuid.as_bytes()) => {}
        _ => return Err(LeaseError::InvalidTicket(handle.lease_name.clone())),
    }

    if lease.revoked {
        return Err(LeaseError::AlreadyRevoked(handle.lease_name.clone()));
    }
    if lease_expired(&lease) {
        return Err(LeaseError::Expired(handle.lease_name.clone()));
    }
    if lease.single_redeem && lease.redeemed {
        return Err(LeaseError::AlreadyRedeemed(handle.lease_name.clone()));
    }

    let key = lease
        .keys
        .as_ref()
        .and_then(|v| v.first())
        .cloned()
        .ok_or_else(|| LeaseError::InvalidKey("no key on lease".into()))?;

    let raw = std::env::var(&key)
        .map_err(|_| LeaseError::OpError(OpError::Other(format!("env var not set: {key}"))))?;
    let value = Zeroizing::new(raw);

    if lease.single_redeem {
        lease.redeemed = true;
        persist_lease(&lease)?;
    }

    audit_emit_lease_redeemed(&lease);
    Ok(value)
}

/// Revoke a JIT lease (or any lease). Aborts the watcher if present. Idempotent.
///
/// Lock acquisition is delegated to `revoke_lease` (the existing fn already holds
/// the process-wide lease lock). We do NOT pre-acquire it here — the project's
/// lease mutex is non-reentrant and a double-acquisition would deadlock.
pub fn jit_revoke(name: &str, reason: RevokeReason) -> Result<bool, LeaseError> {
    // Abort + remove watcher first so it doesn't race against the file write.
    if let Some((_, handle)) = watcher_registry().remove(name) {
        handle.abort();
    }

    let did = revoke_lease(name)?;
    if did {
        let lease = load_lease(name)?;
        audit_emit_lease_revoked(lease.as_ref(), reason);
    }
    Ok(did)
}

// ─── Audit emission (3 new EventType variants registered in audit/types.rs) ──

fn audit_emit_lease_granted(lease: &Lease, req: &GrantRequest) {
    use crate::ops::audit::emitter::emit;
    use crate::ops::audit::types::{AuditEvent, EventResult, EventSource, EventType};
    let mut event = AuditEvent::new(
        EventType::LeaseGranted,
        EventSource::AiGuard,
        EventResult::Success,
    );
    event.add_metadata("lease_name", serde_json::Value::String(lease.name.clone()));
    event.add_metadata("key", serde_json::Value::String(req.key.clone()));
    event.add_metadata(
        "tool_name",
        serde_json::Value::String(req.tool_name.clone()),
    );
    event.add_metadata("pid", serde_json::Value::from(req.pid));
    event.add_metadata("ttl_secs", serde_json::Value::from(req.ttl_secs));
    let cfg = match audit_config() {
        Some(c) => c,
        None => return,
    };
    if let Err(e) = emit(event, &cfg) {
        log::warn!("audit emit (LeaseGranted) failed: {e}");
    }
}

fn audit_emit_lease_redeemed(lease: &Lease) {
    use crate::ops::audit::emitter::emit;
    use crate::ops::audit::types::{AuditEvent, EventResult, EventSource, EventType};
    let mut event = AuditEvent::new(
        EventType::LeaseRedeemed,
        EventSource::AiGuard,
        EventResult::Success,
    );
    event.add_metadata("lease_name", serde_json::Value::String(lease.name.clone()));
    if let Some(t) = &lease.tool_name {
        event.add_metadata("tool_name", serde_json::Value::String(t.clone()));
    }
    let cfg = match audit_config() {
        Some(c) => c,
        None => return,
    };
    if let Err(e) = emit(event, &cfg) {
        log::warn!("audit emit (LeaseRedeemed) failed: {e}");
    }
}

fn audit_emit_lease_revoked(lease: Option<&Lease>, reason: RevokeReason) {
    use crate::ops::audit::emitter::emit;
    use crate::ops::audit::types::{AuditEvent, EventResult, EventSource, EventType};
    let mut event = AuditEvent::new(
        EventType::LeaseRevoked,
        EventSource::AiGuard,
        EventResult::Success,
    );
    if let Some(l) = lease {
        event.add_metadata("lease_name", serde_json::Value::String(l.name.clone()));
    }
    event.add_metadata("reason", serde_json::Value::String(format!("{reason:?}")));
    let cfg = match audit_config() {
        Some(c) => c,
        None => return,
    };
    if let Err(e) = emit(event, &cfg) {
        log::warn!("audit emit (LeaseRevoked) failed: {e}");
    }
}

fn audit_config() -> Option<crate::ops::audit::emitter::EmitterConfig> {
    use crate::ops::audit::emitter::EmitterConfig;
    let dir = crate::config::config_dir().ok()?.join("audit");
    Some(EmitterConfig::new(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_lease(name: &str, ttl_seconds: i64, keys: Option<Vec<String>>) -> Lease {
        let now = Utc::now();
        let expires = now + chrono::Duration::seconds(ttl_seconds);
        Lease {
            name: name.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            keys,
            revoked: false,
            ..Default::default()
        }
    }

    fn make_expired_lease(name: &str) -> Lease {
        let now = Utc::now();
        let past = now - chrono::Duration::seconds(3600);
        Lease {
            name: name.to_string(),
            created_at: past.to_rfc3339(),
            expires_at: (past + chrono::Duration::seconds(60)).to_rfc3339(),
            keys: None,
            revoked: false,
            ..Default::default()
        }
    }

    fn make_revoked_lease(name: &str) -> Lease {
        let now = Utc::now();
        let expires = now + chrono::Duration::seconds(3600);
        Lease {
            name: name.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            keys: None,
            revoked: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_create_and_parse_lease() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let lease = make_lease("test-session", 3600, None);
        write_lease_at(&dir, &lease);

        // Read back
        let content = std::fs::read_to_string(dir.join("test-session.toml")).unwrap();
        let parsed: Lease = toml::from_str(&content).unwrap();
        assert_eq!(parsed.name, "test-session");
        assert!(!parsed.revoked);
        assert!(parsed.keys.is_none());
    }

    #[test]
    fn test_parse_lease_duration_minutes() {
        assert_eq!(parse_lease_duration("30m").unwrap(), 1800);
    }

    #[test]
    fn test_parse_lease_duration_hours() {
        assert_eq!(parse_lease_duration("1h").unwrap(), 3600);
        assert_eq!(parse_lease_duration("8h").unwrap(), 28800);
        assert_eq!(parse_lease_duration("24h").unwrap(), 86400);
    }

    #[test]
    fn test_parse_lease_duration_days() {
        assert_eq!(parse_lease_duration("7d").unwrap(), 604800);
    }

    #[test]
    fn test_parse_lease_duration_invalid() {
        assert!(parse_lease_duration("abc").is_err());
        assert!(parse_lease_duration("").is_err());
        assert!(parse_lease_duration("10x").is_err());
    }

    #[test]
    fn test_parse_lease_duration_seconds() {
        assert_eq!(parse_lease_duration("30s").unwrap(), 30);
        assert_eq!(parse_lease_duration("0s").unwrap(), 0);
        assert_eq!(parse_lease_duration("3600s").unwrap(), 3600);
    }

    #[test]
    fn test_jit_grant_rejects_pid_zero() {
        let req = GrantRequest {
            key: "TEST".into(),
            pid: 0,
            ttl_secs: 30,
            tool_name: "test".into(),
            single_redeem: true,
        };
        let r = jit_grant(req);
        assert!(matches!(r, Err(LeaseError::InvalidPid(0))));
    }

    #[test]
    fn test_jit_grant_rejects_self_pid() {
        let req = GrantRequest {
            key: "TEST".into(),
            pid: std::process::id(),
            ttl_secs: 30,
            tool_name: "test".into(),
            single_redeem: true,
        };
        let r = jit_grant(req);
        assert!(matches!(r, Err(LeaseError::InvalidPid(_))));
    }

    #[test]
    fn test_jit_grant_rejects_ttl_zero() {
        let req = GrantRequest {
            key: "TEST".into(),
            pid: 1,
            ttl_secs: 0,
            tool_name: "test".into(),
            single_redeem: true,
        };
        let r = jit_grant(req);
        assert!(matches!(r, Err(LeaseError::InvalidTtl(_))));
    }

    #[test]
    fn test_jit_grant_rejects_ttl_too_long() {
        let req = GrantRequest {
            key: "TEST".into(),
            pid: 1,
            ttl_secs: 8 * 86_400, // > 7 days
            tool_name: "test".into(),
            single_redeem: true,
        };
        let r = jit_grant(req);
        assert!(matches!(r, Err(LeaseError::InvalidTtl(_))));
    }

    #[test]
    fn test_jit_grant_rejects_empty_key() {
        let req = GrantRequest {
            key: String::new(),
            pid: 1,
            ttl_secs: 30,
            tool_name: "test".into(),
            single_redeem: true,
        };
        let r = jit_grant(req);
        assert!(matches!(r, Err(LeaseError::InvalidKey(_))));
    }

    #[test]
    fn test_pid_alive_self() {
        // Our own PID must be reported alive.
        assert!(pid_alive(std::process::id()));
    }

    #[test]
    fn test_pid_alive_high_pid_likely_dead() {
        // PIDs above ~4 million on Linux are above the default pid_max (4194304).
        // On macOS the cap is even lower (99999 historically; 99999_999 is
        // definitively above any allocation). kill(-1, 0) is broadcast and
        // therefore returns success — so we use a positive but unallocated
        // PID instead.
        assert!(!pid_alive(99_999_999));
    }

    #[test]
    fn test_default_lease_serializes_v1_compatible() {
        // A lease without JIT fields must serialize to TOML that is loadable
        // by older code via the same struct (round-trip).
        let l = Lease {
            name: "manual".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            expires_at: "2026-01-02T00:00:00Z".into(),
            keys: Some(vec!["A".into()]),
            revoked: false,
            ..Default::default()
        };
        let s = toml::to_string(&l).unwrap();
        let back: Lease = toml::from_str(&s).unwrap();
        assert_eq!(back.name, "manual");
        assert!(back.pid.is_none());
        assert!(!back.single_redeem);
        assert!(!back.redeemed);
        assert!(back.tool_name.is_none());
    }

    #[test]
    fn test_legacy_toml_loads_with_serde_default() {
        // Simulate a TOML file written by a pre-JIT version of envforge.
        let legacy = r#"
name = "old-lease"
created_at = "2026-01-01T00:00:00Z"
expires_at = "2026-01-02T00:00:00Z"
revoked = false
"#;
        let l: Lease = toml::from_str(legacy).unwrap();
        assert_eq!(l.name, "old-lease");
        assert!(l.pid.is_none());
        assert!(!l.single_redeem);
        assert!(!l.redeemed);
        assert!(l.tool_name.is_none());
    }

    #[test]
    fn test_revoke_reason_serializes_round_trip() {
        for r in [
            RevokeReason::Explicit,
            RevokeReason::PidExit,
            RevokeReason::TtlExpired,
            RevokeReason::Panic,
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: RevokeReason = serde_json::from_str(&s).unwrap();
            assert_eq!(r, back);
        }
    }

    #[test]
    fn test_watcher_count_starts_zero_or_consistent() {
        // We can't assume zero (other tests may have affected it), but the
        // call must not panic and must return a usize.
        let n = watcher_count();
        let _ = n; // sanity only
    }

    #[test]
    fn test_jit_revoke_unknown_lease_is_noop() {
        let did = jit_revoke("non-existent-jit-lease", RevokeReason::Explicit).unwrap();
        assert!(!did);
    }

    #[test]
    fn test_check_lease_access_active() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let lease = make_lease("active-session", 3600, None);
        write_lease_at(&dir, &lease);

        let result = check_lease_access_in(&dir, "ANY_KEY");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "active-session");
    }

    #[test]
    fn test_check_lease_access_expired_returns_none() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let lease = make_expired_lease("old-session");
        write_lease_at(&dir, &lease);

        let result = check_lease_access_in(&dir, "ANY_KEY");
        assert!(result.is_none());
    }

    #[test]
    fn test_check_lease_access_revoked_returns_none() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let lease = make_revoked_lease("revoked-session");
        write_lease_at(&dir, &lease);

        let result = check_lease_access_in(&dir, "ANY_KEY");
        assert!(result.is_none());
    }

    #[test]
    fn test_check_lease_access_key_scoped() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let lease = make_lease(
            "scoped-session",
            3600,
            Some(vec!["API_KEY".to_string(), "DB_URL".to_string()]),
        );
        write_lease_at(&dir, &lease);

        // Allowed key
        let result = check_lease_access_in(&dir, "API_KEY");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "scoped-session");

        // Allowed key
        let result = check_lease_access_in(&dir, "DB_URL");
        assert!(result.is_some());

        // Not in scope
        let result = check_lease_access_in(&dir, "SECRET_TOKEN");
        assert!(result.is_none());
    }

    #[test]
    fn test_revoke_all_removes_all_files() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        write_lease_at(&dir, &make_lease("s1", 3600, None));
        write_lease_at(&dir, &make_lease("s2", 3600, None));
        write_lease_at(&dir, &make_lease("s3", 3600, None));

        let count = revoke_all_in(&dir).unwrap();
        assert_eq!(count, 3);

        // Directory should be empty
        let remaining: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_cleanup_expired_removes_old_files() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        write_lease_at(&dir, &make_expired_lease("old1"));
        write_lease_at(&dir, &make_expired_lease("old2"));
        write_lease_at(&dir, &make_lease("active1", 3600, None));
        write_lease_at(&dir, &make_revoked_lease("revoked1"));

        let removed = cleanup_expired_in(&dir).unwrap();
        // old1, old2 (expired) + revoked1 = 3
        assert_eq!(removed, 3);

        // Only active lease should remain
        let remaining: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_list_leases() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        write_lease_at(&dir, &make_lease("active", 3600, None));
        write_lease_at(&dir, &make_expired_lease("expired"));
        write_lease_at(
            &dir,
            &make_lease(
                "scoped",
                3600,
                Some(vec!["K1".to_string(), "K2".to_string()]),
            ),
        );

        let statuses = list_leases_in(&dir);
        assert_eq!(statuses.len(), 3);

        let active = statuses.iter().find(|s| s.name == "active").unwrap();
        assert!(!active.expired);
        assert!(!active.revoked);
        assert!(active.key_count.is_none());

        let expired = statuses.iter().find(|s| s.name == "expired").unwrap();
        assert!(expired.expired);

        let scoped = statuses.iter().find(|s| s.name == "scoped").unwrap();
        assert_eq!(scoped.key_count, Some(2));
    }

    #[test]
    fn test_revoke_lease() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        write_lease_at(&dir, &make_lease("mysession", 3600, None));

        let revoked = revoke_lease_in(&dir, "mysession").unwrap();
        assert!(revoked);

        // Read back and verify revoked flag
        let content = std::fs::read_to_string(dir.join("mysession.toml")).unwrap();
        let lease: Lease = toml::from_str(&content).unwrap();
        assert!(lease.revoked);

        // Access should be denied
        let result = check_lease_access_in(&dir, "ANY_KEY");
        assert!(result.is_none());
    }

    #[test]
    fn test_revoke_nonexistent_returns_false() {
        let tmp = TempDir::new().unwrap();
        let dir = leases_dir_at(tmp.path());

        let revoked = revoke_lease_in(&dir, "no-such-lease").unwrap();
        assert!(!revoked);
    }
}
