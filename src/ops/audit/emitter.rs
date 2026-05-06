//! Event emitter for the AI audit trail.
//!
//! Emits [`AuditEvent`] instances to persistent JSONL log files with
//! optional per-source separation, enrichment, and atomic writes.
//!
//! # Usage
//!
//! ```no_run
//! use envforge::ops::audit::emitter::{
//!     emit, EmitterConfig, EnrichmentConfig,
//! };
//! use envforge::ops::audit::types::{AuditEvent, EventType, EventSource, EventResult};
//! use std::path::PathBuf;
//!
//! let config = EmitterConfig::new(PathBuf::from("/tmp/audit"));
//! let event = AuditEvent::new(
//!     EventType::SecretAccessed,
//!     EventSource::AiGuard,
//!     EventResult::Success,
//! );
//! let result = emit(event, &config)?;
//! println!("Wrote event to {} at line {}", result.path.display(), result.line_number);
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use thiserror::Error;

use super::types::{AuditEvent, EventId, EventSource};

// ─── Log Category ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogCategory {
    AiGuard,
    Proxy,
    Sync,
    Cli,
    Tui,
    Hook,
    General,
}

impl LogCategory {
    #[must_use]
    pub fn from_source(source: &EventSource) -> Self {
        match source {
            EventSource::AiGuard => Self::AiGuard,
            EventSource::Proxy => Self::Proxy,
            EventSource::Sync => Self::Sync,
            EventSource::Cli => Self::Cli,
            EventSource::Tui => Self::Tui,
            EventSource::Hook => Self::Hook,
        }
    }

    #[must_use]
    pub fn filename(&self) -> &str {
        match self {
            Self::AiGuard => "ai-guard.jsonl",
            Self::Proxy => "proxy.jsonl",
            Self::Sync => "sync.jsonl",
            Self::Cli => "cli.jsonl",
            Self::Tui => "tui.jsonl",
            Self::Hook => "hook.jsonl",
            Self::General => "audit.jsonl",
        }
    }

    #[must_use]
    pub fn all() -> &'static [LogCategory] {
        &[
            Self::AiGuard,
            Self::Proxy,
            Self::Sync,
            Self::Cli,
            Self::Tui,
            Self::Hook,
            Self::General,
        ]
    }
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.filename().trim_end_matches(".jsonl"))
    }
}

// ─── Enrichment Config ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnrichmentConfig {
    pub add_hostname: bool,
    pub add_process_id: bool,
    pub add_timestamp_iso: bool,
    pub add_environment: bool,
    pub custom_fields: HashMap<String, String>,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            add_hostname: true,
            add_process_id: true,
            add_timestamp_iso: true,
            add_environment: false,
            custom_fields: HashMap::new(),
        }
    }
}

impl EnrichmentConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_hostname(mut self, enabled: bool) -> Self {
        self.add_hostname = enabled;
        self
    }

    #[must_use]
    pub fn with_process_id(mut self, enabled: bool) -> Self {
        self.add_process_id = enabled;
        self
    }

    #[must_use]
    pub fn with_timestamp_iso(mut self, enabled: bool) -> Self {
        self.add_timestamp_iso = enabled;
        self
    }

    #[must_use]
    pub fn with_environment(mut self, enabled: bool) -> Self {
        self.add_environment = enabled;
        self
    }

    #[must_use]
    pub fn with_custom_field(mut self, key: String, value: String) -> Self {
        self.custom_fields.insert(key, value);
        self
    }

    #[must_use]
    pub fn with_custom_fields(mut self, fields: HashMap<String, String>) -> Self {
        self.custom_fields = fields;
        self
    }
}

// ─── Emitter Config ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmitterConfig {
    pub log_dir: PathBuf,
    pub separate_logs: bool,
    pub enrichment: EnrichmentConfig,
    pub atomic_writes: bool,
}

impl EmitterConfig {
    #[must_use]
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            separate_logs: true,
            enrichment: EnrichmentConfig::default(),
            atomic_writes: true,
        }
    }

    #[must_use]
    pub fn with_separate_logs(mut self, separate: bool) -> Self {
        self.separate_logs = separate;
        self
    }

    #[must_use]
    pub fn with_enrichment(mut self, config: EnrichmentConfig) -> Self {
        self.enrichment = config;
        self
    }

    #[must_use]
    pub fn with_atomic_writes(mut self, atomic: bool) -> Self {
        self.atomic_writes = atomic;
        self
    }

    pub fn validate(&self) -> Result<(), EmitterError> {
        if self.log_dir.as_os_str().is_empty() {
            return Err(EmitterError::InvalidConfig(
                "log_dir path cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

// ─── Emit Result ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmitResult {
    pub event_id: EventId,
    pub category: LogCategory,
    pub path: PathBuf,
    pub line_number: u64,
    pub bytes_written: usize,
}

// ─── Emitter Error ─────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum EmitterError {
    #[error("log directory not found: {0}")]
    LogDirNotFound(PathBuf),

    #[error("log directory not writable: {0}")]
    LogDirNotWritable(PathBuf),

    #[error("failed to create log directory {path}: {source}")]
    CreateDirFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write event to {path}: {source}")]
    WriteFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to serialize event: {0}")]
    SerializeFailed(#[from] serde_json::Error),

    #[error("invalid emitter config: {0}")]
    InvalidConfig(String),
}

// ─── Core Functions ─────────────────────────────────────────────────

/// Get the log file path for a given category.
#[must_use]
pub fn log_path(config: &EmitterConfig, category: &LogCategory) -> PathBuf {
    if config.separate_logs {
        config.log_dir.join(category.filename())
    } else {
        config.log_dir.join(LogCategory::General.filename())
    }
}

/// Ensure the log directory exists, creating it if needed.
pub fn ensure_log_dir(config: &EmitterConfig) -> Result<(), EmitterError> {
    config.validate()?;

    if !config.log_dir.exists() {
        std::fs::create_dir_all(&config.log_dir).map_err(|e| EmitterError::CreateDirFailed {
            path: config.log_dir.clone(),
            source: e,
        })?;
    }

    if config.log_dir.exists() && !config.log_dir.is_dir() {
        return Err(EmitterError::LogDirNotFound(config.log_dir.clone()));
    }

    Ok(())
}

/// Enrich an audit event with metadata based on the enrichment config.
pub fn enrich(event: &mut AuditEvent, config: &EnrichmentConfig) {
    if config.add_hostname {
        if let Ok(hostname) = hostname::get() {
            let hostname_str = hostname.to_string_lossy().to_string();
            event.add_metadata("hostname", serde_json::Value::String(hostname_str));
        }
    }

    if config.add_process_id {
        event.add_metadata("pid", serde_json::Value::Number(std::process::id().into()));
    }

    if config.add_timestamp_iso {
        event.add_metadata(
            "enriched_at",
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
    }

    if config.add_environment {
        for key in &["SHELL", "TERM", "USER", "HOME"] {
            if let Ok(val) = std::env::var(key) {
                event.add_metadata(&format!("env_{key}"), serde_json::Value::String(val));
            }
        }
    }

    for (key, value) in &config.custom_fields {
        event.add_metadata(key, serde_json::Value::String(value.clone()));
    }
}

/// Emit an audit event to the appropriate log file.
///
/// Enriches the event based on config, determines the log category from
/// the event source, and appends the serialized event to the log file.
pub fn emit(mut event: AuditEvent, config: &EmitterConfig) -> Result<EmitResult, EmitterError> {
    config.validate()?;

    let category = if config.separate_logs {
        LogCategory::from_source(&event.source)
    } else {
        LogCategory::General
    };

    enrich(&mut event, &config.enrichment);

    emit_to(event, category, config)
}

/// Emit an audit event to a specific log category, bypassing automatic routing.
pub fn emit_to(
    event: AuditEvent,
    category: LogCategory,
    config: &EmitterConfig,
) -> Result<EmitResult, EmitterError> {
    config.validate()?;
    ensure_log_dir(config)?;

    let path = log_path(config, &category);
    let line_json = serde_json::to_string(&event)?;
    let line_with_newline = format!("{line_json}\n");

    let (line_number, bytes_written) = if path.exists() {
        // Append to existing file
        let existing_lines = count_lines(&path)?;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| EmitterError::WriteFailed {
                path: path.clone(),
                source: e,
            })?;
        use std::io::Write;
        file.write_all(line_with_newline.as_bytes())
            .map_err(|e| EmitterError::WriteFailed {
                path: path.clone(),
                source: e,
            })?;
        file.flush().map_err(|e| EmitterError::WriteFailed {
            path: path.clone(),
            source: e,
        })?;
        (existing_lines + 1, line_with_newline.len())
    } else {
        // First write to this log file — use atomic write
        crate::config::atomic_write(&path, &line_with_newline, None).map_err(|e| {
            EmitterError::WriteFailed {
                path: path.clone(),
                source: std::io::Error::other(e.to_string()),
            }
        })?;
        (1, line_with_newline.len())
    };

    Ok(EmitResult {
        event_id: event.id,
        category,
        path,
        line_number,
        bytes_written,
    })
}

/// Count the number of lines in a file.
fn count_lines(path: &Path) -> Result<u64, EmitterError> {
    let content = std::fs::read_to_string(path).map_err(|e| EmitterError::WriteFailed {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(content.lines().filter(|l| !l.is_empty()).count() as u64)
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::audit::types::{EventResult, EventType};

    fn test_config(dir: &Path) -> EmitterConfig {
        EmitterConfig::new(dir.to_path_buf())
    }

    fn test_config_combined(dir: &Path) -> EmitterConfig {
        EmitterConfig::new(dir.to_path_buf()).with_separate_logs(false)
    }

    fn test_event() -> AuditEvent {
        AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        )
    }

    // ─── LogCategory Tests ──────────────────────────────────────

    #[test]
    fn test_log_category_from_source() {
        assert_eq!(
            LogCategory::from_source(&EventSource::AiGuard),
            LogCategory::AiGuard
        );
        assert_eq!(
            LogCategory::from_source(&EventSource::Proxy),
            LogCategory::Proxy
        );
        assert_eq!(
            LogCategory::from_source(&EventSource::Sync),
            LogCategory::Sync
        );
        assert_eq!(
            LogCategory::from_source(&EventSource::Cli),
            LogCategory::Cli
        );
        assert_eq!(
            LogCategory::from_source(&EventSource::Tui),
            LogCategory::Tui
        );
        assert_eq!(
            LogCategory::from_source(&EventSource::Hook),
            LogCategory::Hook
        );
    }

    #[test]
    fn test_log_category_filename() {
        assert_eq!(LogCategory::AiGuard.filename(), "ai-guard.jsonl");
        assert_eq!(LogCategory::Proxy.filename(), "proxy.jsonl");
        assert_eq!(LogCategory::Sync.filename(), "sync.jsonl");
        assert_eq!(LogCategory::Cli.filename(), "cli.jsonl");
        assert_eq!(LogCategory::Tui.filename(), "tui.jsonl");
        assert_eq!(LogCategory::Hook.filename(), "hook.jsonl");
        assert_eq!(LogCategory::General.filename(), "audit.jsonl");
    }

    #[test]
    fn test_log_category_display() {
        assert_eq!(LogCategory::AiGuard.to_string(), "ai-guard");
        assert_eq!(LogCategory::General.to_string(), "audit");
    }

    #[test]
    fn test_log_category_all() {
        assert_eq!(LogCategory::all().len(), 7);
    }

    // ─── EnrichmentConfig Tests ─────────────────────────────────

    #[test]
    fn test_enrichment_config_defaults() {
        let config = EnrichmentConfig::default();
        assert!(config.add_hostname);
        assert!(config.add_process_id);
        assert!(config.add_timestamp_iso);
        assert!(!config.add_environment);
        assert!(config.custom_fields.is_empty());
    }

    #[test]
    fn test_enrichment_config_builder() {
        let config = EnrichmentConfig::new()
            .with_hostname(false)
            .with_process_id(false)
            .with_environment(true)
            .with_custom_field("key".to_string(), "value".to_string());

        assert!(!config.add_hostname);
        assert!(!config.add_process_id);
        assert!(config.add_environment);
        assert_eq!(config.custom_fields["key"], "value");
    }

    // ─── EmitterConfig Tests ────────────────────────────────────

    #[test]
    fn test_emitter_config_defaults() {
        let dir = PathBuf::from("/tmp/audit");
        let config = EmitterConfig::new(dir.clone());
        assert_eq!(config.log_dir, dir);
        assert!(config.separate_logs);
        assert!(config.atomic_writes);
    }

    #[test]
    fn test_emitter_config_validate_empty_path() {
        let config = EmitterConfig::new(PathBuf::new());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_emitter_config_validate_valid() {
        let config = EmitterConfig::new(PathBuf::from("/tmp/audit"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_emitter_config_builder() {
        let config = EmitterConfig::new(PathBuf::from("/tmp/audit"))
            .with_separate_logs(false)
            .with_atomic_writes(false);

        assert!(!config.separate_logs);
        assert!(!config.atomic_writes);
    }

    // ─── Log Path Tests ─────────────────────────────────────────

    #[test]
    fn test_log_path_separate() {
        let config = EmitterConfig::new(PathBuf::from("/var/log/audit"));
        let path = log_path(&config, &LogCategory::AiGuard);
        assert_eq!(path, PathBuf::from("/var/log/audit/ai-guard.jsonl"));
    }

    #[test]
    fn test_log_path_combined() {
        let config = EmitterConfig::new(PathBuf::from("/var/log/audit")).with_separate_logs(false);
        let path = log_path(&config, &LogCategory::AiGuard);
        // Combined mode always writes to General
        assert_eq!(path, PathBuf::from("/var/log/audit/audit.jsonl"));
    }

    // ─── Ensure Log Dir Tests ────────────────────────────────────

    #[test]
    fn test_ensure_log_dir_creates_dir() {
        let dir = tempfile::tempdir().unwrap();
        let log_dir = dir.path().join("audit_logs");
        let config = EmitterConfig::new(log_dir.clone());
        assert!(ensure_log_dir(&config).is_ok());
        assert!(log_dir.exists());
    }

    #[test]
    fn test_ensure_log_dir_existing() {
        let dir = tempfile::tempdir().unwrap();
        let config = EmitterConfig::new(dir.path().to_path_buf());
        assert!(ensure_log_dir(&config).is_ok());
    }

    // ─── Enrich Tests ───────────────────────────────────────────

    #[test]
    fn test_enrich_adds_hostname() {
        let mut event = test_event();
        let config = EnrichmentConfig::new().with_hostname(true);
        enrich(&mut event, &config);
        // Check metadata has hostname
        if let serde_json::Value::Object(map) = &event.metadata {
            assert!(map.contains_key("hostname"), "should have hostname");
        }
    }

    #[test]
    fn test_enrich_adds_pid() {
        let mut event = test_event();
        let config = EnrichmentConfig::new().with_process_id(true);
        enrich(&mut event, &config);
        if let serde_json::Value::Object(map) = &event.metadata {
            assert!(map.contains_key("pid"), "should have pid");
        }
    }

    #[test]
    fn test_enrich_adds_timestamp() {
        let mut event = test_event();
        let config = EnrichmentConfig::new().with_timestamp_iso(true);
        enrich(&mut event, &config);
        if let serde_json::Value::Object(map) = &event.metadata {
            assert!(map.contains_key("enriched_at"), "should have enriched_at");
        }
    }

    #[test]
    fn test_enrich_no_hostname() {
        let mut event = test_event();
        let config = EnrichmentConfig::new().with_hostname(false);
        enrich(&mut event, &config);
        if let serde_json::Value::Object(map) = &event.metadata {
            assert!(!map.contains_key("hostname"), "should not have hostname");
        }
    }

    #[test]
    fn test_enrich_custom_fields() {
        let mut event = test_event();
        let config = EnrichmentConfig::new()
            .with_hostname(false)
            .with_process_id(false)
            .with_timestamp_iso(false)
            .with_custom_field("env".to_string(), "production".to_string());
        enrich(&mut event, &config);
        if let serde_json::Value::Object(map) = &event.metadata {
            assert!(map.contains_key("env"), "should have custom field");
        }
    }

    // ─── Emit Tests (integration with filesystem) ───────────────

    #[test]
    fn test_emitter_emit_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let event = test_event();
        let result = emit(event, &config).unwrap();
        assert!(result.path.exists());
        assert_eq!(result.line_number, 1);
        assert!(result.bytes_written > 0);
    }

    #[test]
    fn test_emitter_emit_separate_logs() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let event1 = AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        );
        let event2 = AuditEvent::new(EventType::SyncPush, EventSource::Sync, EventResult::Success);

        let r1 = emit(event1, &config).unwrap();
        let r2 = emit(event2, &config).unwrap();

        assert_ne!(
            r1.path, r2.path,
            "different sources should go to different files"
        );
        assert!(r1.path.ends_with("ai-guard.jsonl"));
        assert!(r2.path.ends_with("sync.jsonl"));
    }

    #[test]
    fn test_emitter_emit_combined_logs() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config_combined(dir.path());

        let event1 = AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        );
        let event2 = AuditEvent::new(EventType::SyncPush, EventSource::Sync, EventResult::Success);

        let r1 = emit(event1, &config).unwrap();
        let r2 = emit(event2, &config).unwrap();

        assert_eq!(r1.path, r2.path, "combined mode should use same file");
        assert!(r1.path.ends_with("audit.jsonl"));
    }

    #[test]
    fn test_emitter_emit_appends() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let event1 = test_event();
        let event2 = test_event();

        let r1 = emit(event1, &config).unwrap();
        let r2 = emit(event2, &config).unwrap();

        assert_eq!(r1.line_number, 1, "first event should be line 1");
        assert_eq!(r2.line_number, 2, "second event should be line 2");
    }

    #[test]
    fn test_emitter_emit_to_specific_category() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let event = test_event();

        let result = emit_to(event, LogCategory::Proxy, &config).unwrap();
        assert!(result.path.ends_with("proxy.jsonl"));
    }

    #[test]
    fn test_emitter_emit_result_has_event_id() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());
        let event = test_event();
        let event_id = event.id.clone();
        let result = emit(event, &config).unwrap();
        assert_eq!(result.event_id, event_id);
    }

    #[test]
    fn test_emitter_jsonl_format() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let event = AuditEvent::new(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            EventResult::Success,
        )
        .with_secret("API_KEY".to_string(), "read".to_string());

        emit(event, &config).unwrap();

        let content = std::fs::read_to_string(dir.path().join("ai-guard.jsonl")).unwrap();
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed["event_type"], "SecretAccessed");
        assert_eq!(parsed["source"], "AiGuard");
        assert_eq!(parsed["secret_key"], "API_KEY");
    }
}
