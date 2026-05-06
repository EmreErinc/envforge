//! AI Guard integration with the audit trail.
//!
//! Provides hooks for pre-tool and post-tool audit events,
//! secret binding detection, and secret exposure alerts.
//! Bridges the existing AI guard system with the audit emitter.

use super::emitter::{self, EmitResult, EmitterConfig};
use super::types::{AuditEvent, EventId, EventResult, EventSource, EventType, SessionId};

// ─── Guard Audit Result ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GuardAuditResult {
    pub event_id: EventId,
    pub event_type: EventType,
    pub source: EventSource,
    pub result: EventResult,
    pub emit_result: Option<EmitResult>,
}

// ─── Pre-Tool Audit ──────────────────────────────────────────────

/// Record a pre-tool-use audit event.
///
/// Called before an AI tool invocation to log the intent to access
/// a secret or resource.
pub fn audit_pretool(
    tool_name: &str,
    tool_input: &str,
    secret_key: Option<&str>,
    config: &EmitterConfig,
) -> GuardAuditResult {
    let mut event = AuditEvent::new(
        EventType::ContextCreated,
        EventSource::AiGuard,
        EventResult::Success,
    );

    event.tool_type = Some(tool_name.to_string());
    if let Some(key) = secret_key {
        event.secret_key = Some(key.to_string());
        event.operation = Some("pretool-read".to_string());
    }

    let detected_secrets = detect_secrets_in_input(tool_input);
    if !detected_secrets.is_empty() {
        event.result = EventResult::Warning(format!(
            "Potential secrets in input: {}",
            detected_secrets.join(", ")
        ));
    }

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

// ─── Post-Tool Audit ─────────────────────────────────────────────

/// Record a post-tool-use audit event.
///
/// Called after an AI tool invocation to log the result, including
/// any denied access or detected secret exposure in output.
pub fn audit_posttool(
    tool_name: &str,
    tool_output: &str,
    secret_key: Option<&str>,
    was_denied: bool,
    config: &EmitterConfig,
) -> GuardAuditResult {
    let event_type = if was_denied {
        EventType::AccessDenied
    } else {
        EventType::SecretAccessed
    };

    let result = if was_denied {
        EventResult::Denied("ai-guard blocked access".to_string())
    } else {
        EventResult::Success
    };

    let mut event = AuditEvent::new(event_type, EventSource::AiGuard, result);
    event.tool_type = Some(tool_name.to_string());
    if let Some(key) = secret_key {
        event.secret_key = Some(key.to_string());
        event.operation = Some("posttool-read".to_string());
    }

    // Check for secret exposure in the tool output
    let exposed = detect_secrets_in_input(tool_output);
    if !exposed.is_empty() && !was_denied {
        let exposure_event = AuditEvent::new(
            EventType::SecretExposure,
            EventSource::AiGuard,
            EventResult::Warning(format!("Secret exposed in output: {}", exposed.join(", "))),
        );
        let _ = emitter::emit(exposure_event, config);
    }

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

// ─── Secret Binding ──────────────────────────────────────────────

/// Record a secret binding event.
///
/// Called when a secret is bound to a context or session.
pub fn audit_secret_bound(
    secret_key: &str,
    session_id: Option<&SessionId>,
    config: &EmitterConfig,
) -> GuardAuditResult {
    let mut event = AuditEvent::new(
        EventType::SecretBound,
        EventSource::AiGuard,
        EventResult::Success,
    );
    event.secret_key = Some(secret_key.to_string());
    event.operation = Some("bind".to_string());
    if let Some(sid) = session_id {
        event.session_id = Some(sid.clone());
    }

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

// ─── Secret Exposure ─────────────────────────────────────────────

/// Record a secret exposure event.
///
/// Called when a secret is detected in AI output or logs.
pub fn audit_secret_exposure(
    secret_key: &str,
    exposure_context: &str,
    config: &EmitterConfig,
) -> GuardAuditResult {
    let event = AuditEvent::new(
        EventType::SecretExposure,
        EventSource::AiGuard,
        EventResult::Warning(format!(
            "Secret '{}' exposed in {}",
            secret_key, exposure_context
        )),
    );

    let mut event = event;
    event.secret_key = Some(secret_key.to_string());
    event.operation = Some("exposure-detect".to_string());

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

// ─── Secret Detection ────────────────────────────────────────────

const SECRET_PATTERNS: &[(&str, &str)] = &[
    ("sk-", "OpenAI/Stripe API key"),
    ("AKIA", "AWS access key"),
    ("ghp_", "GitHub personal access token"),
    ("gho_", "GitHub OAuth token"),
    ("xoxb-", "Slack bot token"),
    ("SG.", "SendGrid API key"),
    ("eyJ", "JWT token prefix"),
];

fn detect_secrets_in_input(input: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (prefix, name) in SECRET_PATTERNS {
        if let Some(pos) = input.find(prefix) {
            let after = &input[pos + prefix.len()..];
            if after.len() >= 4 {
                found.push(name.to_string());
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

// ─── Session Tracking ────────────────────────────────────────────

/// Record a session start event.
pub fn audit_session_start(session_id: &SessionId, config: &EmitterConfig) -> GuardAuditResult {
    let mut event = AuditEvent::new(
        EventType::SessionStarted,
        EventSource::AiGuard,
        EventResult::Success,
    );
    event.session_id = Some(session_id.clone());
    event.operation = Some("session-start".to_string());

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

/// Record a session end event.
pub fn audit_session_end(session_id: &SessionId, config: &EmitterConfig) -> GuardAuditResult {
    let mut event = AuditEvent::new(
        EventType::SessionEnded,
        EventSource::AiGuard,
        EventResult::Success,
    );
    event.session_id = Some(session_id.clone());
    event.operation = Some("session-end".to_string());

    let emit_result = emitter::emit(event.clone(), config).ok();

    GuardAuditResult {
        event_id: event.id,
        event_type: event.event_type,
        source: event.source,
        result: event.result,
        emit_result,
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn test_config() -> EmitterConfig {
        EmitterConfig::new(std::env::temp_dir().join("envforge-test-audit"))
    }

    #[test]
    fn test_detect_secrets_in_input_openai() {
        let result = detect_secrets_in_input("key=sk-proj-abcdefghij1234567890");
        assert!(result.contains(&"OpenAI/Stripe API key".to_string()));
    }

    #[test]
    fn test_detect_secrets_in_input_aws() {
        let result = detect_secrets_in_input("AWS_KEY=AKIAIOSFODNN7EXAMPLE");
        assert!(result.contains(&"AWS access key".to_string()));
    }

    #[test]
    fn test_detect_secrets_in_input_github() {
        let result = detect_secrets_in_input("TOKEN=ghp_ABCDEFGHIJKLMNOPQRST1234567890");
        assert!(result.contains(&"GitHub personal access token".to_string()));
    }

    #[test]
    fn test_detect_secrets_in_input_clean() {
        let result = detect_secrets_in_input("echo hello world");
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_secrets_in_input_short_prefix() {
        // Too short after prefix — should not match
        let result = detect_secrets_in_input("key=sk-ab");
        assert!(result.is_empty());
    }

    #[test]
    fn test_guard_audit_result_fields() {
        let result = GuardAuditResult {
            event_id: EventId::new(),
            event_type: EventType::SecretAccessed,
            source: EventSource::AiGuard,
            result: EventResult::Success,
            emit_result: None,
        };
        assert_eq!(result.event_type, EventType::SecretAccessed);
        assert_eq!(result.source, EventSource::AiGuard);
        assert!(result.emit_result.is_none());
    }
}
