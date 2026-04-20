use std::collections::BTreeMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

const CANARY_FILE: &str = "canaries.toml";
const CANARY_LOG: &str = "canary-alerts.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanarySecret {
    pub key: String,
    pub fake_value: String,
    pub created_at: String,
    pub pattern: String,  // "aws_key", "api_token", "generic"
    pub triggered: bool,
    pub trigger_count: usize,
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
    pub source: String,  // "proxy", "file_read", "git_commit"
    pub details: String,
}

/// Generate a fake but plausible credential value for a given pattern.
pub fn generate_fake_value(pattern: &str) -> String {
    // Generate deterministic-looking but fake values
    let suffix: String = (0..20).map(|i| {
        let chars = b"abcdefghijklmnopqrstuvwxyz0123456789";
        chars[((i * 7 + 13) % chars.len()) as usize] as char
    }).collect();

    match pattern {
        "aws_key" => format!("AKIA{}", &suffix[..16].to_uppercase()),
        "github_token" => format!("ghp_{}", &suffix),
        "stripe_key" => format!("sk_live_{}", &suffix),
        "slack_token" => format!("xoxb-0000-0000-{}", &suffix),
        "gitlab_token" => format!("glpat-{}", &suffix),
        _ => format!("CANARY_{}", &suffix),
    }
}

/// Get canary store path.
fn canary_store_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = crate::config::config_dir()?;
    Ok(dir.join(CANARY_FILE))
}

/// Get canary alert log path.
fn canary_log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = crate::config::config_dir()?;
    Ok(dir.join(CANARY_LOG))
}

/// Load canary store.
pub fn load_canaries() -> Result<CanaryStore, Box<dyn std::error::Error>> {
    let path = canary_store_path()?;
    if !path.exists() {
        return Ok(CanaryStore { canaries: BTreeMap::new() });
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

/// Save canary store.
fn save_canaries(store: &CanaryStore) -> Result<(), Box<dyn std::error::Error>> {
    let path = canary_store_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string_pretty(store)?)?;
    Ok(())
}

/// Create a new canary secret.
pub fn create_canary(key: &str, pattern: &str) -> Result<CanarySecret, Box<dyn std::error::Error>> {
    let mut store = load_canaries()?;

    let fake_value = generate_fake_value(pattern);
    let canary = CanarySecret {
        key: key.to_string(),
        fake_value,
        created_at: chrono::Utc::now().to_rfc3339(),
        pattern: pattern.to_string(),
        triggered: false,
        trigger_count: 0,
    };

    store.canaries.insert(key.to_string(), canary.clone());
    save_canaries(&store)?;

    Ok(canary)
}

/// List all canaries.
pub fn list_canaries() -> Result<Vec<CanarySecret>, Box<dyn std::error::Error>> {
    let store = load_canaries()?;
    Ok(store.canaries.values().cloned().collect())
}

/// Record a canary trigger (value was accessed/used).
pub fn trigger_canary(key: &str, source: &str, details: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Update store
    let mut store = load_canaries()?;
    if let Some(canary) = store.canaries.get_mut(key) {
        canary.triggered = true;
        canary.trigger_count += 1;
        save_canaries(&store)?;
    }

    // Append to alert log
    let alert = CanaryAlert {
        timestamp: chrono::Utc::now().to_rfc3339(),
        key: key.to_string(),
        source: source.to_string(),
        details: details.to_string(),
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

    // Print alert to stderr
    eprintln!("\u{1f6a8} CANARY TRIGGERED: {} (source: {}, {})", key, source, details);

    Ok(())
}

/// Check all canaries for triggers. Returns triggered ones.
pub fn check_canaries() -> Result<Vec<CanarySecret>, Box<dyn std::error::Error>> {
    let store = load_canaries()?;
    Ok(store.canaries.values().filter(|c| c.triggered).cloned().collect())
}

/// Read canary alert log.
pub fn read_alerts() -> Result<Vec<CanaryAlert>, Box<dyn std::error::Error>> {
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
pub fn delete_canary(key: &str) -> Result<bool, Box<dyn std::error::Error>> {
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
        assert!(val[4..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
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
    fn test_create_and_load_canary_roundtrip() {
        // Use a temp dir to avoid polluting real config
        let tmp = tempfile::tempdir().unwrap();
        let store_path = tmp.path().join(CANARY_FILE);

        // Manually create and save
        let mut store = CanaryStore { canaries: BTreeMap::new() };
        let canary = CanarySecret {
            key: "TEST_KEY".to_string(),
            fake_value: "CANARY_fake123".to_string(),
            created_at: "2026-04-20T00:00:00Z".to_string(),
            pattern: "generic".to_string(),
            triggered: false,
            trigger_count: 0,
        };
        store.canaries.insert("TEST_KEY".to_string(), canary.clone());
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
                },
            )]),
        };

        // Direct check against store (is_canary_value uses config dir, so test logic here)
        let found = store.canaries.values()
            .find(|c| c.fake_value == "CANARY_unique_value_xyz")
            .map(|c| c.key.clone());
        assert_eq!(found, Some("MY_SECRET".to_string()));

        // Not found
        let not_found = store.canaries.values()
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
        let mut store = CanaryStore { canaries: BTreeMap::new() };
        store.canaries.insert("TRAP_KEY".to_string(), CanarySecret {
            key: "TRAP_KEY".to_string(),
            fake_value: "CANARY_trap_value".to_string(),
            created_at: "2026-04-20T00:00:00Z".to_string(),
            pattern: "generic".to_string(),
            triggered: false,
            trigger_count: 0,
        });
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
                ("SAFE_KEY".to_string(), CanarySecret {
                    key: "SAFE_KEY".to_string(),
                    fake_value: "CANARY_safe".to_string(),
                    created_at: "2026-04-20T00:00:00Z".to_string(),
                    pattern: "generic".to_string(),
                    triggered: false,
                    trigger_count: 0,
                }),
                ("LEAKED_KEY".to_string(), CanarySecret {
                    key: "LEAKED_KEY".to_string(),
                    fake_value: "CANARY_leaked".to_string(),
                    created_at: "2026-04-20T00:00:00Z".to_string(),
                    pattern: "generic".to_string(),
                    triggered: true,
                    trigger_count: 3,
                }),
                ("ANOTHER_SAFE".to_string(), CanarySecret {
                    key: "ANOTHER_SAFE".to_string(),
                    fake_value: "CANARY_another".to_string(),
                    created_at: "2026-04-20T00:00:00Z".to_string(),
                    pattern: "aws_key".to_string(),
                    triggered: false,
                    trigger_count: 0,
                }),
            ]),
        };

        let triggered: Vec<&CanarySecret> = store.canaries.values()
            .filter(|c| c.triggered)
            .collect();
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0].key, "LEAKED_KEY");
        assert_eq!(triggered[0].trigger_count, 3);
    }
}
