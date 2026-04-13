use crate::model::ShellFile;

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

    if blocks.is_empty() {
        return (0, 0);
    }

    // Header offset: largest end_line of blocks that start at line 0-2
    let header = blocks
        .iter()
        .filter(|b| b.start_line <= 2)
        .map(|b| b.end_line + 1)
        .max()
        .unwrap_or(0);

    // Footer offset: for blocks near the end of file
    let footer = blocks
        .iter()
        .filter(|b| b.end_line >= total.saturating_sub(5))
        .map(|b| total - b.start_line)
        .max()
        .unwrap_or(0);

    (header, footer)
}
