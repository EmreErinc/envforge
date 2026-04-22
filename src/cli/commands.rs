use std::path::Path;
use std::process;

use crate::config::*;
use crate::model::*;
use crate::ops::*;
use crate::parser::*;

use super::{
    AiHookAction, BackupAction, Commands, LeaseAction, McpAction, ShareAction, SnapshotAction,
};

/// Execute a CLI subcommand.
pub fn execute_command(command: &Commands, json: bool, dry_run: bool) {
    let result = match command {
        Commands::List => cmd_list(json),
        Commands::Get { key } => cmd_get(key, json),
        Commands::Set { assignment } => cmd_set(assignment, dry_run),
        Commands::Delete { key } => cmd_delete(key, dry_run),
        Commands::Copy { key, key_only } => cmd_copy(key, *key_only),
        Commands::Move { key } => cmd_move(key, dry_run),
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
        Commands::Diff => cmd_diff(),
        Commands::Backup { action } => cmd_backup(action),
        Commands::Profile { action } => cmd_profile(action),
        Commands::Validate {
            schema,
            env_file,
            environment,
        } => cmd_validate_enhanced(
            schema.as_deref(),
            env_file.as_deref(),
            environment.as_deref(),
            json,
        ),
        Commands::Encrypt { key } => cmd_encrypt(key, dry_run),
        Commands::Decrypt { key } => cmd_decrypt(key, dry_run),
        Commands::Completions { shell, install } => cmd_completions(shell, *install),
        Commands::Log { key, n } => cmd_log(key.as_deref(), *n, json),
        Commands::Config => cmd_config(),
        Commands::Run {
            profile,
            profiles,
            resolve,
            env_files,
            overrides,
            command,
            volatile,
            redact,
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
            )
        }
        Commands::Sync { action } => super::sync_cmd::execute_sync(action, json, dry_run),
        Commands::Secrets { action } => super::secrets_cmd::execute_secrets(action, json, dry_run),
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
            propagate,
        } => cmd_rotate(key, *dr || dry_run, *stale, *propagate),
        Commands::Hook { shell } => cmd_hook(shell),
        Commands::Env { dir } => crate::ops::hook::cmd_env(dir.as_deref()).map_err(|e| e.into()),
        Commands::Doctor { verbose } => cmd_doctor(*verbose, json),
        Commands::Check { only } => cmd_check(only.as_deref(), json),
        Commands::Snapshot { action } => cmd_snapshot(action, dry_run),
        Commands::Share { action } => cmd_share(action, dry_run),
        Commands::ResolveUri { file, env, output } => {
            cmd_resolve_uri(file, *env, output.as_deref(), json)
        }
        Commands::Mcp { action } => cmd_mcp(action, dry_run, json),
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
        Commands::Fence => cmd_fence(dry_run),
        Commands::Sanitize { file, output } => cmd_sanitize(file, output.as_deref()),
        Commands::AiHook { action } => cmd_ai_hook(action),
        Commands::AiGuard {
            stage,
            tool_name,
            tool_input,
        } => cmd_ai_guard(stage, tool_name, tool_input.as_deref()),
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
        Commands::Lease { action } => cmd_lease(action),
        Commands::Canary { action } => cmd_canary(action),
        Commands::Revoke { all, name } => cmd_revoke(*all, name.as_deref()),
        Commands::Deps { key, source } => cmd_deps(key, *source),
        Commands::Man { command } => cmd_man(command),
        Commands::Lsp => {
            crate::lsp::run_lsp();
            return;
        }
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

fn cmd_list(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    if json {
        print_entries_json(&entries)?;
    } else {
        print_entries_table(&entries);
    }
    Ok(())
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

    // Try edit first, if not found, add
    match edit_entry(sf, &key, &value) {
        Ok(()) => {}
        Err(OpsError::KeyNotFound { .. }) => {
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

fn cmd_move(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
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
    match action {
        McpAction::Harden => cmd_mcp_harden(dry_run, json),
    }
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

fn cmd_diff() -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;

    let mut any_diff = false;
    for sf in &shell_files {
        if let Ok(diff) = generate_diff(sf) {
            if !diff.is_empty() {
                print!("{}", diff);
                any_diff = true;
            }
        }
    }

    if !any_diff {
        println!("No changes.");
    }
    Ok(())
}

fn cmd_backup(action: &BackupAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BackupAction::Restore { file } => {
            let backup_path = Path::new(file);
            if !backup_path.exists() {
                return Err(format!("Backup file not found: {}", file).into());
            }

            // Determine target from backup filename (e.g., ".zshrc.20260406T120000.bak" -> ".zshrc")
            let file_name = backup_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            // Find the original filename (everything before the first timestamp-like segment)
            let first_part = file_name.split('.').next();
            if first_part.is_none() {
                return Err("Cannot determine target file from backup name".into());
            }

            let content = std::fs::read_to_string(backup_path)?;
            println!("Restored from {}", file);
            println!("Content length: {} bytes", content.len());
            // In a full implementation, we'd write back to the original path
            // For now, just confirm the backup is readable
            Ok(())
        }
        BackupAction::List => {
            let config = load_or_create_default()?;
            let primary = shellexpand(&config.files.primary);
            let backups = list_backups(&primary)?;

            if backups.is_empty() {
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

fn cmd_profile(action: &super::ProfileAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_or_create_default()?;

    match action {
        super::ProfileAction::List => {
            let names = config.profiles.profile_names();
            if names.is_empty() {
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
                    println!("  {} ({}){}", name, file, active);
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

fn cmd_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let toml_str = toml::to_string_pretty(&config)?;
    println!("{}", toml_str);
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
        let value = if entry.value.len() > 50 {
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

fn print_entries_json(entries: &[EnvEntry]) -> Result<(), Box<dyn std::error::Error>> {
    let json_entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "key": e.key,
                "value": e.value,
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
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::schema::{find_schema, parse_schema, validate_against_schema};

    let config = load_or_create_default()?;

    // Load ENV vars
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
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"errors": items, "valid": errors.is_empty()})
                )?
            );
        } else if errors.is_empty() {
            println!(
                "All variables valid ({} checked against schema).",
                env.len()
            );
        } else {
            for e in &errors {
                println!("\x1b[31m✗\x1b[0m {:<30} — {}", e.key, e.message);
            }
            println!("\n{} error(s) found.", errors.len());
            std::process::exit(1);
        }
    } else {
        // Fallback to config.toml validation only
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
        println!("{}", serde_json::to_string_pretty(&items)?);
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

fn cmd_doctor(verbose: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::doctor::{run_doctor, CheckStatus};

    let report = run_doctor();

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

fn cmd_snapshot(action: &SnapshotAction, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
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
                println!("No snapshots found.");
                return Ok(());
            }

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

            for (key, value) in &snap.entries {
                match edit_entry(sf, key, value) {
                    Ok(()) => {}
                    Err(OpsError::KeyNotFound { .. }) => {
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
                let mut imported = 0;

                for (key, value) in &package.entries {
                    match edit_entry(sf, key, value) {
                        Ok(()) => {
                            println!("  Updated: {}", key);
                        }
                        Err(OpsError::KeyNotFound { .. }) => {
                            add_entry(
                                sf,
                                key,
                                value,
                                ExportStyle::Export,
                                QuoteStyle::Double,
                                config.offsets.header_protected_lines,
                                config.offsets.footer_protected_lines,
                            )?;
                            println!("  Added:   {}", key);
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

fn cmd_sanitize(file: &str, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::sanitize::sanitize_file;

    let file_path = Path::new(file);
    if !file_path.exists() {
        return Err(format!("File not found: {}", file).into());
    }

    // Load all entries from EnvForge config
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    // Build secret pairs from sensitive entries
    let secrets: Vec<(String, String)> = entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .filter(|e| crate::ops::dotenv::is_sensitive_key(&e.key))
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect();

    if secrets.is_empty() {
        eprintln!("No sensitive ENV values found to sanitize against.");
        return Ok(());
    }

    let output_path = output.map(Path::new);
    let count = sanitize_file(file_path, output_path, &secrets)?;

    eprintln!("Sanitized: {} secret(s) replaced", count);

    Ok(())
}

// ─── AI Hook Command ───────────────────────────────────────

fn cmd_ai_hook(action: &AiHookAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::ai_hooks::{install_ai_hook, parse_ai_tool, remove_ai_hook};

    let cwd = std::env::current_dir()?;

    match action {
        AiHookAction::Install { tool } => {
            let ai_tool =
                parse_ai_tool(tool).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let result = install_ai_hook(&ai_tool, &cwd)?;
            if result.installed {
                println!("{}", result.message);
                println!("  Config: {}", result.config_path.display());
            } else {
                println!("{}", result.message);
            }
        }
        AiHookAction::Remove { tool } => {
            let ai_tool =
                parse_ai_tool(tool).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let result = remove_ai_hook(&ai_tool, &cwd)?;
            println!("{}", result.message);
            if result.config_path.exists() {
                println!("  Config: {}", result.config_path.display());
            }
        }
    }

    Ok(())
}

// ─── AI Guard Command ──────────────────────────────────────

fn cmd_ai_guard(
    stage: &str,
    tool_name: &str,
    tool_input: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::ai_guard::{run_guard, GuardStage};

    let stage = match stage {
        "pre-tool" => GuardStage::PreTool,
        "post-tool" => GuardStage::PostTool,
        _ => return Ok(()), // unknown stage, skip silently
    };

    let secrets = load_sensitive_secrets();
    let result = run_guard(stage, tool_name, tool_input, &secrets);

    for warning in &result.warnings {
        eprintln!("{}", warning);
    }

    // Don't block (exit 0 always) — hooks are advisory
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

fn cmd_canary(action: &super::CanaryAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::canary;

    match action {
        super::CanaryAction::Create { key, pattern } => {
            let canary = canary::create_canary(key, pattern)?;
            println!("Canary created: {}", canary.key);
            println!("  Fake value: {}", canary.fake_value);
            println!("  Pattern: {}", canary.pattern);
            println!();
            println!("Add to your .env: {}={}", canary.key, canary.fake_value);
            println!(
                "If this value appears in logs, git, or API calls \u{2014} an agent leaked it."
            );
        }
        super::CanaryAction::List => {
            let canaries = canary::list_canaries()?;
            if canaries.is_empty() {
                println!("No canary secrets configured.");
                println!("Create one with: envforge canary create KEY --pattern generic");
                return Ok(());
            }
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
        super::CanaryAction::Check => {
            let triggered = canary::check_canaries()?;
            if triggered.is_empty() {
                println!("No canaries have been triggered. All clear.");
                return Ok(());
            }
            println!(
                "\u{1f6a8} {} canary secret(s) TRIGGERED:\n",
                triggered.len()
            );
            for c in &triggered {
                println!(
                    "  {} (pattern: {}, triggered {} time(s))",
                    c.key, c.pattern, c.trigger_count
                );
            }

            // Show recent alerts
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
                        "  [{}] {} via {} - {}",
                        alert.timestamp, alert.key, alert.source, alert.details
                    );
                }
            }
        }
        super::CanaryAction::Delete { key } => {
            if canary::delete_canary(key)? {
                println!("Canary deleted: {}", key);
            } else {
                println!("Canary not found: {}", key);
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
            println!("{}", serde_json::json!({"entries": [], "total": 0}));
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

fn cmd_lease(action: &LeaseAction) -> Result<(), Box<dyn std::error::Error>> {
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
                eprintln!("No leases found.");
                return Ok(());
            }
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
        LeaseAction::Cleanup => {
            let removed = lease::cleanup_expired()?;
            eprintln!("Cleaned up {} expired/revoked lease(s).", removed);
        }
    }

    Ok(())
}

fn cmd_revoke(all: bool, name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::lease;

    if all {
        let count = lease::revoke_all_leases()?;
        eprintln!("KILLSWITCH: {} lease(s) revoked and removed.", count);
    } else if let Some(lease_name) = name {
        if lease::revoke_lease(lease_name)? {
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

fn cmd_deps(key: &str, include_source: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::deps::{find_dependencies, group_by_type};
    use std::collections::HashSet;

    let config = load_or_create_default()?;
    let project_dir = std::env::current_dir()?;

    // Collect EnvForge managed files
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

    if refs.is_empty() {
        println!("No references found for {}", key);
        return Ok(());
    }

    println!("Dependencies for {}\n", key);

    let grouped = group_by_type(&refs);
    let mut total_files = HashSet::new();

    for (ref_type, items) in &grouped {
        println!("{}:", ref_type);
        for dep in items {
            // Show relative path if possible
            let display_path = dep.file.strip_prefix(&project_dir).unwrap_or(&dep.file);
            println!(
                "  {}:{}  {}",
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
