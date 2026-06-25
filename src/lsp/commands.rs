use std::path::Path;

use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::ops::canary::scanner::{scan_reader, scan_text};
use crate::ops::canary::{check_canaries, create_canary, list_canaries, place_canary_in_file};
use crate::ops::fence::{check_fence_status, create_fence, remove_fence, resolve_fence_targets};
use crate::ops::lease::{list_leases, renew_lease};
use crate::ops::session::parse_ttl;

/// Stable list of command IDs the server advertises through
/// `execute_command_provider`. Clients call any of these via
/// `workspace/executeCommand` and the server routes through
/// [`dispatch_command`].
pub const SUPPORTED_COMMANDS: &[&str] = &[
    "envforge.fence.enable",
    "envforge.fence.disable",
    "envforge.fence.toggle",
    "envforge.fence.status",
    "envforge.fence.config",
    "envforge.canary.plant",
    "envforge.canary.list",
    "envforge.canary.scan",
    "envforge.canary.check",
    "envforge.volatile.status",
    "envforge.volatile.extend",
    "envforge.sync.push",
    "envforge.sync.pull",
    "envforge.sync.status",
    "envforge.run.volatile",
    "envforge.reveal.value",
];

/// Outcome shape returned by every command. Stable JSON so plugins can
/// rely on it without depending on internal Rust types.
fn ok(payload: Value) -> Value {
    json!({ "ok": true, "result": payload })
}

fn err(message: impl Into<String>) -> Value {
    json!({ "ok": false, "error": message.into() })
}

/// Route an `executeCommand` request to the matching op. `workspace_root`
/// is required for any filesystem-touching command and is passed as the
/// canonicalized project directory. `_args` is reserved for future
/// per-command argument plumbing — currently every command derives its
/// inputs from `workspace_root` alone.
pub fn dispatch_command(command_id: &str, _args: &[Value], workspace_root: Option<&Path>) -> Value {
    match command_id {
        "envforge.fence.enable" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => {
                if let Err(e) = guard_fence_mutation_rate_limit(workspace_root) {
                    return err(e);
                }
                match create_fence(root, false) {
                    Ok(result) => ok(json!({
                        "files_created": result.files_created.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        "files_updated": result.files_updated.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        "files_skipped": result.files_skipped.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        "files_failed": result.files_failed.iter().map(|(p, e)| json!({"path": p.display().to_string(), "error": e})).collect::<Vec<_>>(),
                    })),
                    Err(e) => err(format!("create_fence failed: {}", e)),
                }
            }
        },
        "envforge.fence.disable" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => {
                if let Err(e) = guard_fence_mutation_rate_limit(workspace_root) {
                    return err(e);
                }
                match remove_fence(root, false) {
                    Ok(result) => ok(json!({
                        "files_removed": result.files_removed.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        "files_updated": result.files_updated.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                        "files_skipped": result.files_skipped.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    })),
                    Err(e) => err(format!("remove_fence failed: {}", e)),
                }
            }
        },
        "envforge.fence.toggle" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => {
                if let Err(e) = guard_fence_mutation_rate_limit(workspace_root) {
                    return err(e);
                }
                let status = match check_fence_status(root) {
                    Ok(s) => s,
                    Err(e) => return err(format!("check_fence_status failed: {}", e)),
                };
                if status.all_fenced {
                    match remove_fence(root, false) {
                        Ok(_) => ok(json!({ "action": "disabled" })),
                        Err(e) => err(format!("remove_fence failed: {}", e)),
                    }
                } else {
                    match create_fence(root, false) {
                        Ok(_) => ok(json!({ "action": "enabled" })),
                        Err(e) => err(format!("create_fence failed: {}", e)),
                    }
                }
            }
        },
        "envforge.fence.status" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => match check_fence_status(root) {
                Ok(status) => {
                    // Extend with per-target resolved state so plugins can show
                    // the enabled-set without a separate fence.config call.
                    let resolved = {
                        let fence_cfg = crate::config::load_or_create_default()
                            .map(|c| c.fence)
                            .unwrap_or_default();
                        resolve_fence_targets(&fence_cfg)
                    };
                    let targets: Vec<Value> = resolved
                        .iter()
                        .map(|r| {
                            json!({
                                "target": r.target.as_str(),
                                "enabled": r.enabled,
                                "source": format!("{:?}", r.source).to_lowercase(),
                            })
                        })
                        .collect();
                    let mut payload = serde_json::to_value(&status).unwrap_or(Value::Null);
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("resolved_targets".to_string(), Value::Array(targets));
                    }
                    ok(payload)
                }
                Err(e) => err(format!("check_fence_status failed: {}", e)),
            },
        },
        "envforge.fence.config" => match workspace_root {
            None => err("workspace root not available"),
            Some(_root) => {
                let fence_cfg = crate::config::load_or_create_default()
                    .map(|c| c.fence)
                    .unwrap_or_default();
                let resolved = resolve_fence_targets(&fence_cfg);
                let targets: Vec<Value> = resolved
                    .iter()
                    .map(|r| {
                        json!({
                            "target": r.target.as_str(),
                            "enabled": r.enabled,
                            "source": format!("{:?}", r.source).to_lowercase(),
                        })
                    })
                    .collect();
                ok(Value::Array(targets))
            }
        },
        "envforge.canary.plant" => {
            // Argument shape:
            // [{
            //   "key":     "<ENV_KEY>",
            //   "pattern": "generic"|"aws_key"|"api_token" (optional),
            //   "file":    "<absolute path to .env*>" (optional — when
            //              present, the canary is placed in-file as a
            //              `# envforge canary: KEY=VALUE` comment line
            //              so the tripwire is actually scannable)
            // }]
            //
            // The fake value is generated server-side. Clients must NOT
            // pass a value — we do not want a canary's payload to flow
            // through plugin code paths where it could be logged.
            let arg = _args.first().cloned().unwrap_or(Value::Null);
            let key = arg
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let pattern = arg
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("generic")
                .to_string();
            let file = arg.get("file").and_then(|v| v.as_str()).map(String::from);
            let Some(key) = key else {
                return err("missing or invalid 'key' argument");
            };
            if key.is_empty() {
                return err("key cannot be empty");
            }
            if let Err(e) = super::security::guard_key_pattern_with_length(&key) {
                return err(e);
            }
            let secret = match create_canary(&key, &pattern) {
                Ok(s) => s,
                Err(e) => return err(format!("create_canary failed: {}", e)),
            };

            // Place the tripwire comment into the requested file. We
            // always use the "bottom" placement strategy from
            // `place_canary_in_file` — it preserves user content and
            // appends a single marker line.
            let placement: Option<bool> = if let Some(file_path) = file.as_deref() {
                match place_canary_in_file(&key, Path::new(file_path), "bottom") {
                    Ok(placed) => Some(placed),
                    Err(e) => {
                        return err(format!("place_canary_in_file failed: {}", e));
                    }
                }
            } else {
                None
            };

            ok(json!({
                "key": secret.key,
                "fake_value": secret.fake_value,
                "pattern": secret.pattern,
                "created_at": secret.created_at,
                "placed_in_file": placement,
                "file": file,
            }))
        }
        "envforge.volatile.extend" => {
            // Argument shape: [{ "name": "<lease name>", "ttl": "<duration, e.g. 30m>" }]
            // TTL parsed via session::parse_ttl so we accept human
            // durations like "30m" / "2h" — same vocabulary the rest
            // of the volatile UX speaks.
            let arg = _args.first().cloned().unwrap_or(Value::Null);
            let name = arg.get("name").and_then(|v| v.as_str()).map(str::to_string);
            let ttl = arg.get("ttl").and_then(|v| v.as_str()).map(str::to_string);
            let Some(name) = name else {
                return err("missing or invalid 'name' argument");
            };
            if name.is_empty() {
                return err("name cannot be empty");
            }
            if let Err(e) = super::security::guard_payload_safety("volatile.extend name", &name) {
                return err(e);
            }
            let Some(ttl) = ttl else {
                return err("missing or invalid 'ttl' argument");
            };
            let ttl_seconds = match parse_ttl(&ttl) {
                Ok(s) => s as i64,
                Err(e) => return err(format!("invalid ttl: {}", e)),
            };
            match renew_lease(&name, ttl_seconds) {
                Ok(Some(lease)) => ok(json!({
                    "name": lease.name,
                    "new_expires_at": lease.expires_at,
                    "ttl_seconds": ttl_seconds,
                })),
                Ok(None) => err(format!("lease '{}' not found", name)),
                Err(e) => err(format!("renew_lease failed: {}", e)),
            }
        }
        "envforge.volatile.status" => match list_leases() {
            Ok(leases) => {
                // Pick the soonest-expiring active lease so the status
                // bar always shows the most-urgent countdown. Expired
                // and revoked leases are dropped — they do not gate
                // any future secret access and showing them as "0s
                // left" would mislead.
                let active = leases
                    .into_iter()
                    .filter(|l| !l.expired && !l.revoked)
                    .min_by_key(|l| l.remaining_seconds.max(0));
                match active {
                    None => ok(Value::Null),
                    Some(l) => ok(json!({
                        "name": l.name,
                        "remaining_seconds": l.remaining_seconds,
                        "expires_at": l.expires_at,
                        "key_count": l.key_count,
                    })),
                }
            }
            Err(e) => err(format!("list_leases failed: {}", e)),
        },
        "envforge.canary.scan" => {
            // Incident-response style scan: walk a text blob or workspace-
            // bounded file looking for v2 canary tokens (`cnry_...`).
            // Plugins wire this to log-paste flows and ad-hoc "did one of
            // my canaries leak into this stack trace?" investigations.
            //
            // Argument shape: [{ "text": "<string>" }] OR [{ "file": "<path>" }]
            // File access is sandboxed to the workspace root.
            let arg = _args.first().cloned().unwrap_or(Value::Null);
            let text = arg.get("text").and_then(|v| v.as_str()).map(str::to_string);
            let file = arg.get("file").and_then(|v| v.as_str()).map(str::to_string);

            let matches: Vec<_> = match (text, file) {
                (Some(t), _) => scan_text(&t),
                (None, Some(f)) => {
                    // Sandboxed file scan via reusable security guards.
                    let canonical =
                        match super::security::guard_workspace_containment(workspace_root, &f) {
                            Ok(p) => p,
                            Err(e) => return err(e),
                        };
                    if let Err(e) = super::security::guard_scan_extension(&canonical) {
                        return err(e);
                    }
                    if let Err(e) = super::security::guard_file_size(&canonical, 10 * 1024 * 1024) {
                        return err(e);
                    }
                    let f_handle = match std::fs::File::open(&canonical) {
                        Ok(h) => h,
                        Err(e) => {
                            return err(format!("open {} failed: {}", canonical.display(), e));
                        }
                    };
                    scan_reader(f_handle)
                }
                (None, None) => {
                    return err("must provide either 'text' or 'file' argument");
                }
            };

            let payload: Vec<Value> = matches
                .into_iter()
                .map(|m| {
                    json!({
                        "token": m.token,
                        "byte_offset": m.byte_offset,
                        "line_number": m.line_number,
                    })
                })
                .collect();
            ok(json!({
                "match_count": payload.len(),
                "matches": payload,
            }))
        }
        "envforge.canary.check" => match check_canaries() {
            Ok(triggered) => {
                let payload: Vec<Value> = triggered
                    .into_iter()
                    .map(|c| {
                        json!({
                            "key": c.key,
                            "pattern": c.pattern,
                            "triggered": c.triggered,
                            "trigger_count": c.trigger_count,
                            "created_at": c.created_at,
                        })
                    })
                    .collect();
                ok(json!({
                    "triggered_count": payload.len(),
                    "triggered": payload,
                }))
            }
            Err(e) => err(format!("check_canaries failed: {}", e)),
        },
        "envforge.canary.list" => match list_canaries() {
            Ok(items) => {
                let payload: Vec<Value> = items
                    .into_iter()
                    .map(|c| {
                        json!({
                            "key": c.key,
                            "pattern": c.pattern,
                            "triggered": c.triggered,
                            "trigger_count": c.trigger_count,
                            "created_at": c.created_at,
                        })
                    })
                    .collect();
                ok(Value::Array(payload))
            }
            Err(e) => err(format!("list_canaries failed: {}", e)),
        },
        "envforge.sync.push" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => {
                let message = _args
                    .first()
                    .and_then(|v| v.get("message"))
                    .and_then(|m| m.as_str());
                if let Some(msg) = message {
                    if let Err(e) = super::security::guard_payload_safety("sync.push message", msg)
                    {
                        return err(e);
                    }
                }
                run_sync_subprocess(root, "push", message)
            }
        },
        "envforge.sync.pull" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => run_sync_subprocess(root, "pull", None),
        },
        "envforge.sync.status" => match workspace_root {
            None => err("workspace root not available"),
            Some(root) => run_sync_subprocess(root, "status", None),
        },
        "envforge.run.volatile" => {
            // Build a structured subprocess descriptor for the plugin.
            // We do NOT return a pre-formed shell string — that risks
            // command injection when downstream plugins eval / sh -c
            // the wrapper. Instead we return `binary` + `args` as a
            // JSON array so plugins construct the subprocess directly.
            //
            // Argument shape:
            //   [{ "command": "<user shell command>",
            //      "ttl":     "<duration string, default '30m'>" }]
            let arg = _args.first().cloned().unwrap_or(Value::Null);
            let command = arg
                .get("command")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let Some(command) = command else {
                return err("missing or invalid 'command' argument");
            };
            if command.trim().is_empty() {
                return err("command cannot be empty");
            }
            if let Err(e) = guard_shell_metacharacters(&command) {
                return err(e);
            }
            let ttl = arg
                .get("ttl")
                .and_then(|v| v.as_str())
                .unwrap_or("30m")
                .to_string();

            let bin = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "envforge".to_string());
            ok(json!({
                "binary": bin,
                "args": ["run", "--volatile", &ttl, "--", &command],
                "hint": "spawn binary with these args directly; do NOT evaluate in a shell",
                "ttl": ttl,
                "original_command": command,
            }))
        }
        "envforge.reveal.value" => {
            // Audit-logged value reveal. Security gates enforced via
            // reusable guard functions (src/lsp/security.rs).
            let arg = _args.first().cloned().unwrap_or(Value::Null);
            let key = arg.get("key").and_then(|v| v.as_str()).map(str::to_string);
            let Some(key) = key else {
                return err("missing or invalid 'key' argument");
            };
            if key.is_empty() {
                return err("key cannot be empty");
            }

            if let Err(e) = super::security::guard_key_pattern(&key) {
                return err(e);
            }
            if let Err(e) = super::security::guard_fence_check(workspace_root, "secret reveal") {
                return err(e);
            }

            if let Err(e) = guard_reveal_rate_limit(workspace_root) {
                return err(e);
            }

            let reason = arg
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("LSP reveal request")
                .to_string();

            if let Err(e) = super::security::guard_message_length("reveal.reason", &reason) {
                return err(e);
            }

            let exe = match std::env::current_exe() {
                Ok(p) => p,
                Err(e) => return err(format!("current_exe failed: {}", e)),
            };
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(["get", &key, "--json"]);
            spawn_clean(&mut cmd);
            let output = cmd.output();
            let output = match output {
                Ok(o) => o,
                Err(e) => return err(format!("spawn failed: {}", e)),
            };
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return err(format!("get failed: {}", stderr.trim()));
            }
            let stdout = {
                let raw = String::from_utf8_lossy(&output.stdout).to_string();
                Zeroizing::new(raw)
            };
            let parsed: Value = match serde_json::from_str(&stdout) {
                Ok(v) => v,
                Err(_) => return err("get output was not valid JSON"),
            };

            // Audit emit (no raw value in message).
            crate::ops::monitor::emit_event(crate::ops::monitor::RuntimeEvent {
                source: crate::ops::monitor::EventSource::Reveal,
                key: Some(key.clone()),
                message: format!("LSP reveal: {} ({})", key, reason),
                timestamp: chrono::Utc::now(),
                severity: crate::ops::monitor::SecuritySeverity::Warn,
            });

            let value = parsed.get("value").cloned().unwrap_or(Value::Null);
            let source_file = parsed.get("source_file").cloned().unwrap_or(Value::Null);

            // Drop Zeroizing-wrapped stdout (trigger heap overwrite).
            drop(stdout);

            // Zeroize subprocess output buffers.
            let mut out = output;
            out.stdout.fill(0);
            out.stderr.fill(0);

            let now = chrono::Utc::now().to_rfc3339();
            ok(json!({
                "key": key,
                "value": value,
                "source_file": source_file,
                "revealed_at": now,
                "reason": reason,
            }))
        }
        other => err(format!("unknown command: {}", other)),
    }
}

// Rate-limit state for reveal and fence operations — per-workspace isolated
// to prevent cross-workspace budget exhaustion (one workspace spamming reveal
// should not starve another).
use std::collections::HashMap;
use std::sync::Mutex;

struct WorkspaceRateState {
    reveal_count: u64,
    fence_mutation_count: u64,
    window_start: u64,
}

static WORKSPACE_RATE_STATE: std::sync::OnceLock<Mutex<HashMap<String, WorkspaceRateState>>> =
    std::sync::OnceLock::new();

const REVEAL_MAX_PER_WINDOW: u64 = 10;
const FENCE_MUTATION_MAX_PER_WINDOW: u64 = 3;
const RATE_WINDOW_SECS: u64 = 300;

fn workspace_key(root: Option<&Path>) -> String {
    root.and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "/".to_string())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[doc(hidden)]
pub fn __test_reset_rate_limits() {
    if let Some(state) = WORKSPACE_RATE_STATE.get() {
        if let Ok(mut state) = state.lock() {
            state.clear();
        }
    }
}

fn guard_reveal_rate_limit(workspace_root: Option<&Path>) -> Result<(), String> {
    let state = WORKSPACE_RATE_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut state = state
        .lock()
        .map_err(|_| "rate limiter lock poisoned".to_string())?;
    let key = workspace_key(workspace_root);
    let entry = state.entry(key).or_insert(WorkspaceRateState {
        reveal_count: 0,
        fence_mutation_count: 0,
        window_start: now_secs(),
    });

    let now = now_secs();
    if now.wrapping_sub(entry.window_start) > RATE_WINDOW_SECS {
        entry.window_start = now;
        entry.reveal_count = 1;
        entry.fence_mutation_count = 0;
    } else {
        entry.reveal_count += 1;
        if entry.reveal_count > REVEAL_MAX_PER_WINDOW {
            return Err(format!(
                "reveal rate limit exceeded: max {} per {}s for this workspace",
                REVEAL_MAX_PER_WINDOW, RATE_WINDOW_SECS
            ));
        }
    }
    Ok(())
}

fn guard_fence_mutation_rate_limit(workspace_root: Option<&Path>) -> Result<(), String> {
    let state = WORKSPACE_RATE_STATE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut state = state
        .lock()
        .map_err(|_| "rate limiter lock poisoned".to_string())?;
    let key = workspace_key(workspace_root);
    let entry = state.entry(key).or_insert(WorkspaceRateState {
        reveal_count: 0,
        fence_mutation_count: 0,
        window_start: now_secs(),
    });

    let now = now_secs();
    if now.wrapping_sub(entry.window_start) > RATE_WINDOW_SECS {
        entry.window_start = now;
        entry.fence_mutation_count = 1;
        entry.reveal_count = 0;
    } else {
        entry.fence_mutation_count += 1;
        if entry.fence_mutation_count > FENCE_MUTATION_MAX_PER_WINDOW {
            return Err(format!(
                "fence mutation rate limited: max {} per {}s for this workspace",
                FENCE_MUTATION_MAX_PER_WINDOW, RATE_WINDOW_SECS
            ));
        }
    }
    Ok(())
}

fn guard_shell_metacharacters(command: &str) -> Result<(), String> {
    static META_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = META_RE
        .get_or_init(|| regex::Regex::new(r"[];&|`$(){}\[<>!#~*?\n\r]").expect("static regex"));
    if re.is_match(command) {
        return Err(format!(
            "command contains shell metacharacters; rejected for safety: '{}'",
            command
        ));
    }
    Ok(())
}

fn spawn_clean(cmd: &mut std::process::Command) {
    cmd.env_clear();
    for key in &["HOME", "PATH", "USER", "LANG", "TMPDIR"] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
}

/// Invoke a `sync push|pull|status` subcommand by re-execing ourselves
/// via `current_exe()`. The LSP server lives in the same binary, so
/// `cargo install env-forge-tui` users will always re-enter the same
/// process build. Subprocess isolates failures and lets us reuse the
/// CLI's existing JSON output format without refactoring sync_cmd.
fn run_sync_subprocess(workspace: &Path, action: &str, message: Option<&str>) -> Value {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return err(format!("current_exe failed: {}", e)),
    };

    let mut args: Vec<String> = vec!["sync".into(), action.into()];
    if action == "push" {
        if let Some(m) = message {
            args.push("--message".into());
            args.push(m.into());
        }
    }
    args.push("--json".into());

    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args).current_dir(workspace);
    spawn_clean(&mut cmd);
    let output = cmd.output();

    let output = match output {
        Ok(o) => o,
        Err(e) => return err(format!("spawn failed: {}", e)),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Try to parse stdout as JSON. Even non-zero exits often produce
    // structured error JSON we want to surface verbatim to the client.
    let parsed: Option<Value> = serde_json::from_str(&stdout).ok();

    if output.status.success() {
        match parsed {
            Some(v) => ok(v),
            None => ok(json!({ "stdout": stdout })),
        }
    } else {
        let payload = json!({
            "exit_code": output.status.code(),
            "stdout": parsed.unwrap_or(Value::String(stdout)),
            "stderr": stderr,
        });
        json!({
            "ok": false,
            "error": format!("sync {} failed", action),
            "detail": payload,
        })
    }
}
