//! Chain of custody for the AI audit trail.
//!
//! Tracks secret lineage, session paths, ownership verification,
//! and custody queries over audit events.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

use super::query_types::TimeRange;
use super::types::AuditEvent;
use super::types::EventId;
use super::types::EventResult;
use super::types::EventSource;
use super::types::EventType;
use super::types::SessionId;

// ─── Custody Link ───────────────────────────────────────────────

/// A single link in a custody chain — who had custody of a secret
/// at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodyLink {
    pub event_id: EventId,
    pub timestamp: DateTime<Utc>,
    pub source: EventSource,
    pub secret_key: String,
    pub operation: String,
    pub session_id: Option<SessionId>,
    pub result: EventResult,
}

impl CustodyLink {
    pub fn from_event(event: &AuditEvent) -> Option<Self> {
        let secret_key = event.secret_key.as_deref()?;
        let operation = event.operation.as_deref().unwrap_or("access");

        Some(Self {
            event_id: event.id.clone(),
            timestamp: event.timestamp,
            source: event.source,
            secret_key: secret_key.to_string(),
            operation: operation.to_string(),
            session_id: event.session_id.clone(),
            result: event.result.clone(),
        })
    }
}

// ─── Secret Lineage ─────────────────────────────────────────────

/// The lineage of a secret — chronological chain of custody transfers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretLineage {
    pub secret_key: String,
    pub links: Vec<CustodyLink>,
}

impl SecretLineage {
    pub fn new(secret_key: String) -> Self {
        Self {
            secret_key,
            links: Vec::new(),
        }
    }

    pub fn add_link(&mut self, link: CustodyLink) {
        self.links.push(link);
    }

    pub fn first_custodian(&self) -> Option<&CustodyLink> {
        self.links.first()
    }

    pub fn last_custodian(&self) -> Option<&CustodyLink> {
        self.links.last()
    }
}

/// Build a secret lineage from events, filtering to events
/// involving the given secret key.
pub fn build_lineage(events: &[AuditEvent], secret_key: &str) -> SecretLineage {
    let mut lineage = SecretLineage::new(secret_key.to_string());

    let mut filtered: Vec<&AuditEvent> = events
        .iter()
        .filter(|e| e.secret_key.as_deref() == Some(secret_key))
        .collect();
    filtered.sort_by_key(|e| e.timestamp);

    for event in filtered {
        if let Some(link) = CustodyLink::from_event(event) {
            lineage.add_link(link);
        }
    }

    lineage
}

// ─── Session Path ───────────────────────────────────────────────

/// A path through events within a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPath {
    pub session_id: SessionId,
    pub events: Vec<CustodyLink>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl SessionPath {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            events: Vec::new(),
            started_at: None,
            ended_at: None,
        }
    }

    pub fn add_event(&mut self, link: CustodyLink) {
        let ts = link.timestamp;
        self.started_at = Some(self.started_at.map_or(ts, |s: DateTime<Utc>| s.min(ts)));
        self.ended_at = Some(self.ended_at.map_or(ts, |e: DateTime<Utc>| e.max(ts)));
        self.events.push(link);
    }

    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.ended_at) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }

    pub fn secret_keys_accessed(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.events.iter().map(|e| e.secret_key.clone()).collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

/// Build a session path from events matching the given session ID.
pub fn build_session_path(events: &[AuditEvent], session_id: &SessionId) -> SessionPath {
    let mut path = SessionPath::new(session_id.clone());

    let mut filtered: Vec<&AuditEvent> = events
        .iter()
        .filter(|e| e.session_id.as_ref() == Some(session_id))
        .collect();
    filtered.sort_by_key(|e| e.timestamp);

    for event in &filtered {
        if let Some(link) = CustodyLink::from_event(event) {
            path.add_event(link);
        }
    }

    // Also track session start/end events even without secret_key
    for event in &filtered {
        if matches!(
            event.event_type,
            EventType::SessionStarted | EventType::SessionEnded
        ) {
            let link = CustodyLink {
                event_id: event.id.clone(),
                timestamp: event.timestamp,
                source: event.source,
                secret_key: event
                    .secret_key
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", event.event_type)),
                operation: event.operation.clone().unwrap_or_default(),
                session_id: event.session_id.clone(),
                result: event.result.clone(),
            };
            if !path.events.iter().any(|e| e.event_id == link.event_id) {
                path.add_event(link);
            }
        }
    }

    path
}

/// Build all session paths from a set of events.
pub fn build_all_session_paths(events: &[AuditEvent]) -> Vec<SessionPath> {
    let mut session_ids: Vec<SessionId> =
        events.iter().filter_map(|e| e.session_id.clone()).collect();
    session_ids.sort_by(|a, b| a.0.cmp(&b.0));
    session_ids.dedup();

    session_ids
        .iter()
        .map(|sid| build_session_path(events, sid))
        .collect()
}

// ─── Ownership Verification ─────────────────────────────────────

/// Result of verifying custody ownership for a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipReport {
    pub secret_key: String,
    pub current_owner: Option<CustodyLink>,
    pub lineage_length: usize,
    pub has_gaps: bool,
    pub sources: Vec<EventSource>,
    pub sessions: Vec<SessionId>,
}

impl OwnershipReport {
    pub fn from_lineage(lineage: &SecretLineage) -> Self {
        let mut sources: Vec<EventSource> = lineage.links.iter().map(|l| l.source).collect();
        sources.sort();
        sources.dedup();

        let sessions: Vec<SessionId> = lineage
            .links
            .iter()
            .filter_map(|l| l.session_id.clone())
            .collect();

        Self {
            secret_key: lineage.secret_key.clone(),
            current_owner: lineage.last_custodian().cloned(),
            lineage_length: lineage.links.len(),
            has_gaps: false,
            sources,
            sessions,
        }
    }
}

/// Verify ownership of a secret by checking its lineage.
///
/// Returns an `OwnershipReport` with current owner, lineage length,
/// and whether there are gaps in the custody chain.
pub fn verify_ownership(
    events: &[AuditEvent],
    secret_key: &str,
    time_range: &TimeRange,
) -> OwnershipReport {
    let filtered: Vec<AuditEvent> = events
        .iter()
        .filter(|e| super::query_engine::matches_time_range(e, time_range))
        .cloned()
        .collect();

    let lineage = build_lineage(&filtered, secret_key);
    let mut report = OwnershipReport::from_lineage(&lineage);

    // Check for gaps: any source transitions without a corresponding
    // access event (e.g., source changed but no handoff event)
    report.has_gaps = has_custody_gaps(&lineage);

    report
}

pub fn has_custody_gaps(lineage: &SecretLineage) -> bool {
    if lineage.links.len() < 2 {
        return false;
    }

    // A gap exists when there's a source transition without an intervening event
    for window in lineage.links.windows(2) {
        if window[0].source != window[1].source && window[0].session_id != window[1].session_id {
            // Source changed and session changed — potential custody gap
            // (in a full implementation, you'd check for explicit handoff events)
            return true;
        }
    }

    false
}

// ─── Custody Queries ─────────────────────────────────────────────

/// Different types of custody queries.
#[derive(Debug, Clone)]
pub enum CustodyQuery {
    /// Find the lineage of a specific secret.
    Lineage { secret_key: String },
    /// Find all events in a specific session.
    Session { session_id: SessionId },
    /// Find all sessions that accessed a secret.
    SecretSessions { secret_key: String },
    /// Find secrets accessed by a specific source.
    BySource { source: EventSource },
    /// Find current ownership of secrets matching a pattern.
    CurrentOwnership { secret_key_pattern: String },
}

/// Result of a custody query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CustodyResult {
    Lineage(SecretLineage),
    SessionPath(SessionPath),
    SecretSessions {
        secret_key: String,
        sessions: Vec<SessionId>,
    },
    BySource {
        source: EventSource,
        secrets: Vec<String>,
    },
    Ownership {
        reports: Vec<OwnershipReport>,
    },
}

/// Execute a custody query against a set of audit events.
pub fn execute_custody_query(
    events: &[AuditEvent],
    query: &CustodyQuery,
) -> Result<CustodyResult, CustodyError> {
    match query {
        CustodyQuery::Lineage { secret_key } => {
            let lineage = build_lineage(events, secret_key);
            Ok(CustodyResult::Lineage(lineage))
        }

        CustodyQuery::Session { session_id } => {
            let path = build_session_path(events, session_id);
            Ok(CustodyResult::SessionPath(path))
        }

        CustodyQuery::SecretSessions { secret_key } => {
            let mut sessions: Vec<SessionId> = events
                .iter()
                .filter(|e| e.secret_key.as_deref() == Some(secret_key))
                .filter_map(|e| e.session_id.clone())
                .collect();
            sessions.sort_by(|a, b| a.0.cmp(&b.0));
            sessions.dedup();
            Ok(CustodyResult::SecretSessions {
                secret_key: secret_key.clone(),
                sessions,
            })
        }

        CustodyQuery::BySource { source } => {
            let mut secrets: Vec<String> = events
                .iter()
                .filter(|e| e.source == *source)
                .filter_map(|e| e.secret_key.clone())
                .collect();
            secrets.sort();
            secrets.dedup();
            Ok(CustodyResult::BySource {
                source: *source,
                secrets,
            })
        }

        CustodyQuery::CurrentOwnership { secret_key_pattern } => {
            let matching_keys = find_matching_secrets(events, secret_key_pattern);
            let time_range = TimeRange::all();
            let reports: Vec<OwnershipReport> = matching_keys
                .iter()
                .map(|key| verify_ownership(events, key, &time_range))
                .collect();
            Ok(CustodyResult::Ownership { reports })
        }
    }
}

/// Find the current owner of a secret (the session that last accessed it).
pub fn find_current_owner(events: &[AuditEvent], secret_key: &str) -> Option<SessionId> {
    let time_range = TimeRange::all();
    let report = verify_ownership(events, secret_key, &time_range);
    report.current_owner.and_then(|link| link.session_id)
}

/// Find all unique secret keys in a set of events.
pub fn find_all_secrets(events: &[AuditEvent]) -> Vec<String> {
    find_matching_secrets(events, "*")
}

/// Find secret keys matching a glob-like pattern.
/// Supports `*` as wildcard and exact match.
pub fn find_matching_secrets(events: &[AuditEvent], pattern: &str) -> Vec<String> {
    let mut keys: Vec<String> = events.iter().filter_map(|e| e.secret_key.clone()).collect();
    keys.sort();
    keys.dedup();

    if pattern == "*" || pattern.is_empty() {
        return keys;
    }

    // Simple glob: * matches any substring
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        keys.retain(|key| {
            let mut pos = 0;
            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }
                if let Some(idx) = key[pos..].find(part) {
                    pos += idx + part.len();
                    if i == parts.len() - 1 && !pattern.ends_with('*') && pos != key.len() {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        });
    } else {
        keys.retain(|key| key == pattern);
    }

    keys
}

// ─── Custody Error ───────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CustodyError {
    #[error("no events found for secret key: {0}")]
    NoEventsForSecret(String),

    #[error("no events found for session: {0}")]
    NoEventsForSession(String),

    #[error("custody query failed: {0}")]
    QueryFailed(String),
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_event(
        event_type: EventType,
        source: EventSource,
        secret_key: Option<&str>,
        hours_ago: i64,
        session_id: Option<&str>,
    ) -> AuditEvent {
        let mut event = AuditEvent::new(event_type, source, EventResult::Success);
        event.timestamp = Utc::now() - chrono::Duration::hours(hours_ago);
        if let Some(key) = secret_key {
            event.secret_key = Some(key.to_string());
            event.operation = Some("access".to_string());
        }
        if let Some(sid) = session_id {
            event.session_id = Some(SessionId(sid.to_string()));
        }
        event
    }

    fn test_events() -> Vec<AuditEvent> {
        vec![
            test_event(
                EventType::SecretAccessed,
                EventSource::AiGuard,
                Some("DB_PASSWORD"),
                5,
                Some("session-1"),
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::Proxy,
                Some("API_KEY"),
                4,
                Some("session-1"),
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::AiGuard,
                Some("DB_PASSWORD"),
                3,
                Some("session-2"),
            ),
            test_event(
                EventType::SecretBound,
                EventSource::Cli,
                Some("DB_PASSWORD"),
                2,
                None,
            ),
            test_event(
                EventType::SecretAccessed,
                EventSource::Proxy,
                Some("API_KEY"),
                1,
                Some("session-3"),
            ),
        ]
    }

    // ─── CustodyLink ──────────────────────────────────────────

    #[test]
    fn test_custody_link_from_event_with_secret() {
        let event = test_event(
            EventType::SecretAccessed,
            EventSource::AiGuard,
            Some("DB_PASSWORD"),
            1,
            Some("session-1"),
        );
        let link = CustodyLink::from_event(&event).unwrap();
        assert_eq!(link.secret_key, "DB_PASSWORD");
        assert_eq!(link.operation, "access");
        assert_eq!(link.source, EventSource::AiGuard);
    }

    #[test]
    fn test_custody_link_from_event_without_secret() {
        let event = AuditEvent::new(
            EventType::SessionStarted,
            EventSource::Cli,
            EventResult::Success,
        );
        assert!(CustodyLink::from_event(&event).is_none());
    }

    // ─── SecretLineage ───────────────────────────────────────

    #[test]
    fn test_build_lineage_basic() {
        let events = test_events();
        let lineage = build_lineage(&events, "DB_PASSWORD");
        assert_eq!(lineage.secret_key, "DB_PASSWORD");
        assert_eq!(lineage.links.len(), 3);
    }

    #[test]
    fn test_build_lineage_no_match() {
        let events = test_events();
        let lineage = build_lineage(&events, "NONEXISTENT");
        assert_eq!(lineage.links.len(), 0);
    }

    #[test]
    fn test_lineage_first_last_custodian() {
        let events = test_events();
        let lineage = build_lineage(&events, "DB_PASSWORD");
        assert!(lineage.first_custodian().is_some());
        assert!(lineage.last_custodian().is_some());
        assert_eq!(
            lineage.first_custodian().unwrap().source,
            EventSource::AiGuard
        );
    }

    // ─── SessionPath ─────────────────────────────────────────

    #[test]
    fn test_build_session_path() {
        let events = test_events();
        let path = build_session_path(&events, &SessionId("session-1".to_string()));
        assert_eq!(path.session_id.0, "session-1");
        assert!(path.events.len() >= 2); // DB_PASSWORD + API_KEY
    }

    #[test]
    fn test_session_path_duration() {
        let events = test_events();
        let path = build_session_path(&events, &SessionId("session-1".to_string()));
        assert!(path.duration().is_some());
    }

    #[test]
    fn test_session_path_secret_keys() {
        let events = test_events();
        let path = build_session_path(&events, &SessionId("session-1".to_string()));
        let keys = path.secret_keys_accessed();
        assert!(keys.contains(&"API_KEY".to_string()));
        assert!(keys.contains(&"DB_PASSWORD".to_string()));
    }

    #[test]
    fn test_build_all_session_paths() {
        let events = test_events();
        let paths = build_all_session_paths(&events);
        assert!(paths.len() >= 3); // session-1, session-2, session-3
    }

    // ─── Ownership Verification ─────────────────────────────

    #[test]
    fn test_verify_ownership_basic() {
        let events = test_events();
        let time_range = TimeRange::all();
        let report = verify_ownership(&events, "DB_PASSWORD", &time_range);
        assert_eq!(report.secret_key, "DB_PASSWORD");
        assert!(report.lineage_length >= 2);
        assert!(report.current_owner.is_some());
    }

    #[test]
    fn test_verify_ownership_no_events() {
        let events = test_events();
        let time_range = TimeRange::all();
        let report = verify_ownership(&events, "NONEXISTENT", &time_range);
        assert_eq!(report.lineage_length, 0);
        assert!(report.current_owner.is_none());
    }

    #[test]
    fn test_ownership_report_sources() {
        let events = test_events();
        let time_range = TimeRange::all();
        let report = verify_ownership(&events, "DB_PASSWORD", &time_range);
        assert!(report.sources.contains(&EventSource::AiGuard));
        assert!(report.sources.contains(&EventSource::Cli));
    }

    #[test]
    fn test_has_custody_gaps_no_gap() {
        let lineage = SecretLineage::new("KEY".to_string());
        // Same source, no gap
        assert!(!has_custody_gaps(&lineage));
    }

    #[test]
    fn test_has_custody_gaps_with_gap() {
        let mut lineage = SecretLineage::new("KEY".to_string());
        lineage.add_link(CustodyLink {
            event_id: EventId::new(),
            timestamp: Utc::now(),
            source: EventSource::AiGuard,
            secret_key: "KEY".to_string(),
            operation: "access".to_string(),
            session_id: Some(SessionId("s1".to_string())),
            result: EventResult::Success,
        });
        lineage.add_link(CustodyLink {
            event_id: EventId::new(),
            timestamp: Utc::now(),
            source: EventSource::Proxy,
            secret_key: "KEY".to_string(),
            operation: "access".to_string(),
            session_id: Some(SessionId("s2".to_string())),
            result: EventResult::Success,
        });
        assert!(has_custody_gaps(&lineage));
    }

    // ─── Custody Queries ─────────────────────────────────────

    #[test]
    fn test_custody_query_lineage() {
        let events = test_events();
        let query = CustodyQuery::Lineage {
            secret_key: "DB_PASSWORD".to_string(),
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::Lineage(lineage) = result {
            assert_eq!(lineage.links.len(), 3);
        } else {
            panic!("Expected Lineage result");
        }
    }

    #[test]
    fn test_custody_query_session() {
        let events = test_events();
        let query = CustodyQuery::Session {
            session_id: SessionId("session-1".to_string()),
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::SessionPath(path) = result {
            assert!(path.events.len() >= 2);
        } else {
            panic!("Expected SessionPath result");
        }
    }

    #[test]
    fn test_custody_query_secret_sessions() {
        let events = test_events();
        let query = CustodyQuery::SecretSessions {
            secret_key: "DB_PASSWORD".to_string(),
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::SecretSessions {
            secret_key,
            sessions,
        } = result
        {
            assert_eq!(secret_key, "DB_PASSWORD");
            assert!(sessions.len() >= 2);
        } else {
            panic!("Expected SecretSessions result");
        }
    }

    #[test]
    fn test_custody_query_by_source() {
        let events = test_events();
        let query = CustodyQuery::BySource {
            source: EventSource::AiGuard,
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::BySource { source, secrets } = result {
            assert_eq!(source, EventSource::AiGuard);
            assert!(secrets.contains(&"DB_PASSWORD".to_string()));
        } else {
            panic!("Expected BySource result");
        }
    }

    #[test]
    fn test_custody_query_current_ownership() {
        let events = test_events();
        let query = CustodyQuery::CurrentOwnership {
            secret_key_pattern: "DB_*".to_string(),
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::Ownership { reports } = result {
            assert!(!reports.is_empty());
            assert!(reports.iter().any(|r| r.secret_key == "DB_PASSWORD"));
        } else {
            panic!("Expected Ownership result");
        }
    }

    #[test]
    fn test_custody_query_current_ownership_wildcard() {
        let events = test_events();
        let query = CustodyQuery::CurrentOwnership {
            secret_key_pattern: "*".to_string(),
        };
        let result = execute_custody_query(&events, &query).unwrap();
        if let CustodyResult::Ownership { reports } = result {
            assert!(reports.len() >= 2); // DB_PASSWORD + API_KEY at minimum
        } else {
            panic!("Expected Ownership result");
        }
    }

    // ─── Pattern Matching ────────────────────────────────────

    #[test]
    fn test_find_matching_secrets_exact() {
        let events = test_events();
        let matches = find_matching_secrets(&events, "API_KEY");
        assert_eq!(matches, vec!["API_KEY"]);
    }

    #[test]
    fn test_find_matching_secrets_wildcard_prefix() {
        let events = test_events();
        let matches = find_matching_secrets(&events, "DB_*");
        assert!(matches.contains(&"DB_PASSWORD".to_string()));
    }

    #[test]
    fn test_find_matching_secrets_wildcard_all() {
        let events = test_events();
        let matches = find_matching_secrets(&events, "*");
        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_find_matching_secrets_no_match() {
        let events = test_events();
        let matches = find_matching_secrets(&events, "NONEXISTENT");
        assert!(matches.is_empty());
    }
}
