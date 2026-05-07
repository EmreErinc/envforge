use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};

use crate::model::{
    DeprecationRecommendation, EnrichedAccessEvent, LowUsageSecret,
    SuggestedTimeline, UnusedSecret,
};

/// Detect dormant secrets: those with no access events in the last N days.
pub fn detect_dormant(
    events: &[EnrichedAccessEvent],
    threshold_days: u32,
) -> Vec<UnusedSecret> {
    let cutoff = Utc::now() - Duration::days(i64::from(threshold_days));

    // Group events by secret_name, find latest timestamp per secret
    let mut latest_access: HashMap<String, DateTime<Utc>> = HashMap::new();
    for event in events {
        let name = &event.raw.secret_name;
        let ts = event.raw.timestamp;
        latest_access
            .entry(name.clone())
            .and_modify(|existing| {
                if ts > *existing {
                    *existing = ts;
                }
            })
            .or_insert(ts);
    }

    // Find secrets whose latest access is older than cutoff
    let now = Utc::now();
    let total_days = f64::from(threshold_days.max(1));

    let mut unused: Vec<UnusedSecret> = latest_access
        .into_iter()
        .filter(|(_, latest)| *latest < cutoff)
        .map(|(name, latest)| {
            let days_since = (now - latest).num_days().max(1) as u64;
            let confidence = (1.0 - (days_since as f64 / total_days * 0.5)).clamp(0.0, 1.0);

            UnusedSecret {
                secret_name: name,
                reason: format!("No access in {} days", days_since),
                days_since_last_access: days_since,
                confidence,
            }
        })
        .collect();

    unused.sort_by_key(|b| std::cmp::Reverse(b.days_since_last_access));
    unused
}

/// Detect zero-access secrets: secret names that appear in an "all known keys" list
/// but have zero events in the audit data.
pub fn detect_zero_access(
    known_keys: &[String],
    events: &[EnrichedAccessEvent],
) -> Vec<UnusedSecret> {
    let accessed: HashSet<&str> = events
        .iter()
        .map(|e| e.raw.secret_name.as_str())
        .collect();

    let mut unused: Vec<UnusedSecret> = known_keys
        .iter()
        .filter(|key| !accessed.contains(key.as_str()))
        .map(|key| UnusedSecret {
            secret_name: key.clone(),
            reason: "Zero access events in audit log".to_string(),
            days_since_last_access: 0,
            confidence: 1.0,
        })
        .collect();

    unused.sort_by(|a, b| a.secret_name.cmp(&b.secret_name));
    unused
}

/// Detect low-usage secrets: those with fewer than max_accesses in the given window_days.
pub fn detect_low_usage(
    events: &[EnrichedAccessEvent],
    max_accesses: u64,
    window_days: u32,
) -> Vec<LowUsageSecret> {
    let cutoff = Utc::now() - Duration::days(i64::from(window_days));

    // Count access events per secret in the time window
    let mut access_counts: HashMap<String, u64> = HashMap::new();
    for event in events {
        if event.raw.timestamp >= cutoff {
            *access_counts
                .entry(event.raw.secret_name.clone())
                .or_insert(0) += 1;
        }
    }

    // Filter those below threshold
    let mut low_usage: Vec<LowUsageSecret> = access_counts
        .into_iter()
        .filter(|(_, count)| *count <= max_accesses && *count > 0)
        .map(|(name, count)| LowUsageSecret {
            secret_name: name,
            access_count: count,
            threshold: max_accesses,
            period: crate::model::TimeWindow::Custom(crate::model::CustomTimeWindow {
                duration_seconds: u64::from(window_days) * 86400,
            }),
        })
        .collect();

    low_usage.sort_by_key(|a| a.access_count);
    low_usage
}

/// Generate deprecation recommendations from dormant and low-usage detections.
pub fn generate_recommendations(
    dormant: &[UnusedSecret],
    low_usage: &[LowUsageSecret],
) -> Vec<DeprecationRecommendation> {
    let now = Utc::now();
    let mut recommendations = Vec::new();

    // Dormant → immediate deprecation recommendation
    for secret in dormant {
        let timeline = SuggestedTimeline {
            review_by: now + Duration::days(7),
            deprecate_by: now + Duration::days(14),
            remove_by: now + Duration::days(30),
        };

        recommendations.push(DeprecationRecommendation {
            secret_name: secret.secret_name.clone(),
            reason: secret.reason.clone(),
            unused: secret.clone(),
            timeline,
            dependent_count: 0,
        });
    }

    // Low-usage → review recommendation (longer timeline)
    for secret in low_usage {
        let timeline = SuggestedTimeline {
            review_by: now + Duration::days(14),
            deprecate_by: now + Duration::days(30),
            remove_by: now + Duration::days(60),
        };

        let unused = UnusedSecret {
            secret_name: secret.secret_name.clone(),
            reason: format!(
                "Low usage: {} accesses in last period (threshold: {})",
                secret.access_count, secret.threshold
            ),
            days_since_last_access: 0,
            confidence: 0.5,
        };

        recommendations.push(DeprecationRecommendation {
            secret_name: secret.secret_name.clone(),
            reason: format!("Low-usage secret ({} accesses)", secret.access_count),
            unused,
            timeline,
            dependent_count: 0,
        });
    }

    // Remove duplicates (dormant takes priority over low-usage)
    let mut seen = HashSet::new();
    recommendations.retain(|r| seen.insert(r.secret_name.clone()));

    recommendations.sort_by(|a, b| {
        b.unused
            .confidence
            .partial_cmp(&a.unused.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    recommendations
}

/// Check if a specific secret is dormant (no access in threshold_days).
pub fn is_dormant(
    events: &[EnrichedAccessEvent],
    secret_name: &str,
    threshold_days: u32,
) -> bool {
    let cutoff = Utc::now() - Duration::days(i64::from(threshold_days));
    let has_recent = events
        .iter()
        .any(|e| e.raw.secret_name == secret_name && e.raw.timestamp >= cutoff);
    !has_recent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccessSource, AccessType, AccessorInfo, AccessorType, RawAccessEvent};
    use uuid::Uuid;

    fn make_event(secret: &str, hours_ago: i64) -> EnrichedAccessEvent {
        let ts = Utc::now() - Duration::hours(hours_ago);
        EnrichedAccessEvent {
            id: Uuid::new_v4(),
            raw: RawAccessEvent {
                secret_name: secret.to_string(),
                access_type: AccessType::Read,
                accessor: AccessorInfo {
                    id: "test".to_string(),
                    accessor_type: AccessorType::User,
                    name: None,
                    ip_address: None,
                    user_agent: None,
                },
                timestamp: ts,
                source: AccessSource::Cli,
                context: None,
            },
            secret_id: secret.to_string(),
            provider: "test".to_string(),
            environment: None,
            risk_level: crate::model::RiskLevel::Low,
            enriched_at: ts,
        }
    }

    #[test]
    fn test_detect_dormant_flags_old_event() {
        let events = vec![make_event("OLD_KEY", 24 * 100)]; // 100 days ago
        let dormant = detect_dormant(&events, 90);
        assert!(!dormant.is_empty());
        assert_eq!(dormant[0].secret_name, "OLD_KEY");
    }

    #[test]
    fn test_detect_dormant_ignores_recent() {
        let events = vec![make_event("FRESH_KEY", 1)]; // 1 hour ago
        let dormant = detect_dormant(&events, 90);
        assert!(dormant.iter().all(|d| d.secret_name != "FRESH_KEY"));
    }

    #[test]
    fn test_detect_zero_access() {
        let known = vec!["USED_KEY".to_string(), "UNUSED_KEY".to_string()];
        let events = vec![make_event("USED_KEY", 1)];
        let unused = detect_zero_access(&known, &events);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].secret_name, "UNUSED_KEY");
        assert!((unused[0].confidence - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_detect_zero_access_empty_known() {
        let known: Vec<String> = vec![];
        let events = vec![make_event("ANY", 1)];
        let unused = detect_zero_access(&known, &events);
        assert!(unused.is_empty());
    }

    #[test]
    fn test_detect_low_usage_below_threshold() {
        let events = vec![
            make_event("LOW_KEY", 1),
            make_event("LOW_KEY", 2),
            make_event("HIGH_KEY", 1),
            make_event("HIGH_KEY", 2),
            make_event("HIGH_KEY", 3),
            make_event("HIGH_KEY", 4),
            make_event("HIGH_KEY", 5),
            make_event("HIGH_KEY", 6),
        ];
        let low = detect_low_usage(&events, 3, 30);
        let low_names: Vec<_> = low.iter().map(|l| l.secret_name.as_str()).collect();
        assert!(low_names.contains(&"LOW_KEY"));
        assert!(!low_names.contains(&"HIGH_KEY"));
    }

    #[test]
    fn test_detect_low_usage_empty_events() {
        let events: Vec<EnrichedAccessEvent> = vec![];
        let low = detect_low_usage(&events, 3, 30);
        assert!(low.is_empty());
    }

    #[test]
    fn test_is_dormant_true() {
        let events = vec![make_event("DORMANT", 24 * 100)];
        assert!(is_dormant(&events, "DORMANT", 90));
    }

    #[test]
    fn test_is_dormant_false() {
        let events = vec![make_event("ACTIVE", 1)];
        assert!(!is_dormant(&events, "ACTIVE", 90));
    }

    #[test]
    fn test_generate_recommendations() {
        let dormant = vec![UnusedSecret {
            secret_name: "DORMANT_KEY".to_string(),
            reason: "No access".to_string(),
            days_since_last_access: 120,
            confidence: 0.95,
        }];
        let low_usage = vec![LowUsageSecret {
            secret_name: "LOW_KEY".to_string(),
            access_count: 2,
            threshold: 5,
            period: crate::model::TimeWindow::Last30Days,
        }];

        let recs = generate_recommendations(&dormant, &low_usage);
        assert_eq!(recs.len(), 2);
        // Dormant recommendation should have shorter timeline
        let dormant_rec = recs.iter().find(|r| r.secret_name == "DORMANT_KEY").unwrap();
        let low_rec = recs.iter().find(|r| r.secret_name == "LOW_KEY").unwrap();
        assert!(dormant_rec.timeline.remove_by < low_rec.timeline.remove_by);
    }

    #[test]
    fn test_generate_recommendations_no_duplicates() {
        let dormant = vec![UnusedSecret {
            secret_name: "SHARED_KEY".to_string(),
            reason: "dormant".to_string(),
            days_since_last_access: 100,
            confidence: 0.9,
        }];
        let low_usage = vec![LowUsageSecret {
            secret_name: "SHARED_KEY".to_string(),
            access_count: 1,
            threshold: 3,
            period: crate::model::TimeWindow::Last30Days,
        }];

        let recs = generate_recommendations(&dormant, &low_usage);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].reason, "dormant"); // dormant takes priority
    }

    #[test]
    fn test_generate_recommendations_empty() {
        let recs = generate_recommendations(&[], &[]);
        assert!(recs.is_empty());
    }
}
