use std::collections::HashMap;

use crate::model::ShellFile;
use crate::ops::crud::soft_delete;
use crate::ops::listing::{collect_all_entries, EntryLocation, EnvEntry};

/// A group of entries sharing the same key.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub key: String,
    pub entries: Vec<DuplicateEntry>,
}

/// A single entry within a duplicate group.
#[derive(Debug, Clone)]
pub struct DuplicateEntry {
    pub value: String,
    pub source_file: String,
    pub line_number: usize,
    pub file_index: usize,
}

/// How to resolve a duplicate.
#[derive(Debug, Clone)]
pub enum DuplicateResolution {
    /// Keep the entry at this index, delete others
    Keep(usize),
    /// Rename the entry at this index to a new key
    Rename(usize, String),
}

/// Detect duplicate keys across all shell files.
///
/// Returns groups where the same key appears more than once (active entries only).
pub fn detect_duplicates(shell_files: &[ShellFile]) -> Vec<DuplicateGroup> {
    let entries = collect_all_entries(shell_files);

    // Group by key (active entries only)
    let mut key_map: HashMap<String, Vec<(usize, &EnvEntry)>> = HashMap::new();

    for (i, entry) in entries.iter().enumerate() {
        if entry.location != EntryLocation::Commented {
            key_map
                .entry(entry.key.clone())
                .or_default()
                .push((i, entry));
        }
    }

    // Filter to only duplicates (>1 entry per key)
    let mut groups: Vec<DuplicateGroup> = key_map
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(key, entries_with_idx)| {
            let entries = entries_with_idx
                .iter()
                .map(|(_, entry)| {
                    let file_index = shell_files
                        .iter()
                        .position(|sf| sf.path == entry.source_file)
                        .unwrap_or(0);

                    DuplicateEntry {
                        value: entry.value.clone(),
                        source_file: entry
                            .source_file
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        line_number: entry.line_number,
                        file_index,
                    }
                })
                .collect();

            DuplicateGroup { key, entries }
        })
        .collect();

    groups.sort_by(|a, b| a.key.cmp(&b.key));
    groups
}

/// Resolve a duplicate by keeping one entry and soft-deleting the others.
///
/// `keep_index` is the index within `group.entries` to keep.
pub fn resolve_duplicate_keep(
    shell_files: &mut [ShellFile],
    group: &DuplicateGroup,
    keep_index: usize,
) -> Result<usize, String> {
    let mut deleted = 0;

    for (i, entry) in group.entries.iter().enumerate() {
        if i == keep_index {
            continue;
        }

        if let Some(sf) = shell_files.get_mut(entry.file_index) {
            if soft_delete(sf, &group.key).is_ok() {
                deleted += 1;
            }
        }
    }

    Ok(deleted)
}

/// Get a set of duplicate key names for quick lookup (used by TUI for highlighting).
pub fn duplicate_key_set(shell_files: &[ShellFile]) -> std::collections::HashSet<String> {
    detect_duplicates(shell_files)
        .into_iter()
        .map(|g| g.key)
        .collect()
}
