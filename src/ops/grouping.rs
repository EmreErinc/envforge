use std::collections::{HashMap, HashSet};

use crate::ops::listing::EnvEntry;

/// A group of ENV entries.
#[derive(Debug, Clone)]
pub struct EnvGroup {
    pub name: String,
    pub entries: Vec<EnvEntry>,
    pub is_user_defined: bool,
}

/// Configuration for user-defined groups.
#[derive(Debug, Clone, Default)]
pub struct GroupConfig {
    /// Ordered list of (group_name, patterns) pairs.
    pub groups: Vec<(String, Vec<String>)>,
}

/// Group entries by prefix or user-defined patterns.
///
/// Priority: user-defined groups first, then auto-detected prefix groups.
/// Entries not matching any group go into "Other".
pub fn group_entries(entries: &[EnvEntry], config: &GroupConfig) -> Vec<EnvGroup> {
    let mut result = Vec::new();
    let mut assigned: HashSet<usize> = HashSet::new();

    for (name, patterns) in &config.groups {
        let mut group_entries = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            if assigned.contains(&i) {
                continue;
            }
            if patterns.iter().any(|p| glob_match(p, &entry.key)) {
                group_entries.push(entry.clone());
                assigned.insert(i);
            }
        }
        if !group_entries.is_empty() {
            result.push(EnvGroup {
                name: name.clone(),
                entries: group_entries,
                is_user_defined: true,
            });
        }
    }

    let remaining: Vec<(usize, &EnvEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| !assigned.contains(i))
        .collect();

    let prefix_groups = detect_prefix_groups(&remaining);

    for (prefix, indices) in &prefix_groups {
        if indices.len() >= 2 {
            let group_entries: Vec<EnvEntry> =
                indices.iter().map(|&i| entries[i].clone()).collect();
            for &i in indices {
                assigned.insert(i);
            }
            result.push(EnvGroup {
                name: format!("{}*", prefix),
                entries: group_entries,
                is_user_defined: false,
            });
        }
    }

    let other_entries: Vec<EnvEntry> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| !assigned.contains(i))
        .map(|(_, e)| e.clone())
        .collect();

    if !other_entries.is_empty() {
        result.push(EnvGroup {
            name: "Other".to_string(),
            entries: other_entries,
            is_user_defined: false,
        });
    }

    result
}

/// Detect common prefixes from entries.
///
/// Splits keys by `_` and finds prefixes that have ≥2 entries.
fn detect_prefix_groups(entries: &[(usize, &EnvEntry)]) -> Vec<(String, Vec<usize>)> {
    let mut prefix_map: HashMap<String, Vec<usize>> = HashMap::new();

    for &(idx, entry) in entries {
        if let Some(prefix) = extract_prefix(&entry.key) {
            prefix_map.entry(prefix).or_default().push(idx);
        }
    }

    let mut groups: Vec<(String, Vec<usize>)> = prefix_map
        .into_iter()
        .filter(|(_, v)| v.len() >= 2)
        .collect();

    groups.sort_by(|a, b| a.0.cmp(&b.0));
    groups
}

/// Extract the prefix from a key (everything before the last `_` segment).
///
/// Example: `DATABASE_URL` → `DATABASE_`, `AWS_S3_BUCKET` → `AWS_`
fn extract_prefix(key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('_').collect();
    if parts.len() >= 2 {
        Some(format!("{}_", parts[0]))
    } else {
        None
    }
}

/// Simple glob matching: `*` matches any sequence of characters.
fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    if let Some(prefix) = pattern_lower.strip_suffix('*') {
        text_lower.starts_with(prefix)
    } else if let Some(suffix) = pattern_lower.strip_prefix('*') {
        text_lower.ends_with(suffix)
    } else {
        text_lower == pattern_lower
    }
}
