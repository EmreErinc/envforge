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
    match copy_via_arboard(text) {
        Ok(()) => return Ok(()),
        Err(_) => {
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
/// If the key looks sensitive (and clipboard isn't globally disabled), the
/// value is auto-cleared after [`DEFAULT_CLIPBOARD_TTL_SECS`] seconds.
pub fn copy_value(entry: &EnvEntry) -> Result<(), ClipboardError> {
    let sensitive = is_sensitive_key(&entry.key);
    if !is_clipboard_enabled() && sensitive {
        return Err(ClipboardError::SensitiveValueBlocked);
    }
    if sensitive {
        copy_to_clipboard_with_ttl(&entry.value, DEFAULT_CLIPBOARD_TTL_SECS)
    } else {
        copy_to_clipboard(&entry.value)
    }
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

fn copy_via_arboard(text: &str) -> Result<(), ClipboardError> {
    let mut ctx =
        arboard::Clipboard::new().map_err(|e| ClipboardError::Unavailable(e.to_string()))?;

    ctx.set_text(text.to_string())
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
        "arboard (pbcopy fallback)"
    } else {
        "arboard (X11/Wayland)"
    }
}

/// Default TTL (seconds) before secret clipboard contents auto-clear.
pub const DEFAULT_CLIPBOARD_TTL_SECS: u64 = 30;

/// Copy a sensitive value to the clipboard with a best-effort auto-clear
/// timer.
///
/// Spawns a detached thread that, after `ttl_secs` seconds, replaces the
/// clipboard contents with an empty string IF they still equal the value
/// we wrote (so we don't clobber whatever the user copied since). This is
/// best-effort: macOS Pasteboard history (`~/Library/Caches`), X11
/// PRIMARY/SECONDARY selections, and clipboard managers can still retain
/// the value beyond our control.
pub fn copy_to_clipboard_with_ttl(text: &str, ttl_secs: u64) -> Result<(), ClipboardError> {
    copy_to_clipboard(text)?;

    if ttl_secs == 0 {
        return Ok(());
    }
    let owned = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(ttl_secs));
        // Only clear if the clipboard still holds what we wrote.
        if let Ok(current) = current_clipboard() {
            if current == owned {
                let _ = copy_to_clipboard("");
            }
        } else {
            // Couldn't read; clear unconditionally as a safety net.
            let _ = copy_to_clipboard("");
        }
    });
    Ok(())
}

fn current_clipboard() -> Result<String, ClipboardError> {
    let mut ctx =
        arboard::Clipboard::new().map_err(|e| ClipboardError::Unavailable(e.to_string()))?;
    ctx.get_text()
        .map_err(|e| ClipboardError::Unavailable(e.to_string()))
}
