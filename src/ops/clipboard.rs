use std::sync::atomic::{AtomicBool, Ordering};

use super::dotenv::is_sensitive_key;
use super::listing::EnvEntry;

/// Global toggle to disable clipboard for sensitive values.
static CLIPBOARD_DISABLED: AtomicBool = AtomicBool::new(false);

/// Disable clipboard operations for sensitive values.
pub fn disable_clipboard() {
    CLIPBOARD_DISABLED.store(true, Ordering::SeqCst);
}

/// Enable clipboard operations.
pub fn enable_clipboard() {
    CLIPBOARD_DISABLED.store(false, Ordering::SeqCst);
}

/// Check whether clipboard is globally enabled.
pub fn is_clipboard_enabled() -> bool {
    !CLIPBOARD_DISABLED.load(Ordering::SeqCst)
}

/// Errors that can occur during clipboard operations.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
    #[error("clipboard disabled for sensitive values")]
    SensitiveValueBlocked,
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
/// Blocks copying if the key looks sensitive and clipboard is disabled.
pub fn copy_value(entry: &EnvEntry) -> Result<(), ClipboardError> {
    if !is_clipboard_enabled() && is_sensitive_key(&entry.key) {
        return Err(ClipboardError::SensitiveValueBlocked);
    }
    copy_to_clipboard(&entry.value)
}

/// Copy just the key name of an ENV entry to the clipboard.
pub fn copy_key(entry: &EnvEntry) -> Result<(), ClipboardError> {
    copy_to_clipboard(&entry.key)
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
