use chrono::Utc;
use envforge::ops::monitor::fingerprint::*;
use envforge::ops::monitor::*;
use std::thread;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_events(tool_type: &ToolType, count: usize, key_prefix: &str) -> Vec<MonitorEvent> {
    (0..count)
        .map(|i| MonitorEvent {
            tool_type: tool_type.clone(),
            secret_key: format!("{key_prefix}{i}"),
            operation: if i % 2 == 0 {
                "read".to_string()
            } else {
                "write".to_string()
            },
            timestamp: Utc::now(),
        })
        .collect()
}

fn make_events_with_ops(tool_type: &ToolType, count: usize, op: &str) -> Vec<MonitorEvent> {
    (0..count)
        .map(|i| MonitorEvent {
            tool_type: tool_type.clone(),
            secret_key: format!("key{i}"),
            operation: op.to_string(),
            timestamp: Utc::now(),
        })
        .collect()
}

fn make_events_with_intervals(
    tool_type: &ToolType,
    count: usize,
    interval_secs: i64,
) -> Vec<MonitorEvent> {
    let base = Utc::now();
    (0..count)
        .map(|i| MonitorEvent {
            tool_type: tool_type.clone(),
            secret_key: format!("key{i}"),
            operation: "read".to_string(),
            timestamp: base + chrono::Duration::seconds((i as i64) * interval_secs),
        })
        .collect()
}

// ─── Fingerprint Generator Tests ─────────────────────────────────────────────

#[test]
fn test_generate_fingerprint_basic() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "secret_");

    let fp = gen.generate_and_store(&tool, &events).unwrap();
    assert_eq!(fp.tool_type, tool);
    assert!(!fp.behavioral_signature.is_empty());
    assert!(fp.confidence > 0.0);
    assert!(fp.confidence <= 1.0);
}

#[test]
fn test_fingerprint_consistency_same_events() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "secret_");

    let fp1 = gen.generate_temporary(&tool, &events).unwrap();
    let fp2 = gen.generate_temporary(&tool, &events).unwrap();
    assert_eq!(fp1.behavioral_signature, fp2.behavioral_signature);
}

#[test]
fn test_fingerprint_different_events() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    // Same keys and tool, but different operation distributions
    let events1 = make_events_with_ops(&tool, 50, "read");
    let events2 = make_events_with_ops(&tool, 50, "write");

    let fp1 = gen.generate_temporary(&tool, &events1).unwrap();
    let fp2 = gen.generate_temporary(&tool, &events2).unwrap();
    // Different operation distributions → different signatures
    assert_ne!(fp1.behavioral_signature, fp2.behavioral_signature);
}

#[test]
fn test_fingerprint_empty_events_error() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    let events: Vec<MonitorEvent> = vec![];

    let err = gen.generate_temporary(&tool, &events).unwrap_err();
    assert!(matches!(err, MonitorError::InsufficientEvents(0, 1)));
}

#[test]
fn test_fingerprint_store_and_retrieve() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "secret_");

    gen.generate_and_store(&tool, &events).unwrap();
    let retrieved = gen.get_fingerprint(&tool).unwrap();
    assert_eq!(retrieved.tool_type, tool);
}

#[test]
fn test_fingerprint_confidence_calculation() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;

    let events_10 = make_events(&tool, 10, "s");
    let fp_10 = gen.generate_temporary(&tool, &events_10).unwrap();
    assert!((fp_10.confidence - 0.1).abs() < f64::EPSILON);

    let events_150 = make_events(&tool, 150, "s");
    let fp_150 = gen.generate_temporary(&tool, &events_150).unwrap();
    assert!((fp_150.confidence - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_fingerprint_overwrite_store() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;
    let events1 = make_events_with_ops(&tool, 50, "read");
    let events2 = make_events_with_ops(&tool, 50, "write");

    gen.generate_and_store(&tool, &events1).unwrap();
    let fp1 = gen.get_fingerprint(&tool).unwrap();

    gen.generate_and_store(&tool, &events2).unwrap();
    let fp2 = gen.get_fingerprint(&tool).unwrap();

    assert_ne!(fp1.behavioral_signature, fp2.behavioral_signature);
}

// ─── Identity Verification Tests ─────────────────────────────────────────────

#[test]
fn test_verify_insufficient_data() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 10, "s");

    let result = sys.verifier.verify(&tool, &events).unwrap();
    assert_eq!(result, VerificationResult::InsufficientData);
}

#[test]
fn test_verify_no_baseline() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "s");

    let result = sys.verifier.verify(&tool, &events).unwrap();
    assert_eq!(result, VerificationResult::NoBaseline);
}

#[test]
fn test_verify_match() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "s");

    // Establish baseline
    sys.generator.generate_and_store(&tool, &events).unwrap();

    // Verify with same events → match
    let result = sys.verifier.verify(&tool, &events).unwrap();
    assert_eq!(result, VerificationResult::Match);
}

#[test]
fn test_verify_mismatch() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let baseline_events = make_events_with_ops(&tool, 50, "read");
    let verify_events = make_events_with_ops(&tool, 50, "write");

    sys.generator
        .generate_and_store(&tool, &baseline_events)
        .unwrap();

    let result = sys.verifier.verify(&tool, &verify_events).unwrap();
    assert!(
        matches!(result, VerificationResult::Mismatch { .. }),
        "Expected Mismatch, got {:?}",
        result
    );
}

#[test]
fn test_verify_impersonation() {
    let sys = FingerprinterSystem::default();
    let tool_a = ToolType::ClaudeCode;
    let tool_b = ToolType::Cursor;

    let events_a = make_events_with_ops(&tool_a, 50, "read");
    let events_b = make_events_with_ops(&tool_b, 50, "write");

    // Establish baselines for both tools
    sys.generator
        .generate_and_store(&tool_a, &events_a)
        .unwrap();
    sys.generator
        .generate_and_store(&tool_b, &events_b)
        .unwrap();

    // Verify tool_b claiming to be tool_a (using tool_b's events)
    let result = sys.verifier.verify(&tool_a, &events_b).unwrap();
    if let VerificationResult::Mismatch {
        confidence,
        divergence,
    } = result
    {
        assert!(
            confidence > 0.5,
            "Expected high confidence for impersonation"
        );
        assert!(
            divergence.contains("impersonation suspected"),
            "Expected impersonation message, got: {}",
            divergence
        );
    } else {
        panic!("Expected Mismatch, got {:?}", result);
    }
}

#[test]
fn test_verify_match_rate_high() {
    // Verify that identical events produce Match consistently.
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events_with_intervals(&tool, 50, 1);

    sys.generator.generate_and_store(&tool, &events).unwrap();

    let result = sys.verifier.verify(&tool, &events).unwrap();
    assert_eq!(result, VerificationResult::Match);
}

// ─── Trust Manager Tests ─────────────────────────────────────────────────────

#[test]
fn test_trust_initial_score() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let score = tm.get_trust_score(&tool);
    assert!(score.is_none(), "No score should exist initially");
}

#[test]
fn test_trust_positive_verification() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let score = tm
        .update_trust(&tool, TrustEvent::PositiveVerification)
        .unwrap();
    assert!((score.score - 0.6).abs() < f64::EPSILON); // 0.5 + 0.1
    assert_eq!(score.sample_size, 1);
}

#[test]
fn test_trust_negative_verification() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let score = tm
        .update_trust(&tool, TrustEvent::NegativeVerification)
        .unwrap();
    assert!((score.score - 0.3).abs() < f64::EPSILON); // 0.5 - 0.2
}

#[test]
fn test_trust_suspicious_behavior() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let score = tm
        .update_trust(&tool, TrustEvent::SuspiciousBehavior)
        .unwrap();
    assert!((score.score - 0.2).abs() < f64::EPSILON); // 0.5 - 0.3
}

#[test]
fn test_trust_normal_behavior() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let score = tm.update_trust(&tool, TrustEvent::NormalBehavior).unwrap();
    assert!((score.score - 0.55).abs() < f64::EPSILON); // 0.5 + 0.05
}

#[test]
fn test_trust_clamping() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    // Clamp to 1.0
    for _ in 0..10 {
        tm.update_trust(&tool, TrustEvent::PositiveVerification)
            .unwrap();
    }
    let score = tm.get_trust_score(&tool).unwrap();
    assert!((score.score - 1.0).abs() < f64::EPSILON);

    // Reset and clamp to 0.0
    tm.reset_score(&tool);
    for _ in 0..10 {
        tm.update_trust(&tool, TrustEvent::SuspiciousBehavior)
            .unwrap();
    }
    let score = tm.get_trust_score(&tool).unwrap();
    assert!((score.score - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_trust_confidence_calculation() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    for _ in 0..50 {
        tm.update_trust(&tool, TrustEvent::NormalBehavior).unwrap();
    }
    let score = tm.get_trust_score(&tool).unwrap();
    assert!((score.confidence - 0.5).abs() < f64::EPSILON);

    for _ in 0..60 {
        tm.update_trust(&tool, TrustEvent::NormalBehavior).unwrap();
    }
    let score = tm.get_trust_score(&tool).unwrap();
    assert!((score.confidence - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_trust_get_all_scores() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool1 = ToolType::ClaudeCode;
    let tool2 = ToolType::Cursor;

    tm.update_trust(&tool1, TrustEvent::PositiveVerification)
        .unwrap();
    tm.update_trust(&tool2, TrustEvent::NormalBehavior).unwrap();

    let all = tm.get_all_scores();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_trust_reset_score() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    tm.update_trust(&tool, TrustEvent::PositiveVerification)
        .unwrap();
    let before = tm.get_trust_score(&tool).unwrap();
    assert!(before.score > 0.5);

    tm.reset_score(&tool);
    let after = tm.get_trust_score(&tool).unwrap();
    assert!((after.score - 0.5).abs() < f64::EPSILON);
    assert_eq!(after.sample_size, 0);
}

#[test]
fn test_trust_decay() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    // Set a high score
    for _ in 0..10 {
        tm.update_trust(&tool, TrustEvent::PositiveVerification)
            .unwrap();
    }
    let before = tm.get_trust_score(&tool).unwrap();
    assert!((before.score - 1.0).abs() < f64::EPSILON);

    // Apply decay (same instant → no decay)
    let after = tm.apply_decay(&tool).unwrap();
    assert!((after.score - 1.0).abs() < f64::EPSILON); // No time has passed
}

#[test]
fn test_trust_not_found_error() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let err = tm.apply_decay(&tool).unwrap_err();
    assert!(matches!(err, MonitorError::TrustScoreNotFound(_)));
}

// ─── Thread Safety Tests ─────────────────────────────────────────────────────

#[test]
fn test_concurrent_fingerprint_generation() {
    let gen = FingerprintGenerator::new();
    let tool = ToolType::ClaudeCode;

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let gen = gen.clone();
            let tool = tool.clone();
            thread::spawn(move || {
                let events = make_events(&tool, 50, &format!("thread_{i}_"));
                gen.generate_and_store(&tool, &events).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Last write wins; the point is no panic / no deadlock.
    assert!(gen.get_fingerprint(&tool).is_some());
}

#[test]
fn test_concurrent_trust_updates() {
    let tm = TrustManager::new(TrustConfig::default());
    let tool = ToolType::ClaudeCode;

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let tm = tm.clone();
            let tool = tool.clone();
            thread::spawn(move || {
                tm.update_trust(&tool, TrustEvent::NormalBehavior).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let score = tm.get_trust_score(&tool).unwrap();
    assert_eq!(score.sample_size, 100);
    assert!((score.confidence - 1.0).abs() < f64::EPSILON);
}

// ─── System Builder Tests ────────────────────────────────────────────────────

#[test]
fn test_fingerprinter_system_default() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "s");

    // Generator works
    let fp = sys.generator.generate_and_store(&tool, &events).unwrap();
    assert!(!fp.behavioral_signature.is_empty());

    // Verifier works
    let result = sys.verifier.verify(&tool, &events).unwrap();
    assert_eq!(result, VerificationResult::Match);

    // Trust manager works
    let score = sys
        .trust_manager
        .update_trust(&tool, TrustEvent::NormalBehavior)
        .unwrap();
    assert_eq!(score.sample_size, 1);
}

#[test]
fn test_fingerprinter_system_clone() {
    let sys = FingerprinterSystem::default();
    let tool = ToolType::ClaudeCode;
    let events = make_events(&tool, 50, "s");

    sys.generator.generate_and_store(&tool, &events).unwrap();

    let fp = sys.generator.get_fingerprint(&tool).unwrap();
    assert!(!fp.behavioral_signature.is_empty());
}
