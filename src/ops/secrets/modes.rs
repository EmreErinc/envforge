use std::collections::HashMap;

use super::cache::{is_reference, resolve_reference, SecretRef};
use super::credentials::read_all_credentials;
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
    let credentials = read_all_credentials(provider_name)?;
    provider.authenticate(&credentials)?;

    let mut secrets = provider.pull(&credentials, path)?;

    // Apply filter if provided
    if let Some(pattern) = filter {
        secrets.retain(|(k, _)| glob_match(pattern, k));
    }

    // Print progress for large pulls
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
        // Track source
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
    let credentials = read_all_credentials(provider_name)?;
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
        return Err(SecretsError::ProviderError {
            provider: provider_name.to_string(),
            message: "no keys to push (check --keys or --filter)".to_string(),
        });
    }

    // Print progress for large pushes
    if filtered.len() > 50 {
        eprintln!("Pushing {} secrets to {}...", filtered.len(), provider_name);
    }

    let count = provider.push(&credentials, path, &filtered)?;

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
                let credentials = read_all_credentials(&secret_ref.provider)?;
                let resolved = resolve_reference(&secret_ref, provider, &credentials)?;
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

/// Simple glob matching.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_inner(&pat_chars, &text_chars)
}

fn glob_match_inner(pattern: &[char], text: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            glob_match_inner(&pattern[1..], text)
                || (!text.is_empty() && glob_match_inner(pattern, &text[1..]))
        }
        (Some('?'), Some(_)) => glob_match_inner(&pattern[1..], &text[1..]),
        (Some(p), Some(t)) if p == t => glob_match_inner(&pattern[1..], &text[1..]),
        _ => false,
    }
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
