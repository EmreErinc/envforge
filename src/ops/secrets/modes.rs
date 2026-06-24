#![allow(deprecated)]
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use zeroize::Zeroize;

use super::cache::{is_reference, resolve_reference, SecretRef};
use super::credentials::{clear_all_credentials, read_all_credentials};
use super::provider::{ProviderRegistry, SecretsError};

/// Result of a pull operation.
#[derive(Debug, Clone)]
pub struct PullResult {
    pub provider: String,
    pub keys_new: Vec<String>,
    pub keys_updated: Vec<String>,
    pub keys_skipped: Vec<String>,
    pub total: usize,
}

/// Result of a push operation.
#[derive(Debug, Clone)]
pub struct PushResult {
    pub provider: String,
    pub keys_pushed: usize,
}

/// Source tracking: which provider a key was pulled from.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct KeySource {
    pub key: String,
    pub provider: String,
    pub path: String,
    pub pulled_at: String,
}

/// Full result of a pull operation: (entries, result, sources).
pub type PullOutput = (Vec<(String, String)>, PullResult, Vec<KeySource>);

/// Pull secrets from a provider.
pub fn pull_secrets(
    registry: &ProviderRegistry,
    provider_name: &str,
    path: &str,
    filter: Option<&str>,
    existing_keys: &HashMap<String, String>,
) -> Result<PullOutput, SecretsError> {
    let provider = registry.get(provider_name)?;
    let mut credentials = read_all_credentials(provider_name)?;
    provider.authenticate(&credentials)?;

    let mut secrets = provider.pull(&credentials, path)?;

    if let Some(pattern) = filter {
        secrets.retain(|(k, _)| glob_match(pattern, k));
    }

    let total = secrets.len();
    if total > 50 {
        eprintln!("Fetching {} secrets from {}...", total, provider_name);
    }

    let mut new_keys = Vec::new();
    let mut updated_keys = Vec::new();
    let mut skipped_keys = Vec::new();
    let mut result_entries = Vec::new();
    let mut sources = Vec::new();
    let now = chrono::Utc::now().to_rfc3339();

    for (key, value) in &secrets {
        sources.push(KeySource {
            key: key.clone(),
            provider: provider_name.to_string(),
            path: path.to_string(),
            pulled_at: now.clone(),
        });

        if let Some(existing_val) = existing_keys.get(key) {
            if existing_val == value {
                skipped_keys.push(key.clone());
            } else {
                updated_keys.push(key.clone());
                result_entries.push((key.clone(), value.clone()));
            }
        } else {
            new_keys.push(key.clone());
            result_entries.push((key.clone(), value.clone()));
        }
    }

    // Zeroize credentials from memory before returning
    for v in credentials.values_mut() {
        v.zeroize();
    }
    credentials.clear();

    let pull_result = PullResult {
        provider: provider_name.to_string(),
        keys_new: new_keys,
        keys_updated: updated_keys,
        keys_skipped: skipped_keys,
        total,
    };

    Ok((result_entries, pull_result, sources))
}

/// Push secrets to a provider.
pub fn push_secrets(
    registry: &ProviderRegistry,
    provider_name: &str,
    path: &str,
    secrets: &[(String, String)],
    filter: Option<&str>,
) -> Result<PushResult, SecretsError> {
    let provider = registry.get(provider_name)?;
    let mut credentials = read_all_credentials(provider_name)?;
    provider.authenticate(&credentials)?;

    let filtered: Vec<(String, String)> = if let Some(pattern) = filter {
        secrets
            .iter()
            .filter(|(k, _)| glob_match(pattern, k))
            .cloned()
            .collect()
    } else {
        secrets.to_vec()
    };

    if filtered.is_empty() {
        // Zeroize before early return
        for v in credentials.values_mut() {
            v.zeroize();
        }
        credentials.clear();
        return Err(SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: "no keys to push (check --keys or --filter)".to_string(),
        });
    }

    if filtered.len() > 50 {
        eprintln!("Pushing {} secrets to {}...", filtered.len(), provider_name);
    }

    let count = provider.push(&credentials, path, &filtered)?;

    // Zeroize credentials from memory before returning
    for v in credentials.values_mut() {
        v.zeroize();
    }
    credentials.clear();

    Ok(PushResult {
        provider: provider_name.to_string(),
        keys_pushed: count,
    })
}

/// Resolve all secret references in a set of entries.
pub fn resolve_all_references(
    entries: &[(String, String)],
    registry: &ProviderRegistry,
) -> Result<Vec<(String, String, bool)>, SecretsError> {
    let mut results = Vec::new();

    for (key, value) in entries {
        if is_reference(value) {
            if let Some(secret_ref) = SecretRef::parse(value) {
                let provider = registry.get(&secret_ref.provider)?;
                let mut credentials = read_all_credentials(&secret_ref.provider)?;
                let resolved = resolve_reference(&secret_ref, provider, &credentials)?;
                for v in credentials.values_mut() {
                    v.zeroize();
                }
                credentials.clear();
                results.push((key.clone(), resolved, true));
            } else {
                results.push((key.clone(), value.clone(), false));
            }
        } else {
            results.push((key.clone(), value.clone(), false));
        }
    }

    Ok(results)
}

/// Simple glob matching (supports `*` and `?`).
///
/// Iterative `O(P × T)` DP. Replaces a recursive backtracker that was
/// exponential on adversarial inputs (Q2 fix in 0.7.5 fifth rescan).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    let (m, n) = (pat.len(), txt.len());
    let mut prev = vec![false; n + 1];
    let mut curr = vec![false; n + 1];
    prev[0] = true;
    for i in 1..=m {
        curr[0] = pat[i - 1] == '*' && prev[0];
        for j in 1..=n {
            curr[j] = match pat[i - 1] {
                '*' => prev[j] || curr[j - 1],
                '?' => prev[j - 1],
                p => p == txt[j - 1] && prev[j - 1],
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(false);
    }
    prev[n]
}

// ─── Volatile Mode ───────────────────────────────────────────

/// Volatile (auto-expiring) credential mode configuration.
///
/// Replaces the old `enabled: bool` + `ttl_seconds: u64` pair, which
/// could silently disable volatile mode via a forgotten boolean.
#[derive(Debug, Clone)]
pub enum VolatileMode {
    /// Volatile mode is disabled — credentials persist indefinitely
    /// until explicitly cleared.  This is the default for compatibility.
    Off,

    /// Volatile mode enabled — credentials auto-expire after the given
    /// TTL.  On expiry, all in-memory credentials are zeroized and the
    /// session is marked as expired.
    On {
        /// TTL in seconds (e.g. 300 = 5 minutes).
        ttl_seconds: u64,
    },

    /// Strict volatile mode — like `On`, but additionally requires
    /// re-authentication before credentials can be re-loaded after
    /// expiry.  This prevents the "re-pull without re-auth" bypass.
    Strict {
        ttl_seconds: u64,
        /// If true, the user must re-authenticate to the provider
        /// after a volatile expiry before new credentials can be pulled.
        reauth: bool,
    },
}

impl VolatileMode {
    /// `true` when volatile mode is active (on or strict).
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }

    /// The TTL in seconds, or zero when off.
    pub fn ttl_seconds(&self) -> u64 {
        match self {
            Self::Off => 0,
            Self::On { ttl_seconds } | Self::Strict { ttl_seconds, .. } => *ttl_seconds,
        }
    }

    /// `true` when re-authentication is required after volatile expiry.
    pub fn requires_reauth(&self) -> bool {
        matches!(self, Self::Strict { reauth: true, .. })
    }
}

impl Default for VolatileMode {
    fn default() -> Self {
        Self::On {
            ttl_seconds: 300, // 5 minutes default — secure by default
        }
    }
}

// ─── Backward-compat type alias ────────────────────────────
/// Kept for external callers that still reference `VolatileConfig`.
/// New code should use [`VolatileMode`] directly.
#[deprecated(since = "0.8.0", note = "Use VolatileMode enum instead")]
pub type VolatileConfig = VolatileMode;

/// Track when credentials were last loaded into memory.
static LAST_LOAD: Mutex<Option<SystemTime>> = Mutex::new(None);

/// Track whether volatile expiry has occurred.
static VOLATILE_EXPIRED: AtomicBool = AtomicBool::new(false);

/// Mark credentials as loaded into memory (called before credential operations).
pub fn mark_credentials_loaded() {
    let mut last = LAST_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *last = Some(SystemTime::now());
    VOLATILE_EXPIRED.store(false, Ordering::SeqCst);
}

/// Check if credentials have expired per volatile mode TTL.
/// Returns true if TTL has elapsed and credentials should be cleared.
pub fn check_volatile_expiry(mode: &VolatileMode) -> bool {
    if !mode.is_enabled() {
        return false;
    }
    if VOLATILE_EXPIRED.load(Ordering::SeqCst) {
        return true;
    }
    let last = LAST_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(ts) = *last {
        if let Ok(elapsed) = ts.elapsed() {
            if elapsed >= Duration::from_secs(mode.ttl_seconds()) {
                VOLATILE_EXPIRED.store(true, Ordering::SeqCst);
                return true;
            }
        }
    }
    false
}

/// Remaining time before volatile session expires, if active.
/// Returns `None` when volatile mode is off or no session is active.
pub fn volatile_remaining(mode: &VolatileMode) -> Option<Duration> {
    if !mode.is_enabled() {
        return None;
    }
    let ttl = Duration::from_secs(mode.ttl_seconds());
    let last = LAST_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ts = (*last)?;
    let elapsed = ts.elapsed().ok()?;
    if elapsed >= ttl {
        Some(Duration::ZERO)
    } else {
        Some(ttl.checked_sub(elapsed).unwrap_or(Duration::ZERO))
    }
}

/// Clear all in-memory credentials and mark session as expired.
/// Called when volatile TTL elapses or user requests explicit clear.
pub fn expire_volatile_session() {
    clear_all_credentials();
    VOLATILE_EXPIRED.store(true, Ordering::SeqCst);
}

/// Clear the entire session: zeroize credentials from memory.
/// Safe to call at any time. No credentials survive this call in memory.
pub fn clear_session() {
    clear_all_credentials();
    let mut last = LAST_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *last = None;
    VOLATILE_EXPIRED.store(false, Ordering::SeqCst);
}

/// Execute a credential operation with volatile expiry guard.
/// If volatile mode is active and TTL has elapsed, returns AuthExpired error.
/// Otherwise, marks load time and executes the operation.
pub fn with_volatile_guard<T>(
    mode: &VolatileMode,
    provider_name: &str,
    f: impl FnOnce() -> Result<T, SecretsError>,
) -> Result<T, SecretsError> {
    if check_volatile_expiry(mode) {
        let ttl_msg = match mode {
            VolatileMode::On { ttl_seconds } | VolatileMode::Strict { ttl_seconds, .. } => {
                format!(" (TTL: {}s)", ttl_seconds)
            }
            VolatileMode::Off => String::new(),
        };
        return Err(SecretsError::AuthFailed {
            provider: provider_name.to_string(),
            message: format!(
                "volatile session expired{}. Re-authenticate to continue.",
                ttl_msg
            ),
        });
    }
    mark_credentials_loaded();
    f()
}

/// Zeroize all strings in a mutable secrets vector, then clear it.
pub fn zeroize_secrets(secrets: &mut Vec<(String, String)>) {
    for (key, value) in secrets.iter_mut() {
        key.zeroize();
        value.zeroize();
    }
    secrets.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_filter() {
        assert!(glob_match("DB_*", "DB_HOST"));
        assert!(glob_match("DB_*", "DB_PORT"));
        assert!(!glob_match("DB_*", "API_KEY"));
    }
}
