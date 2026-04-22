use std::path::PathBuf;

use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};

/// Errors that can occur during CRUD operations.
#[derive(Debug, thiserror::Error)]
pub enum OpsError {
    #[error("key '{key}' not found in {file}")]
    KeyNotFound { key: String, file: PathBuf },

    #[error("key '{key}' appears multiple times in {file} — ambiguous")]
    AmbiguousKey { key: String, file: PathBuf },

    #[error("key '{key}' already exists — use edit instead")]
    KeyAlreadyExists { key: String },

    #[error("key '{key}' is already deleted")]
    AlreadyDeleted { key: String },

    #[error("no safe zone available in {file} — entire file is protected")]
    NoSafeZone { file: PathBuf },

    #[error("key '{key}' is not a deleted entry — cannot undo")]
    NotDeleted { key: String },
}

/// Edit an existing ENV entry's value in the ShellFile.
///
/// Finds the EnvExport node by key and updates its value.
/// Does not write to disk — caller is responsible for that.
pub fn edit_entry(shell_file: &mut ShellFile, key: &str, new_value: &str) -> Result<(), OpsError> {
    let matches: Vec<usize> = shell_file
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            LineNode::EnvExport { key: k, .. } if k == key => Some(i),
            _ => None,
        })
        .collect();

    match matches.len() {
        0 => Err(OpsError::KeyNotFound {
            key: key.to_string(),
            file: shell_file.path.clone(),
        }),
        1 => {
            let idx = matches[0];
            if let LineNode::EnvExport {
                value,
                original_text,
                export_style,
                quote_style,
                key: k,
                inline_comment,
                ..
            } = &mut shell_file.lines[idx]
            {
                *value = new_value.to_string();
                // Regenerate original_text to reflect the new value
                let prefix = match export_style {
                    ExportStyle::Export => "export ",
                    ExportStyle::Bare => "",
                };
                let quoted = match quote_style {
                    QuoteStyle::Double => format!("\"{}\"", new_value),
                    QuoteStyle::Single => format!("'{}'", new_value),
                    QuoteStyle::None => new_value.to_string(),
                };
                let comment_suffix = match inline_comment {
                    Some(c) => c.clone(),
                    None => String::new(),
                };
                *original_text = format!("{}{}={}{}", prefix, k, quoted, comment_suffix);
            }
            Ok(())
        }
        _ => Err(OpsError::AmbiguousKey {
            key: key.to_string(),
            file: shell_file.path.clone(),
        }),
    }
}

/// Soft-delete an ENV entry by converting it to a ManagedComment.
///
/// The line becomes: `#[envforge:deleted:KEY] original_export_text`
pub fn soft_delete(shell_file: &mut ShellFile, key: &str) -> Result<(), OpsError> {
    let idx = find_unique_export(shell_file, key)?;

    let node = &shell_file.lines[idx];
    let original_text = node.original_text().to_string();
    let line_number = node.line_number();

    let new_text = format!("#[envforge:deleted:{}] {}", key, original_text);

    shell_file.lines[idx] = LineNode::ManagedComment {
        line_number,
        original_text: new_text,
        tag: format!("deleted:{}", key),
        original_export: original_text,
    };

    Ok(())
}

/// Undo a soft-delete by converting a ManagedComment back to its original EnvExport.
///
/// Only works on entries tagged with `envforge:deleted:KEY`.
pub fn undo_delete(shell_file: &mut ShellFile, key: &str) -> Result<(), OpsError> {
    let target_tag = format!("deleted:{}", key);

    let idx = shell_file
        .lines
        .iter()
        .position(|node| match node {
            LineNode::ManagedComment { tag, .. } => tag == &target_tag,
            _ => false,
        })
        .ok_or_else(|| OpsError::NotDeleted {
            key: key.to_string(),
        })?;

    let (line_number, original_export) = match &shell_file.lines[idx] {
        LineNode::ManagedComment {
            line_number,
            original_export,
            ..
        } => (*line_number, original_export.clone()),
        _ => unreachable!(),
    };

    // Re-parse the original export line to restore the full EnvExport node
    // For simplicity, we restore it as an Other node with original text,
    // then re-parse. But actually, we can use our parser on single line.
    use crate::parser::parse_shell_content;
    use std::path::Path;

    let reparsed =
        parse_shell_content(&original_export, Path::new("")).map_err(|_| OpsError::NotDeleted {
            key: key.to_string(),
        })?;

    if let Some(mut restored_node) = reparsed.lines.into_iter().next() {
        // Update line number to match original position
        match &mut restored_node {
            LineNode::EnvExport {
                line_number: ln, ..
            } => *ln = line_number,
            _ => {
                // If it doesn't parse as EnvExport, restore as Other
                restored_node = LineNode::Other {
                    line_number,
                    original_text: original_export,
                };
            }
        }
        shell_file.lines[idx] = restored_node;
    }

    Ok(())
}

/// Add a new ENV entry to the ShellFile in the safe zone.
///
/// `header_offset` and `footer_offset` define the protected zones.
/// The new entry is inserted at the end of the safe zone (just before footer).
pub fn add_entry(
    shell_file: &mut ShellFile,
    key: &str,
    value: &str,
    export_style: ExportStyle,
    quote_style: QuoteStyle,
    header_offset: usize,
    footer_offset: usize,
) -> Result<(), OpsError> {
    // Check for duplicate key
    let exists = shell_file.lines.iter().any(|node| match node {
        LineNode::EnvExport { key: k, .. } => k == key,
        _ => false,
    });

    if exists {
        return Err(OpsError::KeyAlreadyExists {
            key: key.to_string(),
        });
    }

    let total_lines = shell_file.lines.len();
    let safe_end = total_lines.saturating_sub(footer_offset);
    let safe_start = header_offset;

    if safe_start >= safe_end {
        return Err(OpsError::NoSafeZone {
            file: shell_file.path.clone(),
        });
    }

    // Build the new line
    let prefix = match export_style {
        ExportStyle::Export => "export ",
        ExportStyle::Bare => "",
    };
    let quoted_value = match quote_style {
        QuoteStyle::Double => format!("\"{}\"", value),
        QuoteStyle::Single => format!("'{}'", value),
        QuoteStyle::None => value.to_string(),
    };
    let text = format!("{}{}={}", prefix, key, quoted_value);

    let new_node = LineNode::EnvExport {
        line_number: safe_end,
        original_text: text,
        key: key.to_string(),
        value: value.to_string(),
        export_style,
        quote_style,
        inline_comment: None,
    };

    // Insert at end of safe zone
    shell_file.lines.insert(safe_end, new_node);

    Ok(())
}

/// Find the unique index of an EnvExport node by key.
fn find_unique_export(shell_file: &ShellFile, key: &str) -> Result<usize, OpsError> {
    let matches: Vec<usize> = shell_file
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            LineNode::EnvExport { key: k, .. } if k == key => Some(i),
            _ => None,
        })
        .collect();

    match matches.len() {
        0 => Err(OpsError::KeyNotFound {
            key: key.to_string(),
            file: shell_file.path.clone(),
        }),
        1 => Ok(matches[0]),
        _ => Err(OpsError::AmbiguousKey {
            key: key.to_string(),
            file: shell_file.path.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_shell_content;
    use std::path::Path;

    fn make_shell_file(content: &str) -> ShellFile {
        parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
    }

    // ─── edit_entry ───────────────────────────────────────────

    #[test]
    fn test_edit_entry_updates_value() {
        let mut sf = make_shell_file("export API_KEY=\"old_value\"");
        edit_entry(&mut sf, "API_KEY", "new_value").unwrap();
        if let LineNode::EnvExport { value, .. } = &sf.lines[0] {
            assert_eq!(value, "new_value");
        } else {
            panic!("Expected EnvExport");
        }
    }

    #[test]
    fn test_edit_entry_preserves_quote_style() {
        let mut sf = make_shell_file("export DB_HOST='localhost'");
        edit_entry(&mut sf, "DB_HOST", "remotehost").unwrap();
        if let LineNode::EnvExport {
            quote_style,
            original_text,
            ..
        } = &sf.lines[0]
        {
            assert_eq!(*quote_style, QuoteStyle::Single);
            assert!(original_text.contains("'remotehost'"));
        } else {
            panic!("Expected EnvExport");
        }
    }

    #[test]
    fn test_edit_entry_preserves_inline_comment() {
        let mut sf = make_shell_file("export PORT=\"8080\" # web server port");
        edit_entry(&mut sf, "PORT", "3000").unwrap();
        if let LineNode::EnvExport {
            original_text,
            inline_comment,
            ..
        } = &sf.lines[0]
        {
            assert!(inline_comment.is_some());
            assert!(original_text.contains("# web server port"));
        } else {
            panic!("Expected EnvExport");
        }
    }

    #[test]
    fn test_edit_entry_key_not_found() {
        let mut sf = make_shell_file("export FOO=\"bar\"");
        let result = edit_entry(&mut sf, "MISSING", "val");
        assert!(matches!(result, Err(OpsError::KeyNotFound { .. })));
    }

    #[test]
    fn test_edit_entry_ambiguous_key() {
        let mut sf = make_shell_file("export DUP=\"a\"\nexport DUP=\"b\"");
        let result = edit_entry(&mut sf, "DUP", "c");
        assert!(matches!(result, Err(OpsError::AmbiguousKey { .. })));
    }

    // ─── soft_delete ──────────────────────────────────────────

    #[test]
    fn test_soft_delete_converts_to_managed_comment() {
        let mut sf = make_shell_file("export API_KEY=\"secret\"");
        soft_delete(&mut sf, "API_KEY").unwrap();
        match &sf.lines[0] {
            LineNode::ManagedComment { tag, .. } => {
                assert_eq!(tag, "deleted:API_KEY");
            }
            other => panic!("Expected ManagedComment, got: {:?}", other),
        }
    }

    #[test]
    fn test_soft_delete_key_not_found() {
        let mut sf = make_shell_file("export FOO=\"bar\"");
        let result = soft_delete(&mut sf, "MISSING");
        assert!(matches!(result, Err(OpsError::KeyNotFound { .. })));
    }

    #[test]
    fn test_soft_delete_ambiguous_key() {
        let mut sf = make_shell_file("export X=\"1\"\nexport X=\"2\"");
        let result = soft_delete(&mut sf, "X");
        assert!(matches!(result, Err(OpsError::AmbiguousKey { .. })));
    }

    // ─── undo_delete ──────────────────────────────────────────

    #[test]
    fn test_undo_delete_restores_export() {
        let mut sf = make_shell_file("export API_KEY=\"secret\"");
        soft_delete(&mut sf, "API_KEY").unwrap();
        assert!(matches!(sf.lines[0], LineNode::ManagedComment { .. }));

        undo_delete(&mut sf, "API_KEY").unwrap();
        match &sf.lines[0] {
            LineNode::EnvExport { key, value, .. } => {
                assert_eq!(key, "API_KEY");
                assert_eq!(value, "secret");
            }
            other => panic!("Expected EnvExport after undo, got: {:?}", other),
        }
    }

    #[test]
    fn test_undo_delete_not_deleted_error() {
        let mut sf = make_shell_file("export FOO=\"bar\"");
        let result = undo_delete(&mut sf, "FOO");
        assert!(matches!(result, Err(OpsError::NotDeleted { .. })));
    }

    #[test]
    fn test_undo_delete_roundtrip_preserves_value() {
        let mut sf = make_shell_file("export DB_URL=\"postgres://localhost\"");
        let original_text = sf.lines[0].original_text().to_string();

        soft_delete(&mut sf, "DB_URL").unwrap();
        undo_delete(&mut sf, "DB_URL").unwrap();

        assert_eq!(sf.lines[0].original_text(), original_text);
    }

    // ─── add_entry ────────────────────────────────────────────

    #[test]
    fn test_add_entry_inserts_at_safe_zone() {
        let mut sf = make_shell_file("# header\nexport EXISTING=\"val\"");
        add_entry(
            &mut sf,
            "NEW_KEY",
            "new_val",
            ExportStyle::Export,
            QuoteStyle::Double,
            0,
            0,
        )
        .unwrap();
        assert_eq!(sf.lines.len(), 3);
        match &sf.lines[2] {
            LineNode::EnvExport { key, value, .. } => {
                assert_eq!(key, "NEW_KEY");
                assert_eq!(value, "new_val");
            }
            other => panic!("Expected EnvExport, got: {:?}", other),
        }
    }

    #[test]
    fn test_add_entry_key_already_exists() {
        let mut sf = make_shell_file("export FOO=\"bar\"");
        let result = add_entry(
            &mut sf,
            "FOO",
            "baz",
            ExportStyle::Export,
            QuoteStyle::Double,
            0,
            0,
        );
        assert!(matches!(result, Err(OpsError::KeyAlreadyExists { .. })));
    }

    #[test]
    fn test_add_entry_no_safe_zone() {
        let mut sf = make_shell_file("export A=\"1\"");
        // header_offset=1, total=1, safe_start >= safe_end
        let result = add_entry(
            &mut sf,
            "B",
            "2",
            ExportStyle::Export,
            QuoteStyle::Double,
            5,
            5,
        );
        assert!(matches!(result, Err(OpsError::NoSafeZone { .. })));
    }

    #[test]
    fn test_add_entry_bare_no_quotes() {
        let mut sf = make_shell_file("# file");
        add_entry(
            &mut sf,
            "PORT",
            "3000",
            ExportStyle::Bare,
            QuoteStyle::None,
            0,
            0,
        )
        .unwrap();
        match &sf.lines[1] {
            LineNode::EnvExport {
                original_text,
                export_style,
                quote_style,
                ..
            } => {
                assert_eq!(original_text, "PORT=3000");
                assert_eq!(*export_style, ExportStyle::Bare);
                assert_eq!(*quote_style, QuoteStyle::None);
            }
            other => panic!("Expected EnvExport, got: {:?}", other),
        }
    }
}
