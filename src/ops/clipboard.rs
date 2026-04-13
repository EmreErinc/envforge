use super::listing::EnvEntry;

/// Errors that can occur during clipboard operations.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
}

/// Copy text to the system clipboard.
///
/// Tries copypasta first, falls back to pbcopy on macOS.
pub fn copy_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    // Try copypasta first
    match copy_via_copypasta(text) {
        Ok(()) => return Ok(()),
        Err(_) => {
            // Fall back to pbcopy on macOS
            if cfg!(target_os = "macos") {
                return copy_via_pbcopy(text);
            }
        }
    }

    Err(ClipboardError::Unavailable(
        "no clipboard backend available".to_string(),
    ))
}

/// Copy just the value of an ENV entry to the clipboard.
pub fn copy_value(entry: &EnvEntry) -> Result<(), ClipboardError> {
    copy_to_clipboard(&entry.value)
}

/// Copy KEY=VALUE of an ENV entry to the clipboard.
pub fn copy_key_value(entry: &EnvEntry) -> Result<(), ClipboardError> {
    let text = format!("{}={}", entry.key, entry.value);
    copy_to_clipboard(&text)
}

fn copy_via_copypasta(text: &str) -> Result<(), ClipboardError> {
    use copypasta::{ClipboardContext, ClipboardProvider};

    let mut ctx =
        ClipboardContext::new().map_err(|e| ClipboardError::Unavailable(e.to_string()))?;

    ctx.set_contents(text.to_string())
        .map_err(|e| ClipboardError::Unavailable(e.to_string()))?;

    Ok(())
}

fn copy_via_pbcopy(text: &str) -> Result<(), ClipboardError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| ClipboardError::Unavailable(format!("pbcopy failed: {}", e)))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| ClipboardError::Unavailable(format!("pbcopy write failed: {}", e)))?;
    }

    let status = child
        .wait()
        .map_err(|e| ClipboardError::Unavailable(format!("pbcopy wait failed: {}", e)))?;

    if status.success() {
        Ok(())
    } else {
        Err(ClipboardError::Unavailable(
            "pbcopy exited with error".to_string(),
        ))
    }
}

/// Get the path to the clipboard provider (for diagnostics).
pub fn clipboard_provider_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "copypasta (pbcopy fallback)"
    } else {
        "copypasta (X11/Wayland)"
    }
}
