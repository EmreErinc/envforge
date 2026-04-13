use std::path::Path;

use envforge::model::*;
use envforge::ops::*;
use envforge::parser::*;

fn make_shell_file(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

// ═══════════════════════════════════════════════════════════════
// .env Parser Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_dotenv_basic() {
    let entries = parse_dotenv_content("FOO=bar\nBAZ=123\n");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "FOO");
    assert_eq!(entries[0].value, "bar");
    assert_eq!(entries[1].key, "BAZ");
    assert_eq!(entries[1].value, "123");
}

#[test]
fn test_parse_dotenv_quoted() {
    let entries = parse_dotenv_content("FOO=\"hello world\"\nBAR='single quoted'\n");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].value, "hello world");
    assert_eq!(entries[1].value, "single quoted");
}

#[test]
fn test_parse_dotenv_comments_and_blanks() {
    let content = "# This is a comment\n\nFOO=bar\n\n# Another comment\nBAZ=123\n";
    let entries = parse_dotenv_content(content);
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_parse_dotenv_empty() {
    let entries = parse_dotenv_content("");
    assert!(entries.is_empty());
}

#[test]
fn test_parse_dotenv_value_with_equals() {
    let entries = parse_dotenv_content("CONNECTION=host=localhost;port=5432\n");
    assert_eq!(entries[0].key, "CONNECTION");
    assert_eq!(entries[0].value, "host=localhost;port=5432");
}

#[test]
fn test_parse_dotenv_empty_value() {
    let entries = parse_dotenv_content("EMPTY=\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].value, "");
}

// ═══════════════════════════════════════════════════════════════
// Import Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_import_adds_new_entries() {
    let mut sf = make_shell_file("export EXISTING=\"keep\"\n");
    let config = envforge::config::AppConfig::default();
    let entries = vec![
        DotenvEntry {
            key: "NEW_KEY".to_string(),
            value: "new_val".to_string(),
        },
        DotenvEntry {
            key: "ANOTHER".to_string(),
            value: "val2".to_string(),
        },
    ];

    let result = import_entries(&mut sf, &entries, &config, false);
    assert_eq!(result.added, 2);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.overwritten, 0);

    let all = collect_entries(&sf);
    assert_eq!(all.len(), 3);
}

#[test]
fn test_import_skips_duplicates_without_force() {
    let mut sf = make_shell_file("export FOO=\"original\"\n");
    let config = envforge::config::AppConfig::default();
    let entries = vec![DotenvEntry {
        key: "FOO".to_string(),
        value: "new_value".to_string(),
    }];

    let result = import_entries(&mut sf, &entries, &config, false);
    assert_eq!(result.added, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.overwritten, 0);

    // Value unchanged
    let all = collect_entries(&sf);
    assert_eq!(all[0].value, "original");
}

#[test]
fn test_import_overwrites_with_force() {
    let mut sf = make_shell_file("export FOO=\"original\"\n");
    let config = envforge::config::AppConfig::default();
    let entries = vec![DotenvEntry {
        key: "FOO".to_string(),
        value: "forced_new".to_string(),
    }];

    let result = import_entries(&mut sf, &entries, &config, true);
    assert_eq!(result.added, 0);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.overwritten, 1);

    let all = collect_entries(&sf);
    assert_eq!(all[0].value, "forced_new");
}

// ═══════════════════════════════════════════════════════════════
// Export Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_export_basic() {
    let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=123\n");
    let entries = collect_entries(&sf);
    let output = export_entries(&entries, false, None);

    assert!(output.contains("FOO=bar"));
    assert!(output.contains("BAZ=123"));
    assert!(output.contains("# Exported by EnvForge"));
}

#[test]
fn test_export_excludes_sensitive() {
    let sf = make_shell_file("export API_KEY=\"secret\"\nexport HOST=\"localhost\"\n");
    let entries = collect_entries(&sf);
    let output = export_entries(&entries, true, None);

    assert!(!output.contains("API_KEY"));
    assert!(output.contains("HOST=localhost"));
}

#[test]
fn test_export_with_filter() {
    let sf = make_shell_file("export DB_HOST=\"localhost\"\nexport API_URL=\"http://api\"\n");
    let entries = collect_entries(&sf);
    let output = export_entries(&entries, false, Some("db"));

    assert!(output.contains("DB_HOST"));
    assert!(!output.contains("API_URL"));
}

#[test]
fn test_export_quotes_values_with_spaces() {
    let sf = make_shell_file("export MSG=\"hello world\"\n");
    let entries = collect_entries(&sf);
    let output = export_entries(&entries, false, None);

    assert!(output.contains("MSG=\"hello world\""));
}

#[test]
fn test_export_skips_deleted() {
    let sf =
        make_shell_file("export ACTIVE=\"val\"\n#[envforge:deleted:OLD] export OLD=\"gone\"\n");
    let entries = collect_entries(&sf);
    let output = export_entries(&entries, false, None);

    assert!(output.contains("ACTIVE=val"));
    assert!(!output.contains("OLD=gone"));
}
