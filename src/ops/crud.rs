use std::path::PathBuf;

use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};
use crate::ops::offset::{find_managed_zone, ENVFORGE_END_MARKER, ENVFORGE_START_MARKER};

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

    #[error("{0}")]
    Other(String),
}

/// Quote and escape a value for serialization into a shell rc file.
///
/// Mirrors `LineNode::serialize` escaping semantics so a value containing the
/// closing quote character cannot break out of its quotes and corrupt the
/// line — which would break the byte-for-byte round-trip invariant and allow
/// shell-syntax injection into the rc file. The on-disk writer always emits
/// `original_text` (`serialize_shell_file` calls `serialize(false)`), so this
/// escaping MUST happen here, at the point `original_text` is constructed.
fn quote_value(value: &str, quote_style: QuoteStyle) -> String {
    // Escape `\` first, then `"`, so the closing quote cannot be forged. This
    // matches what EnvForge's parser reads back: inside double quotes it
    // preserves `\x` escape pairs verbatim, so the line re-parses as a single
    // intact value instead of breaking out at an unescaped `"`.
    let double = |v: &str| format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""));
    match quote_style {
        QuoteStyle::Double => double(value),
        QuoteStyle::Single => {
            // EnvForge's parser treats single quotes as fully literal with NO
            // escape mechanism — the first `'` always closes the run. The
            // POSIX `'\''` close-escape-reopen trick therefore does NOT round
            // trip here (it would truncate the value at the first `'`). When
            // the value contains a `'`, fall back to double quotes, which
            // represent a literal apostrophe cleanly and round-trip.
            if value.contains('\'') {
                double(value)
            } else {
                format!("'{}'", value)
            }
        }
        QuoteStyle::None => value.to_string(),
    }
}

/// Edit an existing ENV entry's value in the ShellFile.
///
/// Finds the EnvExport node by key and updates its value.
/// If the node is outside the managed zone, it is relocated inside.
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
                let prefix = match export_style {
                    ExportStyle::Export => "export ",
                    ExportStyle::Bare => "",
                };
                let quoted = quote_value(new_value, *quote_style);
                let comment_suffix = match inline_comment {
                    Some(c) => c.clone(),
                    None => String::new(),
                };
                *original_text = format!("{}{}={}{}", prefix, k, quoted, comment_suffix);
            }

            relocate_into_zone(shell_file, idx);

            Ok(())
        }
        _ => Err(OpsError::AmbiguousKey {
            key: key.to_string(),
            file: shell_file.path.clone(),
        }),
    }
}

/// If the EnvExport node at `idx` is outside the managed zone,
/// remove it from its current position and re-insert it just
/// before the end marker so it lives inside the zone.
fn relocate_into_zone(shell_file: &mut ShellFile, idx: usize) {
    let zone = match find_managed_zone(shell_file) {
        Some(z) => z,
        None => return,
    };

    if idx > zone.start_idx && idx < zone.end_idx {
        return;
    }

    let node = shell_file.lines.remove(idx);

    let new_end_idx = if idx < zone.end_idx {
        zone.end_idx - 1
    } else {
        zone.end_idx
    };

    shell_file.lines.insert(new_end_idx, node);
}

/// Soft-delete an ENV entry by index, converting it to a ManagedComment.
pub fn soft_delete_at(shell_file: &mut ShellFile, idx: usize) -> Result<(), OpsError> {
    if idx >= shell_file.lines.len() {
        return Err(OpsError::Other("index out of bounds".into()));
    }

    let node = &shell_file.lines[idx];
    let key = match node {
        LineNode::EnvExport { key, .. } => key.clone(),
        _ => return Err(OpsError::Other("not an EnvExport node".into())),
    };

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
    let exists = shell_file.lines.iter().any(|node| match node {
        LineNode::EnvExport { key: k, .. } => k == key,
        _ => false,
    });

    if exists {
        return Err(OpsError::KeyAlreadyExists {
            key: key.to_string(),
        });
    }

    let insert_idx = if let Some(zone) = find_managed_zone(shell_file) {
        zone.end_idx
    } else {
        let total_lines = shell_file.lines.len();
        let safe_end = total_lines.saturating_sub(footer_offset);
        let safe_start = header_offset;
        if safe_start >= safe_end {
            return Err(OpsError::NoSafeZone {
                file: shell_file.path.clone(),
            });
        }
        safe_end
    };

    let prefix = match export_style {
        ExportStyle::Export => "export ",
        ExportStyle::Bare => "",
    };
    let quoted_value = quote_value(value, quote_style);
    let text = format!("{}{}={}", prefix, key, quoted_value);

    let new_node = LineNode::EnvExport {
        line_number: insert_idx,
        original_text: text,
        key: key.to_string(),
        value: value.to_string(),
        export_style,
        quote_style,
        inline_comment: None,
    };

    shell_file.lines.insert(insert_idx, new_node);

    Ok(())
}

/// Rename an ENV entry's key in the ShellFile by index.
pub fn rename_entry_at(
    shell_file: &mut ShellFile,
    idx: usize,
    new_key: &str,
) -> Result<(), OpsError> {
    if idx >= shell_file.lines.len() {
        return Err(OpsError::Other("index out of bounds".into()));
    }

    if let LineNode::EnvExport {
        key,
        value,
        original_text,
        export_style,
        quote_style,
        inline_comment,
        ..
    } = &mut shell_file.lines[idx]
    {
        let prefix = match export_style {
            ExportStyle::Export => "export ",
            ExportStyle::Bare => "",
        };
        let quoted = quote_value(value, *quote_style);
        let comment_suffix = match inline_comment {
            Some(c) => c.clone(),
            None => String::new(),
        };
        *original_text = format!("{}{}={}{}", prefix, new_key, quoted, comment_suffix);
        *key = new_key.to_string();
        Ok(())
    } else {
        Err(OpsError::Other("not an EnvExport node".into()))
    }
}

/// Rename an ENV entry's key in the ShellFile.
///
/// Finds the EnvExport node by old_key and updates the key field.
/// Regenerates `original_text` to reflect the new key name.
/// Does not write to disk — caller is responsible for that.
pub fn rename_entry(
    shell_file: &mut ShellFile,
    old_key: &str,
    new_key: &str,
) -> Result<(), OpsError> {
    let idx = find_unique_export(shell_file, old_key)?;

    if let LineNode::EnvExport {
        key,
        value,
        original_text,
        export_style,
        quote_style,
        inline_comment,
        ..
    } = &mut shell_file.lines[idx]
    {
        let prefix = match export_style {
            ExportStyle::Export => "export ",
            ExportStyle::Bare => "",
        };
        let quoted = quote_value(value, *quote_style);
        let comment_suffix = match inline_comment {
            Some(c) => c.clone(),
            None => String::new(),
        };
        *original_text = format!("{}{}={}{}", prefix, new_key, quoted, comment_suffix);
        *key = new_key.to_string();
    }

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

pub fn find_soft_deleted(shell_file: &ShellFile, key: &str) -> Option<usize> {
    let target_tag = format!("deleted:{}", key);
    shell_file
        .lines
        .iter()
        .position(|node| matches!(node, LineNode::ManagedComment { tag, .. } if tag == &target_tag))
}

pub fn ensure_managed_zone(shell_file: &mut ShellFile) -> bool {
    if find_managed_zone(shell_file).is_some() {
        return true;
    }

    let start_pos = find_best_marker_position(shell_file);
    let end_pos = find_end_marker_position(shell_file, start_pos);

    let start_node = LineNode::EnvforgeStart {
        line_number: start_pos,
        original_text: ENVFORGE_START_MARKER.to_string(),
    };
    let end_node = LineNode::EnvforgeEnd {
        line_number: end_pos + 1,
        original_text: ENVFORGE_END_MARKER.to_string(),
    };

    // M9: marker positions can exceed the line count on a short/marker-less rc
    // (e.g. the (None, None) branch returns `len`, and the end search returns
    // `len + 1`). Clamp both insert indices to the current length so the markers
    // simply append instead of panicking with "insertion index > len".
    let start_idx = start_pos.min(shell_file.lines.len());
    shell_file.lines.insert(start_idx, start_node);
    let end_idx = (end_pos + 1).min(shell_file.lines.len());
    shell_file.lines.insert(end_idx, end_node);

    true
}

fn find_end_marker_position(shell_file: &ShellFile, after_idx: usize) -> usize {
    let search_start = after_idx + 1;
    if search_start >= shell_file.lines.len() {
        return search_start;
    }

    let last_env = shell_file
        .lines
        .iter()
        .enumerate()
        .skip(search_start)
        .rev()
        .find(|(_, node)| {
            matches!(
                node,
                LineNode::EnvExport { .. }
                    | LineNode::ManagedComment { .. }
                    | LineNode::SourceDirective { .. }
            )
        })
        .map(|(i, _)| i + 1);

    let first_protected = shell_file
        .lines
        .iter()
        .enumerate()
        .skip(search_start)
        .find(|(_, node)| {
            let text = node.original_text().trim();
            text.starts_with("# >>> conda")
                || text.starts_with("# <<< conda")
                || text.contains("Q pre block.")
                || text.contains("Q post block.")
        })
        .map(|(i, _)| i);

    match (last_env, first_protected) {
        (Some(env_pos), Some(prot_pos)) if prot_pos > env_pos => env_pos,
        (Some(env_pos), Some(_)) => env_pos,
        (Some(env_pos), None) => env_pos,
        (None, Some(prot_pos)) => prot_pos,
        (None, None) => search_start,
    }
}

fn find_best_marker_position(shell_file: &ShellFile) -> usize {
    let first_env_or_managed = shell_file
        .lines
        .iter()
        .enumerate()
        .find(|(_, node)| {
            matches!(
                node,
                LineNode::EnvExport { .. }
                    | LineNode::ManagedComment { .. }
                    | LineNode::SourceDirective { .. }
            )
        })
        .map(|(i, _)| i);

    let first_protected_from_end = shell_file
        .lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, node)| {
            let text = node.original_text().trim();
            text.starts_with("# >>> conda")
                || text.starts_with("# <<< conda")
                || text.contains("Q pre block.")
                || text.contains("Q post block.")
        })
        .map(|(i, _)| i);

    match (first_env_or_managed, first_protected_from_end) {
        (Some(env_pos), Some(prot_pos)) if prot_pos > env_pos => env_pos,
        (Some(env_pos), Some(_)) => env_pos,
        (Some(env_pos), None) => env_pos,
        (None, Some(prot_pos)) => prot_pos,
        (None, None) => shell_file.lines.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_shell_content, serialize_shell_file};
    use std::path::Path;

    fn make_shell_file(content: &str) -> ShellFile {
        parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
    }

    // ─── ensure_managed_zone (M9: no panic on short/empty files) ──

    #[test]
    fn test_ensure_managed_zone_empty_file_no_panic() {
        let mut sf = make_shell_file("");
        assert!(ensure_managed_zone(&mut sf));
        assert!(serialize_shell_file(&sf).contains(ENVFORGE_START_MARKER));
    }

    #[test]
    fn test_ensure_managed_zone_short_file_no_panic() {
        // Fewer lines than the historical protected-offset → used to panic with
        // "insertion index N should be <= len M".
        let mut sf = make_shell_file("# one line\n");
        assert!(ensure_managed_zone(&mut sf));
        let out = serialize_shell_file(&sf);
        assert!(out.contains(ENVFORGE_START_MARKER));
        assert!(out.contains(ENVFORGE_END_MARKER));
    }

    // ─── edit_entry ───────────────────────────────────────────

    #[test]
    fn test_edit_entry_updates_value() {
        let mut sf = make_shell_file("export API_KEY=\"old_value\"");
        edit_entry(&mut sf, "API_KEY", "new_value").unwrap();
        assert!(
            matches!(&sf.lines[0], LineNode::EnvExport { value, .. } if value == "new_value"),
            "Expected EnvExport with value=new_value"
        );
    }

    #[test]
    fn test_edit_entry_preserves_quote_style() {
        let mut sf = make_shell_file("export DB_HOST='localhost'");
        edit_entry(&mut sf, "DB_HOST", "remotehost").unwrap();
        let LineNode::EnvExport {
            quote_style,
            original_text,
            ..
        } = &sf.lines[0]
        else {
            panic!("Expected EnvExport");
        };
        assert_eq!(*quote_style, QuoteStyle::Single);
        assert!(original_text.contains("'remotehost'"));
    }

    #[test]
    fn test_edit_entry_preserves_inline_comment() {
        let mut sf = make_shell_file("export PORT=\"8080\" # web server port");
        edit_entry(&mut sf, "PORT", "3000").unwrap();
        let LineNode::EnvExport {
            original_text,
            inline_comment,
            ..
        } = &sf.lines[0]
        else {
            panic!("Expected EnvExport");
        };
        assert!(inline_comment.is_some());
        assert!(original_text.contains("# web server port"));
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
        assert!(
            matches!(&sf.lines[0], LineNode::ManagedComment { tag, .. } if tag == "deleted:API_KEY"),
            "Expected ManagedComment with tag=deleted:API_KEY"
        );
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
        let LineNode::EnvExport { key, value, .. } = &sf.lines[0] else {
            panic!("Expected EnvExport after undo");
        };
        assert_eq!(key, "API_KEY");
        assert_eq!(value, "secret");
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
        let LineNode::EnvExport { key, value, .. } = &sf.lines[2] else {
            panic!("Expected EnvExport, got: {:?}", sf.lines[2]);
        };
        assert_eq!(key, "NEW_KEY");
        assert_eq!(value, "new_val");
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
        let LineNode::EnvExport {
            original_text,
            export_style,
            quote_style,
            ..
        } = &sf.lines[1]
        else {
            panic!("Expected EnvExport, got: {:?}", sf.lines[1]);
        };
        assert_eq!(original_text, "PORT=3000");
        assert_eq!(*export_style, ExportStyle::Bare);
        assert_eq!(*quote_style, QuoteStyle::None);
    }

    // ─── relocate_into_zone ─────────────────────────────────

    #[test]
    fn test_edit_entry_relocates_outside_var_into_zone() {
        let mut sf = make_shell_file(
            "export OUTSIDE=\"old\"\n# >>> envforge >>>\nexport INSIDE=\"keep\"\n# <<< envforge <<<\n",
        );

        edit_entry(&mut sf, "OUTSIDE", "updated").unwrap();

        let serialized = serialize_shell_file(&sf);
        let start_pos = serialized.find("# >>> envforge >>>").unwrap();
        let end_pos = serialized.find("# <<< envforge <<<").unwrap();
        let outside_pos = serialized.find("export OUTSIDE=").unwrap();
        let inside_pos = serialized.find("export INSIDE=").unwrap();

        assert!(
            outside_pos > start_pos,
            "OUTSIDE should be after start marker"
        );
        assert!(outside_pos < end_pos, "OUTSIDE should be before end marker");
        assert!(
            inside_pos > start_pos,
            "INSIDE should still be after start marker"
        );
        assert!(
            inside_pos < end_pos,
            "INSIDE should still be before end marker"
        );
    }

    #[test]
    fn test_edit_entry_stays_put_when_already_in_zone() {
        let mut sf = make_shell_file(
            "# >>> envforge >>>\nexport FIRST=\"1\"\nexport SECOND=\"2\"\n# <<< envforge <<<\n",
        );

        edit_entry(&mut sf, "FIRST", "updated").unwrap();

        let serialized = serialize_shell_file(&sf);
        let start_pos = serialized.find("# >>> envforge >>>").unwrap();
        let end_pos = serialized.find("# <<< envforge <<<").unwrap();
        let first_pos = serialized.find("export FIRST=").unwrap();
        let second_pos = serialized.find("export SECOND=").unwrap();

        assert!(first_pos > start_pos);
        assert!(first_pos < end_pos);
        assert!(second_pos > start_pos);
        assert!(second_pos < end_pos);
        assert!(first_pos < second_pos, "order preserved");
    }

    #[test]
    fn test_edit_entry_no_zone_no_relocation() {
        let mut sf = make_shell_file("export FOO=\"old\"\nexport BAR=\"keep\"\n");
        edit_entry(&mut sf, "FOO", "new").unwrap();

        let LineNode::EnvExport { key, value, .. } = &sf.lines[0] else {
            panic!("Expected EnvExport, got: {:?}", sf.lines[0]);
        };
        assert_eq!(key, "FOO");
        assert_eq!(value, "new");
    }

    // ─── ensure_managed_zone wraps existing vars ─────────────

    #[test]
    fn test_ensure_managed_zone_wraps_existing_vars() {
        let mut sf =
            make_shell_file("# header\nexport FOO=\"bar\"\nexport BAZ=\"qux\"\n# footer\n");

        ensure_managed_zone(&mut sf);

        let serialized = serialize_shell_file(&sf);
        let start_pos = serialized.find("# >>> envforge >>>").unwrap();
        let end_pos = serialized.find("# <<< envforge <<<").unwrap();
        let foo_pos = serialized.find("export FOO=").unwrap();
        let baz_pos = serialized.find("export BAZ=").unwrap();

        assert!(foo_pos > start_pos, "FOO should be inside zone");
        assert!(baz_pos > start_pos, "BAZ should be inside zone");
        assert!(foo_pos < end_pos, "FOO should be before end marker");
        assert!(baz_pos < end_pos, "BAZ should be before end marker");
    }

    #[test]
    fn test_ensure_managed_zone_wraps_vars_before_conda() {
        let mut sf = make_shell_file(
            "export FOO=\"bar\"\n# >>> conda initialize >>>\nconda_stuff\n# <<< conda initialize <<<\n",
        );

        ensure_managed_zone(&mut sf);

        let serialized = serialize_shell_file(&sf);
        let start_pos = serialized.find("# >>> envforge >>>").unwrap();
        let end_pos = serialized.find("# <<< envforge <<<").unwrap();
        let foo_pos = serialized.find("export FOO=").unwrap();
        let conda_pos = serialized.find("# >>> conda").unwrap();

        assert!(foo_pos > start_pos, "FOO inside zone");
        assert!(foo_pos < end_pos, "FOO before end marker");
        assert!(conda_pos > end_pos, "conda after envforge zone");
    }
}
