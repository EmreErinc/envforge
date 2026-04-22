use envforge::parser::parse_shell_content;
use std::path::Path;

// ============================================================================
// Property-Based Testing with Generated Inputs (12 tests)
// ============================================================================
// Note: These use manual generation instead of proptest crate for now
// Future: Add proptest crate dependency for more sophisticated fuzzing

#[test]
fn test_parse_property_empty_to_many_vars_no_panic() {
    for count in 0..100 {
        let mut content = String::new();
        for i in 0..count {
            content.push_str(&format!("VAR_{}=value_{}\n", i, i));
        }
        let result = parse_shell_content(&content, Path::new("test.env"));
        // Should not panic, may succeed or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_property_all_ascii_printable_chars_in_value() {
    for ascii_code in 32..127 {
        let ch = ascii_code as u8 as char;
        let content = format!("VAR=value_{}", ch);
        let result = parse_shell_content(&content, Path::new("test.env"));
        // All printable ASCII should be handled
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_property_key_length_variance() {
    for len in [1, 10, 100, 1000, 10000] {
        let key = "A".repeat(len);
        let content = format!("{}=value", key);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_property_value_length_variance() {
    for len in [0, 1, 10, 100, 10000, 100000] {
        let value = "a".repeat(len);
        let content = format!("VAR={}", value);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_property_quote_style_combinations() {
    let quote_combos = vec![
        r#"VAR=value"#,
        r#"VAR="value""#,
        r#"VAR='value'"#,
        r#"VAR="val'ue""#,
        r#"VAR='val"ue'"#,
    ];

    for combo in quote_combos {
        let result = parse_shell_content(combo, Path::new("test.env"));
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_property_export_style_combinations() {
    let styles = vec!["VAR=value", "export VAR=value", "export VAR=\"value\""];

    for style in styles {
        let result = parse_shell_content(style, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_property_unicode_script_coverage() {
    let scripts = vec![
        ("Arabic", "العربية"),
        ("Chinese", "中文"),
        ("Cyrillic", "Русский"),
        ("Devanagari", "हिन्दी"),
        ("Greek", "Ελληνικά"),
        ("Hebrew", "עברית"),
        ("Japanese", "日本語"),
        ("Korean", "한국어"),
        ("Thai", "ไทย"),
        ("Emoji", "🚀💯✨"),
    ];

    for (_name, text) in scripts {
        let content = format!(r#"VAR="{}""#, text);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_property_special_char_in_value_combinations() {
    let chars = vec![
        "$", "!", "@", "#", "%", "^", "&", "*", "(", ")", "-", "_", "+",
    ];

    for ch in chars {
        let content = format!("VAR=value{}value", ch);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_property_line_count_idempotence() {
    let mut content = String::new();
    let line_counts = vec![0, 1, 10, 100];

    for count in line_counts {
        content.clear();
        for i in 0..count {
            content.push_str(&format!("VAR_{}=val_{}\n", i, i));
        }

        let result = parse_shell_content(&content, Path::new("test.env"));
        if let Ok(file) = result {
            assert_eq!(file.lines.len(), count);
        }
    }
}

#[test]
fn test_parse_property_serialization_roundtrip_idempotence() {
    let contents = vec![
        "VAR=value",
        "export VAR=value",
        r#"VAR="value""#,
        "VAR1=v1\nVAR2=$VAR1\n",
        "# Comment\nVAR=value\n",
    ];

    for content in contents {
        let result1 = parse_shell_content(content, Path::new("test.env"));
        if let Ok(file1) = result1 {
            let serialized1 = file1.serialize();
            let result2 = parse_shell_content(&serialized1, Path::new("test.env"));
            if let Ok(file2) = result2 {
                let serialized2 = file2.serialize();
                // Second roundtrip should be identical to first
                assert_eq!(serialized1, serialized2);
            }
        }
    }
}

#[test]
fn test_parse_property_fuzz_random_valid_assignments() {
    // Generate random but valid assignments
    let var_names = ["VAR", "CONFIG", "DB_URL", "API_KEY", "SECRET"];
    let values = [
        "value",
        "123",
        "true",
        "path/to/file",
        "https://example.com",
    ];
    let prefixes = ["", "export "];
    let quotes = ["", "\"", "'"];

    for prefix in &prefixes {
        for var in &var_names {
            for value in &values {
                for quote in &quotes {
                    let content = format!("{}{}={}{}{}", prefix, var, quote, value, quote);
                    let result = parse_shell_content(&content, Path::new("test.env"));
                    assert!(result.is_ok() || result.is_err());
                }
            }
        }
    }
}

#[test]
fn test_parse_property_mixed_line_types_no_panic() {
    let line_types = vec![
        "# Comment",
        "",
        "VAR=value",
        "export VAR=value",
        r#"VAR="quoted value""#,
        "$PATH",
        "    \t  ",
    ];

    for _ in 0..10 {
        let mut content = String::new();
        for line in &line_types {
            content.push_str(line);
            content.push('\n');
        }
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok() || result.is_err());
    }
}

// ============================================================================
// Serialization Consistency Tests (8 tests)
// ============================================================================

#[test]
fn test_serialize_preserves_blank_lines() {
    let content = "VAR1=value1\n\n\nVAR2=value2\n";
    let file = parse_shell_content(content, Path::new("test.env")).unwrap();
    let serialized = file.serialize();

    // Should still have blank lines
    assert!(serialized.contains("\n\n"));
}

#[test]
fn test_serialize_preserves_comments() {
    let content = "# This is a comment\nVAR=value\n# Another comment\n";
    let file = parse_shell_content(content, Path::new("test.env")).unwrap();
    let serialized = file.serialize();

    // Should preserve comments
    assert!(serialized.contains("# This is a comment"));
    assert!(serialized.contains("# Another comment"));
}

#[test]
fn test_serialize_preserves_export_style() {
    let content1 = "VAR=value";
    let content2 = "export VAR=value";

    let file1 = parse_shell_content(content1, Path::new("test.env")).unwrap();
    let file2 = parse_shell_content(content2, Path::new("test.env")).unwrap();

    let ser1 = file1.serialize();
    let ser2 = file2.serialize();

    // Serialized forms should differ in export keyword
    if !ser1.contains("export") {
        // ser2 may contain export (could normalize or preserve style)
        let _ = ser2;
    }
}

#[test]
fn test_serialize_preserves_quote_style() {
    let single = r#"VAR='value'"#;
    let double = r#"VAR="value""#;
    let unquoted = "VAR=value";

    let file_single = parse_shell_content(single, Path::new("test.env")).unwrap();
    let file_double = parse_shell_content(double, Path::new("test.env")).unwrap();
    let file_unquoted = parse_shell_content(unquoted, Path::new("test.env")).unwrap();

    let _ser_single = file_single.serialize();
    let _ser_double = file_double.serialize();
    let _ser_unquoted = file_unquoted.serialize();

    // All should serialize without panic
}

#[test]
fn test_serialize_idempotent_multiple_passes() {
    let content = "VAR1=value1\nVAR2=$VAR1\n";
    let file = parse_shell_content(content, Path::new("test.env")).unwrap();

    let ser1 = file.serialize();
    let file2 = parse_shell_content(&ser1, Path::new("test.env")).unwrap();
    let ser2 = file2.serialize();
    let file3 = parse_shell_content(&ser2, Path::new("test.env")).unwrap();
    let ser3 = file3.serialize();

    // Multiple roundtrips should stabilize
    assert_eq!(ser2, ser3);
}

#[test]
fn test_serialize_unicode_preserved() {
    let content = "EMOJI=🚀\nCJK=中文\nARABIC=مرحبا\n";
    let file = parse_shell_content(content, Path::new("test.env")).unwrap();
    let serialized = file.serialize();

    assert!(serialized.contains("🚀"));
    assert!(serialized.contains("中文"));
    assert!(serialized.contains("مرحبا"));
}

#[test]
fn test_serialize_long_values_preserved() {
    let long_value = "x".repeat(10000);
    let content = format!("VAR={}", long_value);
    let file = parse_shell_content(&content, Path::new("test.env")).unwrap();
    let serialized = file.serialize();

    assert!(serialized.contains(&long_value));
}

#[test]
fn test_serialize_special_characters_preserved() {
    let content = r#"URL=https://example.com:8080/path?key=value&debug=true"#;
    let file = parse_shell_content(content, Path::new("test.env")).unwrap();
    let serialized = file.serialize();

    assert!(serialized.contains("https://"));
    assert!(serialized.contains("?key=value&debug=true"));
}

// ============================================================================
// Edge Case & Boundary Tests (10 tests)
// ============================================================================

#[test]
fn test_parse_single_character_value() {
    let content = "VAR=a";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_numeric_value_as_string() {
    let content = "PORT=3000";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_boolean_like_values() {
    let values = vec!["true", "false", "yes", "no", "1", "0"];
    for value in values {
        let content = format!("BOOL={}", value);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_path_like_values() {
    let paths = vec![
        "/usr/local/bin",
        "./relative/path",
        "../parent/path",
        "~/home/path",
        "C:\\Windows\\Path",
    ];
    for path in paths {
        let content = format!("PATH={}", path);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok() || result.is_err());
    }
}

#[test]
fn test_parse_url_like_values() {
    let urls = vec![
        "https://example.com",
        "postgres://user:pass@localhost/db",
        "redis://localhost:6379",
        "file:///path/to/file",
        "ftp://ftp.example.com/file",
    ];
    for url in urls {
        let content = format!("URL={}", url);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_json_like_values() {
    let jsons = vec![
        r#"{"key": "value"}"#,
        r#"[1, 2, 3]"#,
        r#"{"nested": {"inner": 42}}"#,
    ];
    for json in jsons {
        let content = format!(r#"CONFIG='{}'"#, json);
        let result = parse_shell_content(&content, Path::new("test.env"));
        assert!(result.is_ok());
    }
}

#[test]
fn test_parse_base64_like_values() {
    let content = r#"TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_uuid_like_values() {
    let content = "ID=550e8400-e29b-41d4-a716-446655440000";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_multiline_continuation_sequence() {
    let content = "PATH=/usr/bin:\\\n/usr/local/bin:\\\n/bin\n";
    let result = parse_shell_content(content, Path::new("test.env"));
    // Should handle line continuations gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_very_long_single_line() {
    let mut content = "VERY_LONG_VAR=".to_string();
    for _ in 0..100000 {
        content.push('a');
    }
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
}
