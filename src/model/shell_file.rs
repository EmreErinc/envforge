use std::path::PathBuf;

/// How an environment variable export is written
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportStyle {
    /// `export KEY=VALUE`
    Export,
    /// `KEY=VALUE` (bare assignment)
    Bare,
}

/// What quote style wraps the value
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuoteStyle {
    /// `"value"`
    Double,
    /// `'value'`
    Single,
    /// `value` (no quotes)
    None,
}

/// A single line in a shell configuration file
#[derive(Debug, Clone)]
pub enum LineNode {
    /// Empty or whitespace-only line
    Blank {
        line_number: usize,
        original_text: String,
    },
    /// A comment line (starts with #, but not an envforge tag)
    Comment {
        line_number: usize,
        original_text: String,
        text: String,
    },
    /// An environment variable export statement
    EnvExport {
        line_number: usize,
        original_text: String,
        key: String,
        value: String,
        export_style: ExportStyle,
        quote_style: QuoteStyle,
        inline_comment: Option<String>,
    },
    /// A line commented out by envforge with a management tag
    ManagedComment {
        line_number: usize,
        original_text: String,
        tag: String,
        original_export: String,
    },
    /// `# >>> envforge >>>` managed zone start marker
    EnvforgeStart {
        line_number: usize,
        original_text: String,
    },
    /// `# <<< envforge <<<` managed zone end marker
    EnvforgeEnd {
        line_number: usize,
        original_text: String,
    },
    /// A `source` or `.` directive
    SourceDirective {
        line_number: usize,
        original_text: String,
        path: String,
    },
    /// Any other line (aliases, functions, path manipulation, etc.)
    Other {
        line_number: usize,
        original_text: String,
    },
}

impl LineNode {
    pub fn line_number(&self) -> usize {
        match self {
            LineNode::Blank { line_number, .. }
            | LineNode::Comment { line_number, .. }
            | LineNode::EnvExport { line_number, .. }
            | LineNode::ManagedComment { line_number, .. }
            | LineNode::SourceDirective { line_number, .. }
            | LineNode::Other { line_number, .. }
            | LineNode::EnvforgeStart { line_number, .. }
            | LineNode::EnvforgeEnd { line_number, .. } => *line_number,
        }
    }

    pub fn original_text(&self) -> &str {
        match self {
            LineNode::Blank { original_text, .. }
            | LineNode::Comment { original_text, .. }
            | LineNode::EnvExport { original_text, .. }
            | LineNode::ManagedComment { original_text, .. }
            | LineNode::SourceDirective { original_text, .. }
            | LineNode::Other { original_text, .. }
            | LineNode::EnvforgeStart { original_text, .. }
            | LineNode::EnvforgeEnd { original_text, .. } => original_text,
        }
    }

    /// Serialize this node back to text.
    /// For unmodified nodes, returns original_text.
    /// For modified EnvExport nodes, regenerates from fields.
    pub fn serialize(&self, modified: bool) -> String {
        match self {
            LineNode::EnvExport {
                original_text,
                key,
                value,
                export_style,
                quote_style,
                inline_comment,
                ..
            } => {
                if !modified {
                    return original_text.clone();
                }
                let prefix = match export_style {
                    ExportStyle::Export => "export ",
                    ExportStyle::Bare => "",
                };
                let quoted_value = match quote_style {
                    // Escape `\` and `"` so a value containing the closing
                    // quote character cannot break out and corrupt the
                    // line. Without this, a modified value like `he"llo`
                    // would serialize as `KEY="he"llo"` and re-parse to
                    // a different value, breaking the round-trip
                    // invariant the rest of the codebase relies on.
                    QuoteStyle::Double => {
                        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
                    }
                    // POSIX single-quote rule: there is no escape inside
                    // single quotes; close, escape, reopen.
                    QuoteStyle::Single => format!("'{}'", value.replace('\'', "'\\''")),
                    QuoteStyle::None => value.clone(),
                };
                let comment_suffix = match inline_comment {
                    Some(c) => c.clone(),
                    None => String::new(),
                };
                format!("{}{}={}{}", prefix, key, quoted_value, comment_suffix)
            }
            _ => self.original_text().to_string(),
        }
    }
}

/// The detected shell type
#[derive(Debug, Clone, PartialEq)]
pub enum Shell {
    Zsh,
    Bash,
    Unknown(String),
}

/// A parsed shell configuration file
#[derive(Debug)]
pub struct ShellFile {
    pub path: PathBuf,
    pub lines: Vec<LineNode>,
    pub hash: [u8; 32],
}

impl ShellFile {
    /// Serialize all lines back to a string
    pub fn serialize(&self) -> String {
        self.lines
            .iter()
            .map(|node| node.serialize(false))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── LineNode::line_number ─────────────────────────────────

    #[test]
    fn test_line_number_all_variants() {
        let blank = LineNode::Blank {
            line_number: 0,
            original_text: String::new(),
        };
        assert_eq!(blank.line_number(), 0);

        let comment = LineNode::Comment {
            line_number: 5,
            original_text: "# hi".to_string(),
            text: "hi".to_string(),
        };
        assert_eq!(comment.line_number(), 5);

        let export = LineNode::EnvExport {
            line_number: 10,
            original_text: "export A=\"1\"".to_string(),
            key: "A".to_string(),
            value: "1".to_string(),
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            inline_comment: None,
        };
        assert_eq!(export.line_number(), 10);

        let other = LineNode::Other {
            line_number: 20,
            original_text: "alias ls='ls -la'".to_string(),
        };
        assert_eq!(other.line_number(), 20);
    }

    // ─── LineNode::original_text ──────────────────────────────

    #[test]
    fn test_original_text_returns_stored_text() {
        let node = LineNode::Comment {
            line_number: 0,
            original_text: "# my comment".to_string(),
            text: "my comment".to_string(),
        };
        assert_eq!(node.original_text(), "# my comment");
    }

    // ─── LineNode::serialize ──────────────────────────────────

    #[test]
    fn test_serialize_unmodified_returns_original() {
        let node = LineNode::EnvExport {
            line_number: 0,
            original_text: "export FOO=\"bar\"".to_string(),
            key: "FOO".to_string(),
            value: "bar".to_string(),
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            inline_comment: None,
        };
        assert_eq!(node.serialize(false), "export FOO=\"bar\"");
    }

    #[test]
    fn test_serialize_modified_export_double_quotes() {
        let node = LineNode::EnvExport {
            line_number: 0,
            original_text: "export FOO=\"old\"".to_string(),
            key: "FOO".to_string(),
            value: "new".to_string(),
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            inline_comment: None,
        };
        assert_eq!(node.serialize(true), "export FOO=\"new\"");
    }

    #[test]
    fn test_serialize_modified_bare_no_quotes() {
        let node = LineNode::EnvExport {
            line_number: 0,
            original_text: "PORT=8080".to_string(),
            key: "PORT".to_string(),
            value: "3000".to_string(),
            export_style: ExportStyle::Bare,
            quote_style: QuoteStyle::None,
            inline_comment: None,
        };
        assert_eq!(node.serialize(true), "PORT=3000");
    }

    #[test]
    fn test_serialize_non_export_ignores_modified_flag() {
        let node = LineNode::Comment {
            line_number: 0,
            original_text: "# a comment".to_string(),
            text: "a comment".to_string(),
        };
        // modified flag doesn't affect non-export nodes
        assert_eq!(node.serialize(true), "# a comment");
        assert_eq!(node.serialize(false), "# a comment");
    }

    // ─── ShellFile::serialize ─────────────────────────────────

    #[test]
    fn test_shellfile_serialize_roundtrip() {
        use crate::parser::parse_shell_content;
        use std::path::Path;

        let content = "# header\nexport FOO=\"bar\"\nexport BAZ=\"qux\"";
        let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
        let serialized = sf.serialize();
        assert_eq!(serialized, content);
    }

    #[test]
    fn test_shellfile_serialize_joins_with_newlines() {
        let sf = ShellFile {
            path: PathBuf::from("/test"),
            lines: vec![
                LineNode::Comment {
                    line_number: 0,
                    original_text: "# line1".to_string(),
                    text: "line1".to_string(),
                },
                LineNode::Blank {
                    line_number: 1,
                    original_text: String::new(),
                },
                LineNode::Other {
                    line_number: 2,
                    original_text: "alias x=y".to_string(),
                },
            ],
            hash: [0u8; 32],
        };
        let result = sf.serialize();
        assert_eq!(result, "# line1\n\nalias x=y");
    }
}
