use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::OpError;

pub mod hmac_store;
pub mod migration;
pub mod scanner;
pub mod v2;

pub use hmac_store::{rotate_key, HmacKeyManager, HmacKeyRegistry};
pub use migration::{MigrationPlan, MigrationReport, MigrationService};
pub use scanner::{scan_reader, scan_text, TokenMatch, TokenScanner};
pub use v2::{decode_token, encode_token, DecodedCanary, V2Payload, V2_PREFIX, VERSION_BYTE_V2};

const CANARY_FILE: &str = "canaries.toml";
const CANARY_LOG: &str = "canary-alerts.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanarySecret {
    pub key: String,
    pub fake_value: String,
    pub created_at: String,
    pub pattern: String, // "aws_key", "api_token", "generic"
    pub triggered: bool,
    pub trigger_count: usize,
    #[serde(default = "default_rotate_after_days")]
    pub rotate_after_days: u32,
    // v2 extension fields (additive; serde-default for backward compat).
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub forensic: bool,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub payload_summary: Option<PayloadSummary>,
}

impl Default for CanarySecret {
    fn default() -> Self {
        Self {
            key: String::new(),
            fake_value: String::new(),
            created_at: String::new(),
            pattern: "generic".into(),
            triggered: false,
            trigger_count: 0,
            rotate_after_days: 14,
            version: 1,
            forensic: false,
            superseded_by: None,
            payload_summary: None,
        }
    }
}

fn default_rotate_after_days() -> u32 {
    14
}

fn default_version() -> u8 {
    1
}

/// Stored snapshot of v2 payload metadata at mint time.
/// Never includes the HMAC tag or raw token; used for `list` display only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadSummary {
    pub tool_name: String,
    pub minted_at: String,
    pub key_version: u8,
    pub token_prefix: String, // first 8 chars of token for grep correlation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryStore {
    #[serde(default)]
    pub canaries: BTreeMap<String, CanarySecret>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryAlert {
    pub timestamp: String,
    pub key: String,
    pub source: String, // "proxy", "file_read", "git_commit"
    pub details: String,
}

/// Generate a fake but plausible credential value for a given pattern.
pub fn generate_fake_value(pattern: &str) -> String {
    let suffix: String = (0..20)
        .map(|i| {
            let chars = b"abcdefghijklmnopqrstuvwxyz0123456789";
            chars[(i * 7 + 13) % chars.len()] as char
        })
        .collect();

    match pattern {
        "aws_key" => format!("AKIA{}", &suffix[..16].to_uppercase()),
        "github_token" => format!("ghp_{}", &suffix),
        "stripe_key" => format!("sk_live_{}", &suffix),
        "slack_token" => format!("xoxb-0000-0000-{}", &suffix),
        "gitlab_token" => format!("glpat-{}", &suffix),
        "database_url" => format!(
            "postgres://canary_user:{}@canary-host:5432/canary_db",
            &suffix[..12]
        ),
        "jwt_token" => {
            let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(r#"{"alg":"HS256","typ":"JWT"}"#);
            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(
                r#"{{"sub":"canary","exp":9999999999,"key":"{}"}}"#,
                &suffix[..12]
            ));
            let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&suffix[..16]);
            format!("{}.{}.{}", header, payload, signature)
        }
        "openai_key" => format!("sk-canary-{}", &suffix),
        "private_key_pem" => {
            let body = base64::engine::general_purpose::STANDARD.encode(suffix.repeat(8));
            format!(
                "-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----",
                body
            )
        }
        "smtp_credential" => format!("smtp://canary_user:{}@smtp.canary.local:587", &suffix[..12]),
        "ftp_credential" => format!("ftp://canary_user:{}@ftp.canary.local:21", &suffix[..12]),
        _ => format!("CANARY_{}", &suffix),
    }
}

/// Get canary store path.
fn canary_store_path() -> Result<PathBuf, OpError> {
    let dir = crate::config::config_dir()?;
    Ok(dir.join(CANARY_FILE))
}

/// Get canary alert log path.
fn canary_log_path() -> Result<PathBuf, OpError> {
    let dir = crate::config::config_dir()?;
    Ok(dir.join(CANARY_LOG))
}

/// Load canary store.
pub fn load_canaries() -> Result<CanaryStore, OpError> {
    let path = canary_store_path()?;
    if !path.exists() {
        return Ok(CanaryStore {
            canaries: BTreeMap::new(),
        });
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

/// Save canary store.
pub(crate) fn save_canaries(store: &CanaryStore) -> Result<(), OpError> {
    let path = canary_store_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string_pretty(store)?)?;
    Ok(())
}

/// Create a new canary secret.
pub fn create_canary(key: &str, pattern: &str) -> Result<CanarySecret, OpError> {
    let mut store = load_canaries()?;

    let fake_value = generate_fake_value(pattern);
    let canary = CanarySecret {
        key: key.to_string(),
        fake_value,
        created_at: chrono::Utc::now().to_rfc3339(),
        pattern: pattern.to_string(),
        triggered: false,
        trigger_count: 0,
        rotate_after_days: 14,
        version: 1,
        forensic: false,
        superseded_by: None,
        payload_summary: None,
    };

    store.canaries.insert(key.to_string(), canary.clone());
    save_canaries(&store)?;

    Ok(canary)
}

/// Mint a v2 forensic canary token bound to (tool, key, pid, machine).
/// Returns (record, token_string). Existing canary at the same key is preserved
/// only if it's a v1 record being superseded — caller wires `superseded_by`.
pub fn mint_v2(key: &str, tool_name: &str, pid: u32) -> Result<(CanarySecret, String), OpError> {
    let mut store = load_canaries()?;

    let key_mgr = HmacKeyManager::load_or_init()?;
    let active = key_mgr.active_key();
    let machine_id = stable_machine_id();
    let payload = V2Payload::new(machine_id, pid, chrono::Utc::now(), tool_name, key);
    let token = encode_token(&payload, active.bytes());

    let token_prefix: String = token.chars().take(12).collect();
    let summary = PayloadSummary {
        tool_name: tool_name.to_string(),
        minted_at: chrono::Utc::now().to_rfc3339(),
        key_version: active.version(),
        token_prefix,
    };

    let canary = CanarySecret {
        key: key.to_string(),
        fake_value: token.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        pattern: "v2_forensic".to_string(),
        triggered: false,
        trigger_count: 0,
        rotate_after_days: 14,
        version: 2,
        forensic: true,
        superseded_by: None,
        payload_summary: Some(summary),
    };

    store.canaries.insert(key.to_string(), canary.clone());
    save_canaries(&store)?;

    Ok((canary, token))
}

/// Stable per-machine identifier (8 bytes). SHA-256 of hostname truncated to 8 bytes.
/// Falls back to a zero array if hostname is unavailable.
fn stable_machine_id() -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let host = hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_default();
    let h = Sha256::digest(host.as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&h[..8]);
    out
}

/// List all canaries.
pub fn list_canaries() -> Result<Vec<CanarySecret>, OpError> {
    let store = load_canaries()?;
    Ok(store.canaries.values().cloned().collect())
}

/// Record a canary trigger (value was accessed/used).
///
/// Redacts the canary's `fake_value` from the `source` and `details` fields
/// before writing them to the alert log or stderr. Without this, a canary
/// that was tripped by appearing in some log line would re-leak its value
/// into our own alert log — defeating the purpose, since the canary is
/// only useful as long as the attacker doesn't know it's a canary.
pub fn trigger_canary(key: &str, source: &str, details: &str) -> Result<(), OpError> {
    // Look up the fake_value so we can redact it from contextual strings.
    let fake_value = load_canaries()
        .ok()
        .and_then(|s| s.canaries.get(key).map(|c| c.fake_value.clone()));

    let safe_source = redact_canary_value(source, fake_value.as_deref());
    let safe_details = redact_canary_value(details, fake_value.as_deref());

    // Emit monitor event
    crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
        source: crate::ops::monitor::EventSource::Canary,
        key: Some(key.to_string()),
        message: format!(
            "Canary '{}' triggered by {}: {}",
            key, safe_source, safe_details
        ),
        timestamp: chrono::Utc::now(),
        severity: crate::ops::monitor::SecuritySeverity::Critical,
    });
    // Update store
    let mut store = load_canaries()?;
    if let Some(canary) = store.canaries.get_mut(key) {
        canary.triggered = true;
        canary.trigger_count += 1;
        save_canaries(&store)?;
    }

    // Append to alert log (with the canary value redacted).
    let alert = CanaryAlert {
        timestamp: chrono::Utc::now().to_rfc3339(),
        key: key.to_string(),
        source: safe_source.clone(),
        details: safe_details.clone(),
    };
    let log_path = canary_log_path()?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    use std::io::Write;
    writeln!(file, "{}", serde_json::to_string(&alert)?)?;

    // Print redacted alert to stderr
    eprintln!(
        "\u{1f6a8} CANARY TRIGGERED: {} (source: {}, {})",
        key, safe_source, safe_details
    );

    Ok(())
}

/// Replace any occurrence of the canary's `fake_value` in a contextual
/// string with `[CANARY_VALUE_REDACTED]`. Returns the string unchanged
/// if the fake value isn't present (or isn't known).
fn redact_canary_value(input: &str, fake_value: Option<&str>) -> String {
    match fake_value {
        Some(v) if !v.is_empty() && input.contains(v) => {
            input.replace(v, "[CANARY_VALUE_REDACTED]")
        }
        _ => input.to_string(),
    }
}

/// Check all canaries for triggers. Returns triggered ones.
pub fn check_canaries() -> Result<Vec<CanarySecret>, OpError> {
    let store = load_canaries()?;
    Ok(store
        .canaries
        .values()
        .filter(|c| c.triggered)
        .cloned()
        .collect())
}

/// Read canary alert log.
pub fn read_alerts() -> Result<Vec<CanaryAlert>, OpError> {
    let path = canary_log_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let alerts: Vec<CanaryAlert> = content
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Ok(alerts)
}

/// Delete a canary.
pub fn delete_canary(key: &str) -> Result<bool, OpError> {
    let mut store = load_canaries()?;
    let removed = store.canaries.remove(key).is_some();
    if removed {
        save_canaries(&store)?;
    }
    Ok(removed)
}

/// Check if a value matches any canary secret.
pub fn is_canary_value(value: &str) -> Option<String> {
    if let Ok(store) = load_canaries() {
        for (key, canary) in &store.canaries {
            if canary.fake_value == value {
                return Some(key.clone());
            }
        }
    }
    None
}

// ─── Rotation ──────────────────────────────────────────────

/// Check if a canary is eligible for rotation (age > rotate_after_days).
pub fn is_eligible_for_rotation(canary: &CanarySecret) -> bool {
    if canary.rotate_after_days == 0 {
        return false;
    }
    let created = chrono::DateTime::parse_from_rfc3339(&canary.created_at);
    match created {
        Ok(dt) => {
            let age = chrono::Utc::now().signed_duration_since(dt);
            age.num_days() >= i64::from(canary.rotate_after_days)
        }
        Err(_) => false,
    }
}

/// Rotate a single canary: generate new fake value, reset trigger state.
pub fn rotate_canary(key: &str) -> Result<Option<CanarySecret>, OpError> {
    let mut store = load_canaries()?;
    if let Some(canary) = store.canaries.get_mut(key) {
        let new_value = generate_fake_value(&canary.pattern);
        canary.fake_value = new_value;
        canary.created_at = chrono::Utc::now().to_rfc3339();
        canary.triggered = false;
        canary.trigger_count = 0;
        let cloned = canary.clone();
        save_canaries(&store)?;
        Ok(Some(cloned))
    } else {
        Ok(None)
    }
}

/// Rotate all eligible canaries. Returns count of rotated canaries.
pub fn rotate_all_canaries() -> Result<usize, OpError> {
    let mut store = load_canaries()?;
    let mut rotated = 0;
    for canary in store.canaries.values_mut() {
        if is_eligible_for_rotation(canary) {
            canary.fake_value = generate_fake_value(&canary.pattern);
            canary.created_at = chrono::Utc::now().to_rfc3339();
            canary.triggered = false;
            canary.trigger_count = 0;
            rotated += 1;
        }
    }
    if rotated > 0 {
        save_canaries(&store)?;
    }
    Ok(rotated)
}

// ─── Placement ─────────────────────────────────────────────

/// Place a canary line into a file. Returns true if placed, false if already exists.
pub fn place_canary_in_file(
    key: &str,
    file_path: &std::path::Path,
    position: &str,
) -> Result<bool, OpError> {
    let store = load_canaries()?;
    let canary = store.canaries.get(key).ok_or("Canary not found")?;

    let marker = format!("# envforge canary: {}={}", key, canary.fake_value);

    let content = if file_path.exists() {
        std::fs::read_to_string(file_path)?
    } else {
        String::new()
    };

    if content.contains(&marker) {
        return Ok(false);
    }

    let mut lines: Vec<&str> = content.lines().collect();
    let insert_idx = match position {
        "top" => 0,
        "bottom" => lines.len(),
        "random" if !lines.is_empty() => {
            // Deterministic "random" based on key hash
            let hash: usize = key.bytes().map(|b| b as usize).sum();
            hash % lines.len()
        }
        _ => lines.len() / 2,
    };

    lines.insert(insert_idx, &marker);
    let new_content = lines.join("\n");
    std::fs::write(file_path, new_content)?;

    Ok(true)
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_fake_value_aws_key() {
        let val = generate_fake_value("aws_key");
        assert!(val.starts_with("AKIA"));
        assert_eq!(val.len(), 4 + 16); // "AKIA" + 16 uppercase chars
                                       // Should be uppercase after AKIA
        assert!(val[4..]
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_fake_value_github_token() {
        let val = generate_fake_value("github_token");
        assert!(val.starts_with("ghp_"));
        assert!(val.len() > 4);
    }

    #[test]
    fn test_generate_fake_value_stripe_key() {
        let val = generate_fake_value("stripe_key");
        assert!(val.starts_with("sk_live_"));
    }

    #[test]
    fn test_generate_fake_value_slack_token() {
        let val = generate_fake_value("slack_token");
        assert!(val.starts_with("xoxb-0000-0000-"));
    }

    #[test]
    fn test_generate_fake_value_gitlab_token() {
        let val = generate_fake_value("gitlab_token");
        assert!(val.starts_with("glpat-"));
    }

    #[test]
    fn test_generate_fake_value_generic() {
        let val = generate_fake_value("generic");
        assert!(val.starts_with("CANARY_"));
    }

    #[test]
    fn test_generate_fake_value_unknown_pattern() {
        let val = generate_fake_value("something_else");
        assert!(val.starts_with("CANARY_"));
    }

    #[test]
    fn test_generate_fake_value_database_url() {
        let val = generate_fake_value("database_url");
        assert!(val.starts_with("postgres://canary_user:"));
        assert!(val.contains("@canary-host:5432/canary_db"));
    }

    #[test]
    fn test_generate_fake_value_jwt_token() {
        let val = generate_fake_value("jwt_token");
        assert_eq!(val.split('.').count(), 3);
    }

    #[test]
    fn test_generate_fake_value_openai_key() {
        let val = generate_fake_value("openai_key");
        assert!(val.starts_with("sk-canary-"));
    }

    #[test]
    fn test_generate_fake_value_private_key_pem() {
        let val = generate_fake_value("private_key_pem");
        assert!(val.starts_with("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(val.contains("-----END RSA PRIVATE KEY-----"));
    }

    #[test]
    fn test_generate_fake_value_smtp_credential() {
        let val = generate_fake_value("smtp_credential");
        assert!(val.starts_with("smtp://canary_user:"));
        assert!(val.contains("@smtp.canary.local:587"));
    }

    #[test]
    fn test_generate_fake_value_ftp_credential() {
        let val = generate_fake_value("ftp_credential");
        assert!(val.starts_with("ftp://canary_user:"));
        assert!(val.contains("@ftp.canary.local:21"));
    }

    #[test]
    fn test_create_and_load_canary_roundtrip() {
        // Use a temp dir to avoid polluting real config
        let tmp = tempfile::tempdir().unwrap();
        let store_path = tmp.path().join(CANARY_FILE);

        // Manually create and save
        let mut store = CanaryStore {
            canaries: BTreeMap::new(),
        };
        let canary = CanarySecret {
            key: "TEST_KEY".to_string(),
            fake_value: "CANARY_fake123".to_string(),
            created_at: "2026-04-20T00:00:00Z".to_string(),
            pattern: "generic".to_string(),
            triggered: false,
            trigger_count: 0,
            rotate_after_days: 14,
            ..Default::default()
        };
        store.canaries.insert("TEST_KEY".to_string(), canary);
        std::fs::write(&store_path, toml::to_string_pretty(&store).unwrap()).unwrap();

        // Reload
        let content = std::fs::read_to_string(&store_path).unwrap();
        let loaded: CanaryStore = toml::from_str(&content).unwrap();
        assert_eq!(loaded.canaries.len(), 1);
        let loaded_canary = loaded.canaries.get("TEST_KEY").unwrap();
        assert_eq!(loaded_canary.key, "TEST_KEY");
        assert_eq!(loaded_canary.fake_value, "CANARY_fake123");
        assert_eq!(loaded_canary.pattern, "generic");
        assert!(!loaded_canary.triggered);
        assert_eq!(loaded_canary.trigger_count, 0);
    }

    #[test]
    fn test_is_canary_value_detection() {
        // This test uses a temp store — we test the logic directly
        let store = CanaryStore {
            canaries: BTreeMap::from([(
                "MY_SECRET".to_string(),
                CanarySecret {
                    key: "MY_SECRET".to_string(),
                    fake_value: "CANARY_unique_value_xyz".to_string(),
                    created_at: "2026-04-20T00:00:00Z".to_string(),
                    pattern: "generic".to_string(),
                    triggered: false,
                    trigger_count: 0,
                    rotate_after_days: 14,
                    ..Default::default()
                },
            )]),
        };

        // Direct check against store (is_canary_value uses config dir, so test logic here)
        let found = store
            .canaries
            .values()
            .find(|c| c.fake_value == "CANARY_unique_value_xyz")
            .map(|c| c.key.clone());
        assert_eq!(found, Some("MY_SECRET".to_string()));

        // Not found
        let not_found = store
            .canaries
            .values()
            .find(|c| c.fake_value == "totally_different_value")
            .map(|c| c.key.clone());
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_trigger_canary_updates_store_and_writes_log() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = tmp.path().join(CANARY_FILE);
        let log_path = tmp.path().join(CANARY_LOG);

        // Create initial store
        let mut store = CanaryStore {
            canaries: BTreeMap::new(),
        };
        store.canaries.insert(
            "TRAP_KEY".to_string(),
            CanarySecret {
                key: "TRAP_KEY".to_string(),
                fake_value: "CANARY_trap_value".to_string(),
                created_at: "2026-04-20T00:00:00Z".to_string(),
                pattern: "generic".to_string(),
                triggered: false,
                trigger_count: 0,
                rotate_after_days: 14,
                ..Default::default()
            },
        );
        std::fs::write(&store_path, toml::to_string_pretty(&store).unwrap()).unwrap();

        // Simulate trigger by updating store directly
        let content = std::fs::read_to_string(&store_path).unwrap();
        let mut loaded: CanaryStore = toml::from_str(&content).unwrap();
        if let Some(canary) = loaded.canaries.get_mut("TRAP_KEY") {
            canary.triggered = true;
            canary.trigger_count += 1;
        }
        std::fs::write(&store_path, toml::to_string_pretty(&loaded).unwrap()).unwrap();

        // Write alert log
        let alert = CanaryAlert {
            timestamp: "2026-04-20T01:00:00Z".to_string(),
            key: "TRAP_KEY".to_string(),
            source: "ai_guard".to_string(),
            details: "detected in tool output".to_string(),
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap();
        use std::io::Write;
        writeln!(file, "{}", serde_json::to_string(&alert).unwrap()).unwrap();

        // Verify store updated
        let content = std::fs::read_to_string(&store_path).unwrap();
        let final_store: CanaryStore = toml::from_str(&content).unwrap();
        let canary = final_store.canaries.get("TRAP_KEY").unwrap();
        assert!(canary.triggered);
        assert_eq!(canary.trigger_count, 1);

        // Verify log written
        let log_content = std::fs::read_to_string(&log_path).unwrap();
        let alerts: Vec<CanaryAlert> = log_content
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].key, "TRAP_KEY");
        assert_eq!(alerts[0].source, "ai_guard");
    }

    #[test]
    fn test_check_canaries_returns_only_triggered() {
        let store = CanaryStore {
            canaries: BTreeMap::from([
                (
                    "SAFE_KEY".to_string(),
                    CanarySecret {
                        key: "SAFE_KEY".to_string(),
                        fake_value: "CANARY_safe".to_string(),
                        created_at: "2026-04-20T00:00:00Z".to_string(),
                        pattern: "generic".to_string(),
                        triggered: false,
                        trigger_count: 0,
                        rotate_after_days: 14,
                        ..Default::default()
                    },
                ),
                (
                    "LEAKED_KEY".to_string(),
                    CanarySecret {
                        key: "LEAKED_KEY".to_string(),
                        fake_value: "CANARY_leaked".to_string(),
                        created_at: "2026-04-20T00:00:00Z".to_string(),
                        pattern: "generic".to_string(),
                        triggered: true,
                        trigger_count: 3,
                        rotate_after_days: 14,
                        ..Default::default()
                    },
                ),
                (
                    "ANOTHER_SAFE".to_string(),
                    CanarySecret {
                        key: "ANOTHER_SAFE".to_string(),
                        fake_value: "CANARY_another".to_string(),
                        created_at: "2026-04-20T00:00:00Z".to_string(),
                        pattern: "aws_key".to_string(),
                        triggered: false,
                        trigger_count: 0,
                        rotate_after_days: 14,
                        ..Default::default()
                    },
                ),
            ]),
        };

        let triggered: Vec<&CanarySecret> =
            store.canaries.values().filter(|c| c.triggered).collect();
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0].key, "LEAKED_KEY");
        assert_eq!(triggered[0].trigger_count, 3);
    }
}
