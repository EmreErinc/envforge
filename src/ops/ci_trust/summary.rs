//! Step Summary markdown rendering + GitHub Output emission for CI trust verdicts.

use std::fmt::Write;

use super::classifier::{TrustLevel, TrustReason, TrustVerdict};
use super::quarantine::ScrubReport;

fn emoji_for(level: TrustLevel) -> &'static str {
    match level {
        TrustLevel::Trusted => "🟢",
        TrustLevel::Suspicious => "🟡",
        TrustLevel::Untrusted => "🔴",
    }
}

fn reason_label(reason: TrustReason) -> &'static str {
    match reason {
        TrustReason::Push => "push",
        TrustReason::InternalPr => "internal_pr",
        TrustReason::ForkPr => "fork_pr",
        TrustReason::PullRequestTarget => "pull_request_target",
        TrustReason::ExternalComment => "external_comment",
        TrustReason::WorkflowRunChain => "workflow_run_chain",
        TrustReason::Schedule => "schedule",
        TrustReason::WorkflowDispatch => "workflow_dispatch",
        TrustReason::Unknown => "unknown",
    }
}

fn level_label(level: TrustLevel) -> &'static str {
    match level {
        TrustLevel::Trusted => "Trusted",
        TrustLevel::Suspicious => "Suspicious",
        TrustLevel::Untrusted => "Untrusted",
    }
}

/// Render a Step Summary markdown block for the verdict + (optional) scrub report.
pub fn render_step_summary(verdict: &TrustVerdict, report: Option<&ScrubReport>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "## EnvForge Trust Verdict");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "**Verdict**: {} {}",
        emoji_for(verdict.level),
        level_label(verdict.level)
    );
    let _ = writeln!(s, "**Reason**: `{}`", reason_label(verdict.reason));

    if let Some(r) = report {
        let _ = writeln!(s, "**Quarantine**: applied");
        let _ = writeln!(s, "**Scrubbed keys**: {}", r.scrubbed_keys.len());
        let _ = writeln!(s, "**Preserved keys**: {}", r.preserved_keys.len());
        if !r.scrubbed_keys.is_empty() {
            let _ = writeln!(s);
            let _ = writeln!(s, "<details>");
            let _ = writeln!(s, "<summary>Scrubbed key names</summary>");
            let _ = writeln!(s);
            for hit in &r.pattern_hits {
                let via_label = match hit.via {
                    super::quarantine::MaskedVia::KeyName => "key-name match",
                    super::quarantine::MaskedVia::ValueShape => "value-shape match",
                };
                let _ = writeln!(s, "- `{}` ({})", hit.key, via_label);
            }
            let _ = writeln!(s);
            let _ = writeln!(s, "</details>");
        }
    } else if matches!(verdict.level, TrustLevel::Untrusted) {
        let _ = writeln!(s, "**Quarantine**: not applied");
        let _ = writeln!(
            s,
            "> Set `quarantine: auto` (default) or `quarantine: force` to mask secrets on untrusted triggers."
        );
    } else {
        let _ = writeln!(s, "**Quarantine**: not needed");
    }

    s
}

/// Append `key=value` lines to `$GITHUB_OUTPUT` for downstream workflow steps.
pub fn emit_action_outputs(
    verdict: &TrustVerdict,
    report: Option<&ScrubReport>,
) -> std::io::Result<()> {
    let mut out = String::new();
    let _ = writeln!(out, "quarantine_verdict={}", level_label(verdict.level));
    let _ = writeln!(out, "quarantine_reason={}", reason_label(verdict.reason));
    let applied = report.is_some();
    let _ = writeln!(out, "quarantine_applied={}", applied);
    if let Some(r) = report {
        let _ = writeln!(out, "quarantine_scrubbed_count={}", r.scrubbed_keys.len());
        let _ = writeln!(out, "quarantine_preserved_count={}", r.preserved_keys.len());
    } else {
        let _ = writeln!(out, "quarantine_scrubbed_count=0");
        let _ = writeln!(out, "quarantine_preserved_count=0");
    }

    if let Ok(p) = std::env::var("GITHUB_OUTPUT") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)?;
        f.write_all(out.as_bytes())?;
        Ok(())
    } else {
        // Outside Actions: print to stderr with a marker so it's still inspectable.
        for line in out.lines() {
            eprintln!("OUTPUT::{line}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::ci_trust::classifier::CLASSIFIER_VERSION;
    use crate::ops::ci_trust::quarantine::{MaskHit, MaskedVia, ScrubReport};

    fn untrusted_fork() -> TrustVerdict {
        TrustVerdict {
            level: TrustLevel::Untrusted,
            reason: TrustReason::ForkPr,
            classifier_version: CLASSIFIER_VERSION,
        }
    }

    fn trusted_push() -> TrustVerdict {
        TrustVerdict {
            level: TrustLevel::Trusted,
            reason: TrustReason::Push,
            classifier_version: CLASSIFIER_VERSION,
        }
    }

    fn report_with(scrubbed: &[(&str, MaskedVia)]) -> ScrubReport {
        ScrubReport {
            scrubbed_keys: scrubbed.iter().map(|(k, _)| (*k).to_string()).collect(),
            preserved_keys: vec!["FOO".into()],
            pattern_hits: scrubbed
                .iter()
                .map(|(k, v)| MaskHit {
                    key: (*k).to_string(),
                    via: *v,
                })
                .collect(),
        }
    }

    #[test]
    fn summary_includes_verdict_and_reason() {
        let s = render_step_summary(&untrusted_fork(), None);
        assert!(s.contains("Untrusted"));
        assert!(s.contains("fork_pr"));
        assert!(s.contains("🔴"));
    }

    #[test]
    fn summary_trusted_has_no_quarantine_section_details() {
        let s = render_step_summary(&trusted_push(), None);
        assert!(s.contains("Trusted"));
        assert!(s.contains("not needed"));
        assert!(!s.contains("Scrubbed keys"));
    }

    #[test]
    fn summary_with_report_lists_scrubbed_keys() {
        let r = report_with(&[
            ("STRIPE_KEY", MaskedVia::KeyName),
            ("AWS_KEY", MaskedVia::ValueShape),
        ]);
        let s = render_step_summary(&untrusted_fork(), Some(&r));
        assert!(s.contains("Quarantine"));
        assert!(s.contains("STRIPE_KEY"));
        assert!(s.contains("AWS_KEY"));
        assert!(s.contains("key-name match"));
        assert!(s.contains("value-shape match"));
    }

    #[test]
    fn summary_untrusted_without_report_warns_user() {
        let s = render_step_summary(&untrusted_fork(), None);
        assert!(s.contains("not applied"));
        assert!(s.contains("quarantine"));
    }
}
