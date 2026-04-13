use std::path::PathBuf;

/// How an environment variable export is written
#[derive(Debug, Clone, PartialEq)]
pub enum ExportStyle {
    /// `export KEY=VALUE`
    Export,
    /// `KEY=VALUE` (bare assignment)
    Bare,
}

/// What quote style wraps the value
#[derive(Debug, Clone, PartialEq)]
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
            | LineNode::Other { line_number, .. } => *line_number,
        }
    }

    pub fn original_text(&self) -> &str {
        match self {
            LineNode::Blank { original_text, .. }
            | LineNode::Comment { original_text, .. }
            | LineNode::EnvExport { original_text, .. }
            | LineNode::ManagedComment { original_text, .. }
            | LineNode::SourceDirective { original_text, .. }
            | LineNode::Other { original_text, .. } => original_text,
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
                    QuoteStyle::Double => format!("\"{}\"", value),
                    QuoteStyle::Single => format!("'{}'", value),
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
