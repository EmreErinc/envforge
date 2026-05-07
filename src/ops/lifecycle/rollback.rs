use std::fs;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

use crate::model::{OperationType, RollbackResult, Snapshot, SnapshotMeta};
use crate::ops::OpError;

fn snapshots_dir() -> Result<PathBuf, OpError> {
    let dir = crate::config::config_dir()?.join("lifecycle/snapshots");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ─── Create Snapshot ────────────────────────────────────

/// Create a snapshot of a secret's current state before an operation.
pub fn create_snapshot(
    key: &str,
    operation_type: &OperationType,
    value: Option<&str>,
) -> Result<SnapshotMeta, OpError> {
    use chrono::Utc;

    let config = crate::config::load_or_create_default()?;
    let shell_path = std::path::PathBuf::from(&config.files.primary);
    let source_hash = if shell_path.exists() {
        let content = fs::read_to_string(&shell_path)?;
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(content.as_bytes());
        Some(hex::encode(digest))
    } else {
        None
    };

    let state = crate::ops::lifecycle::orchestrator::get_state(key)?;

    let masked_value = value.map_or_else(String::new, mask_value);

    let meta = SnapshotMeta {
        id: Uuid::new_v4(),
        key: key.to_string(),
        masked_value,
        source_file: Some(shell_path),
        source_hash,
        state,
        operation_type: operation_type.clone(),
        timestamp: Utc::now(),
    };

    let snapshot = Snapshot {
        meta: meta.clone(),
        value: value.map(String::from),
    };

    let dir = snapshots_dir()?;
    let path = dir.join(format!("{}.jsonl", meta.id));

    let content = serde_json::to_string(&snapshot)?;
    write_atomic_snapshot(&path, &content)?;

    Ok(meta)
}

// ─── Rollback ────────────────────────────────────────────

/// Restore a secret from a snapshot.
pub fn rollback(snapshot_id: &Uuid) -> Result<RollbackResult, OpError> {
    let dir = snapshots_dir()?;
    let path = dir.join(format!("{snapshot_id}.jsonl"));

    if !path.exists() {
        return Err(OpError::Other(format!("snapshot not found: {snapshot_id}")));
    }

    let content = fs::read_to_string(&path)?;
    let snapshot: Snapshot = serde_json::from_str(&content).map_err(OpError::Json)?;
    let key = snapshot.meta.key.clone();

    // Restore value if available
    if let Some(ref value) = snapshot.value {
        let config = crate::config::load_or_create_default()?;
        let shell_path = std::path::PathBuf::from(&config.files.primary);

        if shell_path.exists() {
            let mut shell_file = crate::parser::parse_shell_file(&shell_path)?;
            crate::ops::crud::edit_entry(&mut shell_file, &key, value)?;
            let output = crate::parser::serialize_shell_file(&shell_file);
            crate::config::safe_write(&shell_path, &output, None)?;
        }
    }

    Ok(RollbackResult {
        key,
        success: true,
        snapshot_id: *snapshot_id,
    })
}

// ─── List / Delete ──────────────────────────────────────

/// List all snapshots, optionally filtered by key.
pub fn list_snapshots(key_filter: Option<&str>) -> Result<Vec<SnapshotMeta>, OpError> {
    let dir = snapshots_dir()?;
    let mut metas = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(metas),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&content) {
                    if let Some(filter) = key_filter {
                        if snapshot.meta.key != filter {
                            continue;
                        }
                    }
                    metas.push(snapshot.meta);
                }
            }
        }
    }

    metas.sort_by_key(|m| m.timestamp);
    Ok(metas)
}

/// Delete a snapshot by ID.
pub fn delete_snapshot(snapshot_id: &Uuid) -> Result<(), OpError> {
    let dir = snapshots_dir()?;
    let path = dir.join(format!("{snapshot_id}.jsonl"));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

// ─── Helpers ────────────────────────────────────────────

fn mask_value(value: &str) -> String {
    if value.len() < 6 {
        return "****".to_string();
    }
    format!("{}****{}", &value[..2], &value[value.len() - 2..])
}

fn write_atomic_snapshot(path: &std::path::Path, content: &str) -> Result<(), OpError> {
    use tempfile::NamedTempFile;
    let parent = path
        .parent()
        .ok_or_else(|| OpError::Other("invalid snapshot path".into()))?;
    let mut tmp = NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.persist(path)
        .map_err(|e| OpError::Other(e.to_string()))?;
    Ok(())
}
