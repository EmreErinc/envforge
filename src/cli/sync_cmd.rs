use clap::Subcommand;
use serde_json::json;

use crate::config::*;
use crate::ops::sync::*;
use crate::ops::{collect_all_entries, EntryLocation};
use crate::parser::*;

#[derive(Subcommand)]
pub enum SyncAction {
    /// Initialize sync repository
    Init {
        /// Remote git URL to clone from
        #[arg(long)]
        remote: Option<String>,

        /// Custom machine ID
        #[arg(long)]
        machine_id: Option<String>,

        /// Force reinitialize (backup existing)
        #[arg(long)]
        force: bool,

        /// Reject non-SSH remotes (http/https) when cloning; persisted to sync config
        #[arg(long)]
        enforce_ssh: bool,
    },

    /// Push local changes to sync repository
    Push {
        /// Custom commit message
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Pull remote changes to local
    Pull,

    /// Show sync status (local vs snapshot diff)
    Status,

    /// Mark keys for sync or local-only
    Mark {
        /// Key name or glob pattern (optional when --all is used)
        key: Option<String>,

        /// Mark as synced
        #[arg(long, conflicts_with = "local", required_unless_present = "local")]
        sync: bool,

        /// Mark as local-only
        #[arg(long, conflicts_with = "sync", required_unless_present = "sync")]
        local: bool,

        /// Apply to all keys
        #[arg(long)]
        all: bool,
    },

    /// List keys with sync/local status
    ListKeys,

    /// Set a machine-specific override
    Override {
        /// Key name
        key: String,

        /// Value (omit to remove override)
        value: Option<String>,

        /// Remove override
        #[arg(long)]
        remove: bool,

        /// List all overrides
        #[arg(long)]
        list: bool,
    },

    /// Show sync history
    History {
        /// Number of entries
        #[arg(short, long, default_value = "10")]
        n: usize,
    },

    /// Rollback to a previous snapshot
    Rollback {
        /// Commit hash to rollback to
        commit: Option<String>,

        /// Rollback to previous snapshot
        #[arg(long)]
        last: bool,
    },

    /// View sync operation log
    Log {
        /// Number of entries
        #[arg(short, long, default_value = "10")]
        n: usize,
    },

    /// Show machine info
    Machine,
}

pub fn execute_sync(
    action: &SyncAction,
    json: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SyncAction::Init {
            remote,
            machine_id,
            force,
            enforce_ssh,
        } => cmd_sync_init(
            remote.as_deref(),
            machine_id.as_deref(),
            *force,
            *enforce_ssh,
            json,
        ),
        SyncAction::Push { message } => cmd_sync_push(message.as_deref(), dry_run, json),
        SyncAction::Pull => cmd_sync_pull(dry_run, json),
        SyncAction::Status => cmd_sync_status(json),
        SyncAction::Mark {
            key,
            sync,
            local,
            all,
        } => cmd_sync_mark(key.as_deref(), *sync, *local, *all, json),
        SyncAction::ListKeys => cmd_sync_list_keys(json),
        SyncAction::Override {
            key,
            value,
            remove,
            list,
        } => cmd_sync_override(key, value.as_deref(), *remove, *list, json),
        SyncAction::History { n } => cmd_sync_history(*n, json),
        SyncAction::Rollback { commit, last } => cmd_sync_rollback(commit.as_deref(), *last, json),
        SyncAction::Log { n } => cmd_sync_log(*n, json),
        SyncAction::Machine => cmd_sync_machine(json),
    }
}

fn get_sync_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = sync_dir()?;
    if !is_initialized(&path) {
        return Err(Box::new(SyncError::RepoNotInitialized));
    }
    Ok(path)
}

fn get_available_keys() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let mut shell_files = Vec::new();
    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        shell_files.push(parse_shell_file(&primary)?);
    }
    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        shell_files.push(parse_shell_file(&ref_path)?);
    }
    let entries = collect_all_entries(&shell_files);
    Ok(entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .map(|e| e.key.clone())
        .collect())
}

fn get_entries_as_pairs() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let mut shell_files = Vec::new();
    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        shell_files.push(parse_shell_file(&primary)?);
    }
    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        shell_files.push(parse_shell_file(&ref_path)?);
    }
    let entries = collect_all_entries(&shell_files);
    Ok(entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect())
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

// ─── Command Implementations ─────────────────────────────────

fn cmd_sync_init(
    remote: Option<&str>,
    custom_machine_id: Option<&str>,
    force: bool,
    enforce_ssh: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = sync_dir()?;
    let machine_id = generate_machine_id(custom_machine_id)?;

    if is_initialized(&sync_path) && force {
        let backup = backup_existing(&sync_path)?;
        if json {
            println!(
                "{}",
                json!({"version": 1, "backup": backup.to_string_lossy()})
            );
        } else {
            println!("Backed up existing sync to: {}", backup.display());
        }
    }

    if let Some(url) = remote {
        let has_snapshot = init_from_remote(&sync_path, url, &machine_id, enforce_ssh)?;
        if json {
            println!(
                "{}",
                json!({"version": 1, "initialized": true, "remote": url, "machine_id": machine_id, "existing_snapshot": has_snapshot})
            );
        } else {
            println!("Sync initialized from remote: {}", url);
            println!("Machine ID: {}", machine_id);
            if has_snapshot {
                println!("Existing snapshot found. Run `envforge sync pull` to apply.");
            }
        }
    } else {
        init_fresh(&sync_path, &machine_id)?;
        if json {
            println!(
                "{}",
                json!({"version": 1, "initialized": true, "machine_id": machine_id})
            );
        } else {
            println!("Sync initialized at: {}", sync_path.display());
            println!("Machine ID: {}", machine_id);
            println!("No remote configured. Use `envforge sync init --remote <url>` to add one.");
        }
    }

    Ok(())
}

fn cmd_sync_push(
    message: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let entries = get_entries_as_pairs()?;
    let summary = push::push(&sync_path, &entries, message, dry_run)?;

    if json {
        println!(
            "{}",
            json!({
                "version": 1,
                "keys_pushed": summary.keys_pushed,
                "commit_hash": summary.commit_hash,
                "push_result": format!("{:?}", summary.push_result),
                "message": summary.message,
                "dry_run": dry_run,
            })
        );
    } else if dry_run {
        println!("[dry-run] Would push {} keys", summary.keys_pushed);
    } else {
        println!("Pushed {} keys", summary.keys_pushed);
        if let Some(hash) = &summary.commit_hash {
            println!("Commit: {}", hash);
        }
        match summary.push_result {
            PushResult::Success => println!("Pushed to remote"),
            PushResult::NoRemote => println!("No remote configured (local commit only)"),
            PushResult::Rejected => println!("Push rejected. Run `envforge sync pull` first"),
        }
    }

    Ok(())
}

fn cmd_sync_pull(dry_run: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let entries = get_entries_as_pairs()?;
    let summary = pull::pull(&sync_path, &entries, dry_run)?;

    if json {
        println!(
            "{}",
            json!({
                "version": 1,
                "keys_added": summary.keys_added,
                "keys_modified": summary.keys_modified,
                "keys_removed": summary.keys_removed,
                "conflicts": summary.conflicts.len(),
                "dry_run": dry_run,
            })
        );
    } else {
        if dry_run {
            println!("[dry-run] Pull would change:");
        } else {
            println!("Pull complete:");
        }
        println!(
            "  +{} added, ~{} modified, -{} removed",
            summary.keys_added, summary.keys_modified, summary.keys_removed
        );
        if !summary.conflicts.is_empty() {
            println!("  {} conflicts need resolution", summary.conflicts.len());
            for c in &summary.conflicts {
                println!(
                    "    {} : local={:?} vs remote={:?}",
                    c.key, c.local_value, c.remote_value
                );
            }
        }
    }

    Ok(())
}

fn cmd_sync_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let entries = get_entries_as_pairs()?;
    let config_path = sync_path.join("sync-config.toml");
    let config = read_config(&config_path)?;

    let sync_entries: Vec<(String, String)> = entries
        .iter()
        .filter(|(k, _)| marking::get_key_status(k, &config) == KeyStatus::Synced)
        .cloned()
        .collect();

    let snapshot = read_snapshot(
        &sync_path.join("snapshot.toml"),
        &SyncEncryptionPolicy::MigrationUntil("2099-01-01T00:00:00Z".into()),
        false,
    )?;
    let status = diff::compute_status(&sync_entries, &snapshot.entries);
    let diff_result = diff::compute_diff(&sync_entries, &snapshot.entries);

    if json {
        println!(
            "{}",
            json!({
                "version": 1,
                "status": format!("{:?}", status),
                "added": diff_result.added.len(),
                "modified": diff_result.modified.len(),
                "removed": diff_result.removed.len(),
            })
        );
    } else {
        match status {
            SyncStatus::InSync => println!("Everything up to date"),
            SyncStatus::LocalAhead => {
                println!("Local changes not pushed:");
                for d in &diff_result.added {
                    println!("  + {}", d.key);
                }
                for d in &diff_result.modified {
                    println!("  ~ {}", d.key);
                }
                for d in &diff_result.removed {
                    println!("  - {}", d.key);
                }
            }
            SyncStatus::NotInitialized => println!("Sync not initialized"),
        }
    }

    Ok(())
}

fn cmd_sync_mark(
    key: Option<&str>,
    sync: bool,
    _local: bool,
    all: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let config_path = sync_path.join("sync-config.toml");
    let available_keys = get_available_keys()?;

    if !all && key.is_none() {
        return Err("Specify a key name or use --all".into());
    }

    let result = if all {
        marking::mark_all(&config_path, sync, &available_keys)?
    } else {
        let key = key.as_ref().expect("key checked for None above");
        if key.contains('*') || key.contains('?') {
            marking::mark_by_pattern(&config_path, key, sync, &available_keys)?
        } else {
            let keys = vec![(*key).to_string()];
            if sync {
                marking::mark_sync(&config_path, &keys, &available_keys)?
            } else {
                marking::mark_local(&config_path, &keys, &available_keys)?
            }
        }
    };

    if json {
        println!(
            "{}",
            json!({"version": 1, "marked": result.marked_keys, "warnings": result.warnings})
        );
    } else {
        if !result.marked_keys.is_empty() {
            let action = if sync { "sync" } else { "local" };
            println!("Marked {} key(s) as {}:", result.marked_keys.len(), action);
            for k in &result.marked_keys {
                println!("  {}", k);
            }
        }
        for w in &result.warnings {
            eprintln!("Warning: {}", w);
        }
    }

    Ok(())
}

fn cmd_sync_list_keys(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let config_path = sync_path.join("sync-config.toml");
    let config = read_config(&config_path)?;
    let available_keys = get_available_keys()?;
    let status_list = marking::list_keys_with_status(&config, &available_keys);

    if json {
        let entries: Vec<serde_json::Value> = status_list
            .iter()
            .map(|(k, s)| json!({"key": k, "status": format!("{:?}", s)}))
            .collect();
        println!("{}", json!({"version": 1, "keys": entries}));
    } else {
        for (key, status) in &status_list {
            let icon = match status {
                KeyStatus::Synced => "↑",
                KeyStatus::LocalOnly => "⊘",
                KeyStatus::Unset => "·",
            };
            println!("  {} {}", icon, key);
        }
        let synced = status_list
            .iter()
            .filter(|(_, s)| *s == KeyStatus::Synced)
            .count();
        let local = status_list
            .iter()
            .filter(|(_, s)| *s == KeyStatus::LocalOnly)
            .count();
        println!(
            "\n{} synced, {} local-only, {} unset",
            synced,
            local,
            status_list.len() - synced - local
        );
    }

    Ok(())
}

fn cmd_sync_override(
    key: &str,
    value: Option<&str>,
    remove: bool,
    list: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let config_path = sync_path.join("sync-config.toml");
    let config = read_config(&config_path)?;
    let machine_id = &config.sync.machine_id;

    if list {
        let overrides = machine::list_overrides(&sync_path, machine_id)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&overrides)?);
        } else if overrides.is_empty() {
            println!("No machine overrides configured");
        } else {
            for (k, v) in &overrides {
                println!("  {} = {}", k, v);
            }
        }
        return Ok(());
    }

    if remove {
        let removed = machine::remove_override(&sync_path, machine_id, key)?;
        if json {
            println!("{}", json!({"version": 1, "removed": removed, "key": key}));
        } else if removed {
            println!("Removed override for '{}'", key);
        } else {
            println!("No override found for '{}'", key);
        }
        return Ok(());
    }

    if let Some(val) = value {
        let warnings = machine::set_override(&sync_path, machine_id, key, val)?;
        if json {
            println!(
                "{}",
                json!({"version": 1, "set": true, "key": key, "value": val, "warnings": warnings})
            );
        } else {
            println!("Override set: {} = {}", key, val);
            for w in &warnings {
                eprintln!("Warning: {}", w);
            }
        }
    } else {
        eprintln!("Usage: envforge sync override <KEY> <VALUE> or --remove or --list");
    }

    Ok(())
}

fn cmd_sync_history(n: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let commits = history::list_history(&sync_path, n)?;

    if json {
        let entries: Vec<serde_json::Value> = commits
            .iter()
            .map(|c| {
                json!({
                    "hash": c.short_hash,
                    "date": c.date,
                    "message": c.message,
                    "author": c.author,
                })
            })
            .collect();
        println!("{}", json!({"version": 1, "entries": entries}));
    } else if commits.is_empty() {
        println!("No sync history yet");
    } else {
        for c in &commits {
            println!("  {} {} - {}", c.short_hash, c.date, c.message);
        }
    }

    Ok(())
}

fn cmd_sync_rollback(
    commit: Option<&str>,
    last: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;

    let backup = if last {
        history::rollback_last(&sync_path)?
    } else if let Some(hash) = commit {
        history::rollback_to(&sync_path, hash)?
    } else {
        return Err("Specify a commit hash or use --last".into());
    };

    if json {
        println!(
            "{}",
            json!({"version": 1, "rollback": true, "backup": backup.to_string_lossy()})
        );
    } else {
        println!("Rolled back successfully");
        println!("Backup at: {}", backup.display());
    }

    Ok(())
}

fn cmd_sync_log(n: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let entries = history::get_sync_log(&sync_path, n)?;

    if json {
        let items: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "timestamp": e.timestamp,
                    "operation": e.operation,
                    "summary": e.summary,
                })
            })
            .collect();
        println!("{}", json!({"version": 1, "entries": items}));
    } else if entries.is_empty() {
        println!("No sync operations logged yet");
    } else {
        for e in &entries {
            println!("  {} [{}] {}", e.timestamp, e.operation, e.summary);
        }
    }

    Ok(())
}

fn cmd_sync_machine(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let sync_path = get_sync_path()?;
    let info = machine::machine_info(&sync_path)?;

    if json {
        println!(
            "{}",
            json!({
                "version": 1,
                "machine_id": info.machine_id,
                "override_count": info.override_count,
            })
        );
    } else {
        println!("Machine ID: {}", info.machine_id);
        println!("Overrides: {}", info.override_count);
    }

    Ok(())
}
