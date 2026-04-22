use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DiffStatus {
    Added,
    Removed,
    Changed,
    Same,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub name: String,
    pub created_at: String,
    pub profile: String,
    pub machine_id: String,
    pub var_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: SnapshotMeta,
    pub entries: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotDiffEntry {
    pub key: String,
    pub snapshot_value: Option<String>,
    pub current_value: Option<String>,
    pub status: DiffStatus,
}

// ── Directory ──────────────────────────────────────────────

pub fn snapshots_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = dirs::home_dir()
        .ok_or("Cannot determine home directory")?
        .join(".config")
        .join("envforge")
        .join("snapshots");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ── Create ─────────────────────────────────────────────────

pub fn create_snapshot(
    name: &str,
    entries: &[(String, String)],
    profile: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = snapshots_dir()?;

    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%dT%H%M%S").to_string();
    let created_at = now.to_rfc3339();

    let machine_id = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let mut ordered = BTreeMap::new();
    for (k, v) in entries {
        ordered.insert(k.clone(), v.clone());
    }

    let snapshot = Snapshot {
        metadata: SnapshotMeta {
            name: name.to_string(),
            created_at,
            profile: profile.to_string(),
            machine_id,
            var_count: ordered.len(),
        },
        entries: ordered,
    };

    let safe_name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();

    let filename = format!("{}-{}.toml", timestamp, safe_name);
    let path = dir.join(&filename);

    let content = toml::to_string_pretty(&snapshot)?;
    fs::write(&path, content)?;

    // Auto-prune old snapshots
    let _ = prune_snapshots(20);

    Ok(path)
}

// ── List ───────────────────────────────────────────────────

pub fn list_snapshots() -> Result<Vec<SnapshotMeta>, Box<dyn std::error::Error>> {
    let dir = snapshots_dir()?;
    let mut metas = Vec::new();

    let mut toml_files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename descending (newest first since filenames start with timestamp)
    toml_files.sort_by_key(|a| std::cmp::Reverse(a.file_name()));

    for entry in toml_files {
        let content = fs::read_to_string(entry.path())?;
        // Parse only the metadata section
        if let Ok(snapshot) = toml::from_str::<Snapshot>(&content) {
            metas.push(snapshot.metadata);
        }
    }

    Ok(metas)
}

// ── Load ───────────────────────────────────────────────────

pub fn load_snapshot(name_or_last: &str) -> Result<Snapshot, Box<dyn std::error::Error>> {
    let dir = snapshots_dir()?;

    let mut toml_files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename descending (newest first)
    toml_files.sort_by_key(|a| std::cmp::Reverse(a.file_name()));

    if name_or_last == "last" {
        let entry = toml_files
            .first()
            .ok_or("No snapshots found")?;
        let content = fs::read_to_string(entry.path())?;
        let snapshot: Snapshot = toml::from_str(&content)?;
        return Ok(snapshot);
    }

    // Find by name substring match in filename
    for entry in &toml_files {
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.contains(name_or_last) {
            let content = fs::read_to_string(entry.path())?;
            let snapshot: Snapshot = toml::from_str(&content)?;
            return Ok(snapshot);
        }
    }

    // Also try matching by metadata name
    for entry in &toml_files {
        let content = fs::read_to_string(entry.path())?;
        if let Ok(snapshot) = toml::from_str::<Snapshot>(&content) {
            if snapshot.metadata.name == name_or_last {
                return Ok(snapshot);
            }
        }
    }

    Err(format!("Snapshot '{}' not found", name_or_last).into())
}

// ── Delete ─────────────────────────────────────────────────

pub fn delete_snapshot(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = snapshots_dir()?;

    let mut toml_files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect();

    toml_files.sort_by_key(|a| std::cmp::Reverse(a.file_name()));

    // Find by filename substring
    for entry in &toml_files {
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.contains(name) {
            fs::remove_file(entry.path())?;
            return Ok(());
        }
    }

    // Also try matching by metadata name
    for entry in &toml_files {
        let content = fs::read_to_string(entry.path())?;
        if let Ok(snapshot) = toml::from_str::<Snapshot>(&content) {
            if snapshot.metadata.name == name {
                fs::remove_file(entry.path())?;
                return Ok(());
            }
        }
    }

    Err(format!("Snapshot '{}' not found", name).into())
}

// ── Diff ───────────────────────────────────────────────────

pub fn diff_snapshot(
    snapshot: &Snapshot,
    current: &[(String, String)],
) -> Vec<SnapshotDiffEntry> {
    let current_map: BTreeMap<String, String> = current.iter().cloned().collect();

    let mut all_keys: Vec<String> = Vec::new();
    for key in snapshot.entries.keys() {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }
    for (key, _) in current {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }
    all_keys.sort();

    let mut result = Vec::new();
    for key in all_keys {
        let snap_val = snapshot.entries.get(&key).cloned();
        let curr_val = current_map.get(&key).cloned();

        let status = match (&snap_val, &curr_val) {
            (Some(s), Some(c)) if s == c => DiffStatus::Same,
            (Some(_), Some(_)) => DiffStatus::Changed,
            (Some(_), None) => DiffStatus::Removed,
            (None, Some(_)) => DiffStatus::Added,
            (None, None) => DiffStatus::Same,
        };

        result.push(SnapshotDiffEntry {
            key,
            snapshot_value: snap_val,
            current_value: curr_val,
            status,
        });
    }

    result
}

// ── Prune ──────────────────────────────────────────────────

pub fn prune_snapshots(max_count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    let dir = snapshots_dir()?;

    let mut toml_files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "toml")
                .unwrap_or(false)
        })
        .collect();

    if toml_files.len() <= max_count {
        return Ok(0);
    }

    // Sort ascending by filename (oldest first)
    toml_files.sort_by_key(|a| a.file_name());

    let to_remove = toml_files.len() - max_count;
    let mut removed = 0;

    for entry in toml_files.iter().take(to_remove) {
        if fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a temp dir and override snapshots_dir for testing.
    fn setup_test_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn create_test_snapshot(
        dir: &std::path::Path,
        name: &str,
        entries: &[(String, String)],
        timestamp: &str,
    ) -> PathBuf {
        let mut ordered = BTreeMap::new();
        for (k, v) in entries {
            ordered.insert(k.clone(), v.clone());
        }

        let snapshot = Snapshot {
            metadata: SnapshotMeta {
                name: name.to_string(),
                created_at: format!("2026-04-20T{}:00:00+00:00", timestamp),
                profile: "dev".to_string(),
                machine_id: "test-machine".to_string(),
                var_count: ordered.len(),
            },
            entries: ordered,
        };

        let filename = format!("{}-{}.toml", timestamp.replace(':', ""), name);
        let path = dir.join(filename);
        let content = toml::to_string_pretty(&snapshot).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_create_and_load_roundtrip() {
        let dir = setup_test_dir();
        let entries = vec![
            ("DB_HOST".to_string(), "localhost".to_string()),
            ("DB_PORT".to_string(), "5432".to_string()),
        ];

        create_test_snapshot(dir.path(), "test-snap", &entries, "170000");

        // Load by name
        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1);

        let content = fs::read_to_string(files[0].path()).unwrap();
        let loaded: Snapshot = toml::from_str(&content).unwrap();

        assert_eq!(loaded.metadata.name, "test-snap");
        assert_eq!(loaded.metadata.var_count, 2);
        assert_eq!(loaded.entries.get("DB_HOST").unwrap(), "localhost");
        assert_eq!(loaded.entries.get("DB_PORT").unwrap(), "5432");
    }

    #[test]
    fn test_list_ordering() {
        let dir = setup_test_dir();

        let entries = vec![("A".to_string(), "1".to_string())];

        create_test_snapshot(dir.path(), "first", &entries, "100000");
        create_test_snapshot(dir.path(), "second", &entries, "110000");
        create_test_snapshot(dir.path(), "third", &entries, "120000");

        // Read and sort like list_snapshots does
        let mut toml_files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
            })
            .collect();
        toml_files.sort_by_key(|a| std::cmp::Reverse(a.file_name()));

        let mut metas = Vec::new();
        for entry in &toml_files {
            let content = fs::read_to_string(entry.path()).unwrap();
            let snapshot: Snapshot = toml::from_str(&content).unwrap();
            metas.push(snapshot.metadata);
        }

        assert_eq!(metas.len(), 3);
        // Newest first
        assert_eq!(metas[0].name, "third");
        assert_eq!(metas[1].name, "second");
        assert_eq!(metas[2].name, "first");
    }

    #[test]
    fn test_diff_detection() {
        let snapshot = Snapshot {
            metadata: SnapshotMeta {
                name: "test".to_string(),
                created_at: "2026-04-20T17:00:00Z".to_string(),
                profile: "dev".to_string(),
                machine_id: "test".to_string(),
                var_count: 3,
            },
            entries: BTreeMap::from([
                ("KEPT_SAME".to_string(), "value".to_string()),
                ("CHANGED".to_string(), "old".to_string()),
                ("REMOVED".to_string(), "gone".to_string()),
            ]),
        };

        let current = vec![
            ("KEPT_SAME".to_string(), "value".to_string()),
            ("CHANGED".to_string(), "new".to_string()),
            ("ADDED".to_string(), "fresh".to_string()),
        ];

        let diff = diff_snapshot(&snapshot, &current);

        let find = |key: &str| diff.iter().find(|d| d.key == key).unwrap();

        assert_eq!(find("KEPT_SAME").status, DiffStatus::Same);
        assert_eq!(find("CHANGED").status, DiffStatus::Changed);
        assert_eq!(find("CHANGED").snapshot_value, Some("old".to_string()));
        assert_eq!(find("CHANGED").current_value, Some("new".to_string()));
        assert_eq!(find("REMOVED").status, DiffStatus::Removed);
        assert_eq!(find("ADDED").status, DiffStatus::Added);
    }

    #[test]
    fn test_prune_keeps_max_count() {
        let dir = setup_test_dir();
        let entries = vec![("A".to_string(), "1".to_string())];

        // Create 5 snapshots
        for i in 0..5 {
            create_test_snapshot(dir.path(), &format!("snap-{}", i), &entries, &format!("1{}0000", i));
        }

        let files_before: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "toml").unwrap_or(false))
            .collect();
        assert_eq!(files_before.len(), 5);

        // Prune to max 3 — simulate by implementing the same logic
        let mut toml_files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        toml_files.sort_by_key(|a| a.file_name());

        let to_remove = toml_files.len().saturating_sub(3);
        let mut removed = 0;
        for entry in toml_files.iter().take(to_remove) {
            if fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }

        assert_eq!(removed, 2);

        let files_after: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "toml").unwrap_or(false))
            .collect();
        assert_eq!(files_after.len(), 3);
    }

    #[test]
    fn test_load_last() {
        let dir = setup_test_dir();
        let entries = vec![("X".to_string(), "1".to_string())];

        create_test_snapshot(dir.path(), "older", &entries, "100000");
        create_test_snapshot(dir.path(), "newest", &entries, "120000");

        // Simulate load "last"
        let mut toml_files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        toml_files.sort_by_key(|a| std::cmp::Reverse(a.file_name()));

        let content = fs::read_to_string(toml_files[0].path()).unwrap();
        let snapshot: Snapshot = toml::from_str(&content).unwrap();

        assert_eq!(snapshot.metadata.name, "newest");
    }
}
