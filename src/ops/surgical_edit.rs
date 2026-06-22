//! Shared surgical byte-range splice utility — Intent 038, Unit 001 (FR1).
//!
//! [`SurgicalEdit`] replaces an exact byte range in a source buffer with
//! replacement text, leaving every byte outside that range byte-identical
//! *by construction* (locate → splice; no whole-document re-serialisation).
//!
//! This is the format-agnostic write primitive for all opinionated config
//! formats (YAML in intent 038, TOML/JSON in later intents). It is
//! deliberately free of any YAML-specific logic.
//!
//! # Round-trip guarantee
//!
//! Given `output = SurgicalEdit::apply(source, range, replacement)`:
//!
//! - `output[..range.start]  == source[..range.start]`   (byte-identical prefix)
//! - `output[range.start..range.start + replacement.len()] == replacement.as_bytes()`
//! - `output[range.start + replacement.len()..]  == source[range.end..]` (byte-identical suffix)
//!
//! This invariant holds *by construction* — the function never touches bytes
//! outside the range and never re-serialises the document.
//!
//! # LSP integration
//!
//! [`SurgicalEdit::to_text_edit`] converts the same splice into an LSP
//! `TextEdit` with UTF-16-correct positions, reusing the
//! `byte_offset_to_utf16_col` helper present in the 036 codebase.
//!
//! # Usage
//!
//! ```
//! use envforge::ops::surgical_edit::SurgicalEdit;
//!
//! let source = "spring:\n  port: 8080\n";
//! // Locate "8080" and replace with "9090".
//! let start = source.find("8080").unwrap();
//! let end = start + 4;
//! let result = SurgicalEdit::apply(source, start..end, "9090");
//! assert!(result.is_some());
//! assert_eq!(result.unwrap(), "spring:\n  port: 9090\n");
//! ```

use std::ops::Range;

use tower_lsp::lsp_types::{Position, Range as LspRange, TextEdit};

// ── Byte → UTF-16 conversion ─────────────────────────────────────────────────

/// Convert a byte offset within `line` to a UTF-16 code-unit column.
///
/// `line` must not contain a newline character. Returns the column clamped
/// to the number of UTF-16 units in the line when `byte_offset` is beyond
/// the end of the line.
///
/// This mirrors the helper in `src/lsp/config_features.rs` (the 036 bug class).
fn byte_offset_to_utf16_col(line: &str, byte_offset: usize) -> u32 {
    let mut units: u32 = 0;
    let mut byte_pos = 0usize;
    for ch in line.chars() {
        if byte_pos >= byte_offset {
            break;
        }
        units += ch.len_utf16() as u32;
        byte_pos += ch.len_utf8();
    }
    units
}

// ── Position helpers ──────────────────────────────────────────────────────────

/// Convert a byte offset within `content` to a UTF-16 LSP `Position`.
///
/// Returns `Position { line: 0, character: 0 }` when `content` is empty
/// or `byte_offset` is out of bounds.
pub fn byte_offset_to_lsp_position(content: &str, byte_offset: usize) -> Position {
    if byte_offset > content.len() {
        return Position {
            line: 0,
            character: 0,
        };
    }
    let before = &content[..byte_offset];
    // Count 0-based line number and locate the line start.
    let line_num = before.bytes().filter(|&b| b == b'\n').count();
    let line_start_byte = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let col_slice = &content[line_start_byte..byte_offset];
    Position {
        line: line_num as u32,
        character: byte_offset_to_utf16_col(col_slice, col_slice.len()),
    }
}

// ── SurgicalEdit ─────────────────────────────────────────────────────────────

/// A format-agnostic byte-range splice.
///
/// Built from a source buffer, a byte range to replace, and the replacement
/// text. All work is done lazily — `apply` or `to_text_edit` execute the
/// splice.
#[derive(Debug, Clone)]
pub struct SurgicalEdit {
    /// Byte range within the source to replace.
    pub byte_range: Range<usize>,
    /// Replacement text (raw bytes, typically a UTF-8 string).
    pub replacement: String,
}

impl SurgicalEdit {
    /// Construct a new `SurgicalEdit`.
    ///
    /// Returns `None` when `byte_range` is inverted (`start > end`) or
    /// either bound is out of range for `source_len`.
    pub fn new(
        byte_range: Range<usize>,
        replacement: impl Into<String>,
        source_len: usize,
    ) -> Option<Self> {
        if byte_range.start > byte_range.end || byte_range.end > source_len {
            return None;
        }
        Some(Self {
            byte_range,
            replacement: replacement.into(),
        })
    }

    /// Apply the splice to `source`, returning the new string.
    ///
    /// Returns `None` when the byte range is invalid for `source`.
    ///
    /// **Round-trip safety**: only `source[byte_range]` changes; every other
    /// byte is identical to `source`.
    pub fn apply(source: &str, byte_range: Range<usize>, replacement: &str) -> Option<String> {
        let se = Self::new(byte_range, replacement, source.len())?;
        // Verify the range boundaries are on valid UTF-8 char boundaries.
        if !source.is_char_boundary(se.byte_range.start)
            || !source.is_char_boundary(se.byte_range.end)
        {
            return None;
        }
        let mut out = String::with_capacity(
            se.byte_range.start + se.replacement.len() + (source.len() - se.byte_range.end),
        );
        out.push_str(&source[..se.byte_range.start]);
        out.push_str(&se.replacement);
        out.push_str(&source[se.byte_range.end..]);
        Some(out)
    }

    /// Convert this splice into an LSP `TextEdit` with UTF-16-correct positions.
    ///
    /// Returns `None` when the byte range is invalid for `content` or the
    /// start/end byte offsets are not on UTF-8 character boundaries.
    pub fn to_text_edit(&self, content: &str) -> Option<TextEdit> {
        if self.byte_range.start > self.byte_range.end
            || self.byte_range.end > content.len()
            || !content.is_char_boundary(self.byte_range.start)
            || !content.is_char_boundary(self.byte_range.end)
        {
            return None;
        }

        let start_pos = byte_offset_to_lsp_position(content, self.byte_range.start);
        let end_pos = byte_offset_to_lsp_position(content, self.byte_range.end);

        Some(TextEdit {
            range: LspRange {
                start: start_pos,
                end: end_pos,
            },
            new_text: self.replacement.clone(),
        })
    }

    /// Verify byte-identity outside the spliced range.
    ///
    /// Given the `original` source, `edited` output, and the `byte_range` of
    /// the splice (in original coordinates), asserts that every byte outside
    /// the spliced range is identical.
    ///
    /// Returns `true` when the invariant holds; `false` otherwise.
    ///
    /// Primarily used in tests — this is the byte-identity harness.
    pub fn assert_byte_identity(
        original: &str,
        edited: &str,
        original_range: Range<usize>,
        replacement_len: usize,
    ) -> bool {
        let original_bytes = original.as_bytes();
        let edited_bytes = edited.as_bytes();

        // Prefix must be identical.
        if original_bytes[..original_range.start] != edited_bytes[..original_range.start] {
            return false;
        }

        // Suffix must be identical.
        let orig_suffix_start = original_range.end;
        let edit_suffix_start = original_range.start + replacement_len;

        let orig_suffix = &original_bytes[orig_suffix_start..];
        let edit_suffix_end = edit_suffix_start
            .checked_add(orig_suffix.len())
            .filter(|&end| end <= edited_bytes.len())
            .unwrap_or(edited_bytes.len());
        let edit_suffix = &edited_bytes[edit_suffix_start..edit_suffix_end];

        orig_suffix == edit_suffix
    }
}

#[cfg(test)]
mod tests {
    // All tests live in tests/yaml_writes_tests.rs per CLAUDE.md conventions.
    // This mod block is intentionally empty.
}
