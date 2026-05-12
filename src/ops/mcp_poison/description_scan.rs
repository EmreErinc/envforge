//! Per-tool description scanner.
//!
//! Scans raw input first, then canonical form, and dedups findings by
//! `(pattern_id, span)` keeping the most-severe entry.

use crate::ops::mcp_poison::canonical::Canonicalizer;
use crate::ops::mcp_poison::error::ScannerError;
use crate::ops::mcp_poison::finding::PoisonFinding;
use crate::ops::mcp_poison::patterns::all_patterns;

pub struct DescriptionScanner;

impl DescriptionScanner {
    pub const MAX_FINDINGS_PER_INPUT: usize = 100;

    pub fn scan(tool_name: &str, description: &str) -> Result<Vec<PoisonFinding>, ScannerError> {
        let mut findings: Vec<PoisonFinding> = Vec::new();

        // Pass 1: raw text
        Self::scan_text(tool_name, description, false, &mut findings);

        // Pass 2: canonical form
        let canonical = Canonicalizer::canonicalize(description)?;
        Self::scan_text(tool_name, canonical.as_str(), true, &mut findings);

        Self::dedup(&mut findings);
        Ok(findings)
    }

    fn scan_text(
        tool_name: &str,
        text: &str,
        canonicalized: bool,
        findings: &mut Vec<PoisonFinding>,
    ) {
        for pattern in all_patterns() {
            for (start, end) in pattern.find_all(text) {
                if findings.len() >= Self::MAX_FINDINGS_PER_INPUT {
                    return;
                }
                let matched_text = &text[start..end];
                findings.push(PoisonFinding::new(
                    tool_name,
                    pattern.id,
                    pattern.severity,
                    (start, end),
                    matched_text,
                    0,
                    canonicalized,
                ));
            }
        }
    }

    fn dedup(findings: &mut Vec<PoisonFinding>) {
        // Sort by (pattern_id, span, severity-desc, canonicalized-desc) so
        // when we dedup adjacent entries we keep the most-severe one and
        // prefer raw over canonical (raw matches are more reviewer-friendly).
        findings.sort_by(|a, b| {
            a.pattern_id
                .cmp(b.pattern_id)
                .then(a.span.cmp(&b.span))
                .then(b.severity.cmp(&a.severity))
                .then(a.canonicalized.cmp(&b.canonicalized))
        });
        findings.dedup_by(|a, b| a.pattern_id == b.pattern_id && a.span == b.span);
    }
}
