use std::collections::HashMap;

use crate::model::ShellFile;
use crate::ops::crud::{rename_entry_at, soft_delete_at};
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
    pub line_index: usize,
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
                        line_index: entry.line_index,
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
            if soft_delete_at(sf, entry.line_index).is_ok() {
                deleted += 1;
            }
        }
    }

    Ok(deleted)
}

/// Resolve a duplicate by renaming one entry to a new key.
pub fn resolve_duplicate_rename(
    shell_files: &mut [ShellFile],
    group: &DuplicateGroup,
    rename_index: usize,
    new_key: &str,
) -> Result<(), String> {
    let entry = group
        .entries
        .get(rename_index)
        .ok_or_else(|| "Invalid rename index".to_string())?;

    if let Some(sf) = shell_files.get_mut(entry.file_index) {
        rename_entry_at(sf, entry.line_index, new_key).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Get a set of duplicate key names for quick lookup (used by TUI for highlighting).
pub fn duplicate_key_set(shell_files: &[ShellFile]) -> std::collections::HashSet<String> {
    detect_duplicates(shell_files)
        .into_iter()
        .map(|g| g.key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_shell_content;
    use std::path::Path;

    fn make_shell_file_at(content: &str, path: &str) -> ShellFile {
        parse_shell_content(content, Path::new(path)).unwrap()
    }

    #[test]
    fn test_detect_duplicates_same_file() {
        let sf = make_shell_file_at("export FOO=\"a\"\nexport FOO=\"b\"", "/test/.zshrc");
        let groups = detect_duplicates(&[sf]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "FOO");
        assert_eq!(groups[0].entries.len(), 2);
    }

    #[test]
    fn test_detect_duplicates_across_files() {
        let sf1 = make_shell_file_at("export API_KEY=\"val1\"", "/test/.zshrc");
        let sf2 = make_shell_file_at("export API_KEY=\"val2\"", "/test/.bashrc");
        let groups = detect_duplicates(&[sf1, sf2]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "API_KEY");
    }

    #[test]
    fn test_detect_duplicates_no_dupes() {
        let sf = make_shell_file_at("export A=\"1\"\nexport B=\"2\"", "/test/.zshrc");
        let groups = detect_duplicates(&[sf]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_detect_duplicates_ignores_commented() {
        let sf = make_shell_file_at(
            "export FOO=\"active\"\n#[envforge:deleted:FOO] export FOO=\"old\"",
            "/test/.zshrc",
        );
        let groups = detect_duplicates(&[sf]);
        assert!(
            groups.is_empty(),
            "Soft-deleted entry should not count as duplicate"
        );
    }

    #[test]
    fn test_detect_duplicates_sorted_by_key() {
        let sf = make_shell_file_at(
            "export ZZZ=\"1\"\nexport ZZZ=\"2\"\nexport AAA=\"1\"\nexport AAA=\"2\"",
            "/test/.zshrc",
        );
        let groups = detect_duplicates(&[sf]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].key, "AAA");
        assert_eq!(groups[1].key, "ZZZ");
    }

    #[test]
    fn test_resolve_duplicate_keep_across_files() {
        // Duplicates across different files — soft_delete works since each file has one copy
        let sf1 = make_shell_file_at("export DUP=\"a\"", "/f1/.zshrc");
        let sf2 = make_shell_file_at("export DUP=\"b\"", "/f2/.zshrc");
        let groups = detect_duplicates(&[sf1, sf2]);
        assert_eq!(groups.len(), 1);

        let mut files = vec![
            make_shell_file_at("export DUP=\"a\"", "/f1/.zshrc"),
            make_shell_file_at("export DUP=\"b\"", "/f2/.zshrc"),
        ];
        // Keep entry 0 (from file 0), delete entry 1 (from file 1)
        let deleted = resolve_duplicate_keep(&mut files, &groups[0], 0).unwrap();
        assert_eq!(deleted, 1);
    }

    #[test]
    fn test_duplicate_key_set() {
        let sf = make_shell_file_at("export X=\"1\"\nexport X=\"2\"\nexport Y=\"3\"", "/t/.z");
        let set = duplicate_key_set(&[sf]);
        assert!(set.contains("X"));
        assert!(!set.contains("Y"));
    }
}
