use clap::Parser;

use envforge::cli::{execute_command, run_wizard, Cli};
use envforge::config::config_file_path;
use envforge::ui::run_tui;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(ref cmd) => {
            // CLI subcommand mode
            execute_command(cmd, cli.json, cli.dry_run);
        }
        None => {
            // No subcommand → check if config exists, run wizard if needed, then TUI
            let needs_wizard = config_file_path().map(|p| !p.exists()).unwrap_or(true);

            if needs_wizard {
                match run_wizard() {
                    Ok(true) => {
                        println!("Launching EnvForge...");
                        println!();
                    }
                    Ok(false) => {
                        println!("Setup cancelled.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Wizard error: {}", e);
                        std::process::exit(1);
                    }
                }
            }

            if let Err(e) = run_tui() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
