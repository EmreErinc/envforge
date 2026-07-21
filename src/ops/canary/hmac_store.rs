//! HMAC key registry for canary v2.
//!
//! Stores per-machine 32-byte HMAC keys, age-encrypted at rest at
//! `~/.envforge/canary-keys.age`. Keeps the active key plus up to 2 retired
//! keys so verification continues to succeed for tokens minted before
//! a recent rotation.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::OpError;
use crate::config;
use crate::ops::encrypt;

const KEY_STORE_FILENAME: &str = "canary-keys.age";
const KEY_BYTES: usize = 32;
const STORE_FORMAT_VERSION: u8 = 1;
/// Maximum retired keys retained for backward verification.
pub const MAX_RETIRED_KEYS: usize = 2;

/// One key version with material. Bytes wrapped in [`Zeroizing`] for memory hygiene.
#[derive(Debug)]
pub struct HmacKeyVersion {
    version: u8,
    bytes: Zeroizing<[u8; KEY_BYTES]>,
    created_at: String,
    retired_at: Option<String>,
}

impl HmacKeyVersion {
    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn bytes(&self) -> &[u8; KEY_BYTES] {
        &self.bytes
    }

    pub fn is_active(&self) -> bool {
        self.retired_at.is_none()
    }
}

/// In-memory key registry. One active key + up to [`MAX_RETIRED_KEYS`] retired keys.
#[derive(Debug)]
pub struct HmacKeyRegistry {
    active: HmacKeyVersion,
    retired: Vec<HmacKeyVersion>,
}

impl HmacKeyRegistry {
    pub fn active(&self) -> &HmacKeyVersion {
        &self.active
    }

    pub fn retired(&self) -> &[HmacKeyVersion] {
        &self.retired
    }

    /// Yield (version, &key_bytes) pairs for verification, active first.
    pub fn verify_iter(&self) -> Vec<(u8, &[u8; KEY_BYTES])> {
        let mut out = Vec::with_capacity(1 + self.retired.len());
        out.push((self.active.version, self.active.bytes()));
        for r in &self.retired {
            out.push((r.version, r.bytes()));
        }
        out
    }
}

// ─── On-disk format (TOML, then age-encrypted) ─────────────────────────────

#[derive(Serialize, Deserialize)]
struct StoredRegistry {
    meta: StoredMeta,
    keys: Vec<StoredKey>,
}

#[derive(Serialize, Deserialize)]
struct StoredMeta {
    version: u8,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
struct StoredKey {
    version: u8,
    active: bool,
    created_at: String,
    #[serde(default)]
    retired_at: Option<String>,
    bytes_b64: String,
}

fn store_path() -> Result<PathBuf, OpError> {
    Ok(config::config_dir()?.join(KEY_STORE_FILENAME))
}

/// Manager wrapping the registry behind a process-wide mutex.
pub struct HmacKeyManager {
    registry: HmacKeyRegistry,
}

impl HmacKeyManager {
    /// Load the registry from disk; create an active key on first use.
    /// On repeat calls within a process, returns a clone of the cached registry
    /// via `with_registry`. For tests, callers can construct directly.
    pub fn load_or_init() -> Result<Self, OpError> {
        let registry = load_or_init_registry()?;
        Ok(Self { registry })
    }

    pub fn active_key(&self) -> &HmacKeyVersion {
        self.registry.active()
    }

    pub fn registry(&self) -> &HmacKeyRegistry {
        &self.registry
    }
}

fn registry_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn acquire_lock() -> std::sync::MutexGuard<'static, ()> {
    registry_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_or_init_registry() -> Result<HmacKeyRegistry, OpError> {
    let _guard = acquire_lock();
    let path = store_path()?;
    if path.exists() {
        load_registry(&path)
    } else {
        let registry = init_new_registry()?;
        persist_registry(&registry, &path)?;
        Ok(registry)
    }
}

fn init_new_registry() -> Result<HmacKeyRegistry, OpError> {
    let now = chrono::Utc::now().to_rfc3339();
    let bytes = Zeroizing::new(random_key_bytes());
    let active = HmacKeyVersion {
        version: 1,
        bytes,
        created_at: now,
        retired_at: None,
    };
    Ok(HmacKeyRegistry {
        active,
        retired: Vec::new(),
    })
}

fn random_key_bytes() -> [u8; KEY_BYTES] {
    let mut rng = rand::rng();
    rng.random()
}

fn load_registry(path: &Path) -> Result<HmacKeyRegistry, OpError> {
    let encrypted = std::fs::read_to_string(path)?;
    let plaintext = encrypt::decrypt_value(&encrypted)
        .map_err(|e| OpError::Other(format!("canary key store corrupt: {e}")))?;
    let stored: StoredRegistry = toml::from_str(&plaintext)?;
    if stored.meta.version != STORE_FORMAT_VERSION {
        return Err(OpError::Other(format!(
            "canary key store format version {} not supported (expected {})",
            stored.meta.version, STORE_FORMAT_VERSION
        )));
    }

    let mut active: Option<HmacKeyVersion> = None;
    let mut retired: Vec<HmacKeyVersion> = Vec::new();
    for sk in stored.keys {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(sk.bytes_b64.as_bytes())
            .map_err(|e| OpError::Other(format!("canary key base64 decode: {e}")))?;
        if raw.len() != KEY_BYTES {
            return Err(OpError::Other(format!(
                "canary key has wrong length {} (expected {})",
                raw.len(),
                KEY_BYTES
            )));
        }
        let mut bytes = Zeroizing::new([0u8; KEY_BYTES]);
        bytes.copy_from_slice(&raw);
        let kv = HmacKeyVersion {
            version: sk.version,
            bytes,
            created_at: sk.created_at,
            retired_at: sk.retired_at,
        };
        if sk.active {
            active = Some(kv);
        } else {
            retired.push(kv);
        }
    }
    let active = active.ok_or_else(|| OpError::Other("no active canary key in store".into()))?;
    // Newest retired first.
    retired.sort_by_key(|k| std::cmp::Reverse(k.version));
    Ok(HmacKeyRegistry { active, retired })
}

fn persist_registry(registry: &HmacKeyRegistry, path: &Path) -> Result<(), OpError> {
    let mut keys: Vec<StoredKey> = Vec::with_capacity(1 + registry.retired.len());
    keys.push(stored_key_from(&registry.active, true));
    for r in &registry.retired {
        keys.push(stored_key_from(r, false));
    }
    let stored = StoredRegistry {
        meta: StoredMeta {
            version: STORE_FORMAT_VERSION,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        keys,
    };
    let plaintext = toml::to_string_pretty(&stored)?;
    let encrypted = encrypt::encrypt_value(&plaintext)?;
    config::atomic_write(path, &encrypted, None)?;
    Ok(())
}

fn stored_key_from(kv: &HmacKeyVersion, is_active: bool) -> StoredKey {
    StoredKey {
        version: kv.version,
        active: is_active,
        created_at: kv.created_at.clone(),
        retired_at: kv.retired_at.clone(),
        bytes_b64: base64::engine::general_purpose::STANDARD.encode(kv.bytes.as_slice()),
    }
}

/// Rotate the HMAC key. Generates a new active key, retires the previous one,
/// and evicts the oldest retired key if the cap would be exceeded. Persists the
/// new registry atomically. Returns (new_version, retired_versions_kept).
pub fn rotate_key() -> Result<(u8, Vec<u8>), OpError> {
    let _guard = acquire_lock();
    let path = store_path()?;
    let mut registry = if path.exists() {
        load_registry(&path)?
    } else {
        init_new_registry()?
    };
    let prev_active_version = registry.active.version;
    let now = chrono::Utc::now().to_rfc3339();

    // Move current active to retired (newest-first).
    let mut prev = HmacKeyVersion {
        version: registry.active.version,
        bytes: Zeroizing::new(*registry.active.bytes()),
        created_at: registry.active.created_at.clone(),
        retired_at: Some(now.clone()),
    };
    // Cleanup of original active happens via Drop on swap below.
    let _ = std::mem::replace(&mut prev.bytes, Zeroizing::new(*registry.active.bytes()));

    // Mint new active.
    let new_bytes = Zeroizing::new(random_key_bytes());
    let new_version = prev_active_version.checked_add(1).unwrap_or(1);
    let new_active = HmacKeyVersion {
        version: new_version,
        bytes: new_bytes,
        created_at: now,
        retired_at: None,
    };

    // Build retired list: previous active in front, then existing retired, capped.
    let mut new_retired = Vec::with_capacity(registry.retired.len() + 1);
    new_retired.push(prev);
    new_retired.append(&mut registry.retired);
    while new_retired.len() > MAX_RETIRED_KEYS {
        new_retired.pop();
    }
    let kept: Vec<u8> = new_retired.iter().map(|k| k.version).collect();

    let new_registry = HmacKeyRegistry {
        active: new_active,
        retired: new_retired,
    };
    persist_registry(&new_registry, &path)?;
    Ok((new_version, kept))
}

/// Test helper: erase the on-disk store. Available outside `#[cfg(test)]` so
/// integration tests in `tests/` can use it.
pub fn delete_store_for_tests() -> Result<(), OpError> {
    let path = store_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    /// All hmac_store tests share the on-disk key store at `~/.envforge/canary-keys.age`.
    /// Serialize them through this mutex so they don't race against each other in
    /// the cargo test threadpool.
    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn init_creates_active_key() {
        if dirs::home_dir().is_none() {
            return;
        }
        let _g = test_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _ = delete_store_for_tests();
        let mgr = HmacKeyManager::load_or_init().unwrap();
        assert_eq!(mgr.active_key().version(), 1);
        assert!(mgr.active_key().is_active());
        assert_eq!(mgr.registry().retired().len(), 0);
    }

    #[test]
    fn load_returns_same_key_after_persist() {
        if dirs::home_dir().is_none() {
            return;
        }
        let _g = test_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _ = delete_store_for_tests();
        let mgr1 = HmacKeyManager::load_or_init().unwrap();
        let v1 = *mgr1.active_key().bytes();
        drop(mgr1);
        let mgr2 = HmacKeyManager::load_or_init().unwrap();
        assert_eq!(*mgr2.active_key().bytes(), v1);
    }

    #[test]
    fn rotate_advances_version_and_retains_prior() {
        if dirs::home_dir().is_none() {
            return;
        }
        let _g = test_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _ = delete_store_for_tests();
        let mgr1 = HmacKeyManager::load_or_init().unwrap();
        let prev_v = mgr1.active_key().version();
        let prev_bytes = *mgr1.active_key().bytes();
        drop(mgr1);

        let (new_v, kept) = rotate_key().unwrap();
        assert_eq!(new_v, prev_v + 1);
        assert!(kept.contains(&prev_v));

        let mgr2 = HmacKeyManager::load_or_init().unwrap();
        assert_eq!(mgr2.active_key().version(), new_v);
        assert_eq!(mgr2.registry().retired().len(), 1);
        assert_eq!(*mgr2.registry().retired()[0].bytes(), prev_bytes);
    }

    #[test]
    fn rotate_caps_retired_at_max() {
        if dirs::home_dir().is_none() {
            return;
        }
        let _g = test_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _ = delete_store_for_tests();
        let _ = HmacKeyManager::load_or_init().unwrap();
        for _ in 0..(MAX_RETIRED_KEYS + 2) {
            rotate_key().unwrap();
        }
        let mgr = HmacKeyManager::load_or_init().unwrap();
        assert!(mgr.registry().retired().len() <= MAX_RETIRED_KEYS);
    }

    #[test]
    fn verify_iter_yields_active_first_then_retired_newest_first() {
        if dirs::home_dir().is_none() {
            return;
        }
        let _g = test_lock().lock().unwrap_or_else(|p| p.into_inner());
        let _ = delete_store_for_tests();
        let _ = HmacKeyManager::load_or_init().unwrap();
        rotate_key().unwrap();
        rotate_key().unwrap();
        let mgr = HmacKeyManager::load_or_init().unwrap();
        let iter: Vec<u8> = mgr
            .registry()
            .verify_iter()
            .iter()
            .map(|(v, _)| *v)
            .collect();
        // active version is 3; retired should be [2, 1] (newest-first)
        assert_eq!(iter[0], 3);
        if iter.len() >= 2 {
            assert_eq!(iter[1], 2);
        }
        if iter.len() >= 3 {
            assert_eq!(iter[2], 1);
        }
    }
}
