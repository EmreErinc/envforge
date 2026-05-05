use clap::Parser;
use envforge::cli::*;
use envforge::ops::{rename_entry, OpsError};
use envforge::parser::parse_shell_content;
use std::path::Path;

fn make_shell_file(content: &str) -> envforge::model::ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

#[test]
fn test_move_cli_parsing_single_key() {
    let cli = Cli::try_parse_from(["envforge", "move", "MY_VAR"]);
    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        Some(Commands::Move { key, new_key }) => {
            assert_eq!(key, "MY_VAR");
            assert!(new_key.is_none());
        }
        _ => panic!("Expected Move command"),
    }
}

#[test]
fn test_move_cli_parsing_rename() {
    let cli = Cli::try_parse_from(["envforge", "move", "OLD_KEY", "NEW_KEY"]);
    assert!(cli.is_ok());
    let cli = cli.unwrap();
    match cli.command {
        Some(Commands::Move { key, new_key }) => {
            assert_eq!(key, "OLD_KEY");
            assert_eq!(new_key.as_deref(), Some("NEW_KEY"));
        }
        _ => panic!("Expected Move command"),
    }
}

#[test]
fn test_rename_entry_updates_key() {
    let mut sf = make_shell_file("export DATABASE_URL=\"postgres://localhost\"");
    rename_entry(&mut sf, "DATABASE_URL", "DB_CONNECTION").unwrap();
    match &sf.lines[0] {
        envforge::model::LineNode::EnvExport {
            key, original_text, ..
        } => {
            assert_eq!(key, "DB_CONNECTION");
            assert!(original_text.contains("DB_CONNECTION"));
            assert!(!original_text.contains("DATABASE_URL"));
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_rename_entry_preserves_value() {
    let mut sf = make_shell_file("export MY_KEY=\"secret_value\"");
    rename_entry(&mut sf, "MY_KEY", "NEW_KEY").unwrap();
    match &sf.lines[0] {
        envforge::model::LineNode::EnvExport { value, .. } => {
            assert_eq!(value, "secret_value");
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_rename_entry_preserves_export_style() {
    let mut sf = make_shell_file("export FOO=\"bar\"");
    rename_entry(&mut sf, "FOO", "BAZ").unwrap();
    match &sf.lines[0] {
        envforge::model::LineNode::EnvExport {
            original_text,
            export_style,
            ..
        } => {
            assert_eq!(*export_style, envforge::model::ExportStyle::Export);
            assert!(original_text.starts_with("export "));
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_rename_entry_key_not_found() {
    let mut sf = make_shell_file("export FOO=\"bar\"");
    let result = rename_entry(&mut sf, "MISSING", "NEW");
    assert!(matches!(result, Err(OpsError::KeyNotFound { .. })));
}

#[test]
fn test_rename_entry_round_trip_safe() {
    let mut sf = make_shell_file("export DB_HOST=\"localhost\"");
    let _before = envforge::parser::serialize_shell_file(&sf);
    rename_entry(&mut sf, "DB_HOST", "DATABASE_HOST").unwrap();
    let after = envforge::parser::serialize_shell_file(&sf);
    assert!(after.contains("DATABASE_HOST"));
    assert!(!after.contains("DB_HOST="));
    assert!(after.contains("localhost"));
}
