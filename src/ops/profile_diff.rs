use std::collections::HashMap;

use crate::config::AppConfig;
use crate::ops::collect_all_entries;
use crate::ops::listing::EnvEntry;
use crate::parser::parse_shell_file;

// ─── Types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DiffKind {
    /// Key exists only in profile A
    OnlyInA,
    /// Key exists only in profile B
    OnlyInB,
    /// Key exists in both but with different values
    Modified,
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub key: String,
    pub kind: DiffKind,
    pub value_a: Option<String>,
    pub value_b: Option<String>,
}

#[derive(Debug)]
pub struct ProfileDiffResult {
    pub profile_a: String,
    pub profile_b: String,
    pub entries: Vec<DiffEntry>,
}

impl ProfileDiffResult {
    pub fn only_in_a(&self) -> Vec<&DiffEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.kind, DiffKind::OnlyInA))
            .collect()
    }

    pub fn only_in_b(&self) -> Vec<&DiffEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.kind, DiffKind::OnlyInB))
            .collect()
    }

    pub fn modified(&self) -> Vec<&DiffEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.kind, DiffKind::Modified))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─── Diff Function ───────────────────────────────────────────

/// Compare environment variables between two profiles.
pub fn diff_profiles(
    config: &AppConfig,
    profile_a: &str,
    profile_b: &str,
) -> Result<ProfileDiffResult, String> {
    let entries_a = load_entries_for_profile(config, profile_a)?;
    let entries_b = load_entries_for_profile(config, profile_b)?;

    let map_a = to_key_map(&entries_a);
    let map_b = to_key_map(&entries_b);

    let mut diff_entries = Vec::new();

    // Keys only in A or modified
    for (key, value_a) in &map_a {
        match map_b.get(key) {
            Some(value_b) if value_a != value_b => {
                diff_entries.push(DiffEntry {
                    key: key.clone(),
                    kind: DiffKind::Modified,
                    value_a: Some(value_a.clone()),
                    value_b: Some(value_b.clone()),
                });
            }
            None => {
                diff_entries.push(DiffEntry {
                    key: key.clone(),
                    kind: DiffKind::OnlyInA,
                    value_a: Some(value_a.clone()),
                    value_b: None,
                });
            }
            _ => {} // Same value — skip
        }
    }

    // Keys only in B
    for (key, value_b) in &map_b {
        if !map_a.contains_key(key) {
            diff_entries.push(DiffEntry {
                key: key.clone(),
                kind: DiffKind::OnlyInB,
                value_a: None,
                value_b: Some(value_b.clone()),
            });
        }
    }

    diff_entries.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(ProfileDiffResult {
        profile_a: profile_a.to_string(),
        profile_b: profile_b.to_string(),
        entries: diff_entries,
    })
}

// ─── Helpers ─────────────────────────────────────────────────

fn load_entries_for_profile(
    config: &AppConfig,
    profile_name: &str,
) -> Result<Vec<EnvEntry>, String> {
    let profile = config.profiles.entries.get(profile_name).ok_or_else(|| {
        let available = config.profiles.profile_names().join(", ");
        format!(
            "profile '{}' not found. Available: {}",
            profile_name, available
        )
    })?;

    let mut shell_files = Vec::new();

    // Shared file (always included)
    let shared = shellexpand(&config.profiles.shared_file);
    if shared.exists() {
        if let Ok(sf) = parse_shell_file(&shared) {
            shell_files.push(sf);
        }
    }

    // Profile-specific file
    let profile_path = shellexpand(&profile.file);
    if profile_path.exists() {
        if let Ok(sf) = parse_shell_file(&profile_path) {
            shell_files.push(sf);
        }
    }

    Ok(collect_all_entries(&shell_files))
}

fn to_key_map(entries: &[EnvEntry]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in entries {
        if entry.location != crate::ops::EntryLocation::Commented {
            // Last value wins (profile overrides shared)
            map.insert(entry.key.clone(), entry.value.clone());
        }
    }
    map
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}
