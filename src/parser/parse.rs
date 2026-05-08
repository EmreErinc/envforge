use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::model::{ExportStyle, LineNode, ParseError, QuoteStyle, ShellFile};

const ENVFORGE_START_MARKER: &str = "# >>> envforge >>>";
const ENVFORGE_END_MARKER: &str = "# <<< envforge <<<";

// Lazy-initialized regexes for parsing shell files (compiled once at first use)
fn envforge_tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^#\[envforge:([^\]]+)\]\s*(.*)$").expect("invalid envforge tag regex")
    })
}

fn source_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(?:source|\.)\s+(.+)$").expect("invalid source regex"))
}

fn export_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"^\s*(export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*?)$"#)
            .expect("invalid export regex")
    })
}

fn comment_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*#(.*)$").expect("invalid comment regex"))
}

fn blank_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*$").expect("invalid blank regex"))
}

/// Parse a shell configuration file into a ShellFile AST.
///
/// Maximum size of a single shell file we will read and parse.
/// `.zshrc` / `.bashrc` / `.envrc` files are typically a few KB; even
/// large dotfiles top out at a few hundred KB. 10 MiB is generous and
/// rejects crafted / accidentally-huge files (e.g. a symlink to
/// `/dev/zero` or a multi-GB file in a user-pointed dir scanned by
/// `envforge check` / `envforge doctor`). Without this cap, a single
/// pathological file OOMs the process before parsing begins.
pub const MAX_SHELL_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Reads the file, computes its SHA-256 hash, and parses each line
/// into the appropriate LineNode variant.
pub fn parse_shell_file(path: &Path) -> Result<ShellFile, ParseError> {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_SHELL_FILE_BYTES {
            return Err(ParseError::FileTooLarge {
                path: path.to_path_buf(),
                size: meta.len(),
                limit: MAX_SHELL_FILE_BYTES,
            });
        }
    }

    let content = std::fs::read_to_string(path).map_err(|e| ParseError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    parse_shell_content(&content, path)
}

/// Parse shell content from a string (useful for testing).
pub fn parse_shell_content(content: &str, path: &Path) -> Result<ShellFile, ParseError> {
    let hash = compute_hash(content.as_bytes());
    let lines = parse_lines(content);

    Ok(ShellFile {
        path: path.to_path_buf(),
        lines,
        hash,
    })
}

fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

fn parse_lines(content: &str) -> Vec<LineNode> {
    let raw_lines = split_preserving_content(content);

    raw_lines
        .into_iter()
        .enumerate()
        .map(|(idx, line)| {
            parse_single_line(
                idx,
                line,
                envforge_tag_regex(),
                source_regex(),
                export_regex(),
                comment_regex(),
                blank_regex(),
            )
        })
        .collect()
}

/// Split content into lines, preserving the original text of each line
/// (without the line ending, which we'll rejoin with \n on serialize).
fn split_preserving_content(content: &str) -> Vec<String> {
    if content.is_empty() {
        return vec![];
    }

    let mut lines: Vec<String> = content.split('\n').map(String::from).collect();

    // If the content ended with a newline, split produces an extra empty string.
    // We keep it to preserve the trailing newline on serialization.
    // But if the last element is empty and content ends with \n, that's the trailing newline marker.
    if content.ends_with('\n') && lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    lines
}

fn parse_single_line(
    line_number: usize,
    line: String,
    envforge_tag_re: &Regex,
    source_re: &Regex,
    export_re: &Regex,
    comment_re: &Regex,
    blank_re: &Regex,
) -> LineNode {
    // Priority 0: EnvForge managed zone markers
    let trimmed = line.trim();
    if trimmed == ENVFORGE_START_MARKER {
        return LineNode::EnvforgeStart {
            line_number,
            original_text: line,
        };
    }
    if trimmed == ENVFORGE_END_MARKER {
        return LineNode::EnvforgeEnd {
            line_number,
            original_text: line,
        };
    }

    // Priority 1: EnvForge managed comments
    if let Some(caps) = envforge_tag_re.captures(&line) {
        let tag = caps[1].to_string();
        let original_export = caps[2].to_string();
        return LineNode::ManagedComment {
            line_number,
            original_text: line,
            tag,
            original_export,
        };
    }

    // Priority 2: Source directives
    if let Some(caps) = source_re.captures(&line) {
        // But only if it's not a comment
        if !line.trim_start().starts_with('#') {
            let path = caps[1].to_string();
            return LineNode::SourceDirective {
                line_number,
                original_text: line,
                path,
            };
        }
    }

    // Priority 3: Export statements
    if let Some(caps) = export_re.captures(&line) {
        // Exclude lines that start with #
        if !line.trim_start().starts_with('#') {
            let export_keyword = caps.get(1);
            let key = caps[2].to_string();
            let raw_value_and_rest = caps[3].to_string();

            let export_style = if export_keyword.is_some() {
                ExportStyle::Export
            } else {
                ExportStyle::Bare
            };

            let (value, quote_style, inline_comment) = parse_value_and_comment(&raw_value_and_rest);

            return LineNode::EnvExport {
                line_number,
                original_text: line,
                key,
                value,
                export_style,
                quote_style,
                inline_comment,
            };
        }
    }

    // Priority 4: Blank lines
    if blank_re.is_match(&line) {
        return LineNode::Blank {
            line_number,
            original_text: line,
        };
    }

    // Priority 5: Comments (must be after envforge tag check)
    if let Some(caps) = comment_re.captures(&line) {
        let text = caps[1].to_string();
        return LineNode::Comment {
            line_number,
            original_text: line,
            text,
        };
    }

    // Priority 6: Everything else
    LineNode::Other {
        line_number,
        original_text: line,
    }
}

/// Parse the value portion of an export statement, extracting quote style and inline comment.
fn parse_value_and_comment(raw: &str) -> (String, QuoteStyle, Option<String>) {
    let trimmed = raw.trim_start();

    if trimmed.starts_with('"') {
        // Double-quoted value
        parse_quoted_value(trimmed, '"', QuoteStyle::Double)
    } else if trimmed.starts_with('\'') {
        // Single-quoted value
        parse_quoted_value(trimmed, '\'', QuoteStyle::Single)
    } else {
        // Unquoted value — comment starts at first unescaped #
        parse_unquoted_value(trimmed)
    }
}

fn parse_quoted_value(
    s: &str,
    quote_char: char,
    style: QuoteStyle,
) -> (String, QuoteStyle, Option<String>) {
    // Skip the opening quote
    let inner = &s[1..];

    let mut value = String::new();
    let mut chars = inner.chars().peekable();
    let mut found_closing = false;
    let mut rest_after_quote = String::new();

    while let Some(ch) = chars.next() {
        if ch == '\\' && quote_char == '"' {
            // In double quotes, backslash escapes next char
            if let Some(next) = chars.next() {
                value.push('\\');
                value.push(next);
            } else {
                value.push('\\');
            }
        } else if ch == quote_char {
            found_closing = true;
            rest_after_quote = chars.collect();
            break;
        } else {
            value.push(ch);
        }
    }

    if !found_closing {
        // Unterminated quote — treat entire thing as value
        return (format!("{}{}", quote_char, inner), QuoteStyle::None, None);
    }

    let inline_comment = extract_inline_comment(&rest_after_quote);

    (value, style, inline_comment)
}

fn parse_unquoted_value(s: &str) -> (String, QuoteStyle, Option<String>) {
    // Find first # that looks like a comment (preceded by whitespace)
    if let Some(pos) = find_inline_comment_start(s) {
        let value = s[..pos].trim_end().to_string();
        let comment = s[pos..].to_string();
        (value, QuoteStyle::None, Some(comment))
    } else {
        (s.to_string(), QuoteStyle::None, None)
    }
}

fn extract_inline_comment(rest: &str) -> Option<String> {
    let trimmed = rest.trim_start();
    if trimmed.starts_with('#') {
        // Preserve the spacing between closing quote and comment
        let space_prefix = &rest[..rest.len() - trimmed.len()];
        Some(format!("{}{}", space_prefix, trimmed))
    } else if trimmed.is_empty() {
        None
    } else {
        // There's non-comment text after the closing quote, which is unusual.
        // Just ignore it as part of the value for safety.
        None
    }
}

fn find_inline_comment_start(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#' && bytes[i - 1] == b' ' {
            return Some(i - 1);
        }
    }
    None
}

/// Serialize a ShellFile back to its text representation.
/// This produces content suitable for writing back to disk.
pub fn serialize_shell_file(shell_file: &ShellFile) -> String {
    if shell_file.lines.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = shell_file
        .lines
        .iter()
        .map(|node| node.serialize(false))
        .collect();

    let mut result = lines.join("\n");
    // Add trailing newline to match typical file convention
    result.push('\n');
    result
}
