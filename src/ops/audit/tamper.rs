//! Tamper-evident writer for the AI audit trail.
//!
//! Provides hash chain computation, integrity verification, and tamper
//! detection for audit log files. Each event's `entry_hash` is computed
//! from its content plus the previous event's hash, forming an unbroken
//! chain that detects any modification.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::emitter::{self, EmitResult, EmitterConfig, LogCategory};
use super::types::{AuditEvent, EventId};

// ─── Chain State ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainState {
    /// Map from log file path to its last entry hash.
    pub last_hashes: HashMap<String, String>,
    /// Map from log file path to its last event ID.
    pub chain_ids: HashMap<String, String>,
}

impl ChainState {
    pub fn new() -> Self {
        Self {
            last_hashes: HashMap::new(),
            chain_ids: HashMap::new(),
        }
    }

    pub fn get_last_hash(&self, filename: &str) -> Option<&String> {
        self.last_hashes.get(filename)
    }

    pub fn update(&mut self, filename: String, event_id: String, hash: String) {
        self.last_hashes.insert(filename.clone(), hash);
        self.chain_ids.insert(filename, event_id);
    }
}

impl Default for ChainState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Chain Link ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ChainLink {
    pub event_id: EventId,
    pub entry_hash: String,
    pub prev_hash: Option<String>,
}

// ─── Integrity Result ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityResult {
    pub file_path: PathBuf,
    pub total_events: u64,
    pub valid: bool,
    pub breaks: Vec<ChainBreak>,
    pub verified_at: chrono::DateTime<Utc>,
}

// ─── Chain Break ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBreak {
    pub line_number: u64,
    pub expected_hash: String,
    pub actual_hash: String,
    pub event_id: Option<String>,
}

// ─── Tamper Alert ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TamperAlert {
    pub break_info: ChainBreak,
    pub severity: AlertSeverity,
    pub file_path: PathBuf,
    pub message: String,
    pub detected_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Critical,
    Warning,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::Warning => write!(f, "WARNING"),
        }
    }
}

// ─── Tamper Error ────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TamperError {
    #[error(
        "hash chain broken at line {line_number} in {path}: expected {expected}, got {actual}"
    )]
    ChainBroken {
        path: PathBuf,
        line_number: u64,
        expected: String,
        actual: String,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error in {path} at line {line}: {source}")]
    JsonParse {
        path: PathBuf,
        line: u64,
        source: serde_json::Error,
    },

    #[error("emitter error: {0}")]
    Emitter(#[from] emitter::EmitterError),

    #[error("invalid chain state: {0}")]
    InvalidState(String),

    #[error("hash computation failed: {0}")]
    HashFailed(String),
}

// ─── Persistent Chain State ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct ChainStateFile {
    version: u32,
    chains: HashMap<String, ChainEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainEntry {
    last_hash: String,
    last_event_id: String,
}

const CHAIN_STATE_FILENAME: &str = ".chain-state.json";

/// Return true if `log_dir` contains at least one file that looks like an
/// audit log (any non-hidden file other than the chain state itself).
/// Used by [`load_chain_state`] to distinguish "first run, never written"
/// from "state file was deleted to cover tampering".
fn log_dir_has_log_files(log_dir: &Path) -> bool {
    let read = match std::fs::read_dir(log_dir) {
        Ok(r) => r,
        Err(_) => return false,
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if entry.path().is_file() {
            return true;
        }
    }
    false
}

/// Load chain state from disk.
///
/// Detects tampering: if the state file is missing but `log_dir` already
/// contains log files, the state was deleted to mask earlier tampering.
/// Without this check, a `.chain-state.json` deleted by an attacker would
/// silently re-initialize the chain ("fresh chain" instead of "broken
/// chain"), defeating the tamper-evident property.
pub fn load_chain_state(log_dir: &Path) -> Result<ChainState, TamperError> {
    let state_path = log_dir.join(CHAIN_STATE_FILENAME);

    if !state_path.exists() {
        if log_dir_has_log_files(log_dir) {
            return Err(TamperError::InvalidState(format!(
                "chain state file missing at {} but audit logs exist; possible tampering. \
                 Refusing to silently re-initialize chain. To accept the loss, \
                 manually rotate logs or restore from backup.",
                state_path.display()
            )));
        }
        return Ok(ChainState::new());
    }

    let content = std::fs::read_to_string(&state_path)?;
    let file: ChainStateFile =
        serde_json::from_str(&content).map_err(|e| TamperError::JsonParse {
            path: state_path.clone(),
            line: 0,
            source: e,
        })?;

    if file.version != 1 {
        return Err(TamperError::InvalidState(format!(
            "unsupported chain state version: {}",
            file.version
        )));
    }

    let mut state = ChainState::new();
    for (filename, entry) in file.chains {
        state.update(filename, entry.last_event_id, entry.last_hash);
    }

    Ok(state)
}

/// Save chain state to disk.
pub fn save_chain_state(state: &ChainState, log_dir: &Path) -> Result<(), TamperError> {
    let mut chains = HashMap::new();
    for (filename, hash) in &state.last_hashes {
        if let Some(event_id) = state.chain_ids.get(filename) {
            chains.insert(
                filename.clone(),
                ChainEntry {
                    last_hash: hash.clone(),
                    last_event_id: event_id.clone(),
                },
            );
        }
    }

    let file = ChainStateFile { version: 1, chains };

    let content =
        serde_json::to_string_pretty(&file).map_err(|e| TamperError::HashFailed(e.to_string()))?;

    let state_path = log_dir.join(CHAIN_STATE_FILENAME);
    crate::config::atomic_write(&state_path, &content, None)
        .map_err(|e| TamperError::HashFailed(e.to_string()))?;

    Ok(())
}

// ─── Core Functions ──────────────────────────────────────────────

/// Write an audit event with tamper-evident hash chain.
///
/// Computes the entry hash, sets prev_hash from chain state,
/// then delegates to `emitter::emit()` for file writing.
pub fn write_tamper_evident(
    mut event: AuditEvent,
    config: &EmitterConfig,
    state: &mut ChainState,
) -> Result<EmitResult, TamperError> {
    emitter::ensure_log_dir(config)?;

    let category = if config.separate_logs {
        LogCategory::from_source(&event.source)
    } else {
        LogCategory::General
    };

    let filename = category.filename().to_string();
    let prev_hash = state.get_last_hash(&filename).cloned();
    event.prev_hash = prev_hash;

    let entry_hash = event.compute_entry_hash();
    event.entry_hash = entry_hash.clone();

    let result = emitter::emit_to(event, category, config)?;

    state.update(filename, result.event_id.to_string(), entry_hash);

    Ok(result)
}

/// Verify the integrity of an entire log file.
///
/// Reads each line, recomputes hash chains, and detects any breaks.
pub fn verify_integrity(path: &Path) -> Result<IntegrityResult, TamperError> {
    if !path.exists() {
        return Ok(IntegrityResult {
            file_path: path.to_path_buf(),
            total_events: 0,
            valid: true,
            breaks: Vec::new(),
            verified_at: Utc::now(),
        });
    }

    let content = std::fs::read_to_string(path)?;
    let mut breaks = Vec::new();
    let mut total_events: u64 = 0;
    let mut prev_hash: Option<String> = None;

    for (line_num, line) in content.lines().enumerate() {
        let line_num_u64 = (line_num + 1) as u64;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let event: AuditEvent = serde_json::from_str(line).map_err(|e| TamperError::JsonParse {
            path: path.to_path_buf(),
            line: line_num_u64,
            source: e,
        })?;

        total_events += 1;

        if let Some(ref expected_prev) = prev_hash {
            if event.prev_hash.as_deref() != Some(expected_prev) {
                breaks.push(ChainBreak {
                    line_number: line_num_u64,
                    expected_hash: expected_prev.clone(),
                    actual_hash: event.prev_hash.clone().unwrap_or_default(),
                    event_id: Some(event.id.to_string()),
                });
            }
        } else if event.prev_hash.is_some() {
            breaks.push(ChainBreak {
                line_number: line_num_u64,
                expected_hash: String::new(),
                actual_hash: event.prev_hash.clone().unwrap_or_default(),
                event_id: Some(event.id.to_string()),
            });
        }

        let expected_hash = event.compute_entry_hash();
        if event.entry_hash != expected_hash {
            breaks.push(ChainBreak {
                line_number: line_num_u64,
                expected_hash,
                actual_hash: event.entry_hash.clone(),
                event_id: Some(event.id.to_string()),
            });
        }

        prev_hash = Some(event.entry_hash.clone());
    }

    let valid = breaks.is_empty();

    Ok(IntegrityResult {
        file_path: path.to_path_buf(),
        total_events,
        valid,
        breaks,
        verified_at: Utc::now(),
    })
}

/// Check a log file for tampering and generate alerts for any breaks found.
pub fn check_tamper(path: &Path) -> Result<Vec<TamperAlert>, TamperError> {
    let result = verify_integrity(path)?;

    let alerts: Vec<TamperAlert> = result
        .breaks
        .into_iter()
        .map(|chain_break| {
            let message = format!(
                "Tamper detected at line {} in {}: expected hash {}, got {}",
                chain_break.line_number,
                path.display(),
                chain_break.expected_hash,
                chain_break.actual_hash,
            );
            TamperAlert {
                severity: AlertSeverity::Critical,
                file_path: result.file_path.clone(),
                detected_at: result.verified_at,
                break_info: chain_break,
                message,
            }
        })
        .collect();

    Ok(alerts)
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::audit::types::{EventResult, EventSource, EventType};

    fn test_config(dir: &Path) -> EmitterConfig {
        EmitterConfig::new(dir.to_path_buf())
    }

    fn test_event() -> AuditEvent {
        AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        )
    }

    // ─── ChainState Tests ──────────────────────────────────────

    #[test]
    fn test_chain_state_new() {
        let state = ChainState::new();
        assert!(state.last_hashes.is_empty());
        assert!(state.chain_ids.is_empty());
    }

    #[test]
    fn test_chain_state_update() {
        let mut state = ChainState::new();
        state.update(
            "ai-guard.jsonl".to_string(),
            "id-1".to_string(),
            "hash-1".to_string(),
        );
        assert_eq!(
            state.get_last_hash("ai-guard.jsonl"),
            Some(&"hash-1".to_string())
        );
        assert!(state.get_last_hash("proxy.jsonl").is_none());
    }

    #[test]
    fn test_chain_state_default() {
        let state = ChainState::default();
        assert!(state.last_hashes.is_empty());
    }

    // ─── Load/Save Chain State ──────────────────────────────────

    #[test]
    fn test_load_chain_state_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let state = load_chain_state(dir.path()).unwrap();
        assert!(state.last_hashes.is_empty());
    }

    #[test]
    fn test_save_and_load_chain_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = ChainState::new();
        state.update(
            "ai-guard.jsonl".to_string(),
            "event-uuid-1".to_string(),
            "hash-1".to_string(),
        );

        save_chain_state(&state, dir.path()).unwrap();
        let loaded = load_chain_state(dir.path()).unwrap();
        assert_eq!(
            loaded.get_last_hash("ai-guard.jsonl"),
            Some(&"hash-1".to_string())
        );
    }

    // ─── Write Tamper Evident ───────────────────────────────────

    #[test]
    fn test_write_tamper_evident_first_event() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event = test_event();
        let result = write_tamper_evident(event, &config, &mut state).unwrap();

        assert_eq!(result.line_number, 1);
        assert!(result.path.exists());
        assert!(state.get_last_hash("ai-guard.jsonl").is_some());

        //prev_hash should be None for first event
        let content = std::fs::read_to_string(&result.path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert!(parsed["prev_hash"].is_null());
    }

    #[test]
    fn test_write_tamper_evident_chains_events() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event1 = test_event();
        let _result1 = write_tamper_evident(event1, &config, &mut state).unwrap();

        let event2 = test_event();
        let result2 = write_tamper_evident(event2, &config, &mut state).unwrap();

        // Second event should have prev_hash equal to first event's entry_hash
        let content = std::fs::read_to_string(&result2.path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let first_hash = first["entry_hash"].as_str().unwrap();

        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["prev_hash"].as_str().unwrap(), first_hash);
    }

    #[test]
    fn test_write_tamper_evident_different_categories() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event1 = AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        );
        let event2 = AuditEvent::new(EventType::SyncPush, EventSource::Sync, EventResult::Success);

        write_tamper_evident(event1, &config, &mut state).unwrap();
        write_tamper_evident(event2, &config, &mut state).unwrap();

        // ai-guard.jsonl should have its own chain
        assert!(state.get_last_hash("ai-guard.jsonl").is_some());
        assert!(state.get_last_hash("sync.jsonl").is_some());
    }

    // ─── Verify Integrity ────────────────────────────────────────

    #[test]
    fn test_verify_integrity_nonexistent_file() {
        let result = verify_integrity(Path::new("/tmp/nonexistent_test_file.jsonl")).unwrap();
        assert!(result.valid);
        assert_eq!(result.total_events, 0);
        assert!(result.breaks.is_empty());
    }

    #[test]
    fn test_verify_integrity_valid_chain() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event = test_event();
        let result = write_tamper_evident(event, &config, &mut state).unwrap();

        let integrity = verify_integrity(&result.path).unwrap();
        assert!(integrity.valid);
        assert_eq!(integrity.total_events, 1);
        assert!(integrity.breaks.is_empty());
    }

    #[test]
    fn test_verify_integrity_tampered_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event = test_event();
        let result = write_tamper_evident(event, &config, &mut state).unwrap();

        // Tamper with the entry_hash to break the chain
        let content = std::fs::read_to_string(&result.path).unwrap();
        let tampered = content.replace("\"entry_hash\":\"", "\"entry_hash\":\"TAMPERED");
        std::fs::write(&result.path, tampered).unwrap();

        let integrity = verify_integrity(&result.path).unwrap();
        assert!(!integrity.valid);
        assert!(!integrity.breaks.is_empty());
    }

    #[test]
    fn test_check_tamper_valid_log() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event = test_event();
        let result = write_tamper_evident(event, &config, &mut state).unwrap();

        let alerts = check_tamper(&result.path).unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_check_tamper_generates_alert() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let mut state = ChainState::new();

        let event = test_event();
        let result = write_tamper_evident(event, &config, &mut state).unwrap();

        // Tamper with the entry_hash
        let content = std::fs::read_to_string(&result.path).unwrap();
        let tampered = content.replace("\"entry_hash\":\"", "\"entry_hash\":\"TAMPERED");
        std::fs::write(&result.path, tampered).unwrap();

        let alerts = check_tamper(&result.path).unwrap();
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert!(!alerts[0].message.is_empty());
    }

    #[test]
    fn test_alert_severity_display() {
        assert_eq!(AlertSeverity::Critical.to_string(), "CRITICAL");
        assert_eq!(AlertSeverity::Warning.to_string(), "WARNING");
    }
}
