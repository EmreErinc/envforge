use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde_json;

use crate::config::config_dir;
use crate::model::{AnalyticsConfig, AnalyticsError, EnrichedAccessEvent};

const EVENTS_FILE: &str = "events.jsonl";

/// Get the analytics directory path.
fn analytics_dir() -> Result<PathBuf, AnalyticsError> {
    let dir = config_dir().map_err(|e| AnalyticsError::StorageError {
        path: PathBuf::from("config_dir"),
        source: std::io::Error::other(e.to_string()),
    })?;
    let analytics = dir.join("analytics");
    Ok(analytics)
}

/// Ensure the analytics directory exists with correct permissions (0700).
fn ensure_analytics_dir() -> Result<PathBuf, AnalyticsError> {
    let dir = analytics_dir()?;
    fs::create_dir_all(&dir).map_err(|e| AnalyticsError::StorageError {
        path: dir.clone(),
        source: e,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).ok();
    }

    Ok(dir)
}

/// Get the events file path.
fn events_file_path() -> Result<PathBuf, AnalyticsError> {
    let dir = ensure_analytics_dir()?;
    Ok(dir.join(EVENTS_FILE))
}

/// Save events as append-only JSONL. Auto-rotates at max_events.
pub fn save_events(
    events: &[EnrichedAccessEvent],
    config: &AnalyticsConfig,
) -> Result<(), AnalyticsError> {
    if events.is_empty() {
        return Ok(());
    }

    let path = events_file_path()?;

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok();
    }

    let mut writer = std::io::BufWriter::new(file);

    for event in events {
        let json = serde_json::to_string(event).map_err(|e| AnalyticsError::EventParseError {
            source: e,
        })?;
        writeln!(writer, "{}", json).map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        })?;
    }

    writer
        .flush()
        .map_err(|e| AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        })?;

    // Auto-rotate if needed
    rotate_events_file(&path, config.max_events)?;

    Ok(())
}

/// Load all events from events.jsonl.
pub fn load_events() -> Result<Vec<EnrichedAccessEvent>, AnalyticsError> {
    let path = events_file_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&path).map_err(|e| AnalyticsError::StorageError {
        path: path.clone(),
        source: e,
    })?;

    let events: Vec<EnrichedAccessEvent> = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(events)
}

/// Rotate the events file, keeping only the last N entries.
/// Uses atomic write (tempfile + rename) to ensure log integrity.
fn rotate_events_file(path: &PathBuf, max_entries: usize) -> Result<(), AnalyticsError> {
    if max_entries == 0 || !path.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(path).map_err(|e| AnalyticsError::StorageError {
        path: path.clone(),
        source: e,
    })?;

    let lines: Vec<&str> = contents.lines().collect();

    if lines.len() <= max_entries {
        return Ok(());
    }

    // Keep only the last max_entries lines
    let keep = &lines[lines.len() - max_entries..];
    let new_contents = keep.join("\n") + "\n";

    // Atomic write via tempfile + rename
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        }
    })?;

    Write::write_all(&mut tmp, new_contents.as_bytes()).map_err(|e| {
        AnalyticsError::StorageError {
            path: path.clone(),
            source: e,
        }
    })?;

    tmp.persist(path).map_err(|e| AnalyticsError::StorageError {
        path: path.clone(),
        source: e.error,
    })?;

    Ok(())
}
