//! Unified key-set with per-environment values.
//!
//! The spine of the environment-aware `.env` IDE feature: every key declared in
//! any recognized env file is collected into one [`EnvKeySet`], where each key
//! maps to its value **per environment**. This single model feeds:
//!
//! - key completion — the union of all keys,
//! - value completion — values a key holds in other environments,
//! - per-environment hover and cross-environment missing-key
//!   diagnostics.
//!
//! Pure logic: [`build_env_keyset_from_sources`] takes `(env_name, file,
//! content)` tuples and is fully unit-testable; [`build_env_keyset`] is the
//! disk-reading wrapper over a [`ResolvedEnvSet`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::dotenv::is_sensitive_key;
use super::project::ResolvedEnvSet;
use super::schema::EnvSchema;

/// One key's value as observed in a single environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueOccurrence {
    /// Environment name (from the project manifest, e.g. `production`).
    pub env_name: String,
    /// File the value was read from.
    pub file: PathBuf,
    /// 0-based line number of the `KEY=VALUE` assignment (for hover/goto).
    pub line: usize,
    /// The value with surrounding quotes stripped.
    pub value: String,
    /// Whether this key is sensitive (heuristic; schema sensitivity is unioned
    /// in by the redaction layer).
    pub sensitive: bool,
}

/// All per-environment occurrences of a single key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyEntry {
    /// `env_name -> occurrence`. `BTreeMap` for deterministic ordering.
    pub values: BTreeMap<String, ValueOccurrence>,
}

impl KeyEntry {
    /// Environment names that define this key, in deterministic order.
    pub fn environments(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// True if this key is sensitive in any environment it appears in.
    pub fn is_sensitive(&self) -> bool {
        self.values.values().any(|v| v.sensitive)
    }
}

/// The project's keys, each with its per-environment values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvKeySet {
    /// `key -> entry`. `BTreeMap` for deterministic ordering.
    pub keys: BTreeMap<String, KeyEntry>,
}

impl EnvKeySet {
    /// The union of all keys across environments, in deterministic order
    /// (powers key completion).
    pub fn key_names(&self) -> impl Iterator<Item = &str> {
        self.keys.keys().map(String::as_str)
    }

    /// The per-environment occurrences for a key, if known.
    pub fn entry(&self, key: &str) -> Option<&KeyEntry> {
        self.keys.get(key)
    }

    /// Distinct values a key holds across environments, deterministically
    /// ordered and de-duplicated (powers value completion).
    pub fn distinct_values(&self, key: &str) -> Vec<&str> {
        let Some(entry) = self.keys.get(key) else {
            return Vec::new();
        };
        let mut seen = std::collections::BTreeSet::new();
        for occ in entry.values.values() {
            seen.insert(occ.value.as_str());
        }
        seen.into_iter().collect()
    }

    /// Keys present in at least one environment but absent from `env_name`
    /// (powers the cross-environment missing-key diagnostic). Returns
    /// `(key, [environments that define it])` deterministically ordered.
    pub fn missing_in(&self, env_name: &str) -> Vec<(&str, Vec<&str>)> {
        self.keys
            .iter()
            .filter(|(_, entry)| !entry.values.contains_key(env_name))
            .map(|(key, entry)| (key.as_str(), entry.environments().collect::<Vec<_>>()))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Union schema-declared sensitivity into the key-set: a key
    /// is sensitive if the key-name heuristic already flagged it **or** the
    /// schema marks it `sensitive`. Once a key is sensitive, every one of its
    /// per-environment occurrences is flagged so redaction is uniform.
    pub fn apply_schema_sensitivity(&mut self, schema: &EnvSchema) {
        for (key, entry) in &mut self.keys {
            let schema_sensitive = schema
                .variables
                .get(key)
                .map(|v| v.sensitive)
                .unwrap_or(false);
            if schema_sensitive {
                for occ in entry.values.values_mut() {
                    occ.sensitive = true;
                }
            }
        }
    }
}

/// Build the key-set by reading each recognized env file from disk.
///
/// Files that cannot be read are skipped (a missing/unreadable env file simply
/// contributes no keys — it never aborts the build).
pub fn build_env_keyset(set: &ResolvedEnvSet) -> EnvKeySet {
    let sources: Vec<(String, PathBuf, String)> = set
        .envs
        .iter()
        .filter_map(|env| {
            std::fs::read_to_string(&env.path)
                .ok()
                .map(|content| (env.name.clone(), env.path.clone(), content))
        })
        .collect();
    let refs: Vec<(&str, &Path, &str)> = sources
        .iter()
        .map(|(name, path, content)| (name.as_str(), path.as_path(), content.as_str()))
        .collect();
    build_env_keyset_from_sources(&refs)
}

/// Pure builder over already-read sources: `(env_name, file, content)`.
///
/// When a key appears more than once within one environment's file, the **last**
/// assignment wins (matches shell/.env override semantics).
pub fn build_env_keyset_from_sources(sources: &[(&str, &Path, &str)]) -> EnvKeySet {
    let mut set = EnvKeySet::default();

    for (env_name, file, content) in sources {
        for (key, value, line) in parse_with_lines(content) {
            let occ = ValueOccurrence {
                env_name: (*env_name).to_string(),
                file: file.to_path_buf(),
                line,
                value,
                sensitive: is_sensitive_key(&key),
            };
            set.keys
                .entry(key)
                .or_default()
                .values
                .insert((*env_name).to_string(), occ);
        }
    }

    set
}

/// Parse `KEY=VALUE` lines, capturing 0-based line numbers. Mirrors
/// [`super::dotenv::parse_dotenv_content`] (skip blanks/comments, trim, strip
/// surrounding quotes) but retains line positions for hover/goto.
fn parse_with_lines(content: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let key = trimmed[..eq].trim().to_string();
            if key.is_empty() {
                continue;
            }
            let value = strip_surrounding_quotes(trimmed[eq + 1..].trim());
            out.push((key, value, idx));
        }
    }
    out
}

/// Strip one matching pair of surrounding single or double quotes.
fn strip_surrounding_quotes(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
