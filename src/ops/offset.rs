use crate::model::{LineNode, ShellFile};

pub const ENVFORGE_START_MARKER: &str = "# >>> envforge >>>";
pub const ENVFORGE_END_MARKER: &str = "# <<< envforge <<<";

pub struct ManagedZone {
    pub start_idx: usize,
    pub end_idx: usize,
}

pub fn find_managed_zone(shell_file: &ShellFile) -> Option<ManagedZone> {
    let start_idx = shell_file
        .lines
        .iter()
        .position(|node| matches!(node, LineNode::EnvforgeStart { .. }))?;
    let end_idx = shell_file
        .lines
        .iter()
        .position(|node| matches!(node, LineNode::EnvforgeEnd { .. }))?;
    if end_idx > start_idx {
        Some(ManagedZone { start_idx, end_idx })
    } else {
        None
    }
}

pub fn has_managed_zone(shell_file: &ShellFile) -> bool {
    find_managed_zone(shell_file).is_some()
}

/// A detected protected block in a shell config file.
#[derive(Debug, Clone)]
pub struct ProtectedBlock {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// The writable region of a file (between header and footer offsets).
#[derive(Debug, Clone)]
pub struct SafeZone {
    pub start: usize,
    pub end: usize,
}

impl SafeZone {
    /// Check if a line number falls within the safe zone.
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
    }

    /// Return the number of writable lines.
    pub fn size(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Known protected block marker patterns.
/// Detect known protected blocks in a shell file.
///
/// Scans for well-known marker patterns like conda, Amazon Q, nvm, etc.
pub fn detect_protected_blocks(shell_file: &ShellFile) -> Vec<ProtectedBlock> {
    let mut blocks = Vec::new();

    // Detect conda-style blocks with clear start/end markers
    detect_bounded_block(
        shell_file,
        "conda",
        "# >>> conda initialize >>>",
        "# <<< conda initialize <<<",
        &mut blocks,
    );

    // Detect Amazon Q blocks (they use paired pre/post markers)
    detect_line_block(shell_file, "Amazon Q (pre)", "# Q pre block.", &mut blocks);
    detect_line_block(
        shell_file,
        "Amazon Q (post)",
        "# Q post block.",
        &mut blocks,
    );

    blocks
}

/// Detect a block with explicit start and end markers.
fn detect_bounded_block(
    shell_file: &ShellFile,
    name: &str,
    start_marker: &str,
    end_marker: &str,
    blocks: &mut Vec<ProtectedBlock>,
) {
    let mut start_line = None;

    for node in &shell_file.lines {
        let text = node.original_text();
        if text.contains(start_marker) && start_line.is_none() {
            start_line = Some(node.line_number());
        }
        if text.contains(end_marker) && start_line.is_some() {
            blocks.push(ProtectedBlock {
                name: name.to_string(),
                start_line: start_line.unwrap(),
                end_line: node.line_number(),
            });
            start_line = None;
        }
    }
}

/// Detect a block that starts with a marker and extends to the next line
/// (for blocks like Amazon Q that have a comment + one action line).
fn detect_line_block(
    shell_file: &ShellFile,
    name: &str,
    marker: &str,
    blocks: &mut Vec<ProtectedBlock>,
) {
    for (i, node) in shell_file.lines.iter().enumerate() {
        if node.original_text().contains(marker) {
            let end = if i + 1 < shell_file.lines.len() {
                i + 1
            } else {
                i
            };
            blocks.push(ProtectedBlock {
                name: name.to_string(),
                start_line: node.line_number(),
                end_line: end,
            });
        }
    }
}

/// Calculate the safe zone given total lines and offset configuration.
///
/// Returns `None` if the offsets consume the entire file.
pub fn calculate_safe_zone(
    total_lines: usize,
    header_offset: usize,
    footer_offset: usize,
) -> Option<SafeZone> {
    let end = total_lines.saturating_sub(footer_offset);
    let start = header_offset;

    if start >= end {
        None
    } else {
        Some(SafeZone { start, end })
    }
}

/// Suggest header and footer offsets based on detected protected blocks.
///
/// Returns (header_offset, footer_offset).
pub fn suggest_offsets(shell_file: &ShellFile) -> (usize, usize) {
    let blocks = detect_protected_blocks(shell_file);
    let total = shell_file.lines.len();

    if blocks.is_empty() && find_managed_zone(shell_file).is_none() {
        return (0, 0);
    }

    let header = blocks
        .iter()
        .filter(|b| b.start_line <= 2)
        .map(|b| b.end_line + 1)
        .max()
        .unwrap_or(0);

    let footer = blocks
        .iter()
        .filter(|b| b.end_line >= total.saturating_sub(5))
        .map(|b| total - b.start_line)
        .max()
        .unwrap_or(0);

    (header, footer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_shell_content;
    use std::path::Path;

    fn make_shell_file(content: &str) -> ShellFile {
        parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
    }

    // ─── detect_protected_blocks ──────────────────────────────

    #[test]
    fn test_detect_conda_block() {
        let sf = make_shell_file(
            "# some stuff\n\
             # >>> conda initialize >>>\n\
             __conda_setup=1\n\
             # <<< conda initialize <<<\n\
             export FOO=\"bar\"",
        );
        let blocks = detect_protected_blocks(&sf);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "conda");
    }

    #[test]
    fn test_detect_amazon_q_blocks() {
        let sf = make_shell_file(
            "# Q pre block.\n\
             some_q_setup\n\
             export FOO=\"bar\"\n\
             # Q post block.\n\
             some_q_teardown",
        );
        let blocks = detect_protected_blocks(&sf);
        assert_eq!(blocks.len(), 2);
        assert!(blocks.iter().any(|b| b.name == "Amazon Q (pre)"));
        assert!(blocks.iter().any(|b| b.name == "Amazon Q (post)"));
    }

    #[test]
    fn test_detect_no_blocks() {
        let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=\"qux\"");
        let blocks = detect_protected_blocks(&sf);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_detect_multiple_blocks() {
        let sf = make_shell_file(
            "# >>> conda initialize >>>\n\
             conda_stuff\n\
             # <<< conda initialize <<<\n\
             export FOO=\"bar\"\n\
             # Q pre block.\n\
             q_stuff",
        );
        let blocks = detect_protected_blocks(&sf);
        assert_eq!(blocks.len(), 2);
    }

    // ─── calculate_safe_zone ──────────────────────────────────

    #[test]
    fn test_safe_zone_no_offsets() {
        let zone = calculate_safe_zone(10, 0, 0).unwrap();
        assert_eq!(zone.start, 0);
        assert_eq!(zone.end, 10);
    }

    #[test]
    fn test_safe_zone_with_header() {
        let zone = calculate_safe_zone(10, 3, 0).unwrap();
        assert_eq!(zone.start, 3);
        assert_eq!(zone.end, 10);
    }

    #[test]
    fn test_safe_zone_with_footer() {
        let zone = calculate_safe_zone(10, 0, 2).unwrap();
        assert_eq!(zone.start, 0);
        assert_eq!(zone.end, 8);
    }

    #[test]
    fn test_safe_zone_consumed_entirely() {
        assert!(calculate_safe_zone(5, 3, 3).is_none());
    }

    // ─── suggest_offsets ──────────────────────────────────────

    #[test]
    fn test_suggest_offsets_empty_file() {
        let sf = make_shell_file("");
        let (h, f) = suggest_offsets(&sf);
        assert_eq!(h, 0);
        assert_eq!(f, 0);
    }

    #[test]
    fn test_suggest_offsets_with_conda_at_top() {
        let sf = make_shell_file(
            "# >>> conda initialize >>>\n\
             conda_stuff\n\
             # <<< conda initialize <<<\n\
             export FOO=\"bar\"",
        );
        let (h, _f) = suggest_offsets(&sf);
        assert!(h > 0, "Header offset should be > 0 with conda at top");
    }

    // ─── SafeZone methods ─────────────────────────────────────

    #[test]
    fn test_safe_zone_contains() {
        let zone = SafeZone { start: 3, end: 7 };
        assert!(!zone.contains(2));
        assert!(zone.contains(3));
        assert!(zone.contains(6));
        assert!(!zone.contains(7));
    }

    #[test]
    fn test_safe_zone_size() {
        let zone = SafeZone { start: 3, end: 7 };
        assert_eq!(zone.size(), 4);
    }
}
