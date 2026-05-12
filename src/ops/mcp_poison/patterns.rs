//! Pattern set v1 for poisoning detection. Compiled-once via `OnceLock`.
//!
//! Per ADR-013 (non-backtracking-engine policy): all regexes use the
//! `regex` crate (no lookbehind / no backreferences) with bounded
//! `size_limit` and `dfa_size_limit`.

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};

use crate::ops::mcp_poison::finding::Severity;

pub const PATTERN_SET_VERSION: &str = "2026-05-12";

/// One detection rule.
pub struct Pattern {
    pub id: &'static str,
    pub kind: PatternKind,
    pub severity: Severity,
}

pub enum PatternKind {
    Regex(&'static Regex),
    Substring(&'static str),
    UnicodeRange { start: u32, end: u32 },
}

impl Pattern {
    pub fn find_all(&self, text: &str) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        match &self.kind {
            PatternKind::Regex(re) => {
                for m in re.find_iter(text) {
                    spans.push((m.start(), m.end()));
                }
            }
            PatternKind::Substring(needle) => {
                let needle_lc = needle.to_lowercase();
                let hay = text.to_lowercase();
                let mut start = 0usize;
                while let Some(idx) = hay[start..].find(&needle_lc) {
                    let abs = start + idx;
                    spans.push((abs, abs + needle.len()));
                    start = abs + needle.len();
                    if start >= hay.len() {
                        break;
                    }
                }
            }
            PatternKind::UnicodeRange { start, end } => {
                let s = *start;
                let e = *end;
                let mut byte_idx = 0usize;
                for c in text.chars() {
                    let cp = c as u32;
                    if cp >= s && cp <= e {
                        spans.push((byte_idx, byte_idx + c.len_utf8()));
                    }
                    byte_idx += c.len_utf8();
                }
            }
        }
        spans
    }
}

fn build_regex(src: &str) -> Regex {
    RegexBuilder::new(src)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .case_insensitive(true)
        .build()
        .expect("static pattern compiles")
}

macro_rules! re_pattern {
    ($lock:ident, $src:expr) => {{
        static $lock: OnceLock<Regex> = OnceLock::new();
        $lock.get_or_init(|| build_regex($src))
    }};
}

fn ignore_previous() -> &'static Regex {
    re_pattern!(
        IGNORE_PREVIOUS,
        r"ignore (all )?(previous|prior|above) (instructions?|prompts?|context|rules?)"
    )
}

fn disregard_synonyms() -> &'static Regex {
    re_pattern!(
        DISREGARD_SYNONYMS,
        r"(disregard|forget|override|bypass|skip) (all |the )?(previous|prior|above|earlier) (instructions?|rules?|prompts?)"
    )
}

fn new_instructions() -> &'static Regex {
    re_pattern!(
        NEW_INSTRUCTIONS,
        r"(new|updated|revised|fresh) (instructions?:|rules?:|prompt:)"
    )
}

fn role_marker_newline() -> &'static Regex {
    re_pattern!(
        ROLE_MARKER_NEWLINE,
        r"\n\s*(system|assistant|human|user)\s*:"
    )
}

fn xml_role_tag() -> &'static Regex {
    re_pattern!(
        XML_ROLE_TAG,
        r"</?(system|user|assistant|function_calls|tool_use)>"
    )
}

fn exfil_keywords() -> &'static Regex {
    re_pattern!(
        EXFIL_KEYWORDS,
        r"(exfiltrate|leak|send to|curl|webhook|fetch|post).{0,40}(key|token|secret|credential)"
    )
}

/// The full pattern registry. Returns a fresh Vec on each call; the
/// underlying regex compilations are `OnceLock`-cached so this is cheap.
pub fn all_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            id: "ignore_previous",
            kind: PatternKind::Regex(ignore_previous()),
            severity: Severity::Critical,
        },
        Pattern {
            id: "disregard_synonyms",
            kind: PatternKind::Regex(disregard_synonyms()),
            severity: Severity::Critical,
        },
        Pattern {
            id: "new_instructions",
            kind: PatternKind::Regex(new_instructions()),
            severity: Severity::High,
        },
        Pattern {
            id: "role_marker_newline",
            kind: PatternKind::Regex(role_marker_newline()),
            severity: Severity::High,
        },
        Pattern {
            id: "xml_role_tag",
            kind: PatternKind::Regex(xml_role_tag()),
            severity: Severity::High,
        },
        Pattern {
            id: "claude_meta_open",
            kind: PatternKind::Substring("<system-reminder>"),
            severity: Severity::Critical,
        },
        Pattern {
            id: "claude_meta_close",
            kind: PatternKind::Substring("</claude_code>"),
            severity: Severity::Critical,
        },
        Pattern {
            id: "tool_call_inject_open",
            kind: PatternKind::Substring("<function_calls>"),
            severity: Severity::Critical,
        },
        Pattern {
            id: "tool_call_inject_close",
            kind: PatternKind::Substring("</function_calls>"),
            severity: Severity::Critical,
        },
        Pattern {
            id: "tool_call_inject_invoke",
            kind: PatternKind::Substring("<invoke name="),
            severity: Severity::Critical,
        },
        Pattern {
            id: "unicode_tag_smuggle",
            kind: PatternKind::UnicodeRange {
                start: 0xE0000,
                end: 0xE007F,
            },
            severity: Severity::Critical,
        },
        // Zero-width chars: 0x200B-0x200F contiguous range
        Pattern {
            id: "zero_width_chars",
            kind: PatternKind::UnicodeRange {
                start: 0x200B,
                end: 0x200F,
            },
            severity: Severity::High,
        },
        // Zero-width BOM
        Pattern {
            id: "zero_width_bom",
            kind: PatternKind::UnicodeRange {
                start: 0xFEFF,
                end: 0xFEFF,
            },
            severity: Severity::High,
        },
        // Word joiner
        Pattern {
            id: "zero_width_word_joiner",
            kind: PatternKind::UnicodeRange {
                start: 0x2060,
                end: 0x2060,
            },
            severity: Severity::High,
        },
        Pattern {
            id: "bidi_override_a",
            kind: PatternKind::UnicodeRange {
                start: 0x202A,
                end: 0x202E,
            },
            severity: Severity::High,
        },
        Pattern {
            id: "bidi_override_b",
            kind: PatternKind::UnicodeRange {
                start: 0x2066,
                end: 0x2069,
            },
            severity: Severity::High,
        },
        Pattern {
            id: "exfil_keywords",
            kind: PatternKind::Regex(exfil_keywords()),
            severity: Severity::Critical,
        },
    ]
}
