use envforge::lsp::code_action;
use envforge::lsp::code_lens;
use envforge::lsp::document::*;
use envforge::lsp::document_symbol;
use envforge::lsp::folding_range;
use envforge::lsp::server::ManagedVar;
use envforge::lsp::workspace_symbol;
use envforge::ops::dotenv::is_sensitive_key;
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use std::collections::HashMap;
use tower_lsp::lsp_types::*;

fn sample_env() -> &'static str {
    "# Database config\n\nDB_HOST=localhost\nDB_PASSWORD=secret123\n\n# App settings\nAPP_PORT=8080\nAPP_SECRET_KEY=topsecret\n"
}

fn parse_entries(content: &str) -> Vec<EnvDocEntry> {
    parse_env_document(content)
}

#[test]
fn test_parse_env_document_line_types() {
    let entries = parse_entries(sample_env());

    assert_eq!(entries.len(), 8);
    assert_eq!(entries[0].line_type, EnvLineType::Comment);
    assert_eq!(entries[1].line_type, EnvLineType::Blank);
    assert_eq!(entries[2].line_type, EnvLineType::EnvVar);
    assert_eq!(entries[2].key, "DB_HOST");
    assert_eq!(entries[2].value, "localhost");
    assert_eq!(entries[3].line_type, EnvLineType::EnvVar);
    assert_eq!(entries[3].key, "DB_PASSWORD");
    assert_eq!(entries[4].line_type, EnvLineType::Blank);
    assert_eq!(entries[5].line_type, EnvLineType::Comment);
    assert_eq!(entries[6].line_type, EnvLineType::EnvVar);
    assert_eq!(entries[7].line_type, EnvLineType::EnvVar);
}

#[test]
fn test_parse_env_document_blank_lines() {
    let entries = parse_entries("\n\n\n");
    assert_eq!(entries.len(), 3);
    for e in &entries {
        assert_eq!(e.line_type, EnvLineType::Blank);
    }
}

#[test]
fn test_parse_env_document_comments_only() {
    let entries = parse_entries("# comment1\n# comment2\n");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].line_type, EnvLineType::Comment);
    assert_eq!(entries[1].line_type, EnvLineType::Comment);
}

#[test]
fn test_parse_env_document_export_prefix() {
    let entries = parse_entries("export FOO=bar\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "FOO");
    assert_eq!(entries[0].value, "bar");
    assert_eq!(entries[0].line_type, EnvLineType::EnvVar);
}

#[test]
fn test_parse_env_document_no_equals() {
    let entries = parse_entries("SOME_DIRECTIVE\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].line_type, EnvLineType::Other);
}

#[test]
fn test_parse_env_document_empty_key_equals() {
    let entries = parse_entries("=value\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].line_type, EnvLineType::Other);
}

#[test]
fn test_parse_env_document_quoted_values() {
    let entries = parse_entries("FOO=\"hello world\"\nBAR='single quoted'\n");
    assert_eq!(entries[0].value, "hello world");
    assert_eq!(entries[1].value, "single quoted");
}

#[test]
fn test_env_var_entries_filter() {
    let entries = parse_entries(sample_env());
    let vars = env_var_entries(&entries);
    assert_eq!(vars.len(), 4);
    for v in &vars {
        assert_eq!(v.line_type, EnvLineType::EnvVar);
    }
}

#[test]
fn test_parse_env_document_line_numbers() {
    let entries = parse_entries("A=1\n\nB=2\n# comment\nC=3\n");
    assert_eq!(entries[0].line, 0);
    assert_eq!(entries[1].line, 1);
    assert_eq!(entries[2].line, 2);
    assert_eq!(entries[3].line, 3);
    assert_eq!(entries[4].line, 4);
}

#[test]
fn test_document_symbols_returns_env_vars() {
    let entries = parse_entries(sample_env());
    let result = document_symbol::document_symbols(&entries);

    let symbols = match result {
        Some(DocumentSymbolResponse::Nested(s)) => s,
        _ => panic!("expected nested symbols"),
    };

    assert_eq!(symbols.len(), 4);
    assert_eq!(symbols[0].name, "DB_HOST");
    assert_eq!(symbols[1].name, "DB_PASSWORD");
    assert_eq!(symbols[2].name, "APP_PORT");
    assert_eq!(symbols[3].name, "APP_SECRET_KEY");

    for s in &symbols {
        assert_eq!(s.kind, SymbolKind::VARIABLE);
    }
}

#[test]
fn test_document_symbols_empty_input() {
    let entries = parse_entries("# just comments\n\n");
    let result = document_symbol::document_symbols(&entries);
    assert!(result.is_none());
}

#[test]
fn test_document_symbols_detail_truncation() {
    let long_val = "x".repeat(50);
    let entries = parse_entries(&format!("KEY={}\n", long_val));
    let result = document_symbol::document_symbols(&entries);

    let symbols = match result {
        Some(DocumentSymbolResponse::Nested(s)) => s,
        _ => panic!("expected nested symbols"),
    };

    assert_eq!(symbols.len(), 1);
    let detail = symbols[0].detail.as_ref().unwrap();
    assert!(detail.ends_with("..."));
    assert!(detail.len() <= 44);
}

#[test]
fn test_document_symbols_empty_value() {
    let entries = parse_entries("EMPTY=\n");
    let result = document_symbol::document_symbols(&entries);

    let symbols = match result {
        Some(DocumentSymbolResponse::Nested(s)) => s,
        _ => panic!("expected nested symbols"),
    };

    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].detail.is_none());
}

#[test]
fn test_folding_ranges_consecutive_comments() {
    let content = "# line1\n# line2\n# line3\nFOO=bar\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);

    let comment_ranges: Vec<_> = ranges
        .iter()
        .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
        .collect();
    assert_eq!(comment_ranges.len(), 1);
    assert_eq!(comment_ranges[0].start_line, 0);
    assert_eq!(comment_ranges[0].end_line, 2);
}

#[test]
fn test_folding_ranges_consecutive_blanks() {
    let content = "FOO=bar\n\n\n\nBAR=baz\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);

    let region_ranges: Vec<_> = ranges
        .iter()
        .filter(|r| r.kind == Some(FoldingRangeKind::Region))
        .collect();
    assert_eq!(region_ranges.len(), 1);
    assert!(region_ranges[0].end_line > region_ranges[0].start_line);
}

#[test]
fn test_folding_ranges_mixed() {
    let content = "# db\n# config\n\n\nDB_HOST=localhost\nDB_PASS=secret\n\n\n# app1\n# app2\nAPP_PORT=8080\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);

    let comment_count = ranges
        .iter()
        .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
        .count();
    assert_eq!(comment_count, 2);

    let region_count = ranges
        .iter()
        .filter(|r| r.kind == Some(FoldingRangeKind::Region))
        .count();
    assert!(region_count >= 1);
}

#[test]
fn test_folding_ranges_no_folds() {
    let content = "A=1\nB=2\nC=3\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);
    assert!(ranges.is_empty());
}

#[test]
fn test_folding_ranges_single_comment_no_fold() {
    let content = "# alone\nFOO=bar\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);
    assert!(ranges.is_empty());
}

#[test]
fn test_folding_ranges_trailing_comments() {
    let content = "FOO=bar\n# trailing1\n# trailing2\n";
    let entries = parse_entries(content);
    let ranges = folding_range::compute_folding_ranges(&entries);

    let comment_count = ranges
        .iter()
        .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
        .count();
    assert_eq!(comment_count, 1);
    assert_eq!(ranges[0].start_line, 1);
    assert_eq!(ranges[0].end_line, 2);
}

#[test]
fn test_code_lens_sensitive_key() {
    let entries = parse_entries("DB_PASSWORD=secret\nAPI_KEY=key123\nNORMAL=val\n");
    let lenses = code_lens::code_lenses(&entries, None);

    let sensitive_count = lenses
        .iter()
        .filter_map(|l| l.command.as_ref().map(|c| c.title.as_str()))
        .filter(|t| *t == "sensitive")
        .count();
    assert_eq!(sensitive_count, 2);
}

#[test]
fn test_code_lens_with_schema() {
    let entries = parse_entries("DB_HOST=localhost\nAPP_PORT=8080\n");
    let mut schema = EnvSchema {
        variables: HashMap::new(),
    };
    schema.variables.insert(
        "DB_HOST".into(),
        SchemaVariable {
            var_type: VarType::String,
            required: true,
            sensitive: false,
            default: Some("localhost".into()),
            ..Default::default()
        },
    );
    schema.variables.insert(
        "APP_PORT".into(),
        SchemaVariable {
            var_type: VarType::Port,
            required: false,
            sensitive: false,
            default: Some("3000".into()),
            ..Default::default()
        },
    );

    let lenses = code_lens::code_lenses(&entries, Some(&schema));

    let titles: Vec<_> = lenses
        .iter()
        .filter_map(|l| l.command.as_ref().map(|c| c.title.clone()))
        .collect();

    assert!(titles.contains(&"type: string".to_string()));
    assert!(titles.contains(&"required".to_string()));
    assert!(titles.contains(&"type: port".to_string()));
}

#[test]
fn test_code_lens_empty_entries() {
    let entries: Vec<EnvDocEntry> = vec![];
    let lenses = code_lens::code_lenses(&entries, None);
    assert!(lenses.is_empty());
}

#[test]
fn test_code_lens_only_comments() {
    let entries = parse_entries("# comment\n# another\n");
    let lenses = code_lens::code_lenses(&entries, None);
    assert!(lenses.is_empty());
}

#[test]
fn test_code_action_missing_required() {
    let entries = parse_entries("FOO=bar\n");
    let uri = Url::parse("file:///test/.env").unwrap();
    let diagnostics = vec![Diagnostic {
        range: Range::default(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("envforge".into()),
        message: "Missing required variable: DB_HOST".into(),
        related_information: None,
        tags: None,
        data: None,
    }];

    let result = code_action::code_actions(&uri, &entries, &diagnostics, None);

    assert!(result.is_some());
    let actions = result.unwrap();
    assert!(!actions.is_empty());
}

#[test]
fn test_code_action_sensitive_value() {
    let entries = parse_entries("API_KEY=mysecret\n");
    let uri = Url::parse("file:///test/.env").unwrap();
    let diagnostics = vec![Diagnostic {
        range: Range::default(),
        severity: Some(DiagnosticSeverity::WARNING),
        code: None,
        code_description: None,
        source: Some("envforge".into()),
        message: "Sensitive value for 'API_KEY' should use secret reference".into(),
        related_information: None,
        tags: None,
        data: None,
    }];

    let result = code_action::code_actions(&uri, &entries, &diagnostics, None);

    assert!(result.is_some());
}

#[test]
fn test_code_action_no_diagnostics() {
    let entries = parse_entries("FOO=bar\n");
    let uri = Url::parse("file:///test/.env").unwrap();
    let result = code_action::code_actions(&uri, &entries, &[], None);
    assert!(result.is_none());
}

#[test]
fn test_workspace_symbols_basic() {
    let managed_vars = vec![
        ManagedVar {
            key: "DB_HOST".into(),
            value: "localhost".into(),
            source_file: "/test/.env".into(),
        },
        ManagedVar {
            key: "APP_PORT".into(),
            value: "8080".into(),
            source_file: "/test/.env".into(),
        },
    ];

    let symbols = workspace_symbol::workspace_symbols("", &managed_vars, None);
    assert_eq!(symbols.len(), 2);
}

#[test]
fn test_workspace_symbols_query_filter() {
    let managed_vars = vec![
        ManagedVar {
            key: "DB_HOST".into(),
            value: "localhost".into(),
            source_file: "/test/.env".into(),
        },
        ManagedVar {
            key: "APP_PORT".into(),
            value: "8080".into(),
            source_file: "/test/.env".into(),
        },
    ];

    let symbols = workspace_symbol::workspace_symbols("db", &managed_vars, None);
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].name.contains("DB_HOST"));
}

#[test]
fn test_workspace_symbols_sensitive_masking() {
    let managed_vars = vec![ManagedVar {
        key: "API_KEY".into(),
        value: "supersecret".into(),
        source_file: "/test/.env".into(),
    }];

    let symbols = workspace_symbol::workspace_symbols("", &managed_vars, None);
    assert_eq!(symbols.len(), 1);
    assert!(symbols[0].name.contains("***"));
    assert!(!symbols[0].name.contains("supersecret"));
}

#[test]
fn test_workspace_symbols_empty_query() {
    let symbols = workspace_symbol::workspace_symbols("zzz", &[], None);
    assert!(symbols.is_empty());
}

#[test]
fn test_schema_line_map() {
    let content = "[DATABASE]\nhost=localhost\n[APP]\nport=8080\n";
    let map = schema_line_map(content);
    assert_eq!(map.get("DATABASE"), Some(&0u32));
    assert_eq!(map.get("APP"), Some(&2u32));
}

#[test]
fn test_is_sensitive_key_common_patterns() {
    assert!(is_sensitive_key("API_KEY"));
    assert!(is_sensitive_key("DB_PASSWORD"));
    assert!(is_sensitive_key("SECRET_TOKEN"));
    assert!(is_sensitive_key("AWS_SECRET_ACCESS_KEY"));
    assert!(!is_sensitive_key("APP_PORT"));
    assert!(!is_sensitive_key("DB_HOST"));
}
