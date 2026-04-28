use std::path::{Path, PathBuf};

use crate::model::{ExportStyle, LineNode, QuoteStyle, ShellFile};
use crate::ops::crud::OpsError;
use crate::parser::parse_shell_content;

/// Ensure the reference file exists, creating it if necessary.
///
/// Returns the path to the reference file.
pub fn ensure_reference_file(ref_path: &Path) -> Result<PathBuf, OpsError> {
    if !ref_path.exists() {
        if let Some(parent) = ref_path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| OpsError::NoSafeZone {
                file: ref_path.to_path_buf(),
            })?;
        }
        std::fs::write(ref_path, "# EnvForge managed environment variables\n").map_err(|_| {
            OpsError::NoSafeZone {
                file: ref_path.to_path_buf(),
            }
        })?;
    }
    Ok(ref_path.to_path_buf())
}

/// Check if the primary file already has a source directive for the reference file.
pub fn has_source_directive(shell_file: &ShellFile, ref_path: &Path) -> bool {
    let ref_str = ref_path.to_string_lossy();
    shell_file.lines.iter().any(|node| match node {
        LineNode::SourceDirective { path, .. } => path.contains(ref_str.as_ref()),
        LineNode::Other { original_text, .. } => {
            original_text.contains("envforge:source")
                || (original_text.contains("source") && original_text.contains(ref_str.as_ref()))
        }
        _ => false,
    })
}

/// Inject a source directive for the reference file into the primary file's safe zone.
///
/// Adds two lines:
/// ```text
/// # [envforge:source] Managed environment variables
/// [ -f ~/.env_managed ] && source ~/.env_managed
/// ```
pub fn ensure_source_directive(
    shell_file: &mut ShellFile,
    ref_path: &Path,
    header_offset: usize,
    footer_offset: usize,
) -> Result<(), OpsError> {
    if has_source_directive(shell_file, ref_path) {
        return Ok(());
    }

    let total_lines = shell_file.lines.len();
    let safe_end = total_lines.saturating_sub(footer_offset);
    let safe_start = header_offset;

    if safe_start >= safe_end {
        return Err(OpsError::NoSafeZone {
            file: shell_file.path.clone(),
        });
    }

    let ref_str = ref_path.to_string_lossy();

    let comment_node = LineNode::Comment {
        line_number: safe_end,
        original_text: "# [envforge:source] Managed environment variables".to_string(),
        text: " [envforge:source] Managed environment variables".to_string(),
    };

    let source_node = LineNode::Other {
        line_number: safe_end + 1,
        original_text: format!("[ -f \"{}\" ] && source \"{}\"", ref_str, ref_str),
    };

    shell_file.lines.insert(safe_end, comment_node);
    shell_file.lines.insert(safe_end + 1, source_node);

    Ok(())
}

/// Move an ENV entry from the primary file to the reference file.
///
/// In primary: line becomes `#[envforge:moved:KEY -> ref_path] original_text`
/// In reference: `export KEY="value"` is appended.
pub fn move_to_reference(
    primary: &mut ShellFile,
    ref_file: &mut ShellFile,
    key: &str,
    ref_path: &Path,
) -> Result<(), OpsError> {
    // Find the entry in primary
    let idx = find_export_index(primary, key)?;

    let node = &primary.lines[idx];
    let original_text = node.original_text().to_string();
    let line_number = node.line_number();

    // Extract value and style for adding to reference
    let (value, export_style, quote_style) = match node {
        LineNode::EnvExport {
            value,
            export_style,
            quote_style,
            ..
        } => (value.clone(), *export_style, *quote_style),
        _ => unreachable!(),
    };

    // Tag the line in primary as moved
    let ref_str = ref_path.to_string_lossy();
    let new_text = format!("#[envforge:moved:{} -> {}] {}", key, ref_str, original_text);

    primary.lines[idx] = LineNode::ManagedComment {
        line_number,
        original_text: new_text,
        tag: format!("moved:{} -> {}", key, ref_str),
        original_export: original_text,
    };

    // Add to reference file
    let prefix = match export_style {
        ExportStyle::Export => "export ",
        ExportStyle::Bare => "",
    };
    let quoted_value = match quote_style {
        QuoteStyle::Double => format!("\"{}\"", value),
        QuoteStyle::Single => format!("'{}'", value),
        QuoteStyle::None => value.clone(),
    };
    let ref_line = format!("{}{}={}", prefix, key, quoted_value);

    let ref_line_number = ref_file.lines.len();
    ref_file.lines.push(LineNode::EnvExport {
        line_number: ref_line_number,
        original_text: ref_line,
        key: key.to_string(),
        value,
        export_style,
        quote_style,
        inline_comment: None,
    });

    Ok(())
}

/// Restore an ENV entry from the reference file back to the primary file.
///
/// Removes the entry from reference file and restores the original export in primary.
pub fn restore_from_reference(
    primary: &mut ShellFile,
    ref_file: &mut ShellFile,
    key: &str,
) -> Result<(), OpsError> {
    // Find the moved comment in primary
    let primary_idx = primary
        .lines
        .iter()
        .position(|node| match node {
            LineNode::ManagedComment { tag, .. } => tag.starts_with(&format!("moved:{}", key)),
            _ => false,
        })
        .ok_or_else(|| OpsError::KeyNotFound {
            key: key.to_string(),
            file: primary.path.clone(),
        })?;

    // Get the original export text
    let (line_number, original_export) = match &primary.lines[primary_idx] {
        LineNode::ManagedComment {
            line_number,
            original_export,
            ..
        } => (*line_number, original_export.clone()),
        _ => unreachable!(),
    };

    // Re-parse the original export line
    let reparsed = parse_shell_content(&original_export, Path::new("")).map_err(|_| {
        OpsError::KeyNotFound {
            key: key.to_string(),
            file: primary.path.clone(),
        }
    })?;

    if let Some(mut restored_node) = reparsed.lines.into_iter().next() {
        match &mut restored_node {
            LineNode::EnvExport {
                line_number: ln, ..
            } => *ln = line_number,
            _ => {
                restored_node = LineNode::Other {
                    line_number,
                    original_text: original_export,
                };
            }
        }
        primary.lines[primary_idx] = restored_node;
    }

    // Remove from reference file
    if let Some(ref_idx) = ref_file.lines.iter().position(|node| match node {
        LineNode::EnvExport { key: k, .. } => k == key,
        _ => false,
    }) {
        ref_file.lines.remove(ref_idx);
    }

    Ok(())
}

/// Find the index of a unique EnvExport node by key.
fn find_export_index(shell_file: &ShellFile, key: &str) -> Result<usize, OpsError> {
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
    use std::path::Path;

    fn make_shell_file(content: &str) -> ShellFile {
        parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
    }

    fn make_shell_file_at(content: &str, path: &str) -> ShellFile {
        parse_shell_content(content, Path::new(path)).unwrap()
    }

    // ─── has_source_directive ─────────────────────────────────

    #[test]
    fn test_has_source_directive_found() {
        let sf = make_shell_file("# header\nsource /home/user/.env_managed\nexport FOO=\"bar\"");
        assert!(has_source_directive(
            &sf,
            Path::new("/home/user/.env_managed")
        ));
    }

    #[test]
    fn test_has_source_directive_not_found() {
        let sf = make_shell_file("export FOO=\"bar\"");
        assert!(!has_source_directive(
            &sf,
            Path::new("/home/user/.env_managed")
        ));
    }

    // ─── ensure_source_directive ──────────────────────────────

    #[test]
    fn test_ensure_source_directive_inserts() {
        let mut sf = make_shell_file("export FOO=\"bar\"");
        let ref_path = Path::new("/home/user/.env_managed");
        let before = sf.lines.len();
        ensure_source_directive(&mut sf, ref_path, 0, 0).unwrap();
        assert_eq!(sf.lines.len(), before + 2);
    }

    #[test]
    fn test_ensure_source_directive_idempotent() {
        let mut sf = make_shell_file(
            "# [envforge:source] Managed environment variables\nsource /home/user/.env_managed\nexport FOO=\"bar\"",
        );
        let ref_path = Path::new("/home/user/.env_managed");
        let before = sf.lines.len();
        ensure_source_directive(&mut sf, ref_path, 0, 0).unwrap();
        assert_eq!(sf.lines.len(), before, "Should not add duplicate directive");
    }

    // ─── move_to_reference / restore ──────────────────────────

    #[test]
    fn test_move_to_reference_creates_managed_comment() {
        let mut primary = make_shell_file_at("export API_KEY=\"secret\"", "/test/.zshrc");
        let mut ref_file = make_shell_file_at("# managed", "/test/.env_managed");
        let ref_path = Path::new("/test/.env_managed");

        move_to_reference(&mut primary, &mut ref_file, "API_KEY", ref_path).unwrap();

        // Primary should have ManagedComment
        assert!(matches!(primary.lines[0], LineNode::ManagedComment { .. }));
        // Ref file should have the EnvExport
        assert!(ref_file.lines.iter().any(|n| matches!(
            n,
            LineNode::EnvExport { key, .. } if key == "API_KEY"
        )));
    }

    #[test]
    fn test_move_key_not_found() {
        let mut primary = make_shell_file_at("export FOO=\"bar\"", "/test/.zshrc");
        let mut ref_file = make_shell_file_at("# managed", "/test/.env_managed");
        let result = move_to_reference(
            &mut primary,
            &mut ref_file,
            "MISSING",
            Path::new("/test/.env_managed"),
        );
        assert!(matches!(result, Err(OpsError::KeyNotFound { .. })));
    }

    #[test]
    fn test_restore_from_reference_roundtrip() {
        let mut primary = make_shell_file_at("export API_KEY=\"secret\"", "/test/.zshrc");
        let mut ref_file = make_shell_file_at("# managed", "/test/.env_managed");
        let ref_path = Path::new("/test/.env_managed");

        move_to_reference(&mut primary, &mut ref_file, "API_KEY", ref_path).unwrap();
        restore_from_reference(&mut primary, &mut ref_file, "API_KEY").unwrap();

        // Primary should have EnvExport restored
        match &primary.lines[0] {
            LineNode::EnvExport { key, value, .. } => {
                assert_eq!(key, "API_KEY");
                assert_eq!(value, "secret");
            }
            other => panic!("Expected restored EnvExport, got: {:?}", other),
        }
    }

    #[test]
    fn test_restore_key_not_found() {
        let mut primary = make_shell_file_at("export FOO=\"bar\"", "/test/.zshrc");
        let mut ref_file = make_shell_file_at("# managed", "/test/.env_managed");
        let result = restore_from_reference(&mut primary, &mut ref_file, "MISSING");
        assert!(matches!(result, Err(OpsError::KeyNotFound { .. })));
    }
}
