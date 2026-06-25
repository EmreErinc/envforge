//! Safe-by-construction finding emission. Terminal, JSON, and
//! audit-event views never expose raw matched text.

use serde_json::{json, Value as JsonValue};

use crate::ops::mcp_poison::finding::PoisonFinding;

pub struct FindingsEmitter;

impl FindingsEmitter {
    /// Render findings as a multi-line terminal report. The
    /// `unsafe_show_payload` flag has no raw-text path to expose;
    /// it only toggles whether the redacted marker is
    /// shown alongside additional canonicalization metadata.
    pub fn emit_terminal(findings: &[PoisonFinding], unsafe_show_payload: bool) -> String {
        let mut out = String::new();
        for f in findings {
            if unsafe_show_payload {
                out.push_str(&format!(
                    "[{}] tool={} {} (canonical={} layer={})\n  marker: {}\n  sha256: {}\n",
                    f.severity.as_str(),
                    f.tool_name,
                    f.pattern_id,
                    f.canonicalized,
                    f.decoded_layer,
                    f.redacted_marker,
                    f.text_sha256_hex(),
                ));
            } else {
                out.push_str(&format!(
                    "[{}] {} on '{}': {}\n",
                    f.severity.as_str(),
                    f.pattern_id,
                    f.tool_name,
                    f.redacted_marker,
                ));
            }
        }
        out
    }

    pub fn emit_json(findings: &[PoisonFinding]) -> JsonValue {
        let items: Vec<JsonValue> = findings
            .iter()
            .map(|f| {
                json!({
                    "tool_name": f.tool_name,
                    "pattern_id": f.pattern_id,
                    "pattern_set_version": f.pattern_set_version,
                    "severity": f.severity.as_str(),
                    "span": [f.span.0, f.span.1],
                    "text_sha256": f.text_sha256_hex(),
                    "canonicalized": f.canonicalized,
                    "decoded_layer": f.decoded_layer,
                    "redacted_marker": f.redacted_marker,
                })
            })
            .collect();
        json!({ "findings": items })
    }

    /// Per-finding audit-event payload. Explicitly omits any field that
    /// could carry attacker-controlled text.
    pub fn emit_audit_event(finding: &PoisonFinding) -> JsonValue {
        json!({
            "tool_name": finding.tool_name,
            "pattern_id": finding.pattern_id,
            "pattern_set_version": finding.pattern_set_version,
            "severity": finding.severity.as_str(),
            "text_sha256": finding.text_sha256_hex(),
            "canonicalized": finding.canonicalized,
            "decoded_layer": finding.decoded_layer,
        })
    }
}
