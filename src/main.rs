use clap::Parser;

use envforge::cli::{execute_command, run_wizard, Cli};
use envforge::config::config_file_path;
use envforge::ops::monitor::start_persistent_audit_log;
use envforge::ops::secure_memory::disable_core_dumps;
use envforge::ui::run_tui;

fn main() {
    let cli = Cli::parse();
    disable_core_dumps();
    start_persistent_audit_log();

    if let Some(ref cmd) = cli.command {
        execute_command(cmd, cli.json, cli.dry_run);
    } else {
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
