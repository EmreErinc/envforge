use std::io::{self, BufRead, Write};

use crate::config::*;
use crate::ops::*;
use crate::parser::*;

/// Run the first-run setup wizard.
///
/// Guides the user through initial configuration.
/// Returns Ok(true) if config was created, Ok(false) if cancelled.
pub fn run_wizard() -> Result<bool, Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════╗");
    println!("║       EnvForge — First Run Setup     ║");
    println!("╚══════════════════════════════════════╝");
    println!();

    // Step 1: Detect shell
    let shell = detect_shell().unwrap_or(crate::model::Shell::Unknown("unknown".to_string()));
    let shell_name = match &shell {
        crate::model::Shell::Zsh => "zsh",
        crate::model::Shell::Bash => "bash",
        crate::model::Shell::Unknown(s) => s.as_str(),
    };
    println!("Detected shell: {}", shell_name);
    println!();

    // Step 2: Find config files
    let config_files = scan_config_files(&shell)?;
    if config_files.is_empty() {
        println!("No shell config files found.");
        let primary = default_primary_file(&shell)?;
        println!("Will use default: {}", primary.to_string_lossy());
    } else {
        println!("Found config files:");
        for (i, path) in config_files.iter().enumerate() {
            println!("  {} - {}", i + 1, path.display());
        }
    }
    println!();

    // Step 3: Select primary file
    let primary_path = if config_files.len() == 1 {
        println!(
            "Using {} as primary config file.",
            config_files[0].display()
        );
        config_files[0].clone()
    } else if config_files.len() > 1 {
        print!("Select primary file [1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let choice: usize = input.trim().parse().unwrap_or(1);
        let idx = choice.saturating_sub(1).min(config_files.len() - 1);
        config_files[idx].clone()
    } else {
        default_primary_file(&shell)?
    };
    println!();

    // Step 4: Scan for protected blocks
    let mut header_offset = 0;
    let mut footer_offset = 0;

    if primary_path.exists() {
        if let Ok(sf) = parse_shell_file(&primary_path) {
            let blocks = detect_protected_blocks(&sf);
            if blocks.is_empty() {
                println!("No protected blocks detected.");
            } else {
                println!("Detected protected blocks:");
                for block in &blocks {
                    println!(
                        "  - {} (lines {}-{})",
                        block.name, block.start_line, block.end_line
                    );
                }
                let (h, f) = suggest_offsets(&sf);
                header_offset = h;
                footer_offset = f;
                println!();
                println!(
                    "Suggested offsets: header={}, footer={}",
                    header_offset, footer_offset
                );

                print!("Accept suggested offsets? [Y/n]: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().lock().read_line(&mut input)?;
                let answer = input.trim().to_lowercase();
                if answer == "n" || answer == "no" {
                    print!("Header offset [0]: ");
                    io::stdout().flush()?;
                    let mut h_input = String::new();
                    io::stdin().lock().read_line(&mut h_input)?;
                    header_offset = h_input.trim().parse().unwrap_or(0);

                    print!("Footer offset [0]: ");
                    io::stdout().flush()?;
                    let mut f_input = String::new();
                    io::stdin().lock().read_line(&mut f_input)?;
                    footer_offset = f_input.trim().parse().unwrap_or(0);
                }
            }
        }
    }
    println!();

    // Step 5: Reference file strategy
    print!("Use reference file (~/.env_managed) for managed ENVs? [Y/n]: ");
    io::stdout().flush()?;

    let mut ref_input = String::new();
    io::stdin().lock().read_line(&mut ref_input)?;
    let use_reference = !matches!(ref_input.trim().to_lowercase().as_str(), "n" | "no");
    println!();

    // Step 6: Save config
    let config = AppConfig {
        general: GeneralConfig {
            default_shell: shell_name.to_string(),
        },
        files: FilesConfig {
            primary: format!(
                "~/{}",
                primary_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
            reference: "~/.env_managed".to_string(),
            use_reference_file: use_reference,
        },
        offsets: OffsetsConfig {
            header_protected_lines: header_offset,
            footer_protected_lines: footer_offset,
        },
        protected_blocks: ProtectedBlocksConfig { markers: vec![] },
        groups: std::collections::HashMap::new(),
        profiles: crate::config::ProfilesConfig::default(),
        validation: std::collections::HashMap::new(),
        clipboard: ClipboardConfig::default(),
        lifecycle: LifecycleConfig::default(),
        analytics: crate::model::AnalyticsConfig::default(),
    };

    let config_path = config_file_path().map_err(|e| format!("Config path error: {}", e))?;
    save_config(&config, &config_path)?;

    if primary_path.exists() {
        if let Ok(mut sf) = parse_shell_file(&primary_path) {
            if !has_managed_zone(&sf) {
                ensure_managed_zone(&mut sf);
                let content = serialize_shell_file(&sf);
                if let Err(e) = crate::config::safe_write(&primary_path, &content, None) {
                    eprintln!("Warning: could not add managed zone markers: {}", e);
                } else {
                    println!("Added managed zone markers to {}", primary_path.display());
                }
            }
        }
    }

    println!("╔══════════════════════════════════════╗");
    println!("║         Setup Complete!              ║");
    println!("╚══════════════════════════════════════╝");
    println!();
    println!("Config saved to: {}", config_path.display());
    println!("Primary file:    {}", config.files.primary);
    println!(
        "Reference file:  {}",
        if use_reference {
            &config.files.reference
        } else {
            "disabled"
        }
    );
    println!("Header offset:   {}", header_offset);
    println!("Footer offset:   {}", footer_offset);
    println!();

    Ok(true)
}
