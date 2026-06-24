use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ops::listing::EnvEntry;

/// A secret found in a scanned file.
#[derive(Debug, Clone)]
pub struct SecretMatch {
    pub file: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub matched_key: String,
    pub matched_value: String,
}

/// Scan a directory for files containing sensitive ENV values.
pub fn scan_directory(
    path: &Path,
    sensitive_entries: &[EnvEntry],
) -> Result<Vec<SecretMatch>, std::io::Error> {
    let mut matches = Vec::new();

    if path.is_file() {
        scan_file(path, sensitive_entries, &mut matches)?;
    } else if path.is_dir() {
        scan_dir_recursive(path, sensitive_entries, &mut matches)?;
    }

    Ok(matches)
}

/// Scan only git staged files.
pub fn scan_staged(sensitive_entries: &[EnvEntry]) -> Result<Vec<SecretMatch>, std::io::Error> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::other(
            "git command failed — not a git repo?",
        ));
    }

    let file_list = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();

    for file_path in file_list.lines() {
        let path = Path::new(file_path);
        if path.exists() {
            scan_file(path, sensitive_entries, &mut matches)?;
        }
    }

    Ok(matches)
}

/// Get sensitive entries (keys matching SECRET/TOKEN/PASSWORD/KEY/CREDENTIAL).
pub fn filter_sensitive(entries: &[EnvEntry]) -> Vec<EnvEntry> {
    entries
        .iter()
        .filter(|e| {
            let lower = e.key.to_lowercase();
            (lower.contains("secret")
                || lower.contains("token")
                || lower.contains("password")
                || lower.contains("credential")
                || (lower.contains("key") && !lower.contains("keyboard")))
                && !e.value.is_empty()
                && e.value.len() >= 4 // Skip very short values (likely not secrets)
        })
        .cloned()
        .collect()
}

fn scan_dir_recursive(
    dir: &Path,
    entries: &[EnvEntry],
    matches: &mut Vec<SecretMatch>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden dirs, node_modules, target, .git
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "vendor"
                || name == "__pycache__"
            {
                continue;
            }
        }

        if path.is_dir() {
            scan_dir_recursive(&path, entries, matches)?;
        } else if path.is_file() {
            scan_file(&path, entries, matches)?;
        }
    }
    Ok(())
}

fn scan_file(
    path: &Path,
    entries: &[EnvEntry],
    matches: &mut Vec<SecretMatch>,
) -> Result<(), std::io::Error> {
    if is_binary_extension(path) {
        return Ok(());
    }

    // Cap per-file size to defend against OOM when scanning user-pointed
    // directories that may contain a massive text-like file (e.g. a
    // crafted log inside a repo `envforge scan` is asked to walk).
    const MAX_SCAN_FILE_BYTES: u64 = 10 * 1024 * 1024;
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_SCAN_FILE_BYTES {
            return Ok(());
        }
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // Skip unreadable files
    };

    for (line_num, line) in content.lines().enumerate() {
        for entry in entries {
            if line.contains(&entry.value) {
                matches.push(SecretMatch {
                    file: path.to_path_buf(),
                    line_number: line_num + 1,
                    line_content: truncate_line(line, 120),
                    matched_key: entry.key.clone(),
                    matched_value: mask_value(&entry.value),
                });
            }
        }
    }

    Ok(())
}

fn is_binary_extension(path: &Path) -> bool {
    let binary_exts = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "woff", "woff2", "ttf", "eot", "otf",
        "mp3", "mp4", "avi", "mov", "pdf", "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "exe",
        "dll", "so", "dylib", "o", "a", "class", "jar", "pyc", "wasm", "db", "sqlite",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| binary_exts.contains(&ext.to_lowercase().as_str()))
}

fn truncate_line(line: &str, max: usize) -> String {
    if line.len() > max {
        let mut truncated = String::new();
        let mut byte_count = 0;
        for ch in line.chars() {
            let ch_len = ch.len_utf8();
            if byte_count + ch_len > max {
                break;
            }
            truncated.push(ch);
            byte_count += ch_len;
        }
        format!("{}…", truncated)
    } else {
        line.to_string()
    }
}

fn mask_value(value: &str) -> String {
    if value.len() <= 4 {
        "****".to_string()
    } else {
        format!("{}***", crate::ops::sanitize::char_prefix(value, 3))
    }
}
