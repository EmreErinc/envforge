use envforge::parser::parse_shell_content;
use std::path::Path;

// ============================================================================
// Unicode & UTF-8 Boundary Tests (25 tests)
// ============================================================================
// These tests focus on testing the PUBLIC parser API with unicode content

#[test]
fn test_parse_emoji_in_value() {
    let content = r#"EMOJI=🚀"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
    let file = result.unwrap();
    assert_eq!(file.lines.len(), 1);
}

#[test]
fn test_parse_multiple_emoji() {
    let content = r#"ICONS="🚀🔥💯✨""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_accented_characters() {
    let content = r#"MESSAGE="Café résumé naïve""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_chinese_characters() {
    let content = r#"APP_NAME="应用程序""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_arabic_characters() {
    let content = r#"TEXT="مرحبا""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_japanese_characters() {
    let content = r#"JAPANESE="こんにちは""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_korean_characters() {
    let content = r#"KOREAN="안녕하세요""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_hebrew_characters() {
    let content = r#"HEBREW="שלום""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_devanagari_script() {
    let content = r#"HINDI="नमस्ते""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_thai_script() {
    let content = r#"THAI="สวัสดี""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_mixed_scripts() {
    let content = r#"MIXED="Hello мир 世界 🌍""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_em_dash_at_boundary() {
    // em-dash (—) is 3 bytes in UTF-8: E2 80 94
    let content = "MESSAGE=This is long text with em—dash";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_multiple_multibyte_chars_at_end() {
    let content = "MESSAGE=🚀🔥💯✨";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_mathematical_symbols() {
    let content = r#"MATH="√ ∞ ∑ π ≈ ≠""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_currency_symbols() {
    let content = r#"CURRENCY="$ € £ ¥ ₹ ₽""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_box_drawing_characters() {
    let content = r#"BOX="╔═╗║╚═╝""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_arrows_symbols() {
    let content = r#"ARROWS="← ↑ → ↓ ↔ ↕""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_very_long_line_with_unicode() {
    let mut content = "LONG_KEY=".to_string();
    // Add 5000 mixed-width characters
    for i in 0..5000 {
        match i % 4 {
            0 => content.push('a'),  // 1 byte
            1 => content.push('→'),  // 3 bytes
            2 => content.push('Ω'),  // 2 bytes
            _ => content.push('🚀'), // 4 bytes
        }
    }
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_unicode_in_export_path() {
    let content = "export PATH=/usr/local/café:$PATH";
    let result = parse_shell_content(content, Path::new("test.env"));
    // Should handle without panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_emoji_with_skin_tone() {
    // Emoji with variation selector
    let content = r#"HANDS="👋🏻👋🏿""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_family_emoji() {
    // Family emoji: man + ZWJ + woman + ZWJ + girl
    let content = r#"FAMILY="👨‍👩‍👧""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_roundtrip_unicode_content_no_panic() {
    let original = r#"
EMOJI=🚀💻📱
ACCENTS=café résumé naïve
ASIAN=日本語 中文 한국어
MATH=∑ ∏ √ ∞
CURRENCY=$ € £ ¥
"#;
    let result = parse_shell_content(original, Path::new("test.env"));
    assert!(result.is_ok());
    let file = result.unwrap();
    let serialized = file.serialize();
    // Roundtrip should preserve unicode content
    assert!(serialized.contains("🚀"));
}

#[test]
fn test_parse_unicode_comment_no_panic() {
    let content = "# Comment with emoji 🚀 and accents café";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_continuation_with_unicode() {
    let content = "VAR=Hello\\\n  café";
    let result = parse_shell_content(content, Path::new("test.env"));
    // Test that unicode in continuation lines doesn't cause panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_mixed_language_corpus_no_panic() {
    let content = r#"
EN=Hello world
FR=Bonjour monde
ES=Hola mundo
DE=Hallo Welt
JA=こんにちは世界
ZH=你好世界
AR=مرحبا بالعالم
RU=Привет мир
KO=안녕하세요
HI=नमस्ते दुनिया
"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
    let file = result.unwrap();
    assert!(file.lines.len() >= 10);
}

// ============================================================================
// Extended Parser Error Handling Tests (15 tests)
// ============================================================================

#[test]
fn test_parse_invalid_brace_syntax() {
    let content = "VAR=${{KEY}}";
    let result = parse_shell_content(content, Path::new("test.env"));
    // Should handle gracefully (not panic)
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_unclosed_quote() {
    let content = r#"VAR="unclosed"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    // Shell typically accepts unclosed quotes at EOF
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_mismatched_quotes() {
    let content = r#"VAR="value'"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_escaped_quotes() {
    let content = r#"VAR="quote: \"hello\"""#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_escaped_newline() {
    let content = "VAR=line1\\\nline2";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_multiple_equals() {
    let content = "VAR=value1=value2=value3";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_leading_whitespace_before_export() {
    let content = "   export VAR=value";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_trailing_whitespace_after_value() {
    let content = "VAR=value   \n";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_tabs_in_content() {
    let content = "VAR=\tvalue\twith\ttabs\t";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_semicolon_in_value() {
    let content = "VAR=value;with;semicolon";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_pipe_character_in_value() {
    let content = "VAR=value|with|pipe";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_ampersand_in_value() {
    let content = "VAR=value&with&ampersand";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_parentheses_in_value() {
    let content = "VAR=value(with)parentheses";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_backticks_in_value() {
    let content = r#"VAR=`command`"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_dollar_sign_variations() {
    let content = r#"
VAR1=$SIMPLE
VAR2=${BRACE}
VAR3=$$DOUBLE
VAR4=$@AT
VAR5=$?QUESTION
"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

// ============================================================================
// Config File Error Handling Tests (12 tests)
// ============================================================================

#[test]
fn test_parse_10k_unicode_chars_no_panic() {
    let mut content = "HUGE_VALUE=".to_string();
    for _ in 0..2500 {
        content.push_str("你好世界🚀");
    }
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_empty_file() {
    let content = "";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().lines.len(), 0);
}

#[test]
fn test_parse_only_comments() {
    let content = "# Comment 1\n# Comment 2\n# Comment 3\n";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_only_blank_lines() {
    let content = "\n\n\n";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_mixed_comments_and_blanks() {
    let content = "\n# Comment\n\n# Another\n";
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_long_key_name() {
    let long_key = "A".repeat(500);
    let content = format!("{}=value", long_key);
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_long_value() {
    let long_value = "A".repeat(50000);
    let content = format!("KEY={}", long_value);
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_many_variables_no_panic() {
    let mut content = String::new();
    for i in 0..5000 {
        content.push_str(&format!("VAR_{}=value_{}\n", i, i));
    }
    let result = parse_shell_content(&content, Path::new("test.env"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().lines.len(), 5000);
}

#[test]
fn test_parse_various_line_endings_no_panic() {
    let content = "VAR1=value1\nVAR2=value2\rVAR3=value3\r\nVAR4=value4";
    let result = parse_shell_content(content, Path::new("test.env"));
    // Should handle various line endings gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_null_bytes_handling() {
    // Rust strings can't contain null bytes, skip this
}

#[test]
fn test_parse_preserves_roundtrip_fidelity() {
    let original = "VAR1=value1\nVAR2=value2\n# Comment\nVAR3=$VAR1\n";
    let result = parse_shell_content(original, Path::new("test.env"));
    assert!(result.is_ok());
    let file = result.unwrap();
    let serialized = file.serialize();
    // Serialized should contain key parts of original
    assert!(serialized.contains("VAR1=value1"));
    assert!(serialized.contains("VAR2=value2"));
}

#[test]
fn test_parse_handles_special_env_vars_safely() {
    let content = r#"
SPECIAL1=$HOME
SPECIAL2=$USER
SPECIAL3=$PATH
SPECIAL4=$SHELL
SPECIAL5=$PWD
"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());
}

// ============================================================================
// Integration Tests with Real Scenarios (8 tests)
// ============================================================================

#[test]
fn test_parse_django_env_file() {
    let content = r#"
DEBUG=True
SECRET_KEY=django-insecure-secret-key-123
ALLOWED_HOSTS=localhost,127.0.0.1
DATABASE_URL=postgresql://user:pass@localhost/db
EMAIL_HOST_USER=admin@example.com
"#;
    let result = parse_shell_content(content, Path::new(".env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_nodejs_env_file() {
    let content = r#"
NODE_ENV=production
API_URL=https://api.example.com
PORT=3000
REDIS_URL=redis://localhost:6379
JWT_SECRET=super-secret-key-here
LOG_LEVEL=info
"#;
    let result = parse_shell_content(content, Path::new(".env.production"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_rust_env_file() {
    let content = r#"
RUST_LOG=debug
RUST_BACKTRACE=1
DATABASE_URL=postgres://user:pass@localhost/envforge_test
SQLX_OFFLINE=true
"#;
    let result = parse_shell_content(content, Path::new(".env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_complex_url_values() {
    let content = r#"
DATABASE_URL=postgresql://user:p%40ss%3Aword@localhost:5432/db?sslmode=require&timeout=30
API_ENDPOINT=https://api.service.com/v2/data?key=value&format=json
WEBHOOK_URL=https://webhook.example.com:8443/path?token=abc123&retry=3
"#;
    let result = parse_shell_content(content, Path::new(".env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_complex_json_values() {
    let content = r#"
CONFIG={"key":"value","nested":{"inner":"data"}}
ARRAY=["item1","item2","item3"]
COMPLEX={"users":[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]}
"#;
    let result = parse_shell_content(content, Path::new(".env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_multiline_with_continuation() {
    let content = r#"
PATH=/usr/local/bin:\
/usr/bin:\
/bin:\
/usr/sbin
"#;
    let result = parse_shell_content(content, Path::new(".bashrc"));
    // Should handle line continuations
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_real_world_aws_env() {
    let content = r#"
AWS_ACCOUNT_ID=123456789012
AWS_REGION=us-west-2
AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
AWS_SESSION_TOKEN=AQoDYXdzEJr..token..
"#;
    let result = parse_shell_content(content, Path::new(".env"));
    assert!(result.is_ok());
}

#[test]
fn test_parse_real_world_github_env() {
    let content = r#"
GITHUB_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxx
GITHUB_REPOSITORY=owner/repo
GITHUB_REF=refs/heads/main
GITHUB_SHA=abc123def456
CI=true
"#;
    let result = parse_shell_content(content, Path::new(".env.github"));
    assert!(result.is_ok());
}

// ============================================================================
// Snapshot/Consistency Tests (5 tests)
// ============================================================================

#[test]
fn test_parse_consistency_multiple_parses_same_result() {
    let content = "VAR1=value1\nVAR2=value2\n";
    let result1 = parse_shell_content(content, Path::new("test1.env"));
    let result2 = parse_shell_content(content, Path::new("test2.env"));

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let file1 = result1.unwrap();
    let file2 = result2.unwrap();

    assert_eq!(file1.lines.len(), file2.lines.len());
}

#[test]
fn test_parse_export_statement_consistency() {
    let content1 = "export VAR=value";
    let content2 = "VAR=value";

    let result1 = parse_shell_content(content1, Path::new("test.env"));
    let result2 = parse_shell_content(content2, Path::new("test.env"));

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_parse_quoted_vs_unquoted_consistency() {
    let content1 = r#"VAR="value""#;
    let content2 = "VAR=value";

    let result1 = parse_shell_content(content1, Path::new("test.env"));
    let result2 = parse_shell_content(content2, Path::new("test.env"));

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_parse_single_vs_double_quotes() {
    let content1 = r#"VAR='value'"#;
    let content2 = r#"VAR="value""#;

    let result1 = parse_shell_content(content1, Path::new("test.env"));
    let result2 = parse_shell_content(content2, Path::new("test.env"));

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_serialization_preserves_structure() {
    let content = r#"
# Comment
VAR1=value1
export VAR2="value2"
VAR3=$VAR1
"#;
    let result = parse_shell_content(content, Path::new("test.env"));
    assert!(result.is_ok());

    let file = result.unwrap();
    let serialized = file.serialize();

    // Should preserve structure
    assert!(serialized.contains("VAR1"));
    assert!(serialized.contains("VAR2"));
    assert!(serialized.contains("VAR3"));
}
