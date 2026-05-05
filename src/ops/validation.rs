use std::collections::HashMap;

use regex::Regex;

use crate::ops::listing::EnvEntry;

/// A validation failure.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub key: String,
    pub value: String,
    pub rule: String,
    pub message: String,
}

/// Validate entries against rules defined in config.
///
/// Rules map: KEY → validator string (e.g., "url", "number", "bool", "regex:pattern")
pub fn validate_entries(
    entries: &[EnvEntry],
    rules: &HashMap<String, String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for entry in entries {
        if let Some(rule) = rules.get(&entry.key) {
            if let Some(msg) = validate_value(&entry.value, rule) {
                errors.push(ValidationError {
                    key: entry.key.clone(),
                    value: entry.value.clone(),
                    rule: rule.clone(),
                    message: msg,
                });
            }
        }
    }

    errors
}

use super::validation_utils;

/// Validate a single value against a rule string.
/// Returns None if valid, Some(error_message) if invalid.
pub fn validate_value(value: &str, rule: &str) -> Option<String> {
    match rule {
        "nonempty" => {
            if value.trim().is_empty() {
                Some("Value must not be empty".to_string())
            } else {
                None
            }
        }
        "number" => {
            if value.parse::<f64>().is_ok() {
                None
            } else {
                Some(format!("Expected number, got '{}'", value))
            }
        }
        "bool" => {
            if validation_utils::is_valid_bool(value) {
                None
            } else {
                Some(format!("Expected bool, got '{}'", value))
            }
        }
        "url" => {
            if validation_utils::is_valid_url(value) {
                None
            } else {
                Some(format!("Expected URL, got '{}'", value))
            }
        }
        "email" => {
            if validation_utils::is_valid_email(value) {
                None
            } else {
                Some(format!("Expected email, got '{}'", value))
            }
        }
        "port" => {
            if validation_utils::is_valid_port(value) {
                None
            } else {
                Some(format!("Expected port (1-65535), got '{}'", value))
            }
        }
        rule if rule.starts_with("regex:") => {
            let pattern = &rule[6..];
            match Regex::new(pattern) {
                Ok(re) => {
                    if re.is_match(value) {
                        None
                    } else {
                        Some(format!(
                            "Value '{}' doesn't match pattern '{}'",
                            value, pattern
                        ))
                    }
                }
                Err(e) => Some(format!("Invalid regex '{}': {}", pattern, e)),
            }
        }
        _ => {
            // Unknown rule — skip
            None
        }
    }
}

/// Get a set of keys that have validation errors (for TUI warning display).
pub fn invalid_key_set(
    entries: &[EnvEntry],
    rules: &HashMap<String, String>,
) -> HashMap<String, String> {
    let errors = validate_entries(entries, rules);
    errors.into_iter().map(|e| (e.key, e.message)).collect()
}
