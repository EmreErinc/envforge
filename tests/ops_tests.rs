use std::path::Path;

use envforge::model::*;
use envforge::ops::*;
use envforge::parser::*;

// ═══════════════════════════════════════════════════════════════
// Listing Tests
// ═══════════════════════════════════════════════════════════════

fn make_shell_file(content: &str) -> ShellFile {
    parse_shell_content(content, Path::new("/test/.zshrc")).unwrap()
}

#[test]
fn test_collect_entries_from_exports() {
    let sf = make_shell_file("export FOO=\"bar\"\nexport BAZ=123\n");
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "FOO");
    assert_eq!(entries[0].value, "bar");
    assert_eq!(entries[0].location, EntryLocation::InFile);
    assert_eq!(entries[1].key, "BAZ");
    assert_eq!(entries[1].value, "123");
}

#[test]
fn test_collect_entries_includes_commented() {
    let sf =
        make_shell_file("export ACTIVE=\"val\"\n#[envforge:deleted:OLD] export OLD=\"gone\"\n");
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].location, EntryLocation::InFile);
    assert_eq!(entries[1].key, "OLD");
    assert_eq!(entries[1].location, EntryLocation::Commented);
}

#[test]
fn test_collect_entries_skips_non_exports() {
    let sf = make_shell_file("# comment\nalias ll='ls -la'\nexport ONLY=\"one\"\n");
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "ONLY");
}

#[test]
fn test_filter_entries_case_insensitive() {
    let sf = make_shell_file(
        "export DATABASE_URL=\"pg\"\nexport API_KEY=\"secret\"\nexport DB_HOST=\"localhost\"\n",
    );
    let entries = collect_entries(&sf);

    // Key matching
    let filtered = filter_entries(&entries, "db");
    assert_eq!(filtered.len(), 1); // DB_HOST key contains "db"

    let filtered2 = filter_entries(&entries, "key");
    assert_eq!(filtered2.len(), 1); // API_KEY key contains "key"

    // Value matching
    let filtered3 = filter_entries(&entries, "secret");
    assert_eq!(filtered3.len(), 1); // API_KEY value is "secret"

    let filtered4 = filter_entries(&entries, "localhost");
    assert_eq!(filtered4.len(), 1); // DB_HOST value is "localhost"

    // Matches key or value — "a" appears in DATABASE_URL (key), API_KEY (key), and "localhost" (value of DB_HOST)
    let filtered5 = filter_entries(&entries, "a");
    assert_eq!(filtered5.len(), 3);
}

#[test]
fn test_filter_entries_no_match() {
    let sf = make_shell_file("export FOO=\"bar\"\n");
    let entries = collect_entries(&sf);
    let filtered = filter_entries(&entries, "zzz");
    assert!(filtered.is_empty());
}

#[test]
fn test_compare_runtime_finds_active() {
    // PATH is almost certainly in runtime
    let sf = make_shell_file("export PATH=\"/usr/bin\"\n");
    let entries = collect_entries(&sf);
    let comparisons = compare_runtime(&entries);

    let path_comp = comparisons.iter().find(|c| c.key == "PATH").unwrap();
    assert_eq!(path_comp.status, EnvStatus::Active);
    assert!(path_comp.runtime_value.is_some());
}

#[test]
fn test_compare_runtime_finds_file_only() {
    let sf = make_shell_file("export ENVFORGE_TEST_NONEXISTENT_VAR_12345=\"val\"\n");
    let entries = collect_entries(&sf);
    let comparisons = compare_runtime(&entries);

    let comp = comparisons
        .iter()
        .find(|c| c.key == "ENVFORGE_TEST_NONEXISTENT_VAR_12345")
        .unwrap();
    assert_eq!(comp.status, EnvStatus::InFileOnly);
}

#[test]
fn test_collect_all_entries_multiple_files() {
    let sf1 = make_shell_file("export A=\"1\"\n");
    let sf2 = parse_shell_content("export B=\"2\"\n", Path::new("/test/.bashrc")).unwrap();
    let entries = collect_all_entries(&[sf1, sf2]);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "A");
    assert_eq!(entries[1].key, "B");
}

// ═══════════════════════════════════════════════════════════════
// CRUD Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_edit_entry_updates_value() {
    let mut sf = make_shell_file("export MY_VAR=\"old_value\"\n");
    edit_entry(&mut sf, "MY_VAR", "new_value").unwrap();

    let entries = collect_entries(&sf);
    assert_eq!(entries[0].value, "new_value");
}

#[test]
fn test_edit_entry_preserves_style() {
    let mut sf = make_shell_file("export MY_VAR='old_value'\n");
    edit_entry(&mut sf, "MY_VAR", "new_value").unwrap();

    match &sf.lines[0] {
        LineNode::EnvExport {
            quote_style,
            export_style,
            ..
        } => {
            assert_eq!(*quote_style, QuoteStyle::Single);
            assert_eq!(*export_style, ExportStyle::Export);
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_edit_entry_regenerates_text() {
    let mut sf = make_shell_file("export MY_VAR=\"old\"\n");
    edit_entry(&mut sf, "MY_VAR", "new").unwrap();

    let serialized = serialize_shell_file(&sf);
    assert!(serialized.contains("export MY_VAR=\"new\""));
    assert!(!serialized.contains("old"));
}

#[test]
fn test_edit_entry_not_found() {
    let mut sf = make_shell_file("export FOO=\"bar\"\n");
    let result = edit_entry(&mut sf, "MISSING", "val");
    assert!(result.is_err());
}

#[test]
fn test_soft_delete_creates_managed_comment() {
    let mut sf = make_shell_file("export API_KEY=\"secret\"\n");
    soft_delete(&mut sf, "API_KEY").unwrap();

    assert!(matches!(sf.lines[0], LineNode::ManagedComment { .. }));
    let text = sf.lines[0].original_text();
    assert!(text.contains("#[envforge:deleted:API_KEY]"));
    assert!(text.contains("export API_KEY=\"secret\""));
}

#[test]
fn test_soft_delete_not_found() {
    let mut sf = make_shell_file("export FOO=\"bar\"\n");
    let result = soft_delete(&mut sf, "MISSING");
    assert!(result.is_err());
}

#[test]
fn test_undo_delete_restores_export() {
    let mut sf = make_shell_file("export API_KEY=\"secret\"\n");
    soft_delete(&mut sf, "API_KEY").unwrap();

    // Verify it's deleted
    assert!(matches!(sf.lines[0], LineNode::ManagedComment { .. }));

    // Undo
    undo_delete(&mut sf, "API_KEY").unwrap();

    // Verify it's restored
    match &sf.lines[0] {
        LineNode::EnvExport { key, value, .. } => {
            assert_eq!(key, "API_KEY");
            assert_eq!(value, "secret");
        }
        other => panic!("Expected EnvExport after undo, got: {:?}", other),
    }
}

#[test]
fn test_undo_delete_not_deleted() {
    let mut sf = make_shell_file("export FOO=\"bar\"\n");
    let result = undo_delete(&mut sf, "FOO");
    assert!(result.is_err());
}

#[test]
fn test_add_entry_appends_to_safe_zone() {
    let mut sf = make_shell_file("# header\nexport EXISTING=\"val\"\n# footer\n");
    add_entry(
        &mut sf,
        "NEW_VAR",
        "new_val",
        ExportStyle::Export,
        QuoteStyle::Double,
        0, // no header offset
        1, // 1 line footer offset
    )
    .unwrap();

    // New entry should be before the footer
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.key == "NEW_VAR"));
}

#[test]
fn test_add_entry_duplicate_key_fails() {
    let mut sf = make_shell_file("export FOO=\"bar\"\n");
    let result = add_entry(
        &mut sf,
        "FOO",
        "baz",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    );
    assert!(result.is_err());
}

#[test]
fn test_add_entry_no_safe_zone_fails() {
    let mut sf = make_shell_file("line1\nline2\n");
    let result = add_entry(
        &mut sf,
        "NEW",
        "val",
        ExportStyle::Export,
        QuoteStyle::Double,
        5, // header offset larger than file
        5, // footer offset too
    );
    assert!(result.is_err());
}

#[test]
fn test_add_entry_bare_style() {
    let mut sf = make_shell_file("# config\n");
    add_entry(
        &mut sf,
        "MY_VAR",
        "value",
        ExportStyle::Bare,
        QuoteStyle::None,
        0,
        0,
    )
    .unwrap();

    let serialized = serialize_shell_file(&sf);
    assert!(serialized.contains("MY_VAR=value"));
    assert!(!serialized.contains("export"));
}

// ═══════════════════════════════════════════════════════════════
// Full CRUD Cycle Test
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_full_crud_cycle() {
    let mut sf = make_shell_file("# My config\nexport EXISTING=\"keep_me\"\n");

    // 1. List - should have 1 entry
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 1);

    // 2. Add
    add_entry(
        &mut sf,
        "NEW_KEY",
        "new_val",
        ExportStyle::Export,
        QuoteStyle::Double,
        0,
        0,
    )
    .unwrap();
    let entries = collect_entries(&sf);
    assert_eq!(entries.len(), 2);

    // 3. Edit
    edit_entry(&mut sf, "NEW_KEY", "updated_val").unwrap();
    let entries = collect_entries(&sf);
    let edited = entries.iter().find(|e| e.key == "NEW_KEY").unwrap();
    assert_eq!(edited.value, "updated_val");

    // 4. Soft delete
    soft_delete(&mut sf, "NEW_KEY").unwrap();
    let entries = collect_entries(&sf);
    let deleted = entries.iter().find(|e| e.key == "NEW_KEY").unwrap();
    assert_eq!(deleted.location, EntryLocation::Commented);

    // 5. Undo delete
    undo_delete(&mut sf, "NEW_KEY").unwrap();
    let entries = collect_entries(&sf);
    let restored = entries.iter().find(|e| e.key == "NEW_KEY").unwrap();
    assert_eq!(restored.location, EntryLocation::InFile);

    // 6. Original entry untouched
    let original = entries.iter().find(|e| e.key == "EXISTING").unwrap();
    assert_eq!(original.value, "keep_me");
}

// ═══════════════════════════════════════════════════════════════
// Clipboard Tests (basic - actual clipboard is platform-dependent)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_clipboard_provider_name() {
    let name = clipboard_provider_name();
    assert!(!name.is_empty());
}

// ═══════════════════════════════════════════════════════════════
// AI-Safe Context Emission Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_emit_ai_context_with_schema() {
    use envforge::ops::schema::{emit_ai_context, parse_schema_content};

    let toml = r#"
[DATABASE_URL]
type = "url"
required = true
description = "PostgreSQL connection string"
sensitive = false
pattern = "^postgres://"

[API_KEY]
type = "string"
required = true
sensitive = true

[PORT]
type = "port"
required = true
default = "3000"

[LOG_LEVEL]
type = "enum"
values = ["debug", "info", "warn", "error"]
default = "info"
"#;
    let schema = parse_schema_content(toml).unwrap();
    let output = emit_ai_context(Some(&schema), &[]);

    // Header
    assert!(output.contains("# Environment Variables (AI Context)"));
    assert!(output.contains("Safe for AI tools"));

    // Schema fields present
    assert!(output.contains("## DATABASE_URL"));
    assert!(output.contains("- **Type**: url"));
    assert!(output.contains("- **Required**: yes"));
    assert!(output.contains("- **Description**: PostgreSQL connection string"));
    assert!(output.contains("- **Pattern**: ^postgres://"));

    assert!(output.contains("## API_KEY"));
    // API_KEY: sensitive=true in schema AND pattern match
    assert!(output.contains("- **Sensitive**: YES"));

    assert!(output.contains("## PORT"));
    assert!(output.contains("- **Default**: 3000"));

    assert!(output.contains("## LOG_LEVEL"));
    assert!(output.contains("- **Values**: debug, info, warn, error"));

    // NEVER include actual values (pattern metadata like ^postgres:// is fine)
    assert!(!output.contains("postgres://localhost"));
}

#[test]
fn test_emit_ai_context_with_inference() {
    use envforge::ops::schema::emit_ai_context;

    let entries = vec![
        ("DATABASE_URL".to_string(), "postgres://localhost/db".to_string()),
        ("API_SECRET".to_string(), "super-secret-123".to_string()),
        ("PORT".to_string(), "8080".to_string()),
        ("DEBUG".to_string(), "true".to_string()),
        ("ADMIN_EMAIL".to_string(), "admin@example.com".to_string()),
        ("MAX_RETRIES".to_string(), "5".to_string()),
    ];

    let output = emit_ai_context(None, &entries);

    // Verify inferred types
    assert!(output.contains("## DATABASE_URL"));
    assert!(output.contains("- **Type**: url"));

    assert!(output.contains("## API_SECRET"));
    assert!(output.contains("- **Sensitive**: YES"));

    assert!(output.contains("## DEBUG"));
    assert!(output.contains("- **Type**: bool"));

    assert!(output.contains("## ADMIN_EMAIL"));
    assert!(output.contains("- **Type**: email"));

    assert!(output.contains("## MAX_RETRIES"));
    assert!(output.contains("- **Type**: number"));

    // NEVER include actual values
    assert!(!output.contains("postgres://localhost"));
    assert!(!output.contains("super-secret-123"));
    assert!(!output.contains("admin@example.com"));
}

#[test]
fn test_emit_ai_context_sensitive_detection() {
    use envforge::ops::schema::emit_ai_context;

    let entries = vec![
        ("MY_PASSWORD".to_string(), "hunter2".to_string()),
        ("AUTH_TOKEN".to_string(), "tok_abc".to_string()),
        ("AWS_SECRET_KEY".to_string(), "AKIA...".to_string()),
        ("APP_NAME".to_string(), "myapp".to_string()),
    ];

    let output = emit_ai_context(None, &entries);

    // Sensitive keys detected by pattern
    assert!(output.contains("## MY_PASSWORD"));
    assert!(output.contains("## AUTH_TOKEN"));
    assert!(output.contains("## AWS_SECRET_KEY"));

    // Count sensitive markers — should have at least 3
    let sensitive_count = output.matches("Sensitive**: YES").count();
    assert!(
        sensitive_count >= 3,
        "Expected at least 3 sensitive markers, got {}",
        sensitive_count
    );

    // APP_NAME should NOT be sensitive
    let app_name_section = output.split("## APP_NAME").nth(1).unwrap();
    let next_section = app_name_section.find("\n## ").unwrap_or(app_name_section.len());
    let app_name_block = &app_name_section[..next_section];
    assert!(
        app_name_block.contains("**Sensitive**: no"),
        "APP_NAME should not be sensitive"
    );

    // Actual values must never appear
    assert!(!output.contains("hunter2"));
    assert!(!output.contains("tok_abc"));
    assert!(!output.contains("AKIA"));
}

#[test]
fn test_emit_ai_context_schema_plus_inferred() {
    use envforge::ops::schema::{emit_ai_context, parse_schema_content};

    let toml = r#"
[DATABASE_URL]
type = "url"
required = true
description = "DB connection"
"#;
    let schema = parse_schema_content(toml).unwrap();

    // Extra entry not in schema
    let entries = vec![
        ("DATABASE_URL".to_string(), "postgres://localhost/db".to_string()),
        ("EXTRA_TOKEN".to_string(), "tok_xyz".to_string()),
    ];

    let output = emit_ai_context(Some(&schema), &entries);

    // DATABASE_URL from schema
    assert!(output.contains("## DATABASE_URL"));
    assert!(output.contains("- **Description**: DB connection"));

    // EXTRA_TOKEN inferred
    assert!(output.contains("## EXTRA_TOKEN"));
    assert!(output.contains("- **Sensitive**: YES")); // contains TOKEN

    // No values
    assert!(!output.contains("postgres://localhost"));
    assert!(!output.contains("tok_xyz"));
}
