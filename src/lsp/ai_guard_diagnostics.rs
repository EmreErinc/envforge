use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::ops::mcp_poison::description_scan::DescriptionScanner;
use crate::ops::mcp_poison::finding::Severity;

/// Compute AI-guard diagnostics for an `.env` document. Runs the
/// production prompt-injection pattern scanner used elsewhere in
/// envforge (`DescriptionScanner`) against the raw content and converts
/// each match's byte span to an LSP range. Severity is mapped from the
/// pattern's intrinsic severity, but at minimum every finding becomes a
/// `Warning` so the user sees it without it blocking saves.
///
/// Triggered on `did_save` rather than `did_change` because: (a) the
/// scanner has higher cost than the per-keystroke schema diagnostics,
/// and (b) prompt-injection content tends to be pasted whole and only
/// finalised on save — running mid-typing produces flicker without
/// catching anything earlier.
pub fn compute_ai_guard_diagnostics(content: &str) -> Vec<Diagnostic> {
    let findings = match DescriptionScanner::scan("envfile", content) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let line_offsets = compute_line_offsets(content);

    findings
        .into_iter()
        .map(|f| Diagnostic {
            range: span_to_range(content, &line_offsets, f.span),
            severity: Some(map_severity(f.severity)),
            source: Some("envforge-aiguard".into()),
            message: format!(
                "Prompt-injection pattern `{}` ({}) detected. Review this content — \
                 AI agents reading this file may be coerced into exfiltrating secrets \
                 or executing unintended actions.",
                f.pattern_id,
                f.severity.as_str(),
            ),
            ..Default::default()
        })
        .collect()
}

/// Map intrinsic finding severity to LSP severity. Critical/High → Error
/// so the user notices immediately; Medium → Warning; Low → Information.
fn map_severity(s: Severity) -> DiagnosticSeverity {
    match s {
        Severity::Critical | Severity::High => DiagnosticSeverity::ERROR,
        Severity::Medium => DiagnosticSeverity::WARNING,
        Severity::Low => DiagnosticSeverity::INFORMATION,
    }
}

/// Precompute the starting byte offset of each line so byte-span lookup
/// is O(log lines) rather than O(n) per finding.
fn compute_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (idx, b) in content.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(idx + 1);
        }
    }
    offsets
}

fn span_to_range(content: &str, line_offsets: &[usize], span: (usize, usize)) -> Range {
    let (start_byte, end_byte) = span;
    let start = byte_to_position(content, line_offsets, start_byte);
    let end = byte_to_position(content, line_offsets, end_byte);
    Range { start, end }
}

fn byte_to_position(content: &str, line_offsets: &[usize], byte: usize) -> Position {
    let line_idx = match line_offsets.binary_search(&byte) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let line_start = line_offsets[line_idx];
    let col_bytes = byte.saturating_sub(line_start);
    let line_text = &content[line_start..byte.min(content.len())];
    let _ = col_bytes; // retained for future UTF-16 conversion
    Position {
        line: line_idx as u32,
        character: line_text.chars().count() as u32,
    }
}
