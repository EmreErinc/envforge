//! Air-gap trust root: pinned Sigstore Fulcio root + Rekor public key.
//! Bundled at compile time via `include_str!`; user-installable override at
//! `~/.envforge/trust-root.json` (preferred over bundled when present).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::EnvbomError;
use crate::config;

/// Bundled at compile time. Real release builds replace this asset with a
/// curated copy from the Sigstore TUF metadata. The placeholder shipped here
/// keeps the binary buildable in development; users running `envforge envbom
/// verify --airgap` against real Sigstore-signed artifacts must install a
/// current root via `envforge envbom update-trust-root`.
const BUNDLED_TRUST_ROOT: &str = include_str!("../../../assets/sigstore-trust-root.json");

const USER_TRUST_ROOT_FILE: &str = "trust-root.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustRootSource {
    Bundled,
    UserInstalled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirgapTrustRoot {
    pub fulcio_root_pem: String,
    pub rekor_pubkey_pem: String,
    pub bundled_at: String,
    #[serde(default = "default_source_bundled")]
    pub source: TrustRootSource,
}

fn default_source_bundled() -> TrustRootSource {
    TrustRootSource::Bundled
}

impl AirgapTrustRoot {
    /// Load the compile-time-bundled trust root.
    pub fn bundled() -> Result<Self, EnvbomError> {
        let mut root: Self = serde_json::from_str(BUNDLED_TRUST_ROOT)
            .map_err(|e| EnvbomError::TrustRoot(format!("bundled trust root parse: {e}")))?;
        root.source = TrustRootSource::Bundled;
        Ok(root)
    }

    /// Load the user-installed trust root if present at `~/.envforge/trust-root.json`.
    pub fn user_installed() -> Result<Option<Self>, EnvbomError> {
        let path = user_trust_root_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut root: Self = serde_json::from_str(&content)
            .map_err(|e| EnvbomError::TrustRoot(format!("user trust root parse: {e}")))?;
        root.source = TrustRootSource::UserInstalled;
        Ok(Some(root))
    }

    /// Effective trust root: user-installed wins over bundled.
    pub fn effective() -> Result<Self, EnvbomError> {
        match Self::user_installed()? {
            Some(u) => Ok(u),
            None => Self::bundled(),
        }
    }

    /// Install (or replace) the user trust root from a file.
    pub fn install(path: &std::path::Path) -> Result<(), EnvbomError> {
        let content = std::fs::read_to_string(path)?;
        // Validate shape before writing
        let _: Self = serde_json::from_str(&content)
            .map_err(|e| EnvbomError::TrustRoot(format!("input file parse: {e}")))?;
        let target = user_trust_root_path()?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        config::atomic_write(&target, &content, None)
            .map_err(|e| EnvbomError::TrustRoot(format!("atomic write: {e}")))?;
        Ok(())
    }
}

fn user_trust_root_path() -> Result<PathBuf, EnvbomError> {
    Ok(config::config_dir()
        .map_err(|e| EnvbomError::TrustRoot(format!("config dir: {e}")))?
        .join(USER_TRUST_ROOT_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_parses() {
        let r = AirgapTrustRoot::bundled().unwrap();
        assert_eq!(r.source, TrustRootSource::Bundled);
        assert!(!r.fulcio_root_pem.is_empty());
        assert!(!r.rekor_pubkey_pem.is_empty());
    }

    #[test]
    fn bundled_at_is_set() {
        let r = AirgapTrustRoot::bundled().unwrap();
        assert!(!r.bundled_at.is_empty());
    }
}
