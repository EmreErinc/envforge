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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub name: String,
    pub created_at: String,
    pub expires_at: String,
    pub keys: Option<Vec<String>>, // None = all keys
    pub revoked: bool,
}

#[derive(Debug, Clone)]
pub struct LeaseStatus {
    pub name: String,
    pub expires_at: String,
    pub remaining_seconds: i64,
    pub expired: bool,
    pub revoked: bool,
    pub key_count: Option<usize>,
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
    let _guard = acquire_lease_lock();
    let now = Utc::now();
    let expires = now + chrono::Duration::seconds(ttl_seconds);

    let lease = Lease {
        name: name.to_string(),
        created_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        keys,
        revoked: false,
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
    let _guard = acquire_lease_lock();
    let dir = leases_dir()?;
    revoke_lease_in(&dir, name)
}

/// Renew (extend) an existing, non-revoked lease by `ttl_seconds`.
/// Returns the updated lease, or `None` if the lease does not exist.
/// Errors if the lease has already been revoked or has already expired
/// (callers should create a new lease in that case).
pub fn renew_lease(name: &str, ttl_seconds: i64) -> Result<Option<Lease>, OpError> {
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

/// Parse duration string ("1h", "30m", "8h", "24h", "7d") to seconds.
pub fn parse_lease_duration(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if let Some(minutes) = s.strip_suffix('m') {
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
            "Invalid duration '{}'. Use: 30m, 1h, 8h, 24h, 7d",
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

// ─── Tests ─────────────────────────────────────────────────

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
        assert!(parse_lease_duration("10s").is_err());
        assert!(parse_lease_duration("").is_err());
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
