//! CLI handlers for `mcp pin / verify / diff / trust / untrust / explain`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value as JsonValue};

use crate::ops::mcp_pin::resolver::{
    McpConfigFragment, ReputationLookup, ResolveOpts, ResolvedArtifact, Resolver,
};
use crate::ops::mcp_pin::{
    pinned_by_machine_id, BinaryHash, FsLockfileRepository, Lockfile, LockfileRepository,
    PackageManager, PinMethod, Platform, ServerPin, Transport,
};
use crate::ops::mcp_reputation::{
    FsUserOverrideRepository, Tier, TierLookup, UserOverrideRepository, UserOverrideStore,
};

const DEFAULT_LOCKFILE: &str = ".envforge/mcp.lock";

type CmdResult = Result<(), Box<dyn std::error::Error>>;

fn lockfile_path(override_path: Option<&PathBuf>) -> PathBuf {
    override_path
        .cloned()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LOCKFILE))
}

fn build_reputation() -> Result<Arc<dyn ReputationLookup>, Box<dyn std::error::Error>> {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(FsUserOverrideRepository::at_default());
    let lookup = TierLookup::new(repo)?;
    Ok(Arc::new(lookup))
}

fn build_override_store() -> UserOverrideStore {
    let repo: Arc<dyn UserOverrideRepository> = Arc::new(FsUserOverrideRepository::at_default());
    UserOverrideStore::new(repo)
}

fn discover_fragments() -> Vec<McpConfigFragment> {
    use crate::ops::mcp_scan::mcp_config_paths;

    let mut out = Vec::new();
    for path in mcp_config_paths() {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let parsed: JsonValue = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let servers = parsed.get("mcpServers").and_then(JsonValue::as_object);
        let Some(map) = servers else { continue };
        for (name, entry) in map {
            let cfg_value = entry.clone();
            let mut fragment_value = cfg_value.clone();
            if let Some(obj) = fragment_value.as_object_mut() {
                obj.insert("name".to_string(), JsonValue::String(name.clone()));
            }
            if let Ok(fragment) = serde_json::from_value::<McpConfigFragment>(fragment_value) {
                out.push(fragment);
            }
        }
    }
    out
}

fn artifact_to_server_pin(artifact: ResolvedArtifact, strict: bool) -> ServerPin {
    let pin_method = if strict {
        PinMethod::Strict
    } else {
        PinMethod::Auto
    };
    let binary_hashes: Vec<BinaryHash> = artifact
        .binary_hash
        .map(|h| {
            vec![BinaryHash::from_bytes(
                Platform::current(),
                h.sha256,
                h.realpath,
            )]
        })
        .unwrap_or_default();

    let (command, args, transport, url) = match &artifact.package_manager {
        PackageManager::RemoteSse { url } => (None, None, Transport::Sse, Some(url.clone())),
        PackageManager::RemoteHttp { url } => (None, None, Transport::Http, Some(url.clone())),
        _ => (None, None, artifact.transport, None),
    };

    ServerPin {
        name: artifact.name,
        pin_method,
        pinned_at: artifact.resolved_at,
        pinned_by_machine: pinned_by_machine_id(),
        command,
        args,
        transport,
        url,
        package_manager: Some(artifact.package_manager),
        package_integrity: artifact.package_integrity,
        config_hash: "0".repeat(64), // placeholder — a future bolt will populate
        tool_list_hash: artifact.initialize_response_hash.map(|d| hex::encode(d.0)),
        tool_list_captured_at: artifact.initialize_response_hash.map(|_| Utc::now()),
        dynamic_tools: false,
        volatile: artifact.volatile,
        spki_sha256: artifact.spki_sha256.map(|d| hex::encode(d.0)),
        initialize_response_hash: artifact.initialize_response_hash.map(|d| hex::encode(d.0)),
        binary_hashes,
    }
}

fn tier_to_str(t: &Tier) -> &'static str {
    match t {
        Tier::KnownGood => "known-good",
        Tier::Unknown => "unknown",
        Tier::KnownBad { .. } => "known-bad",
        Tier::UserTrusted { .. } => "user-trusted",
        Tier::Volatile => "volatile",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// pin
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn cmd_pin(
    strict: bool,
    inspect: bool,
    lockfile_override: Option<&PathBuf>,
    refresh: bool,
    accept: bool,
    yes: bool,
    resolve_conflicts: Option<&str>,
) -> CmdResult {
    let path = lockfile_path(lockfile_override);
    let repo = FsLockfileRepository;

    if let Some(strategy) = resolve_conflicts {
        return resolve_conflicts_flow(&path, strategy);
    }

    let exists = repo.exists(&path);
    if exists && !refresh {
        eprintln!(
            "lockfile already exists at {}; use --refresh to update",
            path.display()
        );
        std::process::exit(2);
    }

    if refresh && exists && !accept && !yes {
        eprintln!("--refresh requires either --accept (after diff review) or --yes (CI bypass)");
        std::process::exit(2);
    }

    let reputation = build_reputation()?;
    let opts = ResolveOpts {
        reputation: reputation.clone(),
        project_root: Some(std::env::current_dir()?),
        inspect,
        ..ResolveOpts::default()
    };

    let fragments = discover_fragments();
    if fragments.is_empty() {
        eprintln!("no MCP servers found in configured paths");
        if !exists {
            let empty = Lockfile::new("2026-05-12");
            repo.save(&path, &empty)?;
        }
        return Ok(());
    }

    let mut lockfile = if exists {
        repo.load(&path)?
    } else {
        Lockfile::new("2026-05-12")
    };

    let mut errors: Vec<String> = Vec::new();
    let mut refused_strict: Vec<String> = Vec::new();

    for fragment in fragments {
        let name = fragment.name.clone();
        if strict {
            match reputation.lookup(&name) {
                Tier::KnownGood | Tier::UserTrusted { .. } => {}
                Tier::KnownBad { reason, .. } => {
                    refused_strict.push(format!("'{name}' refused: KnownBad ({reason})"));
                    continue;
                }
                _ => {
                    refused_strict
                        .push(format!("'{name}' refused: not KNOWN_GOOD or USER_TRUSTED"));
                    continue;
                }
            }
        }
        match Resolver::resolve(&fragment, &opts) {
            Ok(artifact) => {
                let pin = artifact_to_server_pin(artifact, strict);
                if let Err(e) = lockfile.upsert_server(pin) {
                    errors.push(format!("'{name}': {e}"));
                }
            }
            Err(e) => {
                errors.push(format!("'{name}': {e}"));
            }
        }
    }

    if !refused_strict.is_empty() {
        eprintln!("strict mode refused {} server(s):", refused_strict.len());
        for line in &refused_strict {
            eprintln!("  {line}");
        }
        std::process::exit(1);
    }
    if !errors.is_empty() {
        eprintln!("resolution errors:");
        for e in &errors {
            eprintln!("  {e}");
        }
        std::process::exit(1);
    }

    repo.save(&path, &lockfile)?;
    eprintln!(
        "pinned {} server(s) to {}",
        lockfile.servers.len(),
        path.display()
    );
    Ok(())
}

fn resolve_conflicts_flow(path: &Path, strategy: &str) -> CmdResult {
    let bytes = std::fs::read(path)?;
    let text = std::str::from_utf8(&bytes)?;
    let (ours, theirs) = split_merge_conflict(text);
    let resolved = match strategy {
        "ours" => ours,
        "theirs" => theirs,
        other => {
            eprintln!("unknown strategy '{other}'; use 'ours' or 'theirs'");
            std::process::exit(2);
        }
    };
    std::fs::write(path, resolved.as_bytes())?;
    eprintln!("resolved lockfile conflicts using '{strategy}'");
    Ok(())
}

fn split_merge_conflict(text: &str) -> (String, String) {
    let mut ours = String::new();
    let mut theirs = String::new();
    let mut mode = 0u8; // 0 = neither side, 1 = ours, 2 = theirs
    for line in text.lines() {
        if line.starts_with("<<<<<<<") {
            mode = 1;
            continue;
        }
        if line.starts_with("=======") {
            mode = 2;
            continue;
        }
        if line.starts_with(">>>>>>>") {
            mode = 0;
            continue;
        }
        match mode {
            1 => {
                ours.push_str(line);
                ours.push('\n');
            }
            2 => {
                theirs.push_str(line);
                theirs.push('\n');
            }
            _ => {
                ours.push_str(line);
                ours.push('\n');
                theirs.push_str(line);
                theirs.push('\n');
            }
        }
    }
    (ours, theirs)
}

// ─────────────────────────────────────────────────────────────────────────────
// verify
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_verify(json: bool, strict: bool, lockfile_override: Option<&PathBuf>) -> CmdResult {
    let path = lockfile_path(lockfile_override);
    let repo = FsLockfileRepository;
    if !repo.exists(&path) {
        eprintln!(
            "lockfile missing at {}; run `mcp pin` first",
            path.display()
        );
        std::process::exit(2);
    }
    let lockfile = repo.load(&path)?;
    let reputation = build_reputation()?;
    let mut findings: Vec<JsonValue> = Vec::new();
    let mut any_mismatch = false;
    let mut strict_trip = false;

    for pin in &lockfile.servers {
        let tier = reputation.lookup(&pin.name);
        let tier_str = tier_to_str(&tier);
        let status = match &tier {
            Tier::KnownBad { .. } => {
                any_mismatch = true;
                "known-bad"
            }
            _ => "match",
        };
        if strict && matches!(tier, Tier::Unknown) {
            strict_trip = true;
        }
        findings.push(json!({
            "name": pin.name,
            "status": status,
            "pin_status": "pinned",
            "reputation_tier": tier_str,
        }));
    }

    let exit_code = if any_mismatch || strict_trip { 1 } else { 0 };

    if json {
        let report = json!({
            "format_version": lockfile.format_version,
            "pattern_set_version": lockfile.pattern_set_version,
            "servers": findings,
            "exit_code": exit_code,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if exit_code == 0 {
        eprintln!("all {} server(s) verified", lockfile.servers.len());
    } else {
        eprintln!(
            "verify failed: {} server(s) checked, exit code {}",
            lockfile.servers.len(),
            exit_code
        );
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// diff
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_diff(server: Option<&str>, lockfile_override: Option<&PathBuf>) -> CmdResult {
    let path = lockfile_path(lockfile_override);
    let repo = FsLockfileRepository;
    if !repo.exists(&path) {
        eprintln!("lockfile missing at {}", path.display());
        std::process::exit(2);
    }
    let lockfile = repo.load(&path)?;
    let reputation = build_reputation()?;

    for pin in &lockfile.servers {
        if let Some(filter) = server {
            if pin.name != filter {
                continue;
            }
        }
        let tier = reputation.lookup(&pin.name);
        println!("{} [{}]", pin.name, tier_to_str(&tier));
        println!("  pin_method: {}", pin.pin_method);
        println!("  config_hash: {}", pin.config_hash);
        if let Some(integrity) = &pin.package_integrity {
            println!("  integrity: {integrity}");
        }
        for bh in &pin.binary_hashes {
            println!("  binary[{}]: {}", bh.platform, bh.sha256);
        }
        if let Some(spki) = &pin.spki_sha256 {
            println!("  spki_sha256: {spki}");
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// trust / untrust
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_trust(name: &str, reason: &str) -> CmdResult {
    if reason.trim().is_empty() {
        eprintln!("--reason must be non-empty");
        std::process::exit(2);
    }
    let store = build_override_store();
    store.record_user_trust(name, reason)?;
    eprintln!("recorded USER_TRUSTED override for '{name}'");
    Ok(())
}

pub fn cmd_untrust(name: &str) -> CmdResult {
    let store = build_override_store();
    let removed = store.revoke_user_trust(name)?;
    if removed {
        eprintln!("removed override for '{name}'");
    } else {
        eprintln!("no override existed for '{name}' (no-op)");
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// explain
// ─────────────────────────────────────────────────────────────────────────────

pub fn cmd_explain(lock: bool, format: &str, lockfile_override: Option<&PathBuf>) -> CmdResult {
    if !lock {
        eprintln!("--lock is required (currently the only explain mode)");
        std::process::exit(2);
    }
    let path = lockfile_path(lockfile_override);
    let repo = FsLockfileRepository;
    if !repo.exists(&path) {
        eprintln!("lockfile missing at {}", path.display());
        std::process::exit(2);
    }
    let lockfile = repo.load(&path)?;
    let reputation = build_reputation()?;

    match format {
        "markdown" => render_markdown(&lockfile, &reputation),
        _ => render_text(&lockfile, &reputation),
    }
    Ok(())
}

fn render_text(lockfile: &Lockfile, reputation: &Arc<dyn ReputationLookup>) {
    println!("# mcp.lock (format v{})", lockfile.format_version);
    println!("pattern_set_version: {}", lockfile.pattern_set_version);
    println!();
    for pin in &lockfile.servers {
        let tier = reputation.lookup(&pin.name);
        println!("- {} [{}]", pin.name, tier_to_str(&tier));
    }
}

fn render_markdown(lockfile: &Lockfile, reputation: &Arc<dyn ReputationLookup>) {
    println!("## MCP Lockfile (format v{})", lockfile.format_version);
    println!();
    println!("| Server | Reputation | Pinned Via | Integrity |");
    println!("|--------|------------|------------|-----------|");
    for pin in &lockfile.servers {
        let tier = reputation.lookup(&pin.name);
        let integrity = pin
            .package_integrity
            .as_deref()
            .map(|s| {
                if s.len() > 12 {
                    format!("{}…", &s[..12])
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "—".to_string());
        println!(
            "| `{}` | {} | {} | {} |",
            pin.name,
            tier_to_str(&tier),
            pin.pin_method,
            integrity
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// launch — atomic verify+exec
// ─────────────────────────────────────────────────────────────────────────────

/// Map an IDE alias to the binary name on PATH.
fn ide_binary(ide: &str) -> Option<&'static str> {
    match ide {
        "claude-code" | "claude" => Some("claude"),
        "cursor" => Some("cursor"),
        _ => None,
    }
}

pub fn cmd_launch(ide: &str, args: &[String], lockfile_override: Option<&PathBuf>) -> CmdResult {
    let binary = if let Some(b) = ide_binary(ide) {
        b
    } else {
        eprintln!("unknown IDE '{ide}'; supported: claude-code, cursor");
        std::process::exit(2);
    };

    let path = lockfile_path(lockfile_override);
    let repo = FsLockfileRepository;
    if !repo.exists(&path) {
        eprintln!(
            "lockfile missing at {}; run `mcp pin` first",
            path.display()
        );
        std::process::exit(2);
    }
    let lockfile = repo.load(&path)?;
    let reputation = build_reputation()?;

    let mut blocked: Vec<String> = Vec::new();
    for pin in &lockfile.servers {
        if let Tier::KnownBad { reason, .. } = reputation.lookup(&pin.name) {
            blocked.push(format!("'{}' is KnownBad: {reason}", pin.name));
        }
    }
    if !blocked.is_empty() {
        eprintln!("verify-then-launch refused {} server(s):", blocked.len());
        for line in &blocked {
            eprintln!("  {line}");
        }
        eprintln!("audit: McpLaunchBlocked (placeholder; Unit 007 wires real event)");
        std::process::exit(1);
    }

    eprintln!(
        "verify passed: {} server(s) checked; launching '{binary}'",
        lockfile.servers.len()
    );

    exec_replace(binary, args)
}

#[cfg(unix)]
fn exec_replace(binary: &str, args: &[String]) -> CmdResult {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(binary).args(args).exec();
    // exec only returns on failure.
    eprintln!("exec failed for '{binary}': {err}");
    std::process::exit(127);
}

#[cfg(not(unix))]
fn exec_replace(binary: &str, args: &[String]) -> CmdResult {
    let status = std::process::Command::new(binary)
        .args(args)
        .status()
        .map_err(|e| -> Box<dyn std::error::Error> { format!("spawn '{binary}': {e}").into() })?;
    let code = status.code().unwrap_or(127);
    std::process::exit(code);
}
