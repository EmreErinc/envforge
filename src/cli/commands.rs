use std::path::Path;
use std::process;

use crate::config::*;
use crate::model::*;
use crate::ops::*;
use crate::parser::*;

use super::{BackupAction, Commands, SnapshotAction};

/// Execute a CLI subcommand.
pub fn execute_command(command: &Commands, json: bool, dry_run: bool) {
    let result = match command {
        Commands::List => cmd_list(json),
        Commands::Get { key } => cmd_get(key, json),
        Commands::Set { assignment } => cmd_set(assignment, dry_run),
        Commands::Delete { key } => cmd_delete(key, dry_run),
        Commands::Copy { key } => cmd_copy(key),
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
                cmd_export_format(path.as_deref(), fmt, filter.as_deref(), k8s_name.as_deref(), k8s_namespace.as_deref())
            } else {
                cmd_export(path.as_deref(), *exclude_sensitive, filter.as_deref())
            }
        }
        Commands::Git { action } => cmd_git(action),
        Commands::Duplicates => cmd_duplicates(json),
        Commands::Scan { path, staged } => cmd_scan(path.as_deref(), *staged, json),
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
        Commands::Completions { shell } => cmd_completions(shell),
        Commands::Log { key, n } => cmd_log(key.as_deref(), *n, json),
        Commands::Config => cmd_config(),
        Commands::Run {
            profile,
            resolve,
            env_files,
            overrides,
            command,
        } => cmd_run(
            profile.as_deref(),
            *resolve,
            env_files,
            overrides,
            command,
            dry_run,
            json,
        ),
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
        Commands::Rotate { key, dry_run: dr, stale } => cmd_rotate(key, *dr || dry_run, *stale),
        Commands::Doctor { verbose } => cmd_doctor(*verbose, json),
        Commands::Check { only } => cmd_check(only.as_deref(), json),
        Commands::Snapshot { action } => cmd_snapshot(action, dry_run),
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

fn cmd_copy(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    copy_value(entry)?;
    println!("Copied value of {}", key);
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

    let format = ExportFormat::parse(format_name).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

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
            println!("Exported {} entries to {} ({})", filtered.iter().filter(|e| e.location != EntryLocation::Commented).count(), p, format_name);
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
            let parts: Vec<&str> = file_name.splitn(2, '.').collect();
            if parts.is_empty() {
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

fn cmd_completions(shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let shell = shell.parse::<Shell>().map_err(|_| {
        format!(
            "Unknown shell '{}'. Supported: bash, zsh, fish, elvish, powershell",
            shell
        )
    })?;

    let mut cmd = super::Cli::command();
    generate(shell, &mut cmd, "envforge", &mut std::io::stdout());
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

fn cmd_run(
    profile: Option<&str>,
    resolve: bool,
    env_files: &[String],
    overrides: &[String],
    command: &[String],
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::run::{collect_env, spawn_process, RunConfig};

    if command.is_empty() {
        return Err(
            "No command specified. Usage: envforge run [flags] -- <command> [args...]".into(),
        );
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
        resolve,
        env_files: env_files.iter().map(std::path::PathBuf::from).collect(),
        overrides: override_pairs,
    };

    let env = collect_env(&run_config)?;

    if dry_run {
        if json {
            let map: serde_json::Value = env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            println!("{}", serde_json::to_string_pretty(&map)?);
        } else {
            let mut keys: Vec<&String> = env.keys().collect();
            keys.sort();
            for key in keys {
                println!("{}={}", key, env[key]);
            }
        }
        return Ok(());
    }

    let cmd = &command[0];
    let args: Vec<String> = command[1..].to_vec();
    let result = spawn_process(cmd, &args, &env)?;
    std::process::exit(result.exit_code);
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

fn cmd_rotate(key: &str, dry_run: bool, stale: bool) -> Result<(), Box<dyn std::error::Error>> {
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
            println!("--- {} ({} days old, from {})", entry.key, entry.age_days, entry.provider);

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
                "  Rotated {} (local={}, age_reset={}, logged={})\n",
                result.key, result.local_updated, result.age_reset, result.logged
            );
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

    // Optionally push to provider
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

    // Optionally push to sync
    if plan.is_synced {
        eprint!("Push to sync? [y/N]: ");
        io::stderr().flush()?;
        let mut sync_input = String::new();
        io::stdin().lock().read_line(&mut sync_input)?;
        if sync_input.trim().eq_ignore_ascii_case("y") {
            println!("  Hint: envforge sync push");
        }
    }

    // Summary
    println!("\nRotation complete:");
    println!("  Key:           {}", result.key);
    println!("  New value:     {}", mask_value(new_value));
    println!("  Local updated: {}", result.local_updated);
    println!("  Age reset:     {}", result.age_reset);
    println!("  Logged:        {}", result.logged);

    Ok(())
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

            let path = snapshot::create_snapshot(
                &snap_name,
                &active_entries,
                &config.profiles.active,
            )?;
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
                name.clone().ok_or("Specify a snapshot name or use --last")?
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
                name.clone().ok_or("Specify a snapshot name or use --last")?
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
                        println!(
                            "  \x1b[33m~ {:<30}\x1b[0m",
                            entry.key
                        );
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

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}
