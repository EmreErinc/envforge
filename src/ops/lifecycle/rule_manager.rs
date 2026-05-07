use std::fs;
use std::io::Write;
use std::path::PathBuf;

use tempfile::NamedTempFile;

use crate::config::config_dir;
use crate::model::{LifecycleRule, RuleId, TriggerType};
use crate::ops::OpError;

const RULES_DIR: &str = "lifecycle/rules";

fn rules_dir() -> Result<PathBuf, OpError> {
    let dir = config_dir()?.join(RULES_DIR);
    fs::create_dir_all(&dir)?;
    set_dir_perms(&dir);
    Ok(dir)
}

fn rules_dir_at(base: &std::path::Path) -> PathBuf {
    let dir = base.join(RULES_DIR);
    fs::create_dir_all(&dir).ok();
    dir
}

fn set_dir_perms(dir: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).ok();
    }
    let _ = dir;
}

fn rule_path(rule_id: &RuleId) -> Result<PathBuf, OpError> {
    Ok(rules_dir()?.join(format!("{rule_id}.toml")))
}

fn rule_path_at(base: &std::path::Path, rule_id: &RuleId) -> PathBuf {
    rules_dir_at(base).join(format!("{rule_id}.toml"))
}

fn write_atomic(path: &std::path::Path, content: &str) -> Result<(), OpError> {
    let parent = path
        .parent()
        .ok_or_else(|| OpError::Other("invalid rule path".into()))?;
    let mut tmp = NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.persist(path)
        .map_err(|e| OpError::Other(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).ok();
    }
    Ok(())
}

// ─── CRUD ────────────────────────────────────────────────

/// Create a new lifecycle rule and persist to disk.
pub fn create_rule(rule: LifecycleRule) -> Result<LifecycleRule, OpError> {
    let path = rule_path(&rule.id)?;
    create_rule_at(rule, &path)
}

fn create_rule_at(rule: LifecycleRule, path: &std::path::Path) -> Result<LifecycleRule, OpError> {
    if path.exists() {
        return Err(OpError::Other(format!("rule already exists: {}", rule.id)));
    }

    let content = toml::to_string_pretty(&rule).map_err(OpError::TomlSerialize)?;
    write_atomic(path, &content)?;
    Ok(rule)
}

/// Get a rule by ID.
pub fn get_rule(rule_id: &RuleId) -> Result<LifecycleRule, OpError> {
    let path = rule_path(rule_id)?;
    get_rule_at(rule_id, &path)
}

fn get_rule_at(rule_id: &RuleId, path: &std::path::Path) -> Result<LifecycleRule, OpError> {
    let _ = rule_id;
    let content = fs::read_to_string(path)?;
    toml::from_str(&content).map_err(OpError::TomlDeserialize)
}

/// Update an existing rule (atomic write).
pub fn update_rule(rule: &LifecycleRule) -> Result<(), OpError> {
    let path = rule_path(&rule.id)?;
    update_rule_at(rule, &path)
}

fn update_rule_at(rule: &LifecycleRule, path: &std::path::Path) -> Result<(), OpError> {
    if !path.exists() {
        return Err(OpError::Other(format!("rule not found: {}", rule.id)));
    }

    let mut updated = rule.clone();
    updated.updated_at = chrono::Utc::now();

    let content = toml::to_string_pretty(&updated).map_err(OpError::TomlSerialize)?;
    write_atomic(path, &content)?;
    Ok(())
}

/// Delete a rule by removing its TOML file.
pub fn delete_rule(rule_id: &RuleId) -> Result<(), OpError> {
    let path = rule_path(rule_id)?;
    delete_rule_at(rule_id, &path)
}

fn delete_rule_at(rule_id: &RuleId, path: &std::path::Path) -> Result<(), OpError> {
    if !path.exists() {
        return Err(OpError::Other(format!("rule not found: {rule_id}")));
    }
    fs::remove_file(path)?;
    Ok(())
}

// ─── Test helpers ───────────────────────────────────────

/// Create a rule in a test directory.
pub fn create_rule_in(
    rule: LifecycleRule,
    base: &std::path::Path,
) -> Result<LifecycleRule, OpError> {
    let path = rule_path_at(base, &rule.id);
    create_rule_at(rule, &path)
}

/// Get a rule from a test directory.
pub fn get_rule_from(rule_id: &RuleId, base: &std::path::Path) -> Result<LifecycleRule, OpError> {
    let path = rule_path_at(base, rule_id);
    get_rule_at(rule_id, &path)
}

/// Update a rule in a test directory.
pub fn update_rule_in(rule: &LifecycleRule, base: &std::path::Path) -> Result<(), OpError> {
    let path = rule_path_at(base, &rule.id);
    update_rule_at(rule, &path)
}

/// Delete a rule from a test directory.
pub fn delete_rule_from(rule_id: &RuleId, base: &std::path::Path) -> Result<(), OpError> {
    let path = rule_path_at(base, rule_id);
    delete_rule_at(rule_id, &path)
}

/// List rules from a test directory.
pub fn list_rules_in(base: &std::path::Path) -> Result<Vec<LifecycleRule>, OpError> {
    let dir = rules_dir_at(base);
    let mut rules = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(rules),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(rule) = toml::from_str::<LifecycleRule>(&content) {
                    rules.push(rule);
                }
            }
        }
    }

    Ok(rules)
}

/// Enable a rule in a test directory.
pub fn enable_rule_in(rule_id: &RuleId, base: &std::path::Path) -> Result<(), OpError> {
    let mut rule = get_rule_from(rule_id, base)?;
    rule.enabled = true;
    update_rule_in(&rule, base)
}

/// Disable a rule in a test directory.
pub fn disable_rule_in(rule_id: &RuleId, base: &std::path::Path) -> Result<(), OpError> {
    let mut rule = get_rule_from(rule_id, base)?;
    rule.enabled = false;
    update_rule_in(&rule, base)
}

// ─── Queries ─────────────────────────────────────────────

/// List all rules (scan rules directory for .toml files).
pub fn list_rules() -> Result<Vec<LifecycleRule>, OpError> {
    let dir = rules_dir()?;
    list_rules_in_dir(&dir)
}

fn list_rules_in_dir(dir: &std::path::Path) -> Result<Vec<LifecycleRule>, OpError> {
    let mut rules = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(rules),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(rule) = toml::from_str::<LifecycleRule>(&content) {
                    rules.push(rule);
                }
            }
        }
    }
    Ok(rules)
}

/// List only enabled rules.
pub fn list_enabled_rules() -> Result<Vec<LifecycleRule>, OpError> {
    Ok(list_rules()?.into_iter().filter(|r| r.enabled).collect())
}

/// Get rules that match a specific trigger type.
pub fn get_rules_by_trigger_type(
    trigger_type: &TriggerType,
) -> Result<Vec<LifecycleRule>, OpError> {
    // Match on trigger_type tag in serialized form
    let target = trigger_type.to_string();
    Ok(list_rules()?
        .into_iter()
        .filter(|r| {
            let trigger_str = match &r.trigger {
                crate::model::LifecycleTrigger::Cron { .. } => "cron",
                crate::model::LifecycleTrigger::AgeExceeded { .. } => "age",
                crate::model::LifecycleTrigger::FileChange { .. } => "file-change",
                crate::model::LifecycleTrigger::PolicyViolation { .. } => "policy",
                crate::model::LifecycleTrigger::Composite { .. } => "composite",
            };
            trigger_str == target
        })
        .collect())
}

// ─── Enable / Disable ────────────────────────────────────

/// Enable a rule.
pub fn enable_rule(rule_id: &RuleId) -> Result<(), OpError> {
    let mut rule = get_rule(rule_id)?;
    rule.enabled = true;
    update_rule(&rule)
}

/// Disable a rule.
pub fn disable_rule(rule_id: &RuleId) -> Result<(), OpError> {
    let mut rule = get_rule(rule_id)?;
    rule.enabled = false;
    update_rule(&rule)
}
