use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::model::ShellFile;
use crate::parser::serialize_shell_file;

/// Information about a detected file conflict.
#[derive(Debug)]
pub struct Conflict {
    pub path: PathBuf,
    pub stored_hash: [u8; 32],
    pub current_hash: [u8; 32],
}

/// How the user wants to resolve a conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    /// Re-parse the file from disk, discarding in-memory changes
    Reload,
    /// Overwrite the file with in-memory state
    Overwrite,
}

/// Check if a ShellFile's stored hash matches the current file on disk.
///
/// Returns `Some(Conflict)` if the file was modified externally.
/// Returns `None` if the hashes match or the file doesn't exist.
pub fn check_conflict(shell_file: &ShellFile) -> Option<Conflict> {
    let current_content = std::fs::read(&shell_file.path).ok()?;
    let current_hash = compute_hash(&current_content);

    if current_hash == shell_file.hash {
        None
    } else {
        Some(Conflict {
            path: shell_file.path.clone(),
            stored_hash: shell_file.hash,
            current_hash,
        })
    }
}

/// Generate a unified diff between original file content and modified content.
///
/// Reads the original from disk and compares with the serialized ShellFile.
pub fn generate_diff(shell_file: &ShellFile) -> Result<String, std::io::Error> {
    let original = std::fs::read_to_string(&shell_file.path)?;
    let modified = serialize_shell_file(shell_file);

    Ok(unified_diff(
        &original,
        &modified,
        &shell_file.path.to_string_lossy(),
    ))
}

/// Generate a unified diff from two strings (without requiring file I/O).
pub fn generate_diff_from_strings(original: &str, modified: &str, filename: &str) -> String {
    unified_diff(original, modified, filename)
}

/// Produce a unified diff between two strings.
fn unified_diff(original: &str, modified: &str, filename: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();

    if orig_lines == mod_lines {
        return String::new();
    }

    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", filename));
    output.push_str(&format!("+++ b/{}\n", filename));

    // Simple line-by-line diff (not optimal, but functional)
    let max_len = orig_lines.len().max(mod_lines.len());
    let mut in_hunk = false;
    let mut hunk_start_orig = 0;
    let mut hunk_start_mod = 0;
    let mut hunk_lines: Vec<String> = Vec::new();
    let context_lines = 3;

    for i in 0..max_len {
        let orig = orig_lines.get(i).copied();
        let modif = mod_lines.get(i).copied();

        match (orig, modif) {
            (Some(o), Some(m)) if o == m => {
                if in_hunk {
                    hunk_lines.push(format!(" {}", o));
                }
            }
            (Some(o), Some(m)) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start_orig = i.saturating_sub(context_lines);
                    hunk_start_mod = i.saturating_sub(context_lines);
                    // Add context before
                    for j in hunk_start_orig..i {
                        if let Some(ctx) = orig_lines.get(j) {
                            hunk_lines.push(format!(" {}", ctx));
                        }
                    }
                }
                hunk_lines.push(format!("-{}", o));
                hunk_lines.push(format!("+{}", m));
            }
            (Some(o), None) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start_orig = i.saturating_sub(context_lines);
                    hunk_start_mod = i.saturating_sub(context_lines);
                }
                hunk_lines.push(format!("-{}", o));
            }
            (None, Some(m)) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start_orig = i.saturating_sub(context_lines);
                    hunk_start_mod = i.saturating_sub(context_lines);
                }
                hunk_lines.push(format!("+{}", m));
            }
            (None, None) => {}
        }
    }

    if !hunk_lines.is_empty() {
        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk_start_orig + 1,
            orig_lines.len() - hunk_start_orig,
            hunk_start_mod + 1,
            mod_lines.len() - hunk_start_mod,
        ));
        for line in &hunk_lines {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_identical_strings() {
        let diff = generate_diff_from_strings("hello\nworld\n", "hello\nworld\n", "test.sh");
        assert!(
            diff.is_empty(),
            "Identical strings should produce empty diff"
        );
    }

    #[test]
    fn test_diff_added_line() {
        let diff = generate_diff_from_strings("a\n", "a\nb\n", "test.sh");
        assert!(diff.contains("+b"), "Should show added line");
    }

    #[test]
    fn test_diff_removed_line() {
        let diff = generate_diff_from_strings("a\nb\n", "a\n", "test.sh");
        assert!(diff.contains("-b"), "Should show removed line");
    }

    #[test]
    fn test_diff_changed_line() {
        let diff = generate_diff_from_strings("old\n", "new\n", "test.sh");
        assert!(diff.contains("-old"), "Should show removed old");
        assert!(diff.contains("+new"), "Should show added new");
    }

    #[test]
    fn test_diff_header_format() {
        let diff = generate_diff_from_strings("a\n", "b\n", "myfile.sh");
        assert!(diff.contains("--- a/myfile.sh"));
        assert!(diff.contains("+++ b/myfile.sh"));
    }

    #[test]
    fn test_diff_both_empty() {
        let diff = generate_diff_from_strings("", "", "test.sh");
        assert!(diff.is_empty());
    }
}
