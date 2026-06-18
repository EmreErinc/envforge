//! Regression tests for the dispatch backend behind the named LSP custom
//! requests (H4 — `envforge/fenceStatus`, `envforge/canaryScan`, … in
//! `server.rs`). Each custom method forwards to `dispatch_command` with a
//! FIXED command id; these tests pin that dispatch's contract for the pure,
//! filesystem-free paths. (The custom-method wiring itself is exercised via
//! the live LSP / IDE; the per-command logic is exercised here.)

use envforge::lsp::commands::dispatch_command;
use serde_json::json;

#[test]
fn test_canary_scan_text_no_tokens_is_clean() {
    let r = dispatch_command(
        "envforge.canary.scan",
        &[json!({ "text": "just an ordinary log line, nothing here" })],
        None,
    );
    assert_eq!(r["ok"], json!(true));
    assert_eq!(r["result"]["match_count"], json!(0));
}

#[test]
fn test_canary_scan_requires_text_or_file() {
    let r = dispatch_command("envforge.canary.scan", &[json!({})], None);
    assert_eq!(r["ok"], json!(false), "missing text/file must be an error");
}

#[test]
fn test_run_volatile_rejects_shell_metacharacters() {
    // The descriptor builder must refuse injection attempts before handing
    // anything back to a plugin that may run it.
    let r = dispatch_command(
        "envforge.run.volatile",
        &[json!({ "command": "echo $(whoami)", "ttl": "30m" })],
        None,
    );
    assert_eq!(
        r["ok"],
        json!(false),
        "shell metacharacters must be rejected"
    );
}

#[test]
fn test_run_volatile_returns_structured_descriptor_not_shell_string() {
    let r = dispatch_command(
        "envforge.run.volatile",
        &[json!({ "command": "npm test", "ttl": "15m" })],
        None,
    );
    assert_eq!(r["ok"], json!(true));
    // Structured args, NOT a pre-formed shell string (injection-safe).
    assert_eq!(r["result"]["args"][0], json!("run"));
    assert_eq!(r["result"]["args"][1], json!("--volatile"));
    assert_eq!(r["result"]["original_command"], json!("npm test"));
    assert!(
        r["result"].get("wrapper").is_none(),
        "must not emit a shell wrapper string"
    );
}

#[test]
fn test_unknown_command_is_rejected() {
    let r = dispatch_command("envforge.not.a.command", &[], None);
    assert_eq!(r["ok"], json!(false));
}

#[test]
fn test_reveal_value_requires_key() {
    let r = dispatch_command("envforge.reveal.value", &[json!({})], None);
    assert_eq!(r["ok"], json!(false), "reveal without a key must error");
}
