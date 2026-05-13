use std::path::Path;
use std::process;

use crate::config::*;
use crate::model::*;
use crate::ops::*;
use crate::parser::*;

use super::{
    AiHookAction, BackupAction, Commands, LeaseAction, McpAction, ProjectAction, ProjectEnvAction,
    SessionAction, ShareAction, SnapshotAction,
};

/// Execute a CLI subcommand.
pub fn execute_command(command: &Commands, json: bool, dry_run: bool) {
    let result = match command {
        Commands::List {
            filter,
            group,
            sort,
            reverse,
            reveal,
        } => cmd_list(
            json,
            filter.as_deref(),
            group.as_deref(),
            sort.as_str(),
            *reverse,
            *reveal,
        ),
        Commands::Get { key } => cmd_get(key, json),
        Commands::Set { assignment } => cmd_set(assignment, dry_run),
        Commands::Delete { key } => cmd_delete(key, dry_run),
        Commands::Copy { key, key_only } => cmd_copy(key, *key_only),
        Commands::Move { key, new_key } => cmd_move(key, new_key.as_deref(), dry_run, json),
        Commands::Import { path, force } => cmd_import(path, *force, dry_run),
        Commands::Export {
            path,
            exclude_sensitive,
            safe,
            env_example,
            filter,
            format,
            k8s_name,
            k8s_namespace,
        } => {
            if *safe {
                cmd_export_safe(path.as_deref())
            } else if *env_example {
                cmd_export_env_example(path.as_deref())
            } else if let Some(fmt) = format {
                cmd_export_format(
                    path.as_deref(),
                    fmt,
                    filter.as_deref(),
                    k8s_name.as_deref(),
                    k8s_namespace.as_deref(),
                )
            } else {
                cmd_export(path.as_deref(), *exclude_sensitive, filter.as_deref())
            }
        }
        Commands::Git { action } => cmd_git(action),
        Commands::Duplicates => cmd_duplicates(json),
        Commands::Scan {
            path,
            staged,
            install_hook,
            remove_hook,
            mcp,
        } => {
            if *mcp {
                cmd_scan_mcp(json)
            } else if *install_hook {
                cmd_install_scan_hook()
            } else if *remove_hook {
                cmd_remove_scan_hook()
            } else {
                cmd_scan(path.as_deref(), *staged, json)
            }
        }
        Commands::Diff => cmd_diff(json),
        Commands::Backup { action } => cmd_backup(action, json),
        Commands::Profile { action } => cmd_profile(action, json),
        Commands::Validate {
            schema,
            env_file,
            environment,
            rules,
        } => cmd_validate_enhanced(
            schema.as_deref(),
            env_file.as_deref(),
            environment.as_deref(),
            json,
            rules,
        ),
        Commands::Encrypt { key } => cmd_encrypt(key, dry_run),
        Commands::Decrypt { key } => cmd_decrypt(key, dry_run),
        Commands::Completions { shell, install } => cmd_completions(shell, *install),
        Commands::Log { key, n } => cmd_log(key.as_deref(), *n, json),
        Commands::Config => cmd_config(json),
        Commands::Run {
            profile,
            profiles,
            resolve,
            env_files,
            overrides,
            command,
            volatile,
            redact,
            no_project,
        } => {
            if profile.is_some() && profiles.is_some() {
                eprintln!("Error: Use --profile OR --profiles, not both");
                process::exit(1);
            }
            let profile_list: Vec<String> = profiles
                .as_deref()
                .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            cmd_run(
                profile.as_deref(),
                *resolve,
                env_files,
                overrides,
                command,
                dry_run,
                json,
                *volatile,
                &profile_list,
                *redact,
                *no_project,
            )
        }
        Commands::Sync { action } => super::sync_cmd::execute_sync(action, json, dry_run),
        Commands::Secrets { action } => super::secrets_cmd::execute_secrets(action, json, dry_run),
        Commands::Analytics { action } => {
            super::analytics_cmd::execute_analytics(action, json, dry_run)
        }
        Commands::Docs { schema, output } => cmd_docs(schema.as_deref(), output.as_deref()),
        Commands::Drift {
            schema,
            environment,
            env_files,
        } => cmd_drift(schema.as_deref(), environment.as_deref(), env_files, json),
        Commands::Schema { action } => cmd_schema(action, json),
        Commands::Init { schema, output } => cmd_init_schema(schema.as_deref(), output),
        Commands::Explain { key } => cmd_explain(key, json),
        Commands::Rotate {
            key,
            dry_run: dr,
            stale,
            strategy: _,
            propagate,
        } => cmd_rotate(key, *dr || dry_run, *stale, *propagate),
        Commands::Hook { shell } => cmd_hook(shell),
        Commands::Env { dir } => crate::ops::hook::cmd_env(dir.as_deref()).map_err(|e| e.into()),
        Commands::EnvUnload { dir } => crate::ops::hook::cmd_env_unload(dir).map_err(|e| e.into()),
        Commands::Doctor {
            verbose,
            all,
            fail_on,
        } => cmd_doctor(*verbose, *all, fail_on.as_deref(), json),
        Commands::Check { only } => cmd_check(only.as_deref(), json),
        Commands::Snapshot { action } => cmd_snapshot(action, dry_run, json),
        Commands::Share { action } => cmd_share(action, dry_run),
        Commands::ResolveUri { file, env, output } => {
            cmd_resolve_uri(file, *env, output.as_deref(), json)
        }
        Commands::Mcp { action } => {
            let action = action.as_ref().unwrap_or(&crate::cli::McpAction::Status);
            cmd_mcp(action, dry_run, json)
        }
        Commands::Audit {
            key,
            since,
            machine,
            n,
            ai_leaks,
            access,
        } => cmd_audit(
            key.as_deref(),
            since.as_deref(),
            machine.as_deref(),
            *n,
            json,
            *ai_leaks,
            *access,
        ),
        Commands::AuditTrail { action } => super::audit_cmd::execute_audit_trail(action, json)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() }),
        Commands::Search {
            query,
            fuzzy,
            reveal,
        } => cmd_search(query, json, *fuzzy, *reveal),
        Commands::Fence { status } => {
            if *status {
                cmd_fence_status(json)
            } else {
                cmd_fence(dry_run)
            }
        }
        Commands::Sanitize { file, output } => cmd_sanitize(file, output.as_deref(), json, dry_run),
        Commands::AiHook { action } => cmd_ai_hook(action, dry_run, json),
        Commands::AiGuard {
            stage,
            tool_name,
            tool_input,
        } => cmd_ai_guard(
            stage.as_deref(),
            tool_name.as_deref(),
            tool_input.as_deref(),
            json,
        ),
        Commands::Proxy {
            port,
            keys,
            profile,
            allow_origins,
            require_lease,
            require_approval,
        } => cmd_proxy(
            *port,
            keys.as_deref(),
            profile.as_deref(),
            allow_origins.as_deref(),
            *require_lease,
            *require_approval,
        ),
        Commands::Lease { action } => cmd_lease(action, json),
        Commands::Canary { action } => cmd_canary(action, json),
        Commands::Hardening { action } => cmd_hardening(action),
        Commands::Scanner { action } => cmd_scanner(action),
        Commands::CiTrust { action } => cmd_ci_trust(action),
        Commands::Envbom { action } => cmd_envbom(action),
        Commands::Monitor { action } => cmd_monitor(action),
        Commands::Revoke { all, name } => cmd_revoke(*all, name.as_deref(), json),
        Commands::Deps { key, source } => cmd_deps(key, *source, json),
        Commands::Undo { list } => cmd_undo(*list, json),
        Commands::Offset { show, suggest } => cmd_offset(*show, *suggest, json),
        Commands::Man { command } => cmd_man(command),
        Commands::Lsp => {
            crate::lsp::run_lsp();
            return;
        }
        Commands::Session { action } => cmd_session(action, json),
        Commands::Lifecycle { action } => {
            match crate::cli::lifecycle_cmd::handle_lifecycle(action, json) {
                Ok(()) => Ok(()),
                Err(e) => Err(Box::<dyn std::error::Error>::from(e.to_string())),
            }
        }
        Commands::Project { action } => cmd_project(action, json, dry_run),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn load_context() -> Result<(AppConfig, Vec<ShellFile>), Box<dyn std::error::Error>> {
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

    Ok((config, shell_files))
}

fn cmd_list(
    json: bool,
    filter: Option<&str>,
    group: Option<&str>,
    sort: &str,
    reverse: bool,
    reveal: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (config, shell_files) = load_context()?;
    let mut entries = collect_all_entries(&shell_files);

    // Include shared file entries
    let shared_path = shellexpand_path(&config.profiles.shared_file);
    if shared_path.exists() {
        if let Ok(shared_sf) = parse_shell_file(&shared_path) {
            entries.extend(collect_entries(&shared_sf));
        }
    }

    // Include active profile entries
    if let Some(profile_file) = config.profiles.active_file() {
        let profile_path = shellexpand_path(&profile_file);
        if profile_path.exists() {
            if let Ok(profile_sf) = parse_shell_file(&profile_path) {
                entries.extend(collect_entries(&profile_sf));
            }
        }
    }

    if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        entries.retain(|e| e.key.to_lowercase().contains(&f_lower));
    }

    match sort {
        "value" => entries.sort_by(|a, b| a.value.cmp(&b.value)),
        "file" => entries.sort_by(|a, b| a.source_file.cmp(&b.source_file)),
        _ => entries.sort_by(|a, b| a.key.cmp(&b.key)),
    }
    if reverse {
        entries.reverse();
    }

    if group.is_some() {
        let config = crate::ops::grouping::GroupConfig::default();
        let groups = crate::ops::grouping::group_entries(&entries, &config);
        if json {
            let json_groups: Vec<serde_json::Value> = groups
                .iter()
                .map(|g| {
                    let group_entries: Vec<serde_json::Value> = g
                        .entries
                        .iter()
                        .map(|e| {
                            let value = if !reveal && is_sensitive(&e.key) {
                                mask_value(&e.value)
                            } else {
                                e.value.clone()
                            };
                            serde_json::json!({
                                "key": e.key,
                                "value": value,
                                "source_file": e.source_file.to_string_lossy(),
                                "line_number": e.line_number,
                            })
                        })
                        .collect();
                    serde_json::json!({
                        "group": g.name,
                        "is_user_defined": g.is_user_defined,
                        "entries": group_entries,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_groups)?);
        } else {
            for g in &groups {
                println!("╔══ {} ══", g.name);
                for e in &g.entries {
                    let value = if is_sensitive(&e.key) {
                        mask_value(&e.value)
                    } else {
                        e.value.clone()
                    };
                    println!("  {} = {}", e.key, value);
                }
                println!("╚══");
                println!();
            }
        }
    } else if json {
        print_entries_json(&entries, reveal)?;
    } else {
        print_entries_table(&entries);
    }
    Ok(())
}

fn cmd_search(
    query: &str,
    json: bool,
    fuzzy: bool,
    reveal: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if query.is_empty() {
        return Err("Search query cannot be empty".into());
    }

    let (config, shell_files) = load_context()?;
    let mut entries = collect_all_entries(&shell_files);

    // Include shared file entries
    let shared_path = shellexpand_path(&config.profiles.shared_file);
    if shared_path.exists() {
        if let Ok(shared_sf) = parse_shell_file(&shared_path) {
            entries.extend(collect_entries(&shared_sf));
        }
    }

    // Include active profile entries
    if let Some(profile_file) = config.profiles.active_file() {
        let profile_path = shellexpand_path(&profile_file);
        if profile_path.exists() {
            if let Ok(profile_sf) = parse_shell_file(&profile_path) {
                entries.extend(collect_entries(&profile_sf));
            }
        }
    }

    if fuzzy {
        let results = fuzzy_search(&entries, query);

        if json {
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    let value = if !reveal && is_sensitive(&r.entry.key) {
                        mask_value(&r.entry.value)
                    } else {
                        r.entry.value.clone()
                    };
                    serde_json::json!({
                        "version": 1,
                        "key": r.entry.key,
                        "value": value,
                        "source_file": r.entry.source_file.to_string_lossy(),
                        "line_number": r.entry.line_number,
                        "score": r.score,
                        "matched_indices": r.matched_indices,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results)?);
        } else if results.is_empty() {
            println!("No results found for '{}'.", query);
        } else {
            let max_key = results
                .iter()
                .map(|r| r.entry.key.len())
                .max()
                .unwrap_or(10)
                .min(30);

            println!("{:<width$} {:<50} SCORE", "KEY", "VALUE", width = max_key);
            println!("{}", "-".repeat(max_key + 55 + 8));

            for r in &results {
                let value = if is_sensitive(&r.entry.key) {
                    mask_value(&r.entry.value)
                } else if r.entry.value.len() > 50 {
                    format!("{}…", &r.entry.value[..49])
                } else {
                    r.entry.value.clone()
                };
                println!(
                    "{:<width$} {:<50} {}",
                    r.entry.key,
                    value,
                    r.score,
                    width = max_key
                );
            }
        }
    } else {
        let query_lower = query.to_lowercase();
        let results: Vec<_> = entries
            .iter()
            .filter(|e| {
                e.key.to_lowercase().contains(&query_lower)
                    || e.value.to_lowercase().contains(&query_lower)
            })
            .collect();

        if json {
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|e| {
                    let value = if !reveal && is_sensitive(&e.key) {
                        mask_value(&e.value)
                    } else {
                        e.value.clone()
                    };
                    serde_json::json!({
                        "version": 1,
                        "key": e.key,
                        "value": value,
                        "source_file": e.source_file.to_string_lossy(),
                        "line_number": e.line_number,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_results)?);
        } else if results.is_empty() {
            println!("No results found for '{}'.", query);
        } else {
            let max_key = results
                .iter()
                .map(|e| e.key.len())
                .max()
                .unwrap_or(10)
                .min(30);

            println!("{:<width$} VALUE", "KEY", width = max_key);
            println!("{}", "-".repeat(max_key + 6));

            for e in &results {
                let value = if is_sensitive(&e.key) {
                    mask_value(&e.value)
                } else if e.value.len() > 50 {
                    format!("{}…", &e.value[..49])
                } else {
                    e.value.clone()
                };
                println!("{:<width$} {}", e.key, value, width = max_key);
            }
        }
    }

    Ok(())
}

fn is_sensitive(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("credential")
        || (lower.contains("key") && !lower.contains("keyboard"))
}

fn mask_value(value: &str) -> String {
    if value.len() < 8 {
        return "****".to_string();
    }
    let first2 = &value[..3];
    let last2 = &value[value.len() - 3..];
    format!("{}***{}", first2, last2)
}

fn cmd_get(key: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented);

    match entry {
        Some(e) => {
            if json {
                let obj = serde_json::json!({
                    "key": e.key,
                    "value": e.value,
                    "source_file": e.source_file.to_string_lossy(),
                    "line_number": e.line_number,
                });
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                println!("{}", e.value);
            }
            Ok(())
        }
        None => {
            eprintln!("Key '{}' not found", key);
            process::exit(1);
        }
    }
}

fn cmd_set(assignment: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (key, value) = parse_assignment(assignment)?;
    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let sf = &mut shell_files[0];

    ensure_managed_zone(sf);

    match edit_entry(sf, &key, &value) {
        Ok(()) => {}
        Err(OpsError::KeyNotFound { .. }) => {
            if find_soft_deleted(sf, &key).is_some() {
                undo_delete(sf, &key)?;
                edit_entry(sf, &key, &value)?;
            } else {
                add_entry(
                    sf,
                    &key,
                    &value,
                    ExportStyle::Export,
                    QuoteStyle::Double,
                    config.offsets.header_protected_lines,
                    config.offsets.footer_protected_lines,
                )?;
            }
        }
        Err(e) => return Err(e.into()),
    }

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
        println!("Set {}={}", key, value);
        crate::ops::schema::auto_update_ai_context();
    }
    Ok(())
}

fn cmd_delete(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let sf = &mut shell_files[0];
    soft_delete(sf, key)?;

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
        println!("Deleted {}", key);
    }
    Ok(())
}

fn cmd_copy(key: &str, key_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    if key_only {
        crate::ops::copy_key(entry)?;
        println!("Copied key: {}", key);
    } else {
        copy_value(entry)?;
        println!("Copied value of {}", key);
    }
    Ok(())
}

fn cmd_move(
    key: &str,
    new_key: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(new_name) = new_key {
        return cmd_rename(key, new_name, dry_run, json);
    }

    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let ref_path = shellexpand(&config.files.reference);
    ensure_reference_file(&ref_path)?;

    // Ensure we have the reference file loaded
    if shell_files.len() < 2 {
        shell_files.push(parse_shell_file(&ref_path)?);
    }

    {
        let (first, rest) = shell_files.split_at_mut(1);
        let primary = &mut first[0];
        let ref_file = &mut rest[0];

        ensure_source_directive(
            primary,
            &ref_path,
            config.offsets.header_protected_lines,
            config.offsets.footer_protected_lines,
        )?;

        move_to_reference(primary, ref_file, key, &ref_path)?;
    }

    if dry_run {
        for sf in &shell_files {
            let diff = generate_diff_from_strings(
                &std::fs::read_to_string(&sf.path).unwrap_or_default(),
                &serialize_shell_file(sf),
                &sf.path.to_string_lossy(),
            );
            print!("{}", diff);
        }
    } else {
        for sf in &shell_files {
            let content = serialize_shell_file(sf);
            safe_write(&sf.path, &content, Some(sf.hash))?;
        }
        println!("Moved {} to {}", key, ref_path.display());
    }
    Ok(())
}

fn cmd_rename(
    old_key: &str,
    new_key: &str,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let sf = &mut shell_files[0];
    rename_entry(sf, old_key, new_key)?;

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }

    if json {
        let obj = serde_json::json!({
            "version": 1,
            "old_key": old_key,
            "new_key": new_key,
            "renamed": true,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("Renamed {} → {}", old_key, new_key);
    }

    Ok(())
}

fn cmd_import(path: &str, force: bool, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let dotenv_path = Path::new(path);
    let entries = parse_dotenv(dotenv_path)?;

    if entries.is_empty() {
        println!("No entries found in {}", path);
        return Ok(());
    }

    let sf = &mut shell_files[0];
    let result = import_entries(sf, &entries, &config, force);

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }

    if !dry_run {
        crate::ops::schema::auto_update_ai_context();
    }

    println!(
        "Import complete: {} added, {} skipped, {} overwritten",
        result.added, result.skipped, result.overwritten
    );
    Ok(())
}

fn cmd_export(
    path: Option<&str>,
    exclude_sensitive: bool,
    filter_query: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let output = export_entries(&entries, exclude_sensitive, filter_query);

    match path {
        Some(p) => {
            let out_path = Path::new(p);
            if out_path.exists() {
                eprint!("File '{}' exists. Overwrite? [y/N]: ", p);
                use std::io::{self, BufRead};
                let mut input = String::new();
                io::stdin().lock().read_line(&mut input)?;
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("Export cancelled.");
                    return Ok(());
                }
            }
            std::fs::write(out_path, &output)?;
            println!("Exported {} entries to {}", entries.len(), p);
        }
        None => {
            print!("{}", output);
        }
    }
    Ok(())
}

fn cmd_export_format(
    path: Option<&str>,
    format_name: &str,
    filter_query: Option<&str>,
    k8s_name: Option<&str>,
    k8s_namespace: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::export_format::{export_as, ExportFormat};

    let format =
        ExportFormat::parse(format_name).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let filtered = if let Some(query) = filter_query {
        filter_entries(&entries, query)
    } else {
        entries.to_vec()
    };

    let output = export_as(&filtered, &format, k8s_name, k8s_namespace);

    match path {
        Some(p) => {
            std::fs::write(p, &output)?;
            println!(
                "Exported {} entries to {} ({})",
                filtered
                    .iter()
                    .filter(|e| e.location != EntryLocation::Commented)
                    .count(),
                p,
                format_name
            );
        }
        None => {
            print!("{}", output);
        }
    }
    Ok(())
}

fn cmd_export_safe(path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::dotenv::export_safe;
    use crate::ops::schema::find_schema;

    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    // Collect schema sensitive keys if .env.schema exists
    let mut schema_sensitive = std::collections::HashSet::new();
    if let Some(sf) = find_schema() {
        if let Ok(schema) = crate::ops::schema::parse_schema(&sf) {
            for (name, var) in &schema.variables {
                if var.sensitive {
                    schema_sensitive.insert(name.clone());
                }
            }
        }
    }

    let output = export_safe(&entries, &schema_sensitive);

    if let Some(p) = path {
        std::fs::write(p, &output)?;
        println!("Safe export written to {}", p);
    } else {
        print!("{}", output);
    }
    Ok(())
}

fn cmd_export_env_example(path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::dotenv::export_env_example;
    use crate::ops::schema::{find_schema, parse_schema};

    let sf = find_schema().ok_or(
        "No .env.schema found. Create one first: envforge schema generate --output .env.schema",
    )?;
    let schema = parse_schema(&sf)?;
    let output = export_env_example(&schema);

    if let Some(p) = path {
        std::fs::write(p, &output)?;
        println!(".env.example written to {}", p);
    } else {
        print!("{}", output);
    }
    Ok(())
}

fn cmd_git(action: &super::GitAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        super::GitAction::InstallMergeDriver => {
            // 1. Add merge driver to ~/.gitconfig
            let home = dirs::home_dir().ok_or("Cannot find home directory")?;
            let gitconfig = home.join(".gitconfig");

            let driver_cmd = "envforge git merge %O %A %B";
            std::process::Command::new("git")
                .args([
                    "config",
                    "--global",
                    "merge.envforge.name",
                    "EnvForge .env merge driver",
                ])
                .output()?;
            std::process::Command::new("git")
                .args(["config", "--global", "merge.envforge.driver", driver_cmd])
                .output()?;

            println!("✓ Merge driver registered in {}", gitconfig.display());

            // 2. Add .gitattributes entry
            let gitattributes = std::path::Path::new(".gitattributes");
            let entry = "*.env merge=envforge\n";

            if gitattributes.exists() {
                let content = std::fs::read_to_string(gitattributes)?;
                if !content.contains("merge=envforge") {
                    std::fs::write(gitattributes, format!("{}{}", content, entry))?;
                    println!("✓ .gitattributes updated");
                } else {
                    println!("✓ .gitattributes already configured");
                }
            } else {
                std::fs::write(gitattributes, entry)?;
                println!("✓ .gitattributes created");
            }

            println!("\nEnvForge will now handle .env merges automatically.");
        }

        super::GitAction::RemoveMergeDriver => {
            // Remove from gitconfig
            let _ = std::process::Command::new("git")
                .args(["config", "--global", "--remove-section", "merge.envforge"])
                .output();
            println!("✓ Merge driver removed from ~/.gitconfig");

            // Remove from .gitattributes
            let gitattributes = std::path::Path::new(".gitattributes");
            if gitattributes.exists() {
                let content = std::fs::read_to_string(gitattributes)?;
                let filtered: String = content
                    .lines()
                    .filter(|l| !l.contains("merge=envforge"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if filtered.trim().is_empty() {
                    std::fs::remove_file(gitattributes)?;
                    println!("✓ .gitattributes removed (was empty)");
                } else {
                    std::fs::write(gitattributes, filtered + "\n")?;
                    println!("✓ .gitattributes entry removed");
                }
            }
        }

        super::GitAction::Merge { base, ours, theirs } => {
            // Three-way merge for .env files
            let exit_code = merge_env_files(base, ours, theirs)?;
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
    }
    Ok(())
}

/// Three-way merge for .env files. Returns 0 for clean merge, 1 for conflicts.
fn merge_env_files(
    base_path: &str,
    ours_path: &str,
    theirs_path: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    let base = parse_env_file(base_path);
    let ours = parse_env_file(ours_path);
    let theirs = parse_env_file(theirs_path);

    // Collect all keys
    let mut all_keys: Vec<String> = Vec::new();
    for key in base.keys().chain(ours.keys()).chain(theirs.keys()) {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }
    all_keys.sort();

    let mut result = String::new();
    let mut has_conflicts = false;

    for key in &all_keys {
        let b = base.get(key);
        let o = ours.get(key);
        let t = theirs.get(key);

        match (b, o, t) {
            // Both sides same → no conflict
            (_, Some(ov), Some(tv)) if ov == tv => {
                result.push_str(&format!("{}={}\n", key, ov));
            }
            // Only ours changed (theirs == base or absent)
            (bv, Some(ov), tv) if tv == bv || tv.is_none() => {
                result.push_str(&format!("{}={}\n", key, ov));
            }
            // Only theirs changed (ours == base or absent)
            (bv, ov, Some(tv)) if ov == bv || ov.is_none() => {
                result.push_str(&format!("{}={}\n", key, tv));
            }
            // Both changed differently → real conflict
            (_, Some(ov), Some(tv)) => {
                has_conflicts = true;
                result.push_str(&format!(
                    "<<<<<<< ours\n{}={}\n=======\n{}={}\n>>>>>>> theirs\n",
                    key, ov, key, tv
                ));
            }
            // Deleted in one, changed in other → conflict
            (Some(_), Some(ov), None) => {
                // Ours kept it, theirs deleted it
                result.push_str(&format!("{}={}\n", key, ov));
            }
            (Some(_), None, Some(tv)) => {
                // Theirs kept it, ours deleted it
                result.push_str(&format!("{}={}\n", key, tv));
            }
            // Both deleted or key only in base → skip
            (_, None, None) => {}
            // Key not in base, only in one side
            (None, Some(ov), None) => {
                result.push_str(&format!("{}={}\n", key, ov));
            }
            (None, None, Some(tv)) => {
                result.push_str(&format!("{}={}\n", key, tv));
            }
        }
    }

    // Write merged result to ours file (git convention)
    std::fs::write(ours_path, result)?;

    Ok(if has_conflicts { 1 } else { 0 })
}

fn parse_env_file(path: &str) -> std::collections::HashMap<String, String> {
    crate::ops::dotenv::parse_dotenv(std::path::Path::new(path))
        .unwrap_or_default()
        .into_iter()
        .map(|e| (e.key, e.value))
        .collect()
}

fn cmd_duplicates(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let groups = detect_duplicates(&shell_files);

    if groups.is_empty() {
        println!("No duplicate keys found.");
        return Ok(());
    }

    if json {
        let json_groups: Vec<serde_json::Value> = groups
            .iter()
            .map(|g| {
                let entries: Vec<serde_json::Value> = g
                    .entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "value": e.value,
                            "source_file": e.source_file,
                            "line_number": e.line_number,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "key": g.key,
                    "count": g.entries.len(),
                    "entries": entries,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_groups)?);
    } else {
        println!("Duplicate keys found:\n");
        for group in &groups {
            println!("  {} ({} definitions):", group.key, group.entries.len());
            for entry in &group.entries {
                println!(
                    "    - {} line {} = {}",
                    entry.source_file,
                    entry.line_number,
                    if entry.value.len() > 40 {
                        format!("{}…", &entry.value[..39])
                    } else {
                        entry.value.clone()
                    }
                );
            }
            println!();
        }
        println!(
            "Total: {} duplicate keys. Use TUI to resolve interactively.",
            groups.len()
        );
    }
    Ok(())
}

fn cmd_scan(
    path: Option<&str>,
    staged: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);
    let sensitive = filter_sensitive(&entries);

    if sensitive.is_empty() {
        println!("No sensitive ENV values to scan for.");
        return Ok(());
    }

    let matches = if staged {
        scan_staged(&sensitive)?
    } else {
        let scan_path = Path::new(path.unwrap_or("."));
        scan_directory(scan_path, &sensitive)?
    };

    if matches.is_empty() {
        println!("No secrets found.");
        return Ok(());
    }

    if json {
        let json_matches: Vec<serde_json::Value> = matches
            .iter()
            .map(|m| {
                serde_json::json!({
                    "file": m.file.to_string_lossy(),
                    "line": m.line_number,
                    "key": m.matched_key,
                    "masked_value": m.matched_value,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_matches)?);
    } else {
        println!("Secrets found:\n");
        for m in &matches {
            println!(
                "  {}:{} — {} ({})",
                m.file.display(),
                m.line_number,
                m.matched_key,
                m.matched_value
            );
        }
        println!(
            "\n{} secret(s) found in {} location(s).",
            sensitive.len(),
            matches.len()
        );
    }

    process::exit(1);
}

fn cmd_scan_mcp(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::mcp_scan::{
        findings_to_json, scan_mcp_configs, scanned_file_count, suggestion_for,
    };

    let findings = scan_mcp_configs();
    let files_scanned = scanned_file_count();

    if json {
        let json_val = findings_to_json(&findings, files_scanned);
        println!("{}", serde_json::to_string_pretty(&json_val)?);
        if !findings.is_empty() {
            process::exit(1);
        }
        return Ok(());
    }

    println!("MCP Configuration Secret Scan\n");

    if findings.is_empty() {
        println!("{} file(s) scanned, 0 credential(s) found", files_scanned);
        return Ok(());
    }

    // Group findings by file
    let mut by_file: std::collections::BTreeMap<String, Vec<&crate::ops::mcp_scan::McpFinding>> =
        std::collections::BTreeMap::new();
    for f in &findings {
        by_file
            .entry(f.file.to_string_lossy().to_string())
            .or_default()
            .push(f);
    }

    for (file, file_findings) in &by_file {
        println!("{}", file);
        for f in file_findings {
            println!("  \x1b[33m!\x1b[0m {} = {}", f.path, f.value_preview);
            println!("    \x1b[36m-> {}\x1b[0m", suggestion_for(f));
        }
        println!();
    }

    println!(
        "{} file(s) scanned, {} credential(s) found",
        files_scanned,
        findings.len()
    );

    process::exit(1);
}

fn cmd_mcp(
    action: &McpAction,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cli::mcp_pin_cmd;
    match action {
        McpAction::Harden => cmd_mcp_harden(dry_run, json),
        McpAction::Status => cmd_mcp_status(json),
        McpAction::Pin {
            strict,
            inspect,
            lockfile,
            refresh,
            accept,
            yes,
            resolve_conflicts,
        } => mcp_pin_cmd::cmd_pin(
            *strict,
            *inspect,
            lockfile.as_ref(),
            *refresh,
            *accept,
            *yes,
            resolve_conflicts.as_deref(),
        ),
        McpAction::Verify {
            json,
            strict,
            lockfile,
        } => mcp_pin_cmd::cmd_verify(*json, *strict, lockfile.as_ref()),
        McpAction::Diff { server, lockfile } => {
            mcp_pin_cmd::cmd_diff(server.as_deref(), lockfile.as_ref())
        }
        McpAction::Trust { name, reason } => mcp_pin_cmd::cmd_trust(name, reason),
        McpAction::Untrust { name } => mcp_pin_cmd::cmd_untrust(name),
        McpAction::Explain {
            lock,
            format,
            lockfile,
        } => mcp_pin_cmd::cmd_explain(*lock, format, lockfile.as_ref()),
        McpAction::Launch {
            ide,
            lockfile,
            args,
        } => mcp_pin_cmd::cmd_launch(ide, args, lockfile.as_ref()),
    }
}

fn cmd_mcp_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::mcp_scan::scan_mcp_configs;

    let findings = scan_mcp_configs();

    if json {
        let items: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "file": f.file.to_string_lossy(),
                    "key": f.key,
                    "pattern": f.pattern,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "vulnerable_files": items.iter().map(|i| i["file"].as_str().unwrap()).collect::<std::collections::HashSet<_>>().len(),
                "total_findings": items.len(),
                "findings": items,
            }))?
        );
        return Ok(());
    }

    if findings.is_empty() {
        println!("No plaintext secrets found in MCP configs.");
    } else {
        println!(
            "Found {} potential secret(s) in MCP configs.",
            findings.len()
        );
        println!("Run `envforge mcp harden` to fix them.");
    }

    Ok(())
}

fn cmd_mcp_harden(dry_run: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::mcp_scan::harden_all_mcp_configs;

    let results = harden_all_mcp_configs(dry_run);

    if json {
        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|(path, count, keys, backup)| {
                serde_json::json!({
                    "file": path.to_string_lossy(),
                    "secrets_replaced": count,
                    "keys": keys,
                    "backup": backup.as_ref().map(|b| b.to_string_lossy().to_string()),
                    "dry_run": dry_run,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hardened_files": items.len(),
                "dry_run": dry_run,
                "results": items,
            }))?
        );
        return Ok(());
    }

    if dry_run {
        println!("MCP Config Hardening (dry run)\n");
    } else {
        println!("MCP Config Hardening\n");
    }

    if results.is_empty() {
        println!("No MCP config files with plaintext secrets found.");
        return Ok(());
    }

    let mut all_keys = Vec::new();
    for (path, count, keys, backup) in &results {
        if dry_run {
            println!("\x1b[33m~\x1b[0m {}", path.to_string_lossy());
            println!("  {} secret(s) would be replaced", count);
        } else {
            println!("\x1b[32m\u{2713}\x1b[0m {}", path.to_string_lossy());
            if let Some(bak) = backup {
                println!(
                    "  {} secret(s) replaced (backup: {})",
                    count,
                    bak.to_string_lossy()
                );
            } else {
                println!("  {} secret(s) replaced", count);
            }
        }
        for key in keys {
            println!("  \x1b[36m->\x1b[0m Set: export {}=<your-value>", key);
            all_keys.push(key.clone());
        }
        println!();
    }

    if dry_run {
        println!("Run without --dry-run to apply changes.");
    } else {
        println!("Set these environment variables before running AI tools.");
    }

    Ok(())
}

fn cmd_diff(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;

    let mut all_diffs = Vec::new();
    for sf in &shell_files {
        if let Ok(diff) = generate_diff(sf) {
            if !diff.is_empty() {
                all_diffs.push((sf.path.to_string_lossy().to_string(), diff));
            }
        }
    }

    if json {
        let json_diffs: Vec<serde_json::Value> = all_diffs
            .iter()
            .map(|(file, diff)| {
                serde_json::json!({
                    "file": file,
                    "diff": diff,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "has_changes": !all_diffs.is_empty(),
                "diffs": json_diffs,
            }))?
        );
    } else if all_diffs.is_empty() {
        println!("No changes.");
    } else {
        for (_, diff) in &all_diffs {
            print!("{}", diff);
        }
    }
    Ok(())
}

fn cmd_backup(action: &BackupAction, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BackupAction::Restore { file } => {
            let backup_path = Path::new(file);
            if !backup_path.exists() {
                return Err(format!("Backup file not found: {}", file).into());
            }

            let file_name = backup_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            let first_part = file_name.split('.').next();
            if first_part.is_none() {
                return Err("Cannot determine target file from backup name".into());
            }

            let content = std::fs::read_to_string(backup_path)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "restored_from": file,
                        "content_length": content.len(),
                    }))?
                );
            } else {
                println!("Restored from {}", file);
                println!("Content length: {} bytes", content.len());
            }
            Ok(())
        }
        BackupAction::List => {
            let config = load_or_create_default()?;
            let primary = shellexpand(&config.files.primary);
            let backups = list_backups(&primary)?;

            if json {
                let json_backups: Vec<serde_json::Value> = backups
                    .iter()
                    .map(|b| {
                        serde_json::json!({
                            "path": b.to_string_lossy(),
                            "size": std::fs::metadata(b).map(|m| m.len()).unwrap_or(0),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "count": json_backups.len(),
                        "backups": json_backups,
                    }))?
                );
            } else if backups.is_empty() {
                println!("No backups found.");
            } else {
                for b in &backups {
                    println!("{}", b.display());
                }
            }
            Ok(())
        }
    }
}

fn cmd_profile(
    action: &super::ProfileAction,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_or_create_default()?;

    match action {
        super::ProfileAction::List => {
            let names = config.profiles.profile_names();
            if json {
                let json_profiles: Vec<serde_json::Value> = names
                    .iter()
                    .map(|name| {
                        let file = config
                            .profiles
                            .entries
                            .get(name)
                            .map(|e| e.file.as_str())
                            .unwrap_or("?");
                        serde_json::json!({
                            "version": 1,
                            "name": name,
                            "file": file,
                            "active": *name == config.profiles.active,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json_profiles)?);
            } else if names.is_empty() {
                println!("No profiles defined.");
            } else {
                println!("Profiles:\n");
                for name in &names {
                    let active = if *name == config.profiles.active {
                        " ← active"
                    } else {
                        ""
                    };
                    let file = config
                        .profiles
                        .entries
                        .get(name)
                        .map(|e| e.file.as_str())
                        .unwrap_or("?");
                    println!(" {} ({}){}", name, file, active);
                }
                println!("\nShared: {}", config.profiles.shared_file);
            }
        }
        super::ProfileAction::Switch { name } => {
            let (_, mut shell_files) = load_context()?;
            if let Some(sf) = shell_files.first_mut() {
                switch_profile(&mut config, sf, name)?;
                let content = serialize_shell_file(sf);
                safe_write(&sf.path, &content, Some(sf.hash))?;
            }
            println!("Switched to profile: {}", name);
        }
        super::ProfileAction::Create { name } => {
            let path = create_profile(&mut config, name)?;
            println!("Created profile '{}' at {}", name, path.display());
        }
        super::ProfileAction::Delete { name } => {
            delete_profile(&mut config, name, false)?;
            println!("Deleted profile '{}' (file kept)", name);
        }
        super::ProfileAction::Diff { a, b } => {
            cmd_profile_diff(&config, a, b)?;
        }
    }
    Ok(())
}

fn cmd_encrypt(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    if is_encrypted(&entry.value) {
        println!("{} is already encrypted.", key);
        return Ok(());
    }

    let encrypted = encrypt_value(&entry.value)?;

    if dry_run {
        println!(
            "{} would be encrypted to: {}...{}",
            key,
            &encrypted[..20],
            ENC_SUFFIX
        );
        return Ok(());
    }

    // Find and update in shell file
    let source = entry.source_file.clone();
    if let Some(sf) = shell_files.iter_mut().find(|sf| sf.path == source) {
        edit_entry(sf, key, &encrypted)?;
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }
    println!("Encrypted: {}", key);
    Ok(())
}

fn cmd_decrypt(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    if !is_encrypted(&entry.value) {
        println!("{} is not encrypted.", key);
        return Ok(());
    }

    let decrypted = decrypt_value(&entry.value)?;

    if dry_run {
        println!("{} = {}", key, decrypted);
        return Ok(());
    }

    // Find and update in shell file
    let source = entry.source_file.clone();
    if let Some(sf) = shell_files.iter_mut().find(|sf| sf.path == source) {
        edit_entry(sf, key, &decrypted)?;
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }
    println!("Decrypted: {}", key);
    Ok(())
}

// Need ENC_SUFFIX constant accessible
const ENC_SUFFIX: &str = "]";

fn cmd_validate(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    if config.validation.is_empty() {
        println!("No validation rules defined. Add rules to [validation] in config.toml.");
        return Ok(());
    }

    let errors = validate_entries(&entries, &config.validation);

    if errors.is_empty() {
        println!("All validations passed.");
        return Ok(());
    }

    if json {
        let json_errors: Vec<serde_json::Value> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "key": e.key,
                    "value": e.value,
                    "rule": e.rule,
                    "error": e.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_errors)?);
    } else {
        println!("Validation errors:\n");
        for e in &errors {
            println!("  ⚠ {} ({}): {}", e.key, e.rule, e.message);
        }
        println!("\n{} error(s) found.", errors.len());
    }
    Ok(())
}

fn cmd_log(key: Option<&str>, n: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = read_changelog(key, n)?;

    if entries.is_empty() {
        println!("No changes recorded.");
        return Ok(());
    }

    if json {
        let json_entries: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "profile": e.profile,
                    "action": e.action,
                    "key": e.key,
                    "detail": e.detail,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_entries)?);
    } else {
        for e in &entries {
            println!(
                "  {} [{}] {} {} {}",
                e.timestamp, e.profile, e.action, e.key, e.detail
            );
        }
    }
    Ok(())
}

fn cmd_completions(shell: &str, install: bool) -> Result<(), Box<dyn std::error::Error>> {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    // Kiro CLI / Fig / Amazon Q completion spec
    if shell == "fig" || shell == "kiro" {
        use clap_complete_fig::Fig;
        let mut cmd = super::Cli::command();

        if install {
            let mut buf = Vec::new();
            generate(Fig, &mut cmd, "envforge", &mut buf);
            let spec = String::from_utf8(buf)?;
            install_fig_spec(&spec, shell)?;
        } else {
            generate(Fig, &mut cmd, "envforge", &mut std::io::stdout());
        }
        return Ok(());
    }

    let shell_type = shell.parse::<Shell>().map_err(|_| {
        format!(
            "Unknown shell '{}'. Supported: bash, zsh, fish, elvish, powershell, fig, kiro",
            shell
        )
    })?;

    if install {
        let mut buf = Vec::new();
        generate(shell_type, &mut super::Cli::command(), "envforge", &mut buf);
        let script = String::from_utf8(buf)?;
        install_shell_completion(&script, shell)?;
    } else {
        let mut cmd = super::Cli::command();
        generate(shell_type, &mut cmd, "envforge", &mut std::io::stdout());
    }
    Ok(())
}

fn install_fig_spec(spec: &str, shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set")?;

    // Strip TypeScript type annotations for plain JS compatibility
    let js_spec = spec.replace("const completion: Fig.Spec = {", "const completion = {");

    let specs_dir = match shell {
        "kiro" => {
            let dir = std::path::PathBuf::from(&home).join(".kiro/specs");
            std::fs::create_dir_all(&dir)?;
            dir
        }
        "fig" => {
            let dir = std::path::PathBuf::from(&home).join(".fig/autocomplete/build");
            std::fs::create_dir_all(&dir)?;
            dir
        }
        _ => unreachable!(),
    };

    let spec_path = specs_dir.join("envforge.js");
    std::fs::write(&spec_path, &js_spec)?;
    eprintln!("Installed: {}", spec_path.display());

    if shell == "kiro" {
        // Configure devCompletionsFolder and developerMode
        for (key, value) in [
            (
                "autocomplete.devCompletionsFolder",
                specs_dir.to_str().unwrap_or(""),
            ),
            ("autocomplete.developerMode", "true"),
        ] {
            let status = std::process::Command::new("kiro-cli")
                .args(["settings", key, value])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            match status {
                Ok(s) if s.success() => {
                    eprintln!("Configured: kiro-cli {} → {}", key, value);
                }
                _ => {
                    eprintln!(
                        "Note: Run manually: kiro-cli settings {} \"{}\"",
                        key, value
                    );
                }
            }
        }

        // Also install to ~/.fig/autocomplete/build/ for backward compatibility
        let fig_dir = std::path::PathBuf::from(&home).join(".fig/autocomplete/build");
        if fig_dir.exists() {
            let fig_path = fig_dir.join("envforge.js");
            std::fs::write(&fig_path, &js_spec)?;
            eprintln!("Installed: {}", fig_path.display());
        }

        eprintln!("Done. Run 'kiro-cli restart' then open a new terminal.");
    } else {
        eprintln!("Done. Restart your terminal for completions to take effect.");
    }
    Ok(())
}

fn install_shell_completion(script: &str, shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set")?;

    let dest = match shell {
        "zsh" => {
            let dir = std::path::PathBuf::from(&home).join(".zfunc");
            std::fs::create_dir_all(&dir)?;
            dir.join("_envforge")
        }
        "bash" => {
            let dir =
                std::path::PathBuf::from(&home).join(".local/share/bash-completion/completions");
            std::fs::create_dir_all(&dir)?;
            dir.join("envforge")
        }
        "fish" => {
            let dir = std::path::PathBuf::from(&home).join(".config/fish/completions");
            std::fs::create_dir_all(&dir)?;
            dir.join("envforge.fish")
        }
        _ => {
            eprintln!(
                "--install not supported for '{}'. Pipe output to a file instead.",
                shell
            );
            return Ok(());
        }
    };

    std::fs::write(&dest, script)?;
    eprintln!("Installed: {}", dest.display());

    match shell {
        "zsh" => eprintln!("Ensure ~/.zfunc is in your fpath: fpath=(~/.zfunc $fpath)"),
        "bash" => eprintln!("Source it: source {}", dest.display()),
        "fish" => eprintln!("Fish will auto-load from {}", dest.display()),
        _ => {}
    }

    Ok(())
}

fn cmd_config(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    if json {
        let json_val = serde_json::json!({
            "version": 1,
            "files": {
                "primary": config.files.primary,
                "reference": config.files.reference,
                "use_reference_file": config.files.use_reference_file,
            },
            "profiles": {
                "active": config.profiles.active,
                "names": config.profiles.profile_names(),
            },
            "offsets": {
                "header_protected_lines": config.offsets.header_protected_lines,
                "footer_protected_lines": config.offsets.footer_protected_lines,
            },
        });
        println!("{}", serde_json::to_string_pretty(&json_val)?);
    } else {
        let toml_str = toml::to_string_pretty(&config)?;
        println!("{}", toml_str);
    }
    Ok(())
}

fn cmd_explain(key: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::explain::{explain_key, explanation_to_json, format_explanation};

    let explanation = explain_key(key);

    if json {
        let json_val = explanation_to_json(&explanation);
        println!("{}", serde_json::to_string_pretty(&json_val)?);
    } else {
        print!("{}", format_explanation(&explanation));
    }

    if !explanation.found {
        process::exit(1);
    }

    Ok(())
}

// ── Shell auto-load hook ────────────────────────────────────

fn cmd_hook(shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::hook::generate_hook;

    match generate_hook(shell) {
        Ok(script) => {
            print!("{}", script);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

// ── Pre-commit hook ─────────────────────────────────────────

const HOOK_MARKER: &str = "# EnvForge secret scan";
const HOOK_COMMAND: &str = "envforge scan --staged";

/// Walk up from cwd to find the `.git` directory.
fn find_git_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::env::current_dir()?;
    loop {
        let git = dir.join(".git");
        if git.is_dir() {
            return Ok(git);
        }
        if !dir.pop() {
            return Err("Not a git repository".into());
        }
    }
}

fn cmd_install_scan_hook() -> Result<(), Box<dyn std::error::Error>> {
    let git_dir = find_git_dir()?;
    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join("pre-commit");

    let envforge_block = format!("{}\n{}\n", HOOK_MARKER, HOOK_COMMAND);

    if hook_path.exists() {
        let content = std::fs::read_to_string(&hook_path)?;
        if content.contains(HOOK_COMMAND) {
            println!("Hook already installed.");
            return Ok(());
        }
        // Append our block to the existing hook
        let new_content = format!("{}\n{}", content.trim_end(), envforge_block);
        std::fs::write(&hook_path, new_content)?;
    } else {
        let content = format!("#!/bin/sh\n{}", envforge_block);
        std::fs::write(&hook_path, content)?;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("✓ Pre-commit hook installed at .git/hooks/pre-commit");
    Ok(())
}

fn cmd_remove_scan_hook() -> Result<(), Box<dyn std::error::Error>> {
    let git_dir = find_git_dir()?;
    let hook_path = git_dir.join("hooks").join("pre-commit");

    if !hook_path.exists() {
        println!("No pre-commit hook found.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&hook_path)?;
    if !content.contains(HOOK_MARKER) && !content.contains(HOOK_COMMAND) {
        println!("No EnvForge hook found in pre-commit.");
        return Ok(());
    }

    // Remove our lines (marker comment + command)
    let filtered: String = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed != HOOK_MARKER && trimmed != HOOK_COMMAND
        })
        .collect::<Vec<_>>()
        .join("\n");

    let trimmed = filtered.trim();

    // If only shebang (or empty) remains, delete the file
    if trimmed.is_empty() || trimmed == "#!/bin/sh" || trimmed == "#!/bin/bash" {
        std::fs::remove_file(&hook_path)?;
    } else {
        std::fs::write(&hook_path, format!("{}\n", trimmed))?;
    }

    println!("✓ Pre-commit hook removed");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────

fn parse_assignment(s: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let eq_pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid assignment '{}': expected KEY=VALUE", s))?;

    let key = s[..eq_pos].to_string();
    let mut value = s[eq_pos + 1..].to_string();

    // Strip surrounding quotes if present
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value = value[1..value.len() - 1].to_string();
    }

    if key.is_empty() {
        return Err("Key cannot be empty".into());
    }

    Ok((key, value))
}

fn print_entries_table(entries: &[EnvEntry]) {
    if entries.is_empty() {
        println!("No environment variables found.");
        return;
    }

    let max_key = entries
        .iter()
        .map(|e| e.key.len())
        .max()
        .unwrap_or(10)
        .min(30);

    println!(
        "{:<width$}  {:<50}  LOCATION",
        "KEY",
        "VALUE",
        width = max_key
    );
    println!("{}", "-".repeat(max_key + 55 + 15));

    for entry in entries {
        let value = if is_sensitive(&entry.key) {
            mask_value(&entry.value)
        } else if entry.value.len() > 50 {
            format!("{}…", &entry.value[..49])
        } else {
            entry.value.clone()
        };

        let location = entry
            .source_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let prefix = match entry.location {
            EntryLocation::Commented => "# ",
            _ => "",
        };

        println!(
            "{}{:<width$}  {:<50}  {}",
            prefix,
            entry.key,
            value,
            location,
            width = max_key
        );
    }
}

fn print_entries_json(
    entries: &[EnvEntry],
    reveal: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let json_entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let value = if !reveal && is_sensitive(&e.key) {
                mask_value(&e.value)
            } else {
                e.value.clone()
            };
            serde_json::json!({
                "key": e.key,
                "value": value,
                "source_file": e.source_file.to_string_lossy(),
                "line_number": e.line_number,
                "location": match e.location {
                    EntryLocation::InFile => "in_file",
                    EntryLocation::InReference => "in_reference",
                    EntryLocation::Commented => "commented",
                },
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&json_entries)?);
    Ok(())
}

fn cmd_validate_enhanced(
    schema_path: Option<&str>,
    env_file: Option<&str>,
    environment: Option<&str>,
    json: bool,
    rules: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::{find_schema, parse_schema, validate_against_schema};

    let config = load_or_create_default()?;

    let env: std::collections::HashMap<String, String> = if let Some(env_path) = env_file {
        let entries = crate::ops::dotenv::parse_dotenv(std::path::Path::new(env_path))?;
        entries.into_iter().map(|e| (e.key, e.value)).collect()
    } else {
        let (_, shell_files) = load_context()?;
        let entries = collect_all_entries(&shell_files);
        entries
            .into_iter()
            .filter(|e| e.location != EntryLocation::Commented)
            .map(|e| (e.key, e.value))
            .collect()
    };

    // Parse --rules KEY=rule pairs
    let mut extra_rules: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for rule in rules {
        if let Some((key, validator)) = rule.split_once('=') {
            extra_rules.insert(key.to_string(), validator.to_string());
        }
    }

    let mut all_errors = Vec::new();

    // Run value-rule validation if --rules provided
    if !extra_rules.is_empty() {
        let env_entries: Vec<crate::ops::EnvEntry> = env
            .iter()
            .map(|(k, v)| crate::ops::EnvEntry {
                key: k.clone(),
                value: v.clone(),
                source_file: std::path::PathBuf::new(),
                line_number: 0,
                line_index: 0,
                location: EntryLocation::InFile,
                export_style: ExportStyle::Export,
                quote_style: QuoteStyle::Double,
                is_dirty: false,
            })
            .collect();
        let rule_errors = crate::ops::validation::validate_entries(&env_entries, &extra_rules);
        for e in &rule_errors {
            all_errors.push(serde_json::json!({
                "key": e.key,
                "message": e.message,
                "rule": e.rule,
                "source": "rules",
            }));
        }
        if !json {
            for e in &rule_errors {
                println!(
                    "\x1b[31m✗\x1b[0m {:<30} — {} (rule: {})",
                    e.key, e.message, e.rule
                );
            }
        }
    }

    // Try to load schema
    let schema_file = schema_path
        .map(std::path::PathBuf::from)
        .or_else(find_schema);

    if let Some(sf) = schema_file {
        let schema = parse_schema(&sf)?;
        let errors = validate_against_schema(&env, &schema, environment, &config.validation);

        if json {
            let items: Vec<serde_json::Value> = errors
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "key": e.key,
                        "message": e.message,
                        "expected": e.expected,
                        "actual": e.actual,
                        "source": "schema",
                    })
                })
                .collect();
            all_errors.extend(items);
        } else if errors.is_empty() {
            println!(
                "All variables valid ({} checked against schema).",
                env.len()
            );
        } else {
            for e in &errors {
                println!("\x1b[31m✗\x1b[0m {:<30} — {}", e.key, e.message);
            }
        }

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"errors": all_errors, "valid": all_errors.is_empty()})
                )?
            );
        } else if !errors.is_empty() {
            println!("\n{} error(s) found.", errors.len());
            std::process::exit(1);
        }
    } else if !extra_rules.is_empty() {
        // Only had --rules, no schema
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"errors": all_errors, "valid": all_errors.is_empty()})
                )?
            );
        }
        if !all_errors.is_empty() {
            println!("\n{} error(s) found.", all_errors.len());
            std::process::exit(1);
        }
    } else {
        return cmd_validate(json);
    }

    Ok(())
}

fn cmd_docs(
    schema_path: Option<&str>,
    output_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::{find_schema, generate_docs, parse_schema};

    let sf = schema_path
        .map(std::path::PathBuf::from)
        .or_else(find_schema)
        .ok_or("No .env.schema found. Specify --schema or create .env.schema in project root.")?;

    let schema = parse_schema(&sf)?;
    let docs = generate_docs(&schema);

    if let Some(out) = output_path {
        std::fs::write(out, &docs)?;
        println!("Documentation written to {}", out);
    } else {
        print!("{}", docs);
    }

    Ok(())
}

fn cmd_drift(
    schema_path: Option<&str>,
    _environment: Option<&str>,
    env_files: &[String],
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::{detect_drift, find_schema, parse_schema, DriftStatus};

    let schema = schema_path
        .map(std::path::PathBuf::from)
        .or_else(find_schema)
        .and_then(|p| parse_schema(&p).ok());

    let mut envs: Vec<(String, std::collections::HashMap<String, String>)> = Vec::new();
    for path in env_files {
        let entries = crate::ops::dotenv::parse_dotenv(std::path::Path::new(path))?;
        let map = entries.into_iter().map(|e| (e.key, e.value)).collect();
        // Extract env name from filename: .env.production -> production
        let name = std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        envs.push((name, map));
    }

    let drift = detect_drift(&envs, schema.as_ref());

    if json {
        let items: Vec<serde_json::Value> = drift
            .iter()
            .map(|d| {
                serde_json::json!({
                    "key": d.key,
                    "status": match d.status {
                        DriftStatus::Same => "same",
                        DriftStatus::Differs => "differs",
                        DriftStatus::Missing => "missing",
                    },
                    "values": d.values,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "drift": items,
            }))?
        );
        return Ok(());
    }

    // Matrix display
    let env_names: Vec<&str> = envs.iter().map(|(n, _)| n.as_str()).collect();

    // Header
    print!("{:<30}", "Variable");
    for name in &env_names {
        print!(" {:<20}", name);
    }
    println!();
    print!("{}", "-".repeat(30));
    for _ in &env_names {
        print!(" {}", "-".repeat(20));
    }
    println!();

    let mut differ_count = 0;
    let mut missing_count = 0;

    for entry in &drift {
        if entry.status == DriftStatus::Same {
            continue; // Only show differences
        }
        let color = match entry.status {
            DriftStatus::Differs => "\x1b[33m",
            DriftStatus::Missing => "\x1b[31m",
            DriftStatus::Same => "",
        };
        print!("{}{:<30}\x1b[0m", color, entry.key);
        for name in &env_names {
            let val = entry
                .values
                .get(*name)
                .and_then(|v| v.as_deref())
                .unwrap_or("(missing)");
            let display = if val.len() > 18 {
                format!("{}...", &val[..15])
            } else {
                val.to_string()
            };
            let cell_color = if val == "(missing)" { "\x1b[31m" } else { "" };
            print!(" {}{:<20}\x1b[0m", cell_color, display);
        }
        println!();
        match entry.status {
            DriftStatus::Differs => differ_count += 1,
            DriftStatus::Missing => missing_count += 1,
            _ => {}
        }
    }

    let same_count = drift
        .iter()
        .filter(|d| d.status == DriftStatus::Same)
        .count();
    println!(
        "\n{} same, {} differ, {} missing across {} environments",
        same_count,
        differ_count,
        missing_count,
        env_names.len()
    );

    Ok(())
}

fn cmd_schema(action: &super::SchemaAction, _json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::generate_schema;

    match action {
        super::SchemaAction::Generate { output } => {
            let (_, shell_files) = load_context()?;
            let entries = collect_all_entries(&shell_files);
            let env: std::collections::HashMap<String, String> = entries
                .into_iter()
                .filter(|e| e.location != EntryLocation::Commented)
                .map(|e| (e.key, e.value))
                .collect();

            let schema = generate_schema(&env);

            if let Some(out) = output {
                std::fs::write(out, &schema)?;
                println!("Schema written to {} ({} variables)", out, env.len());
            } else {
                print!("{}", schema);
            }
        }
        super::SchemaAction::JsonSchema => {
            let schema = crate::ops::schema_json::generate_json_schema();
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        super::SchemaAction::EmitAi { output, infer } => {
            use crate::ops::schema::{emit_ai_context, find_schema, parse_schema};

            let schema = find_schema().and_then(|p| parse_schema(&p).ok());

            let entries: Vec<(String, String)> = if *infer || schema.is_none() {
                let (_, shell_files) = load_context()?;
                let all = collect_all_entries(&shell_files);
                all.into_iter()
                    .filter(|e| e.location != EntryLocation::Commented)
                    .map(|e| (e.key, e.value))
                    .collect()
            } else {
                vec![]
            };

            let content = emit_ai_context(schema.as_ref(), &entries);

            if let Some(path) = output {
                std::fs::write(path, &content)?;
                println!("AI context written to {}", path);
            } else {
                print!("{}", content);
            }
        }
    }
    Ok(())
}

fn cmd_init_schema(
    schema_path: Option<&str>,
    output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::{find_schema, parse_schema};
    use std::io::{self, BufRead, Write};

    let sf = schema_path
        .map(std::path::PathBuf::from)
        .or_else(find_schema)
        .ok_or("No .env.schema found. Specify --schema or create .env.schema in project root.")?;

    let schema = parse_schema(&sf)?;
    let output_path = std::path::Path::new(output);

    if output_path.exists() {
        eprint!("{} already exists. Overwrite? [y/N] ", output);
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let stdin = io::stdin();
    let mut result = Vec::new();

    // Sort: required first, then optional
    let mut vars: Vec<(&String, &crate::ops::schema::SchemaVariable)> =
        schema.variables.iter().collect();
    vars.sort_by(|a, b| b.1.required.cmp(&a.1.required).then(a.0.cmp(b.0)));

    for (name, var) in &vars {
        let req = if var.required { " (required)" } else { "" };
        let type_hint = var.var_type.display();

        if let Some(ref desc) = var.description {
            eprintln!("\n\x1b[36m{}\x1b[0m — {}", name, desc);
        } else {
            eprintln!("\n\x1b[36m{}\x1b[0m", name);
        }

        let default_hint = var
            .default
            .as_deref()
            .map(|d| format!(" [default: {}]", d))
            .unwrap_or_default();

        eprint!("  {} {}{}{}: ", name, type_hint, req, default_hint);
        io::stderr().flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let input = input.trim();

        let value = if input.is_empty() {
            if let Some(ref default) = var.default {
                default.clone()
            } else if var.required {
                eprintln!("  This variable is required.");
                continue;
            } else {
                continue;
            }
        } else {
            input.to_string()
        };

        result.push(format!("{}={}", name, value));
    }

    let content = result.join("\n") + "\n";
    std::fs::write(output_path, content)?;
    println!(
        "\n.env created at {} with {} variables.",
        output,
        result.len()
    );

    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Legitimate API with shell run configuration"
)]
fn cmd_run(
    profile: Option<&str>,
    resolve: bool,
    env_files: &[String],
    overrides: &[String],
    command: &[String],
    dry_run: bool,
    json: bool,
    volatile: bool,
    profiles: &[String],
    redact: bool,
    no_project: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::dotenv::is_sensitive_key;
    use crate::ops::run::{collect_env, spawn_process, spawn_process_with_redaction, RunConfig};

    if command.is_empty() {
        return Err(
            "No command specified. Usage: envforge run [flags] -- <command> [args...]".into(),
        );
    }

    let mut resolve = resolve;

    if volatile {
        eprintln!("Volatile mode: secrets resolved in memory only");
        resolve = true; // always resolve in volatile mode
        if !env_files.is_empty() {
            eprintln!("Warning: --volatile ignores --env-file (no disk file reads for secrets)");
        }
    }

    let override_pairs: Vec<(String, String)> = overrides
        .iter()
        .map(|s| {
            let parts: Vec<&str> = s.splitn(2, '=').collect();
            if parts.len() == 2 {
                Ok((parts[0].to_string(), parts[1].to_string()))
            } else {
                Err(format!("Invalid override format '{}'. Use KEY=VALUE", s))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let run_config = RunConfig {
        profile: profile.map(String::from),
        profiles: profiles.to_vec(),
        resolve,
        env_files: if volatile {
            Vec::new() // volatile mode skips .env disk files
        } else {
            env_files.iter().map(std::path::PathBuf::from).collect()
        },
        overrides: override_pairs,
        redact,
        no_project,
    };

    let env = collect_env(&run_config)?;

    if dry_run {
        if json {
            let map: serde_json::Value = env
                .iter()
                .map(|(k, v)| {
                    let display_value = if volatile && is_sensitive_key(k) {
                        "****".to_string()
                    } else {
                        v.clone()
                    };
                    (k.clone(), serde_json::Value::String(display_value))
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&map)?);
        } else {
            let mut keys: Vec<&String> = env.keys().collect();
            keys.sort();
            for key in keys {
                if volatile && is_sensitive_key(key) {
                    println!("{}=****", key);
                } else {
                    println!("{}={}", key, env[key]);
                }
            }
        }
        return Ok(());
    }

    let cmd = &command[0];
    let args: Vec<String> = command[1..].to_vec();

    if redact {
        let sensitive_pairs: Vec<(String, String)> = env
            .iter()
            .filter(|(k, _)| is_sensitive_key(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let result = spawn_process_with_redaction(cmd, &args, &env, &sensitive_pairs)?;
        std::process::exit(result.exit_code);
    } else {
        let result = spawn_process(cmd, &args, &env)?;
        std::process::exit(result.exit_code);
    }
}

fn cmd_profile_diff(
    config: &AppConfig,
    profile_a: &str,
    profile_b: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::profile_diff::diff_profiles;

    let result = diff_profiles(config, profile_a, profile_b)?;

    if result.is_empty() {
        println!(
            "Profiles '{}' and '{}' are identical.",
            profile_a, profile_b
        );
        return Ok(());
    }

    let only_a = result.only_in_a();
    let only_b = result.only_in_b();
    let modified = result.modified();

    if !only_a.is_empty() {
        println!(
            "\x1b[31m--- only in {}: {} key(s)\x1b[0m",
            profile_a,
            only_a.len()
        );
        for e in &only_a {
            println!(
                "  \x1b[31m- {} = {}\x1b[0m",
                e.key,
                e.value_a.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    if !only_b.is_empty() {
        println!(
            "\x1b[32m+++ only in {}: {} key(s)\x1b[0m",
            profile_b,
            only_b.len()
        );
        for e in &only_b {
            println!(
                "  \x1b[32m+ {} = {}\x1b[0m",
                e.key,
                e.value_b.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    if !modified.is_empty() {
        println!("\x1b[33m~~~ modified: {} key(s)\x1b[0m", modified.len());
        for e in &modified {
            println!("  \x1b[33m~ {}\x1b[0m", e.key);
            println!(
                "    \x1b[31m- {}\x1b[0m",
                e.value_a.as_deref().unwrap_or("")
            );
            println!(
                "    \x1b[32m+ {}\x1b[0m",
                e.value_b.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    println!(
        "Summary: {} only in {}, {} only in {}, {} modified",
        only_a.len(),
        profile_a,
        only_b.len(),
        profile_b,
        modified.len()
    );

    Ok(())
}

fn cmd_check(only: Option<&str>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::check::{parse_category_filter, print_report, report_to_json, run_checks};

    let filter = if let Some(only_str) = only {
        Some(
            parse_category_filter(only_str)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?,
        )
    } else {
        None
    };

    let report = run_checks(filter.as_deref());

    if json {
        let json_val = report_to_json(&report);
        println!("{}", serde_json::to_string_pretty(&json_val)?);
    } else {
        print_report(&report);
    }

    if report.has_errors() {
        process::exit(1);
    }

    Ok(())
}

fn cmd_doctor(
    verbose: bool,
    all: bool,
    fail_on: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::doctor::{build_mcp_section, run_doctor, CheckStatus, DoctorOpts};

    let report = run_doctor();
    let mcp_opts = DoctorOpts {
        include_unknown: all,
    };
    let mcp_section = build_mcp_section(&mcp_opts);

    if json {
        let checks: Vec<serde_json::Value> = report
            .checks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "status": match c.status {
                        CheckStatus::Ok => "ok",
                        CheckStatus::Warning => "warning",
                        CheckStatus::Error => "error",
                    },
                    "message": c.message,
                    "details": c.details,
                    "hint": c.hint,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "checks": checks,
                "summary": {
                    "ok": report.ok_count(),
                    "warnings": report.warning_count(),
                    "errors": report.error_count(),
                }
            }))?
        );
        return Ok(());
    }

    for check in &report.checks {
        let icon = match check.status {
            CheckStatus::Ok => "\x1b[32m✓\x1b[0m",
            CheckStatus::Warning => "\x1b[33m⚠\x1b[0m",
            CheckStatus::Error => "\x1b[31m✗\x1b[0m",
        };
        println!("{} {:<18} — {}", icon, check.name, check.message);
        if verbose && !check.details.is_empty() {
            for detail in &check.details {
                println!("  {:<20} {}", "", detail);
            }
        }
        if check.status != CheckStatus::Ok {
            if let Some(hint) = &check.hint {
                println!("  {:<20} \x1b[36m→ {}\x1b[0m", "", hint);
            }
        }
    }

    println!();
    println!(
        "  {} checks: {} ok, {} warning(s), {} error(s)",
        report.checks.len(),
        report.ok_count(),
        report.warning_count(),
        report.error_count()
    );

    // MCP supply-chain section (Bolt 081 / Intent 034)
    if let Some(mcp) = &mcp_section {
        if !json {
            println!();
            println!("MCP supply-chain:");
            if !mcp.lockfile_exists {
                println!("  no lockfile (.envforge/mcp.lock) — run `envforge mcp pin` to enable");
            } else {
                println!(
                    "  pinned servers: {} ({} KnownBad, {} UNKNOWN shown)",
                    mcp.pinned_server_count, mcp.known_bad_count, mcp.unknown_count,
                );
                if mcp.feed_stale {
                    println!(
                        "  feed STALE (version {}; upgrade binary for fresh reputation)",
                        mcp.feed_version
                    );
                }
                for name in &mcp.known_bad_servers {
                    println!("  ✗ KnownBad: {name}");
                }
                if all {
                    for name in &mcp.unknown_servers {
                        println!("  ? UNKNOWN: {name}");
                    }
                }
            }
        }
    }

    // --fail-on subsystem exit-code (Bolt 081 / Story 002)
    if let Some(subsystem) = fail_on {
        if subsystem == "mcp" {
            if let Some(mcp) = &mcp_section {
                if mcp.has_critical_findings() {
                    std::process::exit(2);
                }
            }
        }
    }

    let _ = verbose;
    Ok(())
}

fn cmd_rotate(
    key: &str,
    dry_run: bool,
    stale: bool,
    propagate: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::rotate::{apply_rotation, mask_value, plan_rotation};
    use crate::ops::secrets::age::get_age_report;
    use std::io::{self, BufRead, Write};

    if stale {
        // Rotate all stale secrets interactively
        let entries = get_age_report(90)?;
        let stale_entries: Vec<_> = entries.into_iter().filter(|e| e.stale).collect();

        if stale_entries.is_empty() {
            println!("All secrets within 90-day threshold. Nothing to rotate.");
            return Ok(());
        }

        println!(
            "Found {} stale secret(s) (>90 days old):\n",
            stale_entries.len()
        );

        for entry in &stale_entries {
            println!(
                "--- {} ({} days old, from {})",
                entry.key, entry.age_days, entry.provider
            );

            let plan = match plan_rotation(&entry.key) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("  Cannot plan rotation for {}: {}", entry.key, e);
                    continue;
                }
            };

            println!("  Current value: {}", plan.current_masked);
            println!("  Source: {}", plan.source_file.display());

            if dry_run {
                println!("  [dry-run] Would rotate {}", entry.key);
                println!();
                continue;
            }

            eprint!("  Rotate / Skip / Quit? [r/s/q]: ");
            io::stderr().flush()?;
            let mut input = String::new();
            io::stdin().lock().read_line(&mut input)?;
            let choice = input.trim().to_lowercase();

            if choice == "q" {
                println!("Rotation cancelled.");
                return Ok(());
            }
            if choice != "r" {
                println!("  Skipped.\n");
                continue;
            }

            eprint!("  New value: ");
            io::stderr().flush()?;
            let mut new_value = String::new();
            io::stdin().lock().read_line(&mut new_value)?;
            let new_value = new_value.trim();

            if new_value.is_empty() {
                println!("  Empty value, skipping.\n");
                continue;
            }

            let result = apply_rotation(&entry.key, new_value, &plan)?;
            println!(
                "  Rotated {} (local={}, age_reset={}, logged={})",
                result.key, result.local_updated, result.age_reset, result.logged
            );

            if propagate {
                propagate_rotation(&entry.key, new_value, &plan);
            }

            println!();
        }

        println!("Stale rotation complete.");
        return Ok(());
    }

    // Single key rotation
    let plan = plan_rotation(key)?;

    println!("Rotating: {}", key);
    println!("  Current value: {}", plan.current_masked);
    println!("  Source file:   {}", plan.source_file.display());
    if plan.is_encrypted {
        println!("  Encrypted:     yes");
    }
    if plan.has_provider {
        println!(
            "  Provider:      {} ({})",
            plan.provider_name.as_deref().unwrap_or("?"),
            plan.provider_path.as_deref().unwrap_or("")
        );
    }
    if plan.is_synced {
        println!("  Synced:        yes");
    }

    if dry_run {
        println!("\n[dry-run] Would rotate {}. No changes made.", key);
        return Ok(());
    }

    eprint!("\nNew value: ");
    io::stderr().flush()?;
    let mut new_value = String::new();
    io::stdin().lock().read_line(&mut new_value)?;
    let new_value = new_value.trim();

    if new_value.is_empty() {
        println!("Empty value. Rotation cancelled.");
        return Ok(());
    }

    // Confirm
    eprint!("Replace {}? [y/N]: ", key);
    io::stderr().flush()?;
    let mut confirm = String::new();
    io::stdin().lock().read_line(&mut confirm)?;
    if !confirm.trim().eq_ignore_ascii_case("y") {
        println!("Rotation cancelled.");
        return Ok(());
    }

    let result = apply_rotation(key, new_value, &plan)?;

    let mut vault_status = "skipped";
    let mut sync_status = "skipped";

    if propagate {
        // Auto-push to provider
        if plan.has_provider {
            match propagate_to_provider(key, new_value, &plan) {
                Ok(_) => vault_status = "\u{2713}",
                Err(_) => vault_status = "\u{26a0} (failed)",
            }
        }

        // Auto-push to sync
        if plan.is_synced {
            match propagate_to_sync(key, new_value) {
                Ok(_) => sync_status = "\u{2713}",
                Err(_) => sync_status = "\u{26a0} (failed)",
            }
        }
    } else {
        // Interactive prompt behavior (existing)
        if plan.has_provider {
            eprint!(
                "Push to {}? [y/N]: ",
                plan.provider_name.as_deref().unwrap_or("provider")
            );
            io::stderr().flush()?;
            let mut push_input = String::new();
            io::stdin().lock().read_line(&mut push_input)?;
            if push_input.trim().eq_ignore_ascii_case("y") {
                println!(
                    "  Hint: envforge secrets push --to {} --keys {}",
                    plan.provider_name.as_deref().unwrap_or("provider"),
                    key
                );
            }
        }

        if plan.is_synced {
            eprint!("Push to sync? [y/N]: ");
            io::stderr().flush()?;
            let mut sync_input = String::new();
            io::stdin().lock().read_line(&mut sync_input)?;
            if sync_input.trim().eq_ignore_ascii_case("y") {
                println!("  Hint: envforge sync push");
            }
        }
    }

    // Summary
    println!("\nRotation complete:");
    println!("  Key:           {}", result.key);
    println!("  New value:     {}", mask_value(new_value));
    println!("  Local updated: {}", result.local_updated);
    println!("  Age reset:     {}", result.age_reset);
    println!("  Logged:        {}", result.logged);

    if propagate {
        println!(
            "\nRotated: local \u{2713}, vault {}, sync {}",
            vault_status, sync_status
        );
    }

    Ok(())
}

/// Auto-propagate rotation to provider and sync (used with --propagate).
fn propagate_rotation(key: &str, new_value: &str, plan: &crate::ops::rotate::RotationPlan) {
    if plan.has_provider {
        let _ = propagate_to_provider(key, new_value, plan);
    }

    if plan.is_synced {
        let _ = propagate_to_sync(key, new_value);
    }
}

/// Push a rotated secret to its provider. Returns Ok on success.
fn propagate_to_provider(
    key: &str,
    new_value: &str,
    plan: &crate::ops::rotate::RotationPlan,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::secrets::modes::push_secrets;
    use crate::ops::secrets::providers::create_default_registry;

    let provider_name = plan.provider_name.as_deref().unwrap_or("?");
    let provider_path = plan.provider_path.as_deref().unwrap_or("");

    let registry = create_default_registry();
    let secrets = vec![(key.to_string(), new_value.to_string())];

    match push_secrets(&registry, provider_name, provider_path, &secrets, None) {
        Ok(result) => {
            println!(
                "  \u{2713} Pushed to {} ({} keys)",
                provider_name, result.keys_pushed
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("  \u{26a0} Provider push failed: {}", e);
            Err(e.into())
        }
    }
}

/// Push a rotated secret to sync. Returns Ok on success.
fn propagate_to_sync(key: &str, new_value: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::sync::push::push as sync_push;
    use crate::ops::sync::{is_initialized, sync_dir};

    let sync_path = sync_dir()?;
    if !is_initialized(&sync_path) {
        eprintln!("  \u{26a0} Sync not initialized");
        return Err("sync not initialized".into());
    }

    let entries = vec![(key.to_string(), new_value.to_string())];
    match sync_push(&sync_path, &entries, Some("rotated secret"), false) {
        Ok(_) => {
            println!("  \u{2713} Pushed to sync");
            Ok(())
        }
        Err(e) => {
            eprintln!("  \u{26a0} Sync push failed: {}", e);
            Err(e.into())
        }
    }
}

fn cmd_snapshot(
    action: &SnapshotAction,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::snapshot;

    match action {
        SnapshotAction::Create { name } => {
            let config = load_or_create_default()?;
            let (_cfg, shell_files) = load_context()?;
            let entries = collect_all_entries(&shell_files);

            let active_entries: Vec<(String, String)> = entries
                .iter()
                .filter(|e| e.location != EntryLocation::Commented)
                .map(|e| (e.key.clone(), e.value.clone()))
                .collect();

            let snap_name = name
                .clone()
                .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());

            if dry_run {
                println!(
                    "Would create snapshot '{}' with {} variables",
                    snap_name,
                    active_entries.len()
                );
                return Ok(());
            }

            let path =
                snapshot::create_snapshot(&snap_name, &active_entries, &config.profiles.active)?;
            println!(
                "Snapshot '{}' created ({} variables)\n  {}",
                snap_name,
                active_entries.len(),
                path.display()
            );
        }

        SnapshotAction::List => {
            let metas = snapshot::list_snapshots()?;

            if metas.is_empty() {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "version": 1,
                            "snapshots": [],
                        })
                    );
                } else {
                    println!("No snapshots found.");
                }
                return Ok(());
            }

            if json {
                let json_snapshots: Vec<serde_json::Value> = metas
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "name": m.name,
                            "profile": m.profile,
                            "machine_id": m.machine_id,
                            "var_count": m.var_count,
                            "created_at": m.created_at,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "snapshots": json_snapshots,
                    }))?
                );
            } else {
                println!(
                    "{:<25} {:<12} {:<15} {:<6}",
                    "NAME", "PROFILE", "MACHINE", "VARS"
                );
                println!("{}", "-".repeat(60));
                for m in &metas {
                    println!(
                        "{:<25} {:<12} {:<15} {:<6}",
                        m.name, m.profile, m.machine_id, m.var_count
                    );
                }
                println!("\n{} snapshot(s)", metas.len());
            }
        }

        SnapshotAction::Restore { name, last } => {
            let identifier = if *last {
                "last".to_string()
            } else {
                name.clone()
                    .ok_or("Specify a snapshot name or use --last")?
            };

            let snap = snapshot::load_snapshot(&identifier)?;

            if dry_run {
                println!(
                    "Would restore {} variables from snapshot '{}'",
                    snap.entries.len(),
                    snap.metadata.name
                );
                return Ok(());
            }

            // Auto-backup current state before restoring
            let config = load_or_create_default()?;
            let (_cfg, shell_files) = load_context()?;
            let current_entries: Vec<(String, String)> = collect_all_entries(&shell_files)
                .iter()
                .filter(|e| e.location != EntryLocation::Commented)
                .map(|e| (e.key.clone(), e.value.clone()))
                .collect();

            snapshot::create_snapshot("pre-restore", &current_entries, &config.profiles.active)?;

            // Load fresh context for writing
            let (_cfg, mut shell_files) = load_context()?;
            if shell_files.is_empty() {
                return Err("No shell config files found".into());
            }

            let sf = &mut shell_files[0];
            ensure_managed_zone(sf);

            for (key, value) in &snap.entries {
                match edit_entry(sf, key, value) {
                    Ok(()) => {}
                    Err(OpsError::KeyNotFound { .. }) => {
                        if find_soft_deleted(sf, key).is_some() {
                            undo_delete(sf, key)?;
                            edit_entry(sf, key, value)?;
                        } else {
                            add_entry(
                                sf,
                                key,
                                value,
                                ExportStyle::Export,
                                QuoteStyle::Double,
                                config.offsets.header_protected_lines,
                                config.offsets.footer_protected_lines,
                            )?;
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            let content = serialize_shell_file(sf);
            safe_write(&sf.path, &content, Some(sf.hash))?;

            println!(
                "Restored {} variables from snapshot '{}'",
                snap.entries.len(),
                snap.metadata.name
            );
            println!("  (pre-restore backup created automatically)");
        }

        SnapshotAction::Diff { name, last } => {
            let identifier = if *last {
                "last".to_string()
            } else {
                name.clone()
                    .ok_or("Specify a snapshot name or use --last")?
            };

            let snap = snapshot::load_snapshot(&identifier)?;

            let (_cfg, shell_files) = load_context()?;
            let current: Vec<(String, String)> = collect_all_entries(&shell_files)
                .iter()
                .filter(|e| e.location != EntryLocation::Commented)
                .map(|e| (e.key.clone(), e.value.clone()))
                .collect();

            let diff = snapshot::diff_snapshot(&snap, &current);
            let changes: Vec<_> = diff
                .iter()
                .filter(|d| d.status != snapshot::DiffStatus::Same)
                .collect();

            if changes.is_empty() {
                println!(
                    "No differences between snapshot '{}' and current environment.",
                    snap.metadata.name
                );
                return Ok(());
            }

            println!("Diff: snapshot '{}' vs current\n", snap.metadata.name);

            for entry in &changes {
                match entry.status {
                    snapshot::DiffStatus::Added => {
                        println!(
                            "  \x1b[32m+ {:<30} = {}\x1b[0m",
                            entry.key,
                            entry.current_value.as_deref().unwrap_or("")
                        );
                    }
                    snapshot::DiffStatus::Removed => {
                        println!(
                            "  \x1b[31m- {:<30} = {}\x1b[0m",
                            entry.key,
                            entry.snapshot_value.as_deref().unwrap_or("")
                        );
                    }
                    snapshot::DiffStatus::Changed => {
                        println!("  \x1b[33m~ {:<30}\x1b[0m", entry.key);
                        println!(
                            "    \x1b[31m- {}\x1b[0m",
                            entry.snapshot_value.as_deref().unwrap_or("")
                        );
                        println!(
                            "    \x1b[32m+ {}\x1b[0m",
                            entry.current_value.as_deref().unwrap_or("")
                        );
                    }
                    snapshot::DiffStatus::Same => {}
                }
            }

            let added = changes
                .iter()
                .filter(|d| d.status == snapshot::DiffStatus::Added)
                .count();
            let removed = changes
                .iter()
                .filter(|d| d.status == snapshot::DiffStatus::Removed)
                .count();
            let changed = changes
                .iter()
                .filter(|d| d.status == snapshot::DiffStatus::Changed)
                .count();

            println!(
                "\nSummary: {} added, {} removed, {} changed",
                added, removed, changed
            );
        }

        SnapshotAction::Delete { name } => {
            if dry_run {
                println!("Would delete snapshot '{}'", name);
                return Ok(());
            }

            snapshot::delete_snapshot(name)?;
            println!("Deleted snapshot '{}'", name);
        }
    }

    Ok(())
}

fn cmd_share(action: &ShareAction, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::share::{create_share, is_expired, receive_share};

    match action {
        ShareAction::Create {
            recipient,
            keys,
            all,
            filter,
            output,
            expire,
        } => {
            let (_config, shell_files) = load_context()?;
            let all_entries = collect_all_entries(&shell_files);

            // Filter entries based on flags
            let selected: Vec<(String, String)> = if let Some(key_list) = keys {
                let requested: Vec<&str> = key_list.split(',').map(|s| s.trim()).collect();
                all_entries
                    .iter()
                    .filter(|e| e.location != EntryLocation::Commented)
                    .filter(|e| requested.contains(&e.key.as_str()))
                    .map(|e| (e.key.clone(), e.value.clone()))
                    .collect()
            } else if *all {
                all_entries
                    .iter()
                    .filter(|e| e.location != EntryLocation::Commented)
                    .map(|e| (e.key.clone(), e.value.clone()))
                    .collect()
            } else if let Some(pattern) = filter {
                let pattern_lower = pattern.to_lowercase();
                all_entries
                    .iter()
                    .filter(|e| e.location != EntryLocation::Commented)
                    .filter(|e| e.key.to_lowercase().contains(&pattern_lower))
                    .map(|e| (e.key.clone(), e.value.clone()))
                    .collect()
            } else {
                return Err("Specify --keys, --all, or --filter to select entries to share".into());
            };

            if selected.is_empty() {
                return Err("No entries matched the selection criteria".into());
            }

            if dry_run {
                println!("Would create share file with {} key(s):", selected.len());
                for (k, _) in &selected {
                    println!("  {}", k);
                }
                return Ok(());
            }

            let encrypted = create_share(&selected, recipient, *expire)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

            std::fs::write(output, &encrypted)?;

            let pubkey_display = if recipient.len() > 20 {
                format!("{}...", &recipient[..20])
            } else {
                recipient.clone()
            };

            println!("✓ Share file created: {} ({} keys)", output, selected.len());
            println!("  Recipient: {}", pubkey_display);
            println!("  Send this file to your team member");
        }

        ShareAction::Receive { file, import } => {
            let data =
                std::fs::read(file).map_err(|e| format!("Cannot read file '{}': {}", file, e))?;

            let package = receive_share(&data)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

            // Show summary
            println!("Share received:");
            println!("  From:    {}", package.metadata.created_by);
            println!("  Date:    {}", package.metadata.created_at);
            println!("  Keys:    {}", package.metadata.key_count);

            if is_expired(&package) {
                println!(
                    "  \x1b[33m⚠ This share has expired ({})\x1b[0m",
                    package.metadata.expires_at.as_deref().unwrap_or("?")
                );
            }

            println!();

            if *import {
                if dry_run {
                    println!("Would import {} key(s):", package.entries.len());
                    for key in package.entries.keys() {
                        println!("  {}", key);
                    }
                    return Ok(());
                }

                let (config, mut shell_files) = load_context()?;
                if shell_files.is_empty() {
                    return Err("No shell config files found".into());
                }

                let sf = &mut shell_files[0];
                ensure_managed_zone(sf);
                let mut imported = 0;

                for (key, value) in &package.entries {
                    match edit_entry(sf, key, value) {
                        Ok(()) => {
                            println!(" Updated: {}", key);
                        }
                        Err(OpsError::KeyNotFound { .. }) => {
                            if find_soft_deleted(sf, key).is_some() {
                                undo_delete(sf, key)?;
                                edit_entry(sf, key, value)?;
                                println!(" Restored: {}", key);
                            } else {
                                add_entry(
                                    sf,
                                    key,
                                    value,
                                    ExportStyle::Export,
                                    QuoteStyle::Double,
                                    config.offsets.header_protected_lines,
                                    config.offsets.footer_protected_lines,
                                )?;
                                println!(" Added: {}", key);
                            }
                        }
                        Err(e) => {
                            eprintln!("  Failed:  {} ({})", key, e);
                            continue;
                        }
                    }
                    imported += 1;
                }

                let content = serialize_shell_file(sf);
                safe_write(&sf.path, &content, Some(sf.hash))?;
                println!("\nImported {} key(s)", imported);
            } else {
                // Print keys as KEY=VALUE
                for (key, value) in &package.entries {
                    println!("{}={}", key, value);
                }
            }
        }
    }

    Ok(())
}

fn cmd_audit(
    key: Option<&str>,
    since: Option<&str>,
    machine: Option<&str>,
    n: usize,
    json: bool,
    ai_leaks: bool,
    access: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if access {
        return cmd_audit_access(n, json);
    }
    if ai_leaks {
        use crate::ops::audit::scan_ai_leaks;
        let cwd = std::env::current_dir()?;
        let leaks = scan_ai_leaks(&cwd, n)?;
        if leaks.is_empty() {
            if json {
                println!("{}", serde_json::json!({"leaks": [], "total": 0}));
            } else {
                println!("No secret leaks found in AI-assisted commits.");
            }
        } else if json {
            let items: Vec<serde_json::Value> = leaks
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "commit": l.commit_hash,
                        "date": l.date,
                        "author": l.author,
                        "ai_tool": l.ai_tool,
                        "file": l.file_path,
                        "patterns": l.leaked_patterns,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "leaks": items,
                    "total": leaks.len(),
                }))?
            );
        } else {
            println!("AI-Assisted Commit Secret Leaks\n");
            for leak in &leaks {
                println!("  {} {} [{}]", leak.commit_hash, leak.date, leak.ai_tool);
                println!("    File: {}", leak.file_path);
                for pattern in &leak.leaked_patterns {
                    println!("    \u{26a0} {}", pattern);
                }
                println!();
            }
            println!("{} leak(s) found in AI-assisted commits.", leaks.len());
        }
        return Ok(());
    }

    use crate::ops::audit::get_audit_trail;
    use crate::ops::sync::{is_initialized, sync_dir};

    let sync_path = sync_dir()?;
    if !is_initialized(&sync_path) {
        return Err("Sync not initialized. Run `envforge sync init` first.".into());
    }

    let entries = get_audit_trail(&sync_path, key, since, machine, n)?;

    if entries.is_empty() {
        println!("No sync history. Push changes first.");
        return Ok(());
    }

    if json {
        let json_entries: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "machine": e.machine_id,
                    "action": e.action,
                    "key": e.key,
                    "commit": e.commit_hash,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_entries)?);
    } else {
        println!(
            "{:<25} {:<18} {:<10} {:<30} COMMIT",
            "TIMESTAMP", "MACHINE", "ACTION", "KEY"
        );
        println!("{}", "-".repeat(100));
        for e in &entries {
            println!(
                "{:<25} {:<18} {:<10} {:<30} {}",
                e.timestamp, e.machine_id, e.action, e.key, e.commit_hash
            );
        }
        println!("\n{} entries shown.", entries.len());
    }

    Ok(())
}

fn cmd_resolve_uri(
    file: &str,
    env_format: bool,
    output: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::secrets::providers::create_default_registry;
    use crate::ops::uri_resolve::{
        format_as_env, format_as_export, format_summary, parse_uri_file, resolve_uris,
    };

    let file_path = Path::new(file);
    if !file_path.exists() {
        return Err(format!("File not found: {}", file).into());
    }

    let entries = parse_uri_file(file_path)?;
    if entries.is_empty() {
        println!("No entries found in {}", file);
        return Ok(());
    }

    let registry = create_default_registry();
    let resolved = resolve_uris(&entries, &registry);

    // Report errors to stderr first
    for entry in &resolved {
        if let Some(ref err) = entry.error {
            eprintln!(
                "Warning: failed to resolve {} ({}): {}",
                entry.key, entry.value, err
            );
        }
    }

    if json {
        let items: Vec<serde_json::Value> = resolved
            .iter()
            .map(|e| {
                let mut obj = serde_json::json!({
                    "key": e.key,
                    "value": e.value,
                    "was_uri": e.was_uri,
                });
                if let Some(ref err) = e.error {
                    obj["error"] = serde_json::Value::String(err.clone());
                }
                obj
            })
            .collect();
        let output_json = serde_json::to_string_pretty(&items)?;
        if let Some(out_path) = output {
            std::fs::write(out_path, &output_json)?;
            eprintln!("Written to {}", out_path);
        } else {
            println!("{}", output_json);
        }
    } else {
        let formatted = if env_format {
            format_as_env(&resolved)
        } else {
            format_as_export(&resolved)
        };

        if let Some(out_path) = output {
            std::fs::write(out_path, &formatted)?;
            eprintln!("Written to {}", out_path);
        } else {
            print!("{}", formatted);
        }
    }

    // Summary to stderr so it doesn't pollute piped output
    eprintln!("{}", format_summary(&resolved));

    // Exit with error if any URIs failed
    let has_errors = resolved.iter().any(|e| e.error.is_some());
    if has_errors {
        process::exit(1);
    }

    Ok(())
}

fn cmd_fence(dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::fence::create_fence;

    let project_dir = std::env::current_dir()?;
    let result = create_fence(&project_dir, dry_run)?;

    if dry_run {
        println!("AI Secret Fence (dry run)\n");
    } else {
        println!("AI Secret Fence\n");
    }

    for path in &result.files_created {
        let display = path.strip_prefix(&project_dir).unwrap_or(path).display();
        println!("\x1b[32m\u{2713}\x1b[0m Created {}", display);
    }

    for path in &result.files_updated {
        let display = path.strip_prefix(&project_dir).unwrap_or(path).display();
        println!("\x1b[32m\u{2713}\x1b[0m Updated {}", display);
    }

    for path in &result.files_skipped {
        let display = path.strip_prefix(&project_dir).unwrap_or(path).display();
        println!("\x1b[90m- Skipped {} (already configured)\x1b[0m", display);
    }

    let total = result.files_created.len() + result.files_updated.len();
    if total > 0 {
        println!(
            "\n{} file(s) {}. AI tools will now respect secret boundaries.",
            total,
            if dry_run {
                "would be written"
            } else {
                "written"
            }
        );
    } else {
        println!("\nAll files already configured. Nothing to do.");
    }

    Ok(())
}

fn cmd_fence_status(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::fence::check_fence_status;

    let project_dir = std::env::current_dir()?;
    let status = check_fence_status(&project_dir)?;

    if json {
        let obj = serde_json::json!({
            "version": 1,
            "all_fenced": status.all_fenced,
            "files": status.files,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("AI Secret Fence Status\n");
        for f in &status.files {
            let icon = if f.fenced {
                "\x1b[32m✓\x1b[0m"
            } else if f.exists {
                "\x1b[33m⚠\x1b[0m"
            } else {
                "\x1b[31m✗\x1b[0m"
            };
            let label = if f.fenced {
                "fenced"
            } else if f.exists {
                "exists (not fenced)"
            } else {
                "missing"
            };
            println!(" {} {} — {}", icon, f.path, label);
        }
        println!();
        if status.all_fenced {
            println!("\x1b[32mAll fence files configured.\x1b[0m");
        } else {
            println!("\x1b[33mSome fence files missing or incomplete. Run `envforge fence` to set up.\x1b[0m");
        }
    }

    Ok(())
}

fn cmd_sanitize(
    file: &str,
    output: Option<&str>,
    json: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::sanitize::sanitize_file;

    let file_path = Path::new(file);
    if !file_path.exists() {
        return Err(format!("File not found: {}", file).into());
    }

    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let secrets: Vec<(String, String)> = entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .filter(|e| crate::ops::dotenv::is_sensitive_key(&e.key))
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect();

    if secrets.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "file": file,
                    "sanitized": false,
                    "count": 0,
                    "message": "No sensitive ENV values found",
                }))?
            );
        } else {
            eprintln!("No sensitive ENV values found to sanitize against.");
        }
        return Ok(());
    }

    if dry_run {
        if json {
            let secret_keys: Vec<&str> = secrets.iter().map(|(k, _)| k.as_str()).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "file": file,
                    "dry_run": true,
                    "secrets_would_replace": secret_keys,
                    "count": secrets.len(),
                }))?
            );
        } else {
            eprintln!(
                "Dry run: would replace {} secret(s) in {}",
                secrets.len(),
                file
            );
            for (key, _) in &secrets {
                eprintln!("  {} → ${{{}}}", key, key);
            }
        }
    } else {
        let output_path = output.map(Path::new);
        let count = sanitize_file(file_path, output_path, &secrets)?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "file": file,
                    "sanitized": true,
                    "count": count,
                }))?
            );
        } else {
            eprintln!("Sanitized: {} secret(s) replaced", count);
        }
    }

    Ok(())
}

// ─── AI Hook Command ───────────────────────────────────────

fn cmd_ai_hook(
    action: &AiHookAction,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::ai_hooks::{install_ai_hook, parse_ai_tool, remove_ai_hook};

    let cwd = std::env::current_dir()?;

    match action {
        AiHookAction::Install { tool } => {
            let ai_tool =
                parse_ai_tool(tool).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            if dry_run {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "version": 1,
                            "action": "install",
                            "tool": tool,
                            "dry_run": true,
                            "message": format!("Would install {} hook", tool),
                        }))?
                    );
                } else {
                    println!("Dry run: would install {} hook", tool);
                }
            } else {
                let result = install_ai_hook(&ai_tool, &cwd)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "version": 1,
                            "action": "install",
                            "tool": tool,
                            "installed": result.installed,
                            "message": result.message,
                            "config_path": result.config_path.to_string_lossy(),
                        }))?
                    );
                } else if result.installed {
                    println!("{}", result.message);
                    println!(" Config: {}", result.config_path.display());
                } else {
                    println!("{}", result.message);
                }
            }
        }
        AiHookAction::Remove { tool } => {
            let ai_tool =
                parse_ai_tool(tool).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            if dry_run {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "version": 1,
                            "action": "remove",
                            "tool": tool,
                            "dry_run": true,
                            "message": format!("Would remove {} hook", tool),
                        }))?
                    );
                } else {
                    println!("Dry run: would remove {} hook", tool);
                }
            } else {
                let result = remove_ai_hook(&ai_tool, &cwd)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "version": 1,
                            "action": "remove",
                            "tool": tool,
                            "message": result.message,
                            "config_path": result.config_path.to_string_lossy(),
                        }))?
                    );
                } else {
                    println!("{}", result.message);
                    if result.config_path.exists() {
                        println!(" Config: {}", result.config_path.display());
                    }
                }
            }
        }
        AiHookAction::Status => {
            let status = crate::ops::ai_hooks::check_hook_status(&cwd);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "status": status,
                    }))?
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
        }
    }

    Ok(())
}

// ─── AI Guard Command ──────────────────────────────────────

fn cmd_ai_guard(
    stage: Option<&str>,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::ai_guard::{run_guard, GuardStage};

    let stage = match stage {
        Some(s) => s,
        None => return Ok(()),
    };
    let tool_name = match tool_name {
        Some(t) => t,
        None => return Ok(()),
    };

    let stage_enum = match stage {
        "pre-tool" => GuardStage::PreTool,
        "post-tool" => GuardStage::PostTool,
        _ => return Ok(()),
    };
    let stage_str = stage;

    let secrets = load_sensitive_secrets();

    // Load AI Guard config from project config if available
    let (hardening_config, scanner_registry) = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            crate::ops::project::config::detect_project_config(&cwd).and_then(|detected| {
                std::fs::read_to_string(&detected.config_path)
                    .ok()
                    .and_then(|content| match detected.format {
                        crate::ops::project::config::ConfigFormat::Toml => {
                            toml::from_str::<crate::ops::project::config::ProjectConfig>(&content)
                                .ok()
                        }
                        _ => None,
                    })
                    .map(|config| (config.ai_guard.hardening, config.ai_guard.scanners))
            })
        })
        .unwrap_or_default();

    // Run external scanners if configured
    let scanner_findings = if let Some(input) = tool_input {
        if !scanner_registry.is_empty() {
            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            rt.block_on(crate::ops::external_scanner::run_scanners(
                &scanner_registry,
                input,
            ))
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let result = run_guard(
        stage_enum,
        tool_name,
        tool_input,
        &secrets,
        Some(&hardening_config),
        if scanner_findings.is_empty() {
            None
        } else {
            Some(&scanner_findings)
        },
    );

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "stage": stage_str,
                "tool": tool_name,
                "blocked": result.blocked,
                "warnings": result.warnings,
            }))?
        );
    } else {
        for warning in &result.warnings {
            eprintln!("{}", warning);
        }
    }

    Ok(())
}

fn load_sensitive_secrets() -> Vec<(String, String)> {
    let config = match crate::config::load_or_create_default() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let primary = shellexpand_path(&config.files.primary);
    let sf = match crate::parser::parse_shell_file(&primary) {
        Ok(sf) => sf,
        Err(_) => return vec![],
    };
    let entries = collect_all_entries(&[sf]);
    entries
        .into_iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .filter(|e| is_sensitive_key(&e.key))
        .filter(|e| e.value.len() >= 8) // skip short values
        .map(|e| (e.key, e.value))
        .collect()
}

fn shellexpand_path(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

// ─── Proxy Command ─────────────────────────────────────────

fn cmd_proxy(
    port: u16,
    keys: Option<&str>,
    profile: Option<&str>,
    allow_origins: Option<&str>,
    require_lease: bool,
    require_approval: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::proxy::start_proxy;
    use crate::ops::run::{collect_env, RunConfig};

    let allowed_keys: Option<Vec<String>> = keys.map(|k| {
        k.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let allowed_origins: Option<Vec<String>> = allow_origins.map(|o| {
        o.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let run_config = RunConfig {
        profile: profile.map(|s| s.to_string()),
        profiles: Vec::new(),
        resolve: true,
        env_files: Vec::new(),
        overrides: Vec::new(),
        redact: false,
        no_project: false,
    };

    let env = collect_env(&run_config)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    if require_lease {
        eprintln!("Lease enforcement: ON (requests require an active lease)");
    }
    if require_approval {
        eprintln!("Human approval: ON (each secret access requires approval)");
    }

    start_proxy(
        port,
        &env,
        allowed_keys.as_deref(),
        allowed_origins.as_deref(),
        require_lease,
        require_approval,
    )?;

    Ok(())
}

// ─── Canary Commands ──────────────────────────────────────────

fn cmd_canary(action: &super::CanaryAction, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::canary;

    match action {
        super::CanaryAction::Create { key, pattern } => {
            let canary = canary::create_canary(key, pattern)?;
            if json {
                let obj = serde_json::json!({
                    "version": 1,
                    "key": canary.key,
                    "fake_value": canary.fake_value,
                    "pattern": canary.pattern,
                    "created_at": canary.created_at,
                    "triggered": canary.triggered,
                    "trigger_count": canary.trigger_count,
                });
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                println!("Canary created: {}", canary.key);
                println!(" Fake value: {}", canary.fake_value);
                println!(" Pattern: {}", canary.pattern);
                println!();
                println!("Add to your .env: {}={}", canary.key, canary.fake_value);
                println!("If this value appears in logs, git, or API calls — an agent leaked it.");
            }
        }
        super::CanaryAction::List => {
            let canaries = canary::list_canaries()?;
            if canaries.is_empty() {
                if json {
                    println!("[]");
                } else {
                    println!("No canary secrets configured.");
                    println!("Create one with: envforge canary create KEY --pattern generic");
                }
                return Ok(());
            }
            if json {
                let json_canaries: Vec<serde_json::Value> = canaries
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "version": 1,
                            "key": c.key,
                            "pattern": c.pattern,
                            "fake_value": c.fake_value,
                            "created_at": c.created_at,
                            "triggered": c.triggered,
                            "trigger_count": c.trigger_count,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json_canaries)?);
            } else {
                println!(
                    "{:<25} {:<15} {:<10} {:<8}",
                    "KEY", "PATTERN", "TRIGGERED", "COUNT"
                );
                println!("{}", "-".repeat(60));
                for c in &canaries {
                    println!(
                        "{:<25} {:<15} {:<10} {:<8}",
                        c.key,
                        c.pattern,
                        if c.triggered { "YES" } else { "no" },
                        c.trigger_count,
                    );
                }
                println!("\nTotal: {} canary secret(s)", canaries.len());
            }
        }
        super::CanaryAction::Check => {
            let triggered = canary::check_canaries()?;
            if triggered.is_empty() {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "version": 1,
                            "triggered": [],
                            "total": 0,
                        })
                    );
                } else {
                    println!("No canaries have been triggered. All clear.");
                }
                return Ok(());
            }
            if json {
                let json_triggered: Vec<serde_json::Value> = triggered
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "version": 1,
                            "key": c.key,
                            "pattern": c.pattern,
                            "trigger_count": c.trigger_count,
                        })
                    })
                    .collect();
                let alerts = canary::read_alerts()?;
                let json_alerts: Vec<serde_json::Value> = alerts
                    .iter()
                    .rev()
                    .take(10)
                    .map(|a| {
                        serde_json::json!({
                            "timestamp": a.timestamp,
                            "key": a.key,
                            "source": a.source,
                            "details": a.details,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "version": 1,
                        "triggered": json_triggered,
                        "total": triggered.len(),
                        "recent_alerts": json_alerts,
                    })
                );
            } else {
                println!(
                    "\u{1f6a8} {} canary secret(s) TRIGGERED:\n",
                    triggered.len()
                );
                for c in &triggered {
                    println!(
                        " {} (pattern: {}, triggered {} time(s))",
                        c.key, c.pattern, c.trigger_count
                    );
                }

                let alerts = canary::read_alerts()?;
                if !alerts.is_empty() {
                    println!("\nRecent alerts:");
                    let start = if alerts.len() > 10 {
                        alerts.len() - 10
                    } else {
                        0
                    };
                    for alert in &alerts[start..] {
                        println!(
                            " [{}] {} via {} - {}",
                            alert.timestamp, alert.key, alert.source, alert.details
                        );
                    }
                }
            }
        }
        super::CanaryAction::Delete { key } => {
            let found = canary::delete_canary(key)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "version": 1,
                        "key": key,
                        "deleted": found,
                    })
                );
            } else if found {
                println!("Canary deleted: {}", key);
            } else {
                println!("Canary not found: {}", key);
            }
        }
        super::CanaryAction::Rotate { all, key, dry_run } => {
            if *all {
                let rotated = if *dry_run {
                    let canaries = canary::list_canaries()?;
                    let eligible: Vec<_> = canaries
                        .iter()
                        .filter(|c| canary::is_eligible_for_rotation(c))
                        .collect();
                    println!("Would rotate {} canaries:", eligible.len());
                    for c in &eligible {
                        println!("  - {} (age > {} days)", c.key, c.rotate_after_days);
                    }
                    eligible.len()
                } else {
                    canary::rotate_all_canaries()?
                };
                if !dry_run {
                    println!("Rotated {} canaries.", rotated);
                }
            } else if let Some(key) = key {
                if *dry_run {
                    let canaries = canary::list_canaries()?;
                    if let Some(c) = canaries.iter().find(|c| c.key == *key) {
                        if canary::is_eligible_for_rotation(c) {
                            println!("Would rotate {} (age > {} days)", key, c.rotate_after_days);
                        } else {
                            println!("Canary {} is not yet eligible for rotation.", key);
                        }
                    } else {
                        println!("Canary not found: {}", key);
                    }
                } else {
                    match canary::rotate_canary(key)? {
                        Some(rotated) => {
                            println!("Rotated canary: {}", rotated.key);
                            println!("New value: {}", rotated.fake_value);
                        }
                        None => {
                            println!("Canary not found: {}", key);
                        }
                    }
                }
            } else {
                eprintln!("Usage: envforge canary rotate --all or --key <KEY>");
            }
        }
        super::CanaryAction::Place {
            key,
            file,
            position,
        } => {
            let path = std::path::Path::new(file);
            match canary::place_canary_in_file(key, path, position)? {
                true => println!("Placed canary {} in {} at '{}'", key, file, position),
                false => println!("Canary {} already placed in {}", key, file),
            }
        }
        // ─── v2 forensic canaries ────────────────
        super::CanaryAction::MintV2 {
            key,
            tool,
            pid,
            json: emit_json,
        } => {
            let actual_pid = pid.unwrap_or_else(std::process::id);
            let (record, token) = canary::mint_v2(key, tool, actual_pid)?;
            if *emit_json {
                let obj = serde_json::json!({
                    "version": 2,
                    "key": record.key,
                    "token": token,
                    "tool": tool,
                    "pid": actual_pid,
                    "minted_at": record.created_at,
                });
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                println!("{}", token);
            }
        }
        super::CanaryAction::Decode { token, json: j } => {
            // v1 fast path: token doesn't start with v2 prefix.
            if !token.starts_with(canary::V2_PREFIX) {
                if *j {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "version": 1,
                            "opaque": true,
                            "hmac_valid": false,
                        }))?
                    );
                } else {
                    println!("v1 token: opaque (no decodable payload)");
                }
                return Ok(());
            }
            let mgr = canary::HmacKeyManager::load_or_init()?;
            let registry = mgr.registry();
            let candidates = registry.verify_iter();
            match canary::decode_token(token, candidates.iter().map(|(v, k)| (*v, *k))) {
                Ok(decoded) => {
                    if *j {
                        let payload_obj = decoded.payload.as_ref().map(|p| {
                            serde_json::json!({
                                "machine_id_hex": hex::encode(p.machine_id),
                                "pid": p.pid,
                                "timestamp_unix": p.timestamp_unix(),
                                "agent_name_hash_hex": hex::encode(p.agent_name_hash),
                                "key_name_hash_hex": hex::encode(p.key_name_hash),
                            })
                        });
                        let obj = serde_json::json!({
                            "version": decoded.version,
                            "hmac_valid": decoded.hmac_valid,
                            "key_version_used": decoded.key_version_used,
                            "age_seconds": decoded.age_seconds,
                            "payload": payload_obj,
                        });
                        println!("{}", serde_json::to_string_pretty(&obj)?);
                    } else {
                        let banner = if decoded.hmac_valid {
                            "✅ HMAC valid"
                        } else {
                            "⚠ HMAC INVALID — token may be forged or from rotated key"
                        };
                        println!("{banner}");
                        if let Some(p) = &decoded.payload {
                            println!("  machine_id: {}", hex::encode(p.machine_id));
                            println!("  pid:        {}", p.pid);
                            println!("  ts_unix:    {}", p.timestamp_unix());
                            println!("  agent_hash: {}", hex::encode(p.agent_name_hash));
                            println!("  key_hash:   {}", hex::encode(p.key_name_hash));
                            if let Some(age) = decoded.age_seconds {
                                println!("  age_secs:   {}", age);
                            }
                            if let Some(v) = decoded.key_version_used {
                                println!("  hmac_key_v: {}", v);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("decode error: {e}");
                    std::process::exit(2);
                }
            }
        }
        super::CanaryAction::Scan {
            input,
            strict,
            json: j,
        } => {
            let matches: Vec<canary::TokenMatch> = if input == "-" {
                canary::scan_reader(std::io::stdin().lock())
            } else {
                let f = std::fs::File::open(input)?;
                canary::scan_reader(f)
            };
            if *j {
                let arr: Vec<_> = matches
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "token": m.token,
                            "byte_offset": m.byte_offset,
                            "line_number": m.line_number,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&arr)?);
            } else {
                for m in &matches {
                    let line = m
                        .line_number
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into());
                    println!("line {line}: {}", m.token);
                }
                if matches.is_empty() {
                    println!("(no canary tokens found)");
                }
            }
            if *strict && !matches.is_empty() {
                std::process::exit(1);
            }
        }
        super::CanaryAction::RotateKey { dry_run } => {
            if *dry_run {
                println!("dry-run: would rotate canary HMAC key (active version → +1; oldest retired key evicted if cap reached)");
            } else {
                let (new_v, kept) = canary::rotate_key()?;
                println!(
                    "rotated: new active key version {new_v}; retired versions kept: {kept:?}"
                );
            }
        }
        super::CanaryAction::Migrate {
            dry_run,
            replace,
            bulk,
            tool,
        } => {
            if !*bulk && replace.is_none() {
                eprintln!("specify --replace <key> or --bulk");
                std::process::exit(2);
            }
            let plan = canary::MigrationService::plan(replace.as_deref())?;
            if plan.steps.is_empty() {
                println!("(nothing to migrate)");
            } else {
                for s in &plan.steps {
                    println!("  {} -> {:?}: {}", s.original_key, s.action, s.reason);
                }
            }
            let report = canary::MigrationService::execute(&plan, *dry_run, tool)?;
            println!(
                "planned={} executed={} skipped={} failures={}",
                report.planned,
                report.executed,
                report.skipped,
                report.failures.len()
            );
            for (k, e) in &report.failures {
                eprintln!("  FAIL {k}: {e}");
            }
        }
    }

    Ok(())
}

fn cmd_hardening(action: &super::HardeningAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::hardening::HardeningConfig;
    use crate::ops::project::config::{detect_project_config, ConfigFormat, ProjectConfig};

    // Try to load project config for hardening settings
    let mut config = HardeningConfig::default();
    let mut config_path: Option<std::path::PathBuf> = None;
    let mut config_format: Option<ConfigFormat> = None;

    if let Some(detected) = std::env::current_dir()
        .ok()
        .and_then(|cwd| detect_project_config(&cwd))
    {
        if let Ok(content) = std::fs::read_to_string(&detected.config_path) {
            let parsed: Option<ProjectConfig> = match detected.format {
                ConfigFormat::Toml => toml::from_str(&content).ok(),
                ConfigFormat::Json => serde_json::from_str(&content).ok(),
                ConfigFormat::Yaml => None, // serde_yaml not in dependencies
            };
            if let Some(project_config) = parsed {
                config = project_config.ai_guard.hardening;
                config_path = Some(detected.config_path);
                config_format = Some(detected.format);
            }
        }
    }

    match action {
        super::HardeningAction::Show => {
            println!("Adversarial Input Hardening Configuration");
            println!("{}", "=".repeat(50));
            println!(
                "  control_chars      : {}",
                if config.control_chars {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  base64_decode      : {} (min_length: {})",
                if config.base64_decode {
                    "enabled"
                } else {
                    "disabled"
                },
                config.base64_min_length
            );
            println!(
                "  split_strings      : {}",
                if config.split_strings {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  encoding_chain     : {} (max_depth: {})",
                if config.encoding_chain {
                    "enabled"
                } else {
                    "disabled"
                },
                config.encoding_chain_max_depth
            );
            if config_path.is_none() {
                println!("\nNo project config found. Using defaults.");
                println!("Run 'envforge project init' to create a config file.");
            }
        }
        super::HardeningAction::Enable { layer } => {
            match layer.as_str() {
                "control_chars" => config.control_chars = true,
                "base64_decode" => config.base64_decode = true,
                "split_strings" => config.split_strings = true,
                "encoding_chain" => config.encoding_chain = true,
                _ => {
                    eprintln!("Unknown layer: {}", layer);
                    eprintln!(
                        "Valid layers: control_chars, base64_decode, split_strings, encoding_chain"
                    );
                    return Ok(());
                }
            }
            if let Some(path) = config_path {
                if let Some(format) = config_format {
                    // Read existing config, update hardening section, write back
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let parsed: Option<ProjectConfig> = match format {
                            ConfigFormat::Toml => toml::from_str(&content).ok(),
                            ConfigFormat::Json => serde_json::from_str(&content).ok(),
                            ConfigFormat::Yaml => None,
                        };
                        if let Some(mut project_config) = parsed {
                            project_config.ai_guard.hardening = config;
                            let updated = match format {
                                ConfigFormat::Toml => toml::to_string_pretty(&project_config)?,
                                ConfigFormat::Json => {
                                    serde_json::to_string_pretty(&project_config)?
                                }
                                ConfigFormat::Yaml => String::new(),
                            };
                            std::fs::write(&path, updated)?;
                            println!("Enabled hardening layer: {}", layer);
                            return Ok(());
                        }
                    }
                }
            }
            eprintln!("No project config found. Run 'envforge project init' first.");
        }
        super::HardeningAction::Disable { layer } => {
            match layer.as_str() {
                "control_chars" => config.control_chars = false,
                "base64_decode" => config.base64_decode = false,
                "split_strings" => config.split_strings = false,
                "encoding_chain" => config.encoding_chain = false,
                _ => {
                    eprintln!("Unknown layer: {}", layer);
                    eprintln!(
                        "Valid layers: control_chars, base64_decode, split_strings, encoding_chain"
                    );
                    return Ok(());
                }
            }
            if let Some(path) = config_path {
                if let Some(format) = config_format {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let parsed: Option<ProjectConfig> = match format {
                            ConfigFormat::Toml => toml::from_str(&content).ok(),
                            ConfigFormat::Json => serde_json::from_str(&content).ok(),
                            ConfigFormat::Yaml => None,
                        };
                        if let Some(mut project_config) = parsed {
                            project_config.ai_guard.hardening = config;
                            let updated = match format {
                                ConfigFormat::Toml => toml::to_string_pretty(&project_config)?,
                                ConfigFormat::Json => {
                                    serde_json::to_string_pretty(&project_config)?
                                }
                                ConfigFormat::Yaml => String::new(),
                            };
                            std::fs::write(&path, updated)?;
                            println!("Disabled hardening layer: {}", layer);
                            return Ok(());
                        }
                    }
                }
            }
            eprintln!("No project config found. Run 'envforge project init' first.");
        }
    }

    Ok(())
}

fn cmd_scanner(action: &super::ScannerAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::external_scanner::{run_single_scanner, ScannerRegistry};
    use crate::ops::project::config::{detect_project_config, ConfigFormat, ProjectConfig};

    // Load project config for scanner registry
    let mut registry = ScannerRegistry::default();
    let mut config_path: Option<std::path::PathBuf> = None;
    let mut config_format: Option<ConfigFormat> = None;

    if let Some(detected) = std::env::current_dir()
        .ok()
        .and_then(|cwd| detect_project_config(&cwd))
    {
        if let Ok(content) = std::fs::read_to_string(&detected.config_path) {
            let parsed: Option<ProjectConfig> = match detected.format {
                ConfigFormat::Toml => toml::from_str(&content).ok(),
                ConfigFormat::Json => serde_json::from_str(&content).ok(),
                ConfigFormat::Yaml => None,
            };
            if let Some(project_config) = parsed {
                registry = project_config.ai_guard.scanners;
                config_path = Some(detected.config_path);
                config_format = Some(detected.format);
            }
        }
    }

    match action {
        super::ScannerAction::List => {
            if registry.is_empty() {
                println!("No external scanners configured.");
                println!("Add to .envforge.project.toml:");
                println!("  [scanners.myscanner]");
                println!("  command = \"gitleaks\"");
                println!("  args = [\"detect\", \"--no-git\"]");
                return Ok(());
            }
            println!("{:<20} {:<10} {:<30} TIMEOUT", "NAME", "STATUS", "COMMAND");
            println!("{}", "-".repeat(80));
            for (name, scanner) in &registry.scanners {
                let status = if scanner.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                let cmd = format!("{} {}", scanner.command, scanner.args.join(" "));
                println!(
                    "{:<20} {:<10} {:<30} {}ms",
                    name, status, cmd, scanner.timeout_ms
                );
            }
        }
        super::ScannerAction::Test { name } => {
            let scanner = registry.get(name);
            if scanner.is_none() {
                eprintln!("Scanner not found or disabled: {}", name);
                return Ok(());
            }
            let config = scanner.unwrap().clone();
            let sample = "This is sample content for testing the scanner.";
            println!("Testing scanner: {}", name);
            println!("Command: {} {}", config.command, config.args.join(" "));
            println!("Sample content: {}", sample);
            println!();

            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            let result = rt.block_on(run_single_scanner(name, &config, sample));
            match result {
                Some(finding) => {
                    println!("Findings ({}):", finding.findings.len());
                    for line in &finding.findings {
                        println!("  - {}", line);
                    }
                }
                None => {
                    println!("No findings (scanner exited clean).");
                }
            }
        }
        super::ScannerAction::Run { name, content } => {
            let scanner = registry.get(name);
            if scanner.is_none() {
                eprintln!("Scanner not found or disabled: {}", name);
                return Ok(());
            }
            let config = scanner.unwrap().clone();
            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            let result = rt.block_on(run_single_scanner(name, &config, content));
            match result {
                Some(finding) => {
                    for line in &finding.findings {
                        println!("{}", line);
                    }
                }
                None => {
                    println!("No findings.");
                }
            }
        }
        super::ScannerAction::Enable { name } => {
            if let Some(scanner) = registry.scanners.get_mut(name) {
                scanner.enabled = true;
            } else {
                eprintln!("Scanner not found: {}", name);
                return Ok(());
            }
            save_scanner_registry(&registry, config_path, config_format)?;
            println!("Enabled scanner: {}", name);
        }
        super::ScannerAction::Disable { name } => {
            if let Some(scanner) = registry.scanners.get_mut(name) {
                scanner.enabled = false;
            } else {
                eprintln!("Scanner not found: {}", name);
                return Ok(());
            }
            save_scanner_registry(&registry, config_path, config_format)?;
            println!("Disabled scanner: {}", name);
        }
    }

    Ok(())
}

fn save_scanner_registry(
    registry: &crate::ops::external_scanner::ScannerRegistry,
    config_path: Option<std::path::PathBuf>,
    config_format: Option<crate::ops::project::config::ConfigFormat>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::project::config::{ConfigFormat, ProjectConfig};

    if let Some(path) = config_path {
        if let Some(format) = config_format {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let parsed: Option<ProjectConfig> = match format {
                    ConfigFormat::Toml => toml::from_str(&content).ok(),
                    ConfigFormat::Json => serde_json::from_str(&content).ok(),
                    ConfigFormat::Yaml => None,
                };
                if let Some(mut project_config) = parsed {
                    project_config.ai_guard.scanners = registry.clone();
                    let updated = match format {
                        ConfigFormat::Toml => toml::to_string_pretty(&project_config)?,
                        ConfigFormat::Json => serde_json::to_string_pretty(&project_config)?,
                        ConfigFormat::Yaml => String::new(),
                    };
                    std::fs::write(&path, updated)?;
                }
            }
        }
    }
    Ok(())
}

fn cmd_monitor(action: &super::MonitorAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::monitor::health::run_all_checks;
    use crate::ops::monitor::{init_event_bus, subscribe_events, EventSource};
    use std::io::{self, Write};

    match action {
        super::MonitorAction::Status => {
            println!("Running health checks...\n");
            let results = run_all_checks();

            let mut healthy = 0;
            let mut degraded = 0;
            let mut failed = 0;

            for r in &results {
                let icon = match r.status {
                    crate::ops::monitor::HealthStatus::Healthy => {
                        healthy += 1;
                        "\u{2705}"
                    }
                    crate::ops::monitor::HealthStatus::Degraded => {
                        degraded += 1;
                        "\u{26a0}\u{fe0f}"
                    }
                    crate::ops::monitor::HealthStatus::Failed => {
                        failed += 1;
                        "\u{274c}"
                    }
                };
                let latency = r
                    .latency_ms
                    .map(|l| format!(" ({}ms)", l))
                    .unwrap_or_default();
                println!(
                    "{} [{:<10}] {}{} — {}",
                    icon, r.category, r.name, latency, r.message
                );
            }

            println!();
            let total = healthy + degraded + failed;
            println!(
                "{} checks: {} ok, {} warning(s), {} error(s)",
                total, healthy, degraded, failed
            );
        }
        super::MonitorAction::Stream => {
            init_event_bus(256);
            println!("Streaming events... (Ctrl+C to stop)\n");

            let mut rx = subscribe_events().ok_or("Event bus not initialized")?;

            // Show initial marker
            println!(
                "[{}] Monitoring started — waiting for events...",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
            );

            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            loop {
                match rt.block_on(rx.recv()) {
                    Ok(event) => {
                        let icon = match event.source {
                            EventSource::Canary | EventSource::Fence => "\u{1f6a8}",
                            EventSource::AiGuard => "\u{26a0}",
                            _ => "\u{2139}",
                        };
                        println!(
                            "{} [{}] {}: {}",
                            icon,
                            event.timestamp.format("%H:%M:%S"),
                            event.source,
                            event.message
                        );
                        io::stdout().flush().ok();
                    }
                    Err(_) => {
                        // Channel closed
                        println!("Event stream ended.");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn cmd_audit_access(n: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::proxy::read_audit_log;

    let entries = read_audit_log()?;
    if entries.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::json!({"version": 1, "entries": [], "total": 0})
            );
        } else {
            println!("No proxy access audit entries found.");
            println!("Start the proxy with `envforge proxy` to generate audit logs.");
        }
        return Ok(());
    }

    // Take the last N entries
    let start = if entries.len() > n {
        entries.len() - n
    } else {
        0
    };
    let display = &entries[start..];

    if json {
        let items: Vec<serde_json::Value> = display
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "action": e.action,
                    "key": e.key,
                    "keys_served": e.keys_served,
                    "client_addr": e.client_addr,
                    "user_agent": e.user_agent,
                    "granted": e.granted,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "entries": items,
                "total": entries.len(),
            }))?
        );
    } else {
        println!(
            "Proxy Access Audit Log ({} of {} entries)\n",
            display.len(),
            entries.len()
        );
        println!(
            "{:<24} {:<10} {:<20} {:<22} {:<8}",
            "TIMESTAMP", "ACTION", "KEY", "CLIENT", "GRANTED"
        );
        println!("{}", "-".repeat(86));
        for e in display {
            let keys_label = e.keys_served.map(|n| format!("({} keys)", n));
            let key_display = e
                .key
                .as_deref()
                .unwrap_or(keys_label.as_deref().unwrap_or("-"));
            let granted_str = if e.granted { "yes" } else { "NO" };
            println!(
                "{:<24} {:<10} {:<20} {:<22} {:<8}",
                e.timestamp, e.action, key_display, e.client_addr, granted_str
            );
        }
    }
    Ok(())
}

fn cmd_lease(action: &LeaseAction, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::lease;

    match action {
        LeaseAction::Create { name, ttl, keys } => {
            let ttl_seconds = lease::parse_lease_duration(ttl)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            let lease_name = name.clone().unwrap_or_else(|| {
                format!("session-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
            });

            let key_list: Option<Vec<String>> = keys.as_ref().map(|k| {
                k.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            });

            let created = lease::create_lease(&lease_name, ttl_seconds, key_list)?;
            eprintln!("Lease created: {}", created.name);
            eprintln!("  Expires: {}", created.expires_at);
            if let Some(ref keys) = created.keys {
                eprintln!("  Keys: {}", keys.join(", "));
            } else {
                eprintln!("  Keys: ALL");
            }
        }
        LeaseAction::List => {
            let statuses = lease::list_leases()?;
            if statuses.is_empty() {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "version": 1,
                            "leases": [],
                        })
                    );
                } else {
                    eprintln!("No leases found.");
                }
                return Ok(());
            }

            if json {
                let items: Vec<serde_json::Value> = statuses
                .iter()
                .map(|s| {
                    let status_str = if s.revoked {
                        "revoked"
                    } else if s.expired {
                        "expired"
                    } else {
                        "active"
                    };
                    serde_json::json!({
                        "name": s.name,
                        "status": status_str,
                        "remaining_seconds": if s.expired || s.revoked { None } else { Some(s.remaining_seconds) },
                        "key_count": s.key_count,
                    })
                })
                .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "leases": items,
                    }))?
                );
            } else {
                println!("{:<25} {:<12} {:<12} KEYS", "NAME", "STATUS", "REMAINING");
                println!("{}", "-".repeat(65));
                for s in &statuses {
                    let status = if s.revoked {
                        "REVOKED"
                    } else if s.expired {
                        "EXPIRED"
                    } else {
                        "ACTIVE"
                    };
                    let remaining = if s.expired || s.revoked {
                        "-".to_string()
                    } else {
                        format_duration_short(s.remaining_seconds)
                    };
                    let keys = match s.key_count {
                        Some(n) => format!("{} key(s)", n),
                        None => "ALL".to_string(),
                    };
                    println!("{:<25} {:<12} {:<12} {}", s.name, status, remaining, keys);
                }
            }
        }
        LeaseAction::Cleanup => {
            let removed = lease::cleanup_expired()?;
            eprintln!("Cleaned up {} expired/revoked lease(s).", removed);
        }
        LeaseAction::Grant {
            key,
            pid,
            ttl,
            tool,
            multi_redeem,
            json: emit_json,
        } => {
            let ttl_secs_i64 = lease::parse_lease_duration(ttl)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let ttl_secs = u64::try_from(ttl_secs_i64)
                .map_err(|_| -> Box<dyn std::error::Error> { "ttl must be positive".into() })?;
            let req = lease::GrantRequest {
                key: key.clone(),
                pid: *pid,
                ttl_secs,
                tool_name: tool.clone(),
                single_redeem: !multi_redeem,
            };
            let handle = lease::jit_grant(req)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
            if *emit_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "uuid": handle.uuid,
                        "lease_name": handle.lease_name,
                    }))?
                );
            } else {
                // Two-line shell-friendly output: capture with `eval` or `read`.
                println!("{}", handle.uuid);
                println!("{}", handle.lease_name);
            }
        }
        LeaseAction::Revoke { name } => {
            let did = lease::jit_revoke(name, lease::RevokeReason::Explicit)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
            if did {
                eprintln!("revoked lease: {name}");
            } else {
                eprintln!("lease not found: {name}");
                std::process::exit(1);
            }
        }
        LeaseAction::Status {
            name,
            json: emit_json,
        } => {
            let leases = lease::list_leases()?;
            let s = match leases.into_iter().find(|l| &l.name == name) {
                Some(s) => s,
                None => {
                    eprintln!("lease not found: {name}");
                    std::process::exit(1);
                }
            };
            if *emit_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "name": s.name,
                        "expires_at": s.expires_at,
                        "remaining_seconds": s.remaining_seconds,
                        "expired": s.expired,
                        "revoked": s.revoked,
                        "key_count": s.key_count,
                        "pid": s.pid,
                        "redeemed": s.redeemed,
                    }))?
                );
            } else {
                let pid_s = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
                println!("name:      {}", s.name);
                println!("expires:   {}", s.expires_at);
                println!("remaining: {} secs", s.remaining_seconds);
                println!("expired:   {}", s.expired);
                println!("revoked:   {}", s.revoked);
                println!("redeemed:  {}", s.redeemed);
                println!("pid:       {}", pid_s);
                println!(
                    "keys:      {}",
                    s.key_count
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "(all)".into())
                );
            }
        }
    }

    Ok(())
}

fn cmd_session(action: &SessionAction, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::session;

    match action {
        SessionAction::Start { tool, ttl } => {
            let tool_type = if let Some(t) = tool {
                t.parse::<crate::model::AiTool>()
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?
            } else {
                session::detect_ai_tool()
            };

            let ttl_seconds =
                session::parse_ttl(ttl).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            let manager = session::SessionManager::new();
            let s = manager
                .create_session(tool_type, ttl_seconds)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": s.id.as_str(),
                        "tool": s.tool_type.as_str(),
                        "state": s.state.to_string(),
                        "expires_at": s.expires_at.to_rfc3339(),
                    })
                );
            } else {
                eprintln!("Session started: {}", s.id);
                eprintln!("  Tool: {}", s.tool_type);
                eprintln!(
                    "  Expires: {} ({})",
                    s.expires_at.to_rfc3339(),
                    session::format_duration(ttl_seconds as i64)
                );
            }
        }
        SessionAction::Stop { id } => {
            let manager = session::SessionManager::new();
            let target_id = id
                .clone()
                .unwrap_or_else(|| std::env::var("ENVFORGE_SESSION_ID").unwrap_or_default());

            if target_id.is_empty() {
                return Err("No session ID provided and ENVFORGE_SESSION_ID not set.".into());
            }

            let s = manager
                .stop_session(&target_id)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": s.id.as_str(),
                        "state": s.state.to_string(),
                    })
                );
            } else {
                eprintln!("Session stopped: {}", s.id);
            }
        }
        SessionAction::List => {
            let manager = session::SessionManager::new();
            let sessions = manager.list_sessions();

            if sessions.is_empty() {
                if json {
                    println!("{}", serde_json::json!({"version": 1, "sessions": []}));
                } else {
                    eprintln!("No sessions found.");
                }
                return Ok(());
            }

            if json {
                let items: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id.as_str(),
                            "tool": s.tool_type.as_str(),
                            "state": s.state.to_string(),
                            "remaining": s.remaining_seconds,
                            "expires_at": s.expires_at.to_rfc3339(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "version": 1,
                        "sessions": items,
                    }))?
                );
            } else {
                println!(
                    "{:<40} {:<16} {:<12} {:<16}",
                    "ID", "TOOL", "STATE", "REMAINING"
                );
                println!("{}", "-".repeat(90));
                for s in &sessions {
                    let remaining = session::format_duration(s.remaining_seconds);
                    println!(
                        "{:<40} {:<16} {:<12} {:<16}",
                        s.id.as_str(),
                        s.tool_type.as_str(),
                        s.state.to_string(),
                        remaining
                    );
                }
            }
        }
        SessionAction::Show { id } => {
            let manager = session::SessionManager::new();
            let s = manager
                .get_session(id)
                .ok_or_else(|| -> Box<dyn std::error::Error> {
                    format!("Session not found: {}", id).into()
                })?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": s.id.as_str(),
                        "tool": s.tool_type.as_str(),
                        "state": s.state.to_string(),
                        "created_at": s.created_at.to_rfc3339(),
                        "expires_at": s.expires_at.to_rfc3339(),
                        "remaining_seconds": s.remaining_seconds(),
                    })
                );
            } else {
                println!("Session: {}", s.id);
                println!("  Tool:      {}", s.tool_type);
                println!("  State:     {}", s.state);
                println!("  Created:   {}", s.created_at.to_rfc3339());
                println!("  Expires:   {}", s.expires_at.to_rfc3339());
                println!(
                    "  Remaining: {}",
                    session::format_duration(s.remaining_seconds())
                );
            }
        }
        SessionAction::Cleanup => {
            let manager = session::SessionManager::new();
            let removed = manager.cleanup_expired();
            eprintln!("Cleaned up {} expired session(s).", removed);
        }
    }

    Ok(())
}

fn cmd_revoke(all: bool, name: Option<&str>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::lease;

    if all {
        let count = lease::revoke_all_leases()?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "action": "revoke_all",
                    "count": count,
                    "message": format!("{} lease(s) revoked", count),
                }))?
            );
        } else {
            eprintln!("KILLSWITCH: {} lease(s) revoked and removed.", count);
        }
    } else if let Some(lease_name) = name {
        let found = lease::revoke_lease(lease_name)?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "action": "revoke",
                    "lease": lease_name,
                    "found": found,
                    "message": if found {
                        format!("Revoked lease: {}", lease_name)
                    } else {
                        format!("Lease not found: {}", lease_name)
                    },
                }))?
            );
        } else if found {
            eprintln!("Revoked lease: {}", lease_name);
        } else {
            eprintln!("Lease not found: {}", lease_name);
        }
    } else {
        return Err(
            "Specify --all or a lease name. Usage: envforge revoke --all OR envforge revoke <name>"
                .into(),
        );
    }

    Ok(())
}

fn format_duration_short(seconds: i64) -> String {
    if seconds <= 0 {
        return "expired".to_string();
    }
    let hours = seconds / 3600;
    let mins = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

// ─── Deps command ─────────────────────────────────────────

fn cmd_deps(key: &str, include_source: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::deps::{find_dependencies, group_by_type};
    use std::collections::HashSet;

    let config = load_or_create_default()?;
    let project_dir = std::env::current_dir()?;

    let mut managed_files = Vec::new();
    managed_files.push(shellexpand(&config.files.primary));
    if config.files.use_reference_file {
        managed_files.push(shellexpand(&config.files.reference));
    }
    managed_files.push(shellexpand(&config.profiles.shared_file));
    for entry in config.profiles.entries.values() {
        managed_files.push(shellexpand(&entry.file));
    }

    let refs = find_dependencies(key, &project_dir, include_source, &managed_files)?;

    if json {
        let json_refs: Vec<serde_json::Value> = refs
            .iter()
            .map(|dep| {
                let display_path = dep.file.strip_prefix(&project_dir).unwrap_or(&dep.file);
                serde_json::json!({
                    "file": display_path.to_string_lossy(),
                    "line": dep.line,
                    "context": dep.context,
                    "type": dep.ref_type.to_string(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "key": key,
                "references": json_refs,
                "total": refs.len(),
            }))?
        );
    } else if refs.is_empty() {
        println!("No references found for {}", key);
    } else {
        println!("Dependencies for {}\n", key);

        let grouped = group_by_type(&refs);
        let mut total_files = HashSet::new();

        for (ref_type, items) in &grouped {
            println!("{}:", ref_type);
            for dep in items {
                let display_path = dep.file.strip_prefix(&project_dir).unwrap_or(&dep.file);
                println!(
                    " {}:{} {}",
                    display_path.display(),
                    dep.line,
                    truncate_context(&dep.context, 60)
                );
                total_files.insert(dep.file.clone());
            }
            println!();
        }

        println!(
            "Total: {} references across {} files",
            refs.len(),
            total_files.len()
        );
    }

    Ok(())
}

fn cmd_undo(list: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if list {
        let config = load_or_create_default()?;
        let primary = shellexpand(&config.files.primary);
        let backups = list_backups(&primary)?;

        if json {
            let json_backups: Vec<serde_json::Value> = backups
                .iter()
                .map(|b| {
                    serde_json::json!({
                        "path": b.to_string_lossy(),
                        "size": std::fs::metadata(b).map(|m| m.len()).unwrap_or(0),
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "snapshots": json_backups,
                }))?
            );
        } else if backups.is_empty() {
            println!("No undo snapshots available.");
        } else {
            println!("Available undo snapshots:\n");
            for b in &backups {
                let name = b.file_name().unwrap_or_default().to_string_lossy();
                let size = std::fs::metadata(b).map(|m| m.len()).unwrap_or(0);
                println!("  {} ({} bytes)", name, size);
            }
        }
        return Ok(());
    }

    let config = load_or_create_default()?;
    let primary = shellexpand(&config.files.primary);
    let backups = list_backups(&primary)?;

    if let Some(latest) = backups.last() {
        let backup_content = std::fs::read_to_string(latest)?;
        crate::config::safe_write(&primary, &backup_content, None)?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "action": "undo",
                    "restored_from": latest.to_string_lossy(),
                    "target": primary.to_string_lossy(),
                }))?
            );
        } else {
            println!(
                "Undone: restored {} from {}",
                primary.display(),
                latest.display()
            );
        }
    } else if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1,
                "action": "undo",
                "error": "No backup snapshots available",
            }))?
        );
    } else {
        eprintln!("No backup snapshots available for undo.");
    }

    Ok(())
}

fn cmd_offset(show: bool, suggest: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::offset;

    let (config, shell_files) = load_context()?;

    if shell_files.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "version": 1,
                    "error": "No shell files found",
                }))?
            );
        } else {
            eprintln!("No shell files found.");
        }
        return Ok(());
    }

    let sf = &shell_files[0];

    let zone = offset::find_managed_zone(sf);
    let blocks = if show {
        offset::detect_protected_blocks(sf)
    } else {
        vec![]
    };
    let (header, footer) = if suggest || show {
        offset::suggest_offsets(sf)
    } else {
        (
            config.offsets.header_protected_lines,
            config.offsets.footer_protected_lines,
        )
    };

    if json {
        let json_blocks: Vec<serde_json::Value> = blocks
            .iter()
            .map(|b| {
                serde_json::json!({
                    "name": b.name,
                    "start_line": b.start_line,
                    "end_line": b.end_line,
                })
            })
            .collect();

        let mut obj = serde_json::json!({
            "version": 1,
            "file": sf.path.to_string_lossy(),
            "total_lines": sf.lines.len(),
            "header_offset": header,
            "footer_offset": footer,
            "managed_zone": zone.as_ref().map(|z| serde_json::json!({
                "start_idx": z.start_idx,
                "end_idx": z.end_idx,
            })),
        });

        if show {
            obj["protected_blocks"] = serde_json::json!(json_blocks);
        }

        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        if let Some(z) = &zone {
            println!("Managed zone: lines {}–{}", z.start_idx + 1, z.end_idx + 1);
        } else {
            println!("No managed zone detected.");
        }

        if show && !blocks.is_empty() {
            println!("\nProtected blocks:");
            for b in &blocks {
                println!(
                    "  {} (lines {}–{})",
                    b.name,
                    b.start_line + 1,
                    b.end_line + 1
                );
            }
        }

        println!("\nHeader offset: {}", header);
        println!("Footer offset: {}", footer);
    }

    Ok(())
}

fn cmd_man(command: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::man::{format_man_index, format_man_page, load_man_pages, suggest_similar};

    let pages = load_man_pages();

    if command.is_empty() {
        // Show index
        print!("{}", format_man_index(&pages));
        return Ok(());
    }

    // Build query: "sync push" from ["sync", "push"]
    let query = command.join(" ");

    // Try exact match (short name)
    if let Some(page) = pages.get(&query) {
        print!("{}", format_man_page(page));
        return Ok(());
    }

    // Try "envforge <query>"
    let full = format!("envforge {}", query);
    if let Some(page) = pages.get(&full) {
        print!("{}", format_man_page(page));
        return Ok(());
    }

    // Not found — suggest similar
    let suggestions = suggest_similar(&query, &pages);
    eprintln!("No man page for '{}'.", query);
    if !suggestions.is_empty() {
        eprintln!("\nDid you mean:");
        for s in &suggestions {
            eprintln!("  envforge man {}", s);
        }
    }
    eprintln!("\nRun 'envforge man' for the full command index.");
    process::exit(1);
}

fn truncate_context(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ─── Project Commands ──────────────────────────────────────

fn cmd_project(
    action: &ProjectAction,
    json: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::project;

    let cwd = std::env::current_dir()?;

    match action {
        ProjectAction::Init { format, force } => {
            let format = project::ConfigFormat::parse(format)?;
            let project_name = project::derive_project_name(&cwd);

            if dry_run {
                let filename = format.default_filename();
                println!("Would create: {}/{}", cwd.display(), filename);
                println!("Would create: {}/.env.development", cwd.display());
                return Ok(());
            }

            let opts = project::InitOptions {
                root: cwd.clone(),
                format,
                project_name: project_name.clone(),
                default_env_name: "development".to_string(),
                env_file_path: ".env.development".into(),
                schema_path: ".env.schema".into(),
                force: *force,
            };

            let result = project::init_project(&opts)?;

            if json {
                let out = serde_json::json!({
                    "config_path": result.config_path.display().to_string(),
                    "env_file": result.env_file_path.display().to_string(),
                    "project_name": result.project_name,
                    "environment": result.environment_name,
                    "format": format!("{:?}", result.format).to_lowercase(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Project initialized: {}", project_name);
                println!("  Config: {}", result.config_path.display());
                println!("  Env:    {}", result.env_file_path.display());
                println!("  Format: {:?}", result.format);
                println!();

                // Offer .gitignore
                match project::add_to_gitignore(&cwd) {
                    Ok(true) => println!("  Added .env.* patterns to .gitignore"),
                    Ok(false) => println!("  .gitignore already has .env.* patterns"),
                    Err(e) => eprintln!("  Warning: could not update .gitignore: {}", e),
                }
            }
            Ok(())
        }

        ProjectAction::Config { set } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let mut config = project::load_project_config(&detected)?;

            if let Some(kv) = set {
                if dry_run {
                    println!("Would set: {}", kv);
                    return Ok(());
                }
                let parts: Vec<&str> = kv.splitn(2, '=').collect();
                if parts.len() != 2 {
                    return Err("Expected format: key=value".into());
                }
                match parts[0] {
                    "name" => config.project.name = parts[1].to_string(),
                    "schema_path" => config.project.schema_path = parts[1].into(),
                    "active_environment" => {
                        project::find_environment(&config, parts[1])?;
                        config.project.active_environment = parts[1].to_string();
                    }
                    other => return Err(format!("Unknown config key: {}", other).into()),
                }
                project::save_project_config(&config, &detected.config_path, detected.format)?;
                println!("Updated: {} = {}", parts[0], parts[1]);
            } else if json {
                let serialized =
                    project::serialize_project_config(&config, project::ConfigFormat::Json)?;
                println!("{}", serialized);
            } else {
                println!("Project: {}", config.project.name);
                println!("Schema:  {}", config.project.schema_path.display());
                println!("Active:  {}", config.project.active_environment);
                println!("Format:  {:?}", detected.format);
                println!("Config:  {}", detected.config_path.display());
                println!();
                println!("Environments:");
                for env in &config.environments {
                    let marker = if env.name == config.project.active_environment {
                        " *"
                    } else {
                        ""
                    };
                    println!("  {} ({}){}", env.name, env.env_file.display(), marker);
                }
            }
            Ok(())
        }

        ProjectAction::Status => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;

            let env_path = project::active_env_path(&config, &detected.project_root)?;
            let key_count = if env_path.exists() {
                std::fs::read_to_string(&env_path)
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| {
                        let t = l.trim();
                        !t.is_empty() && !t.starts_with('#')
                    })
                    .count()
            } else {
                0
            };

            let schema_exists = detected
                .project_root
                .join(&config.project.schema_path)
                .exists();

            if json {
                let out = serde_json::json!({
                    "project_name": config.project.name,
                    "active_environment": config.project.active_environment,
                    "environments": config.environments.len(),
                    "key_count": key_count,
                    "schema_exists": schema_exists,
                    "config_path": detected.config_path.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Project: {}", config.project.name);
                println!(
                    "Active:  {} ({} keys)",
                    config.project.active_environment, key_count
                );
                println!("Envs:    {}", config.environments.len());
                println!(
                    "Schema:  {}",
                    if schema_exists { "found" } else { "missing" }
                );
                println!("Config:  {}", detected.config_path.display());
            }
            Ok(())
        }

        ProjectAction::Wizard { force } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;

            let report = project::run_wizard(&cwd, &detected, *force, dry_run)?;

            if json {
                let out = serde_json::json!({
                    "steps_run": report.steps_run,
                    "schema_keys": report.schema_keys,
                    "values_set": report.values_set,
                    "values_skipped": report.values_skipped,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!();
                println!("Wizard complete:");
                println!("  Steps run: {}", report.steps_run.join(", "));
                println!("  Schema keys: {}", report.schema_keys);
                println!(
                    "  Values: {} set, {} skipped",
                    report.values_set, report.values_skipped
                );
            }
            Ok(())
        }
        ProjectAction::Env { action } => cmd_project_env(action, json, dry_run),
        ProjectAction::Validate { environment } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;

            // Determine which env to validate
            let env_name = environment
                .as_deref()
                .unwrap_or(&config.project.active_environment);
            let env_entry = project::find_environment(&config, env_name)?;
            let env_path = detected.project_root.join(&env_entry.env_file);
            let schema_path = detected.project_root.join(&config.project.schema_path);

            if !schema_path.exists() {
                return Err(Box::new(project::ProjectError::SchemaNotFound {
                    path: schema_path,
                }));
            }

            // Parse schema
            let schema = crate::ops::schema::parse_schema(&schema_path)?;

            // Parse env file into HashMap
            let env_map = project::parse_dotenv_simple(&env_path)?;

            // Validate
            let errors = crate::ops::schema::validate_against_schema(
                &env_map,
                &schema,
                environment.as_deref(),
                &std::collections::HashMap::new(),
            );

            if json {
                let err_list: Vec<serde_json::Value> = errors
                    .iter()
                    .map(|e| serde_json::json!({"key": e.key, "message": e.message}))
                    .collect();
                let out = serde_json::json!({
                    "environment": env_name,
                    "valid": errors.is_empty(),
                    "errors": err_list,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else if errors.is_empty() {
                println!("Validation passed: {} ({} keys)", env_name, env_map.len());
            } else {
                eprintln!(
                    "Validation failed for '{}': {} error(s)",
                    env_name,
                    errors.len()
                );
                for err in &errors {
                    eprintln!("  - {}: {}", err.key, err.message);
                }
                std::process::exit(1);
            }
            Ok(())
        }

        ProjectAction::Scan { staged, mcp } => {
            if *mcp {
                let findings = crate::ops::mcp_scan::scan_mcp_configs();
                if json {
                    let out: Vec<serde_json::Value> = findings
                        .iter()
                        .map(|f| serde_json::json!({"file": f.file.display().to_string(), "key": f.key, "pattern": f.pattern}))
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else if findings.is_empty() {
                    println!("No credentials found in MCP configs.");
                } else {
                    for f in &findings {
                        eprintln!("  {} : {} = {}", f.file.display(), f.key, f.value_preview);
                    }
                    eprintln!("{} credential(s) found", findings.len());
                    std::process::exit(1);
                }
            } else {
                // Build EnvEntry list from project env for value-based scanning
                let detected = project::detect_project_config(&cwd)
                    .ok_or(project::ProjectError::ConfigNotFound)?;
                let config = project::load_project_config(&detected)?;
                let env_path = project::active_env_path(&config, &detected.project_root)?;
                let env_map = project::parse_dotenv_simple(&env_path)?;

                // Build minimal EnvEntry structs for scanner
                let entries: Vec<EnvEntry> = env_map
                    .iter()
                    .map(|(k, v)| EnvEntry {
                        key: k.clone(),
                        value: v.clone(),
                        source_file: env_path.clone(),
                        line_number: 0,
                        line_index: 0,
                        location: EntryLocation::InFile,
                        export_style: ExportStyle::Bare,
                        quote_style: QuoteStyle::None,
                        is_dirty: false,
                    })
                    .collect();

                if *staged {
                    let matches = crate::ops::scanner::scan_staged(&entries)?;
                    if matches.is_empty() {
                        println!("No secrets found in staged files.");
                    } else {
                        for m in &matches {
                            eprintln!(
                                "  {}:{}: {}",
                                m.file.display(),
                                m.line_number,
                                m.matched_key
                            );
                        }
                        std::process::exit(1);
                    }
                } else {
                    let matches = crate::ops::scanner::scan_directory(&cwd, &entries)?;
                    if matches.is_empty() {
                        println!("No secrets found.");
                    } else {
                        for m in &matches {
                            eprintln!(
                                "  {}:{}: {}",
                                m.file.display(),
                                m.line_number,
                                m.matched_key
                            );
                        }
                        eprintln!("{} secret(s) found", matches.len());
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }

        ProjectAction::Schema { action } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;

            match action {
                super::ProjectSchemaAction::Generate { output } => {
                    let env_path = project::active_env_path(&config, &detected.project_root)?;
                    let env_map = project::parse_dotenv_simple(&env_path)?;
                    let schema_content = crate::ops::schema::generate_schema(&env_map);

                    let out_path = output
                        .as_ref()
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| detected.project_root.join(&config.project.schema_path));

                    if dry_run {
                        println!("Would write schema to: {}", out_path.display());
                        print!("{}", schema_content);
                    } else {
                        std::fs::write(&out_path, &schema_content)?;
                        println!(
                            "Schema generated: {} ({} keys)",
                            out_path.display(),
                            env_map.len()
                        );
                    }
                }
                super::ProjectSchemaAction::EmitAi { output, infer } => {
                    let schema = if *infer {
                        None
                    } else {
                        let sp = detected.project_root.join(&config.project.schema_path);
                        if sp.exists() {
                            Some(crate::ops::schema::parse_schema(&sp)?)
                        } else {
                            None
                        }
                    };

                    let env_path = project::active_env_path(&config, &detected.project_root)?;
                    let env_map = project::parse_dotenv_simple(&env_path)?;
                    let entries: Vec<(String, String)> = env_map.into_iter().collect();

                    let content = crate::ops::schema::emit_ai_context(schema.as_ref(), &entries);

                    if let Some(out) = output {
                        if dry_run {
                            println!("Would write AI context to: {}", out);
                        } else {
                            std::fs::write(out, &content)?;
                            println!("AI context written to: {}", out);
                        }
                    } else {
                        print!("{}", content);
                    }
                }
            }
            Ok(())
        }

        ProjectAction::Fence => {
            crate::ops::fence::create_fence(&cwd, dry_run)?;
            Ok(())
        }

        ProjectAction::Sanitize { file, output } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;
            let env_path = project::active_env_path(&config, &detected.project_root)?;
            let env_map = project::parse_dotenv_simple(&env_path)?;
            let secrets: Vec<(String, String)> = env_map.into_iter().collect();

            let content = std::fs::read_to_string(file)?;
            let (sanitized, count) = crate::ops::sanitize::sanitize_content(&content, &secrets);

            if let Some(out) = output {
                if dry_run {
                    println!("Would sanitize {} ({} replacements) → {}", file, count, out);
                } else {
                    std::fs::write(out, &sanitized)?;
                    println!("Sanitized: {} replacements → {}", count, out);
                }
            } else {
                print!("{}", sanitized);
            }
            Ok(())
        }

        ProjectAction::Export {
            path,
            safe,
            format,
            filter,
        } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;
            let env_path = project::active_env_path(&config, &detected.project_root)?;
            let env_map = project::parse_dotenv_simple(&env_path)?;

            let mut entries: Vec<(String, String)> = env_map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            // Apply filter
            if let Some(pattern) = filter {
                let pat = pattern.replace('*', "");
                entries.retain(|(k, _)| k.contains(&pat));
            }

            // Redact if --safe
            if *safe {
                let sensitive = ["SECRET", "TOKEN", "PASSWORD", "KEY", "CREDENTIAL"];
                for entry in &mut entries {
                    if sensitive.iter().any(|s| entry.0.to_uppercase().contains(s)) {
                        entry.1 = "[REDACTED]".to_string();
                    }
                }
            }

            // Format output
            let output_str = match format.as_deref() {
                Some("json") => {
                    let map: serde_json::Map<String, serde_json::Value> = entries
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    serde_json::to_string_pretty(&map)?
                }
                Some("yaml" | "yml") => entries
                    .iter()
                    .map(|(k, v)| format!("{}: \"{}\"", k, v.replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => entries
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n"),
            };

            if let Some(p) = path {
                if dry_run {
                    println!("Would export to: {}", p);
                } else {
                    std::fs::write(p, &output_str)?;
                    println!("Exported {} keys to {}", entries.len(), p);
                }
            } else {
                println!("{}", output_str);
            }
            Ok(())
        }

        ProjectAction::Pull {
            from,
            path,
            filter,
            environment,
        } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;

            let env_name = environment
                .as_deref()
                .unwrap_or(&config.project.active_environment);
            let env_entry = project::find_environment(&config, env_name)?;
            let env_path = detected.project_root.join(&env_entry.env_file);

            // Use existing secrets pull logic
            let registry = crate::ops::secrets::providers::create_default_registry();
            let provider = registry.get(from)?;
            let creds = crate::ops::secrets::credentials::read_all_credentials(from)?;
            let mut secrets = provider.pull(&creds, path)?;

            // Apply filter
            if let Some(pattern) = filter {
                let pat = pattern.replace('*', "");
                secrets.retain(|(k, _)| k.contains(&pat));
            }

            if dry_run {
                println!(
                    "Would pull {} keys into {}",
                    secrets.len(),
                    env_path.display()
                );
                for (k, _) in &secrets {
                    println!("  {}", k);
                }
                return Ok(());
            }

            // Write to project .env file
            let mut content = String::new();
            content.push_str(&format!("# Pulled from {} at {}\n", from, path));
            for (k, v) in &secrets {
                if v.contains(' ') || v.contains('"') {
                    content.push_str(&format!("{}=\"{}\"\n", k, v.replace('"', "\\\"")));
                } else {
                    content.push_str(&format!("{}={}\n", k, v));
                }
            }
            std::fs::write(&env_path, &content)?;

            if json {
                let out = serde_json::json!({
                    "provider": from,
                    "environment": env_name,
                    "keys": secrets.len(),
                    "env_file": env_path.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "Pulled {} keys from {} into {} ({})",
                    secrets.len(),
                    from,
                    env_name,
                    env_path.display()
                );
            }
            Ok(())
        }

        ProjectAction::Push {
            to,
            path,
            keys,
            all,
            filter,
        } => {
            let detected = project::detect_project_config(&cwd)
                .ok_or(project::ProjectError::ConfigNotFound)?;
            let config = project::load_project_config(&detected)?;
            let env_path = project::active_env_path(&config, &detected.project_root)?;
            let env_map = project::parse_dotenv_simple(&env_path)?;

            let mut secrets: Vec<(String, String)> = env_map.into_iter().collect();
            secrets.sort_by(|a, b| a.0.cmp(&b.0));

            // Filter
            if let Some(key_list) = keys {
                let wanted: Vec<&str> = key_list.split(',').map(|s| s.trim()).collect();
                secrets.retain(|(k, _)| wanted.contains(&k.as_str()));
            } else if let Some(pattern) = filter {
                let pat = pattern.replace('*', "");
                secrets.retain(|(k, _)| k.contains(&pat));
            } else if !all {
                return Err("Specify --keys, --filter, or --all".into());
            }

            if dry_run {
                println!("Would push {} keys to {} ({})", secrets.len(), to, path);
                for (k, _) in &secrets {
                    println!("  {}", k);
                }
                return Ok(());
            }

            let registry = crate::ops::secrets::providers::create_default_registry();
            let provider = registry.get(to)?;
            let creds = crate::ops::secrets::credentials::read_all_credentials(to)?;
            let count = provider.push(&creds, path, &secrets)?;

            if json {
                let out = serde_json::json!({"provider": to, "pushed": count});
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Pushed {} keys to {} ({})", count, to, path);
            }
            Ok(())
        }
    }
}

fn cmd_project_env(
    action: &ProjectEnvAction,
    json: bool,
    _dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::project;

    let cwd = std::env::current_dir()?;
    let detected =
        project::detect_project_config(&cwd).ok_or(project::ProjectError::ConfigNotFound)?;
    let mut config = project::load_project_config(&detected)?;

    match action {
        ProjectEnvAction::Create { name, description } => {
            project::validate_env_name(name)?;

            if config.environments.iter().any(|e| e.name == *name) {
                return Err(Box::new(project::ProjectError::EnvironmentExists {
                    name: name.clone(),
                }));
            }

            let env_file = format!(".env.{}", name);
            config.environments.push(project::ProjectEnvironment {
                name: name.clone(),
                env_file: env_file.clone().into(),
                description: description.clone(),
            });

            project::save_project_config(&config, &detected.config_path, detected.format)?;

            // Create empty .env file
            let env_path = detected.project_root.join(&env_file);
            if !env_path.exists() {
                std::fs::write(
                    &env_path,
                    format!("# EnvForge project environment: {}\n", name),
                )?;
            }

            if json {
                let out = serde_json::json!({"created": name, "env_file": env_file});
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Environment created: {} ({})", name, env_file);
            }
        }

        ProjectEnvAction::List => {
            if json {
                let envs: Vec<serde_json::Value> = config
                    .environments
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "name": e.name,
                            "env_file": e.env_file.display().to_string(),
                            "active": e.name == config.project.active_environment,
                            "description": e.description,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&envs)?);
            } else {
                for env in &config.environments {
                    let marker = if env.name == config.project.active_environment {
                        " *"
                    } else {
                        ""
                    };
                    let desc = env
                        .description
                        .as_deref()
                        .map(|d| format!(" — {}", d))
                        .unwrap_or_default();
                    println!(
                        "  {} ({}){}{}",
                        env.name,
                        env.env_file.display(),
                        marker,
                        desc
                    );
                }
            }
        }

        ProjectEnvAction::Switch { name } => {
            project::find_environment(&config, name)?;
            config.project.active_environment = name.clone();
            project::save_project_config(&config, &detected.config_path, detected.format)?;

            let env_file = config
                .environments
                .iter()
                .find(|e| e.name == *name)
                .map(|e| e.env_file.display().to_string())
                .unwrap_or_default();

            if json {
                let out = serde_json::json!({"active": name, "env_file": env_file});
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Switched to '{}' ({})", name, env_file);
            }
        }

        ProjectEnvAction::Delete { name } => {
            if *name == config.project.active_environment {
                return Err("Cannot delete the active environment. Switch first.".into());
            }
            project::find_environment(&config, name)?;
            config.environments.retain(|e| e.name != *name);
            project::save_project_config(&config, &detected.config_path, detected.format)?;

            if json {
                let out = serde_json::json!({"deleted": name});
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("Environment deleted: {}", name);
            }
        }

        ProjectEnvAction::Diff { a, b } => {
            let env_a = project::find_environment(&config, a)?;
            let env_b = project::find_environment(&config, b)?;

            let path_a = detected.project_root.join(&env_a.env_file);
            let path_b = detected.project_root.join(&env_b.env_file);

            // Reuse existing dotenv parsing
            let parse_env = |p: &std::path::Path| -> Vec<(String, String)> {
                std::fs::read_to_string(p)
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|l| {
                        let t = l.trim();
                        if t.is_empty() || t.starts_with('#') {
                            return None;
                        }
                        let stripped = t.strip_prefix("export ").unwrap_or(t);
                        let mut parts = stripped.splitn(2, '=');
                        let key = parts.next()?.trim().to_string();
                        let val = parts
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                        Some((key, val))
                    })
                    .collect()
            };

            let vars_a: std::collections::HashMap<String, String> =
                parse_env(&path_a).into_iter().collect();
            let vars_b: std::collections::HashMap<String, String> =
                parse_env(&path_b).into_iter().collect();

            let mut all_keys: Vec<&String> = vars_a.keys().chain(vars_b.keys()).collect();
            all_keys.sort();
            all_keys.dedup();

            let mut same = 0;
            let mut changed = 0;
            let mut only_a = 0;
            let mut only_b = 0;

            if !json {
                println!("{:<30} {:<20} {:<20}", "KEY", a, b);
                println!("{}", "-".repeat(70));
            }

            let mut rows = Vec::new();
            for key in &all_keys {
                match (vars_a.get(*key), vars_b.get(*key)) {
                    (Some(va), Some(vb)) if va == vb => {
                        same += 1;
                    }
                    (Some(va), Some(vb)) => {
                        changed += 1;
                        if !json {
                            println!(
                                "~ {:<28} {:<20} {:<20}",
                                key,
                                truncate_context(va, 18),
                                truncate_context(vb, 18)
                            );
                        }
                        rows.push(
                            serde_json::json!({"key": key, "status": "changed", "a": va, "b": vb}),
                        );
                    }
                    (Some(va), None) => {
                        only_a += 1;
                        if !json {
                            println!(
                                "- {:<28} {:<20} {:<20}",
                                key,
                                truncate_context(va, 18),
                                "(missing)"
                            );
                        }
                        rows.push(serde_json::json!({"key": key, "status": "only_a", "a": va}));
                    }
                    (None, Some(vb)) => {
                        only_b += 1;
                        if !json {
                            println!(
                                "+ {:<28} {:<20} {:<20}",
                                key,
                                "(missing)",
                                truncate_context(vb, 18)
                            );
                        }
                        rows.push(serde_json::json!({"key": key, "status": "only_b", "b": vb}));
                    }
                    (None, None) => {}
                }
            }

            if json {
                let out = serde_json::json!({
                    "a": a, "b": b,
                    "same": same, "changed": changed,
                    "only_a": only_a, "only_b": only_b,
                    "differences": rows,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!();
                println!(
                    "{} same, {} changed, {} only in {}, {} only in {}",
                    same, changed, only_a, a, only_b, b
                );
            }
        }
    }

    Ok(())
}

fn cmd_ci_trust(action: &super::CiTrustAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::ci_trust;

    match action {
        super::CiTrustAction::Classify { json } => {
            let v = ci_trust::cached_or_compute();
            if *json {
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                let level = match v.level {
                    ci_trust::TrustLevel::Trusted => "Trusted",
                    ci_trust::TrustLevel::Suspicious => "Suspicious",
                    ci_trust::TrustLevel::Untrusted => "Untrusted",
                };
                println!("level={level}");
                println!("reason={:?}", v.reason);
            }
        }
        super::CiTrustAction::Quarantine {
            force,
            off,
            allow_key,
            json,
        } => {
            let verdict = ci_trust::cached_or_compute();
            let apply = if *off {
                false
            } else if *force {
                true
            } else {
                matches!(verdict.level, ci_trust::TrustLevel::Untrusted)
            };
            let source = if *force {
                ci_trust::DecisionSource::Cli
            } else if *off {
                ci_trust::DecisionSource::Off
            } else {
                ci_trust::DecisionSource::Auto
            };
            let decision = ci_trust::QuarantineDecision {
                apply,
                allow_keys: allow_key.clone(),
                source,
            };
            let env: std::collections::HashMap<String, String> = std::env::vars().collect();
            let (scrubbed_env, report) = ci_trust::apply(&env, &decision);

            // Cache the report alongside the verdict so `summary` can pick it up.
            if let Some(p) = std::env::var_os("RUNNER_TEMP") {
                let path = std::path::PathBuf::from(p).join("envforge-scrub-report.json");
                let _ = std::fs::write(
                    &path,
                    serde_json::to_string_pretty(&report).unwrap_or_default(),
                );
            }

            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if apply {
                // Emit `unset KEY` for scrubbed keys plus `export KEY='VALUE'` for
                // preserved keys so `eval "$(envforge ci-trust quarantine)"` in a
                // parent shell installs the scrubbed environment in place.
                for k in &report.scrubbed_keys {
                    println!("unset {k}");
                }
                let mut sorted: Vec<_> = scrubbed_env.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in sorted {
                    let q = v.replace('\'', "'\\''");
                    println!("export {k}='{q}'");
                }
            }
        }
        super::CiTrustAction::Summary => {
            let v = ci_trust::cached_or_compute();
            let report: Option<ci_trust::ScrubReport> =
                if let Some(p) = std::env::var_os("RUNNER_TEMP") {
                    let path = std::path::PathBuf::from(p).join("envforge-scrub-report.json");
                    std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                } else {
                    None
                };
            let s = ci_trust::render_step_summary(&v, report.as_ref());
            print!("{s}");
            ci_trust::emit_action_outputs(&v, report.as_ref())?;
        }
    }
    Ok(())
}

fn cmd_envbom(action: &super::EnvbomAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::envbom;

    match action {
        super::EnvbomAction::Emit {
            profile,
            output,
            reproducible_now,
        } => {
            let project_id = std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "envforge-project".into());

            let pairs: Vec<(String, Option<String>)> =
                std::env::vars().map(|(k, v)| (k, Some(v))).collect();

            let bom = envbom::build_bom(
                &project_id,
                profile.as_deref(),
                pairs,
                reproducible_now.as_deref(),
            )
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

            let bytes = envbom::canonical_json(&bom)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

            match output {
                Some(p) => {
                    std::fs::write(p, &bytes)?;
                    eprintln!("wrote BOM to {}", p.display());
                }
                None => {
                    use std::io::Write;
                    std::io::stdout().write_all(&bytes)?;
                }
            }
        }
        super::EnvbomAction::Verify {
            path,
            against_current,
            strict_schema,
            strict_current,
            json,
        } => {
            let current = if *against_current {
                let project_id = std::env::current_dir()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "envforge-project".into());
                let pairs: Vec<(String, Option<String>)> =
                    std::env::vars().map(|(k, v)| (k, Some(v))).collect();
                let bom = envbom::build_bom(&project_id, None, pairs, None)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
                Some(bom)
            } else {
                None
            };

            let opts = envbom::VerifyOptions {
                against_current: current,
                strict_schema: *strict_schema,
                strict_current: *strict_current,
            };

            let report = match envbom::verify(path, &opts) {
                Ok(r) => r,
                Err(envbom::EnvbomError::InvalidBom(msg)) => {
                    eprintln!("structural fail: {msg}");
                    std::process::exit(1);
                }
                Err(envbom::EnvbomError::VerificationFailed(msg)) => {
                    eprintln!("verify failed: {msg}");
                    std::process::exit(2);
                }
                Err(e) => return Err(e.to_string().into()),
            };

            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "structural: {}",
                    if report.structural_ok { "ok" } else { "fail" }
                );
                if let Some(d) = &report.diff {
                    println!(
                        "diff:       added={} removed={} changed={} unchanged={}",
                        d.added.len(),
                        d.removed.len(),
                        d.changed.len(),
                        d.unchanged_count
                    );
                }
                for w in &report.warnings {
                    eprintln!("warning: {w}");
                }
            }

            if *strict_current {
                if let Some(d) = &report.diff {
                    if !d.added.is_empty() || !d.removed.is_empty() || !d.changed.is_empty() {
                        std::process::exit(4);
                    }
                }
            }
        }
    }
    Ok(())
}
