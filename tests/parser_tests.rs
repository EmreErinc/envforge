use std::path::Path;

use envforge::model::*;
use envforge::parser::*;

// ═══════════════════════════════════════════════════════════════
// AST Model Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_empty_file() {
    let result = parse_shell_content("", Path::new("/test/.zshrc")).unwrap();
    assert!(result.lines.is_empty());
}

#[test]
fn test_parse_blank_lines() {
    let content = "\n\n";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert_eq!(result.lines.len(), 2);
    assert!(matches!(result.lines[0], LineNode::Blank { .. }));
    assert!(matches!(result.lines[1], LineNode::Blank { .. }));
}

// ═══════════════════════════════════════════════════════════════
// Export Parsing Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_export_double_quoted() {
    let content = r#"export DATABASE_URL="postgres://localhost/mydb""#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert_eq!(result.lines.len(), 1);
    match &result.lines[0] {
        LineNode::EnvExport {
            key,
            value,
            export_style,
            quote_style,
            ..
        } => {
            assert_eq!(key, "DATABASE_URL");
            assert_eq!(value, "postgres://localhost/mydb");
            assert_eq!(*export_style, ExportStyle::Export);
            assert_eq!(*quote_style, QuoteStyle::Double);
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_parse_export_single_quoted() {
    let content = "export API_KEY='abc123'";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::EnvExport {
            key,
            value,
            quote_style,
            ..
        } => {
            assert_eq!(key, "API_KEY");
            assert_eq!(value, "abc123");
            assert_eq!(*quote_style, QuoteStyle::Single);
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_parse_export_no_quotes() {
    let content = "export NODE_ENV=production";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::EnvExport {
            key,
            value,
            quote_style,
            ..
        } => {
            assert_eq!(key, "NODE_ENV");
            assert_eq!(value, "production");
            assert_eq!(*quote_style, QuoteStyle::None);
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_parse_bare_assignment() {
    let content = "MY_VAR=hello";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::EnvExport {
            key,
            value,
            export_style,
            ..
        } => {
            assert_eq!(key, "MY_VAR");
            assert_eq!(value, "hello");
            assert_eq!(*export_style, ExportStyle::Bare);
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_parse_export_with_inline_comment() {
    let content = r#"export API_KEY="secret123" # my api key"#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::EnvExport {
            key,
            value,
            inline_comment,
            ..
        } => {
            assert_eq!(key, "API_KEY");
            assert_eq!(value, "secret123");
            assert!(inline_comment.is_some());
            assert!(inline_comment.as_ref().unwrap().contains("my api key"));
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

#[test]
fn test_parse_value_with_equals() {
    let content = r#"export CONNECTION="host=localhost;port=5432""#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::EnvExport { key, value, .. } => {
            assert_eq!(key, "CONNECTION");
            assert_eq!(value, "host=localhost;port=5432");
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════
// Comment & Tag Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_comment() {
    let content = "# This is a comment";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert!(matches!(result.lines[0], LineNode::Comment { .. }));
}

#[test]
fn test_parse_envforge_deleted_tag() {
    let content = r#"#[envforge:deleted:API_KEY] export API_KEY="old_value""#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::ManagedComment {
            tag,
            original_export,
            ..
        } => {
            assert_eq!(tag, "deleted:API_KEY");
            assert!(original_export.contains("API_KEY"));
        }
        other => panic!("Expected ManagedComment, got: {:?}", other),
    }
}

#[test]
fn test_parse_envforge_moved_tag() {
    let content = r#"#[envforge:moved:DB_URL -> ~/.env_managed] export DB_URL="postgres://""#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert!(matches!(result.lines[0], LineNode::ManagedComment { .. }));
}

// ═══════════════════════════════════════════════════════════════
// Source Directive Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_source_directive() {
    let content = "source ~/.env_managed";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::SourceDirective { path, .. } => {
            assert_eq!(path, "~/.env_managed");
        }
        other => panic!("Expected SourceDirective, got: {:?}", other),
    }
}

#[test]
fn test_parse_dot_source_directive() {
    let content = ". ~/.profile";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    match &result.lines[0] {
        LineNode::SourceDirective { path, .. } => {
            assert_eq!(path, "~/.profile");
        }
        other => panic!("Expected SourceDirective, got: {:?}", other),
    }
}

#[test]
fn test_parse_conditional_source() {
    let content = "[ -f ~/.env_managed ] && source ~/.env_managed";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    // This is a complex conditional, should be Other (not a simple source directive)
    assert!(matches!(result.lines[0], LineNode::Other { .. }));
}

// ═══════════════════════════════════════════════════════════════
// Other Line Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_parse_alias() {
    let content = "alias ll='ls -la'";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert!(matches!(result.lines[0], LineNode::Other { .. }));
}

#[test]
fn test_parse_path_manipulation() {
    let content = r#"export PATH="$HOME/.local/bin:$PATH""#;
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    // PATH manipulation is still an EnvExport
    match &result.lines[0] {
        LineNode::EnvExport { key, .. } => {
            assert_eq!(key, "PATH");
        }
        other => panic!("Expected EnvExport, got: {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════
// Round-Trip Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_round_trip_simple() {
    let content = "export MY_VAR=\"hello world\"\nexport OTHER=123\n";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Round-trip failed: content not identical"
    );
}

#[test]
fn test_round_trip_mixed_content() {
    let content = "\
# My shell config
export DATABASE_URL=\"postgres://localhost/mydb\"

# Aliases
alias ll='ls -la'
alias gs='git status'

export NODE_ENV=production
export API_KEY='secret123'

source ~/.env_managed

# End of config
";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(content, serialized, "Round-trip failed for mixed content");
}

#[test]
fn test_round_trip_with_envforge_tags() {
    let content = "\
export ACTIVE_VAR=\"value1\"
#[envforge:deleted:OLD_VAR] export OLD_VAR=\"old_value\"
#[envforge:moved:MOVED_VAR -> ~/.env_managed] export MOVED_VAR=\"moved\"
export ANOTHER=\"value2\"
";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(content, serialized, "Round-trip failed with envforge tags");
}

#[test]
fn test_round_trip_blank_lines_preserved() {
    let content = "\
export A=1


export B=2

";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Round-trip failed: blank lines changed"
    );
}

// ═══════════════════════════════════════════════════════════════
// SHA-256 Hash Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_hash_computed() {
    let content = "export FOO=bar\n";
    let result = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    // Hash should not be all zeros
    assert_ne!(result.hash, [0u8; 32]);
}

#[test]
fn test_hash_deterministic() {
    let content = "export FOO=bar\n";
    let result1 = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    let result2 = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    assert_eq!(result1.hash, result2.hash);
}

#[test]
fn test_hash_changes_with_content() {
    let result1 = parse_shell_content("export A=1\n", Path::new("/test/.zshrc")).unwrap();
    let result2 = parse_shell_content("export A=2\n", Path::new("/test/.zshrc")).unwrap();
    assert_ne!(result1.hash, result2.hash);
}

// ═══════════════════════════════════════════════════════════════
// Shell Detection Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_default_primary_file_zsh() {
    let path = default_primary_file(&Shell::Zsh).unwrap();
    assert!(path.to_string_lossy().ends_with(".zshrc"));
}

#[test]
fn test_default_primary_file_bash() {
    let path = default_primary_file(&Shell::Bash).unwrap();
    assert!(path.to_string_lossy().ends_with(".bashrc"));
}

#[test]
fn test_scan_config_files_returns_existing_only() {
    // This test verifies that scan only returns files that exist.
    // On any system, at least some config files may or may not exist,
    // but the function should not error.
    let shell = Shell::Zsh;
    let result = scan_config_files(&shell);
    assert!(result.is_ok());
    // All returned paths should exist
    for path in result.unwrap() {
        assert!(path.exists(), "Returned path does not exist: {:?}", path);
    }
}

// ═══════════════════════════════════════════════════════════════
// Snapshot Tests (Real-World Config Patterns)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_snapshot_typical_zshrc() {
    let content = "\
# ~/.zshrc

# Oh My Zsh
export ZSH=\"$HOME/.oh-my-zsh\"
ZSH_THEME=\"robbyrussell\"
plugins=(git docker kubectl)
source $ZSH/oh-my-zsh.sh

# User configuration
export LANG=en_US.UTF-8
export EDITOR='vim'

# Aliases
alias zshconfig=\"vim ~/.zshrc\"
alias ohmyzsh=\"vim ~/.oh-my-zsh\"

# NVM
export NVM_DIR=\"$HOME/.nvm\"
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"

# Go
export GOPATH=\"$HOME/go\"
export PATH=\"$GOPATH/bin:$PATH\"
";
    let result = parse_shell_content(content, Path::new("/home/user/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Snapshot: typical zshrc round-trip failed"
    );

    // Verify specific nodes
    let exports: Vec<&LineNode> = result
        .lines
        .iter()
        .filter(|n| matches!(n, LineNode::EnvExport { .. }))
        .collect();
    // ZSH, ZSH_THEME, LANG, EDITOR, NVM_DIR, GOPATH, PATH = 7 exports
    // (plugins=(git docker kubectl) is also parsed as bare assignment)
    assert!(
        exports.len() >= 7,
        "Expected at least 7 exports in typical zshrc, got {}",
        exports.len()
    );
}

#[test]
fn test_snapshot_conda_block() {
    let content = "\
export MY_VAR=\"hello\"

# >>> conda initialize >>>
# !! Contents within this block are managed by 'conda init' !!
__conda_setup=\"$('/opt/anaconda3/bin/conda' 'shell.zsh' 'hook' 2> /dev/null)\"
if [ $? -eq 0 ]; then
    eval \"$__conda_setup\"
else
    if [ -f \"/opt/anaconda3/etc/profile.d/conda.sh\" ]; then
        . \"/opt/anaconda3/etc/profile.d/conda.sh\"
    else
        export PATH=\"/opt/anaconda3/bin:$PATH\"
    fi
fi
unset __conda_setup
# <<< conda initialize <<<

export OTHER_VAR=\"world\"
";
    let result = parse_shell_content(content, Path::new("/home/user/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Snapshot: conda block round-trip failed"
    );
}

#[test]
fn test_snapshot_amazon_q_block() {
    let content = "\
export MY_ENV=\"value\"

# Q pre block. Keep at the top of this file.
[[ -f \"${HOME}/Library/Application Support/amazon-q/shell/zshrc.pre.zsh\" ]] && builtin source \"${HOME}/Library/Application Support/amazon-q/shell/zshrc.pre.zsh\"

export ANOTHER=\"test\"

# Q post block. Keep at the bottom of this file.
[[ -f \"${HOME}/Library/Application Support/amazon-q/shell/zshrc.post.zsh\" ]] && builtin source \"${HOME}/Library/Application Support/amazon-q/shell/zshrc.post.zsh\"
";
    let result = parse_shell_content(content, Path::new("/home/user/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Snapshot: Amazon Q block round-trip failed"
    );
}

#[test]
fn test_snapshot_envforge_managed_file() {
    let content = "\
# EnvForge managed file
export ACTIVE_KEY=\"active_value\"
#[envforge:deleted:REMOVED_KEY] export REMOVED_KEY=\"removed_value\"
#[envforge:moved:MOVED_KEY -> ~/.env_managed] export MOVED_KEY=\"moved_value\"
export PLAIN_VAR=plain_value
export QUOTED_VAR='single_quoted'
";
    let result = parse_shell_content(content, Path::new("/home/user/.zshrc")).unwrap();
    let serialized = serialize_shell_file(&result);
    assert_eq!(
        content, serialized,
        "Snapshot: envforge managed file round-trip failed"
    );

    // Verify managed comments are parsed correctly
    let managed_count = result
        .lines
        .iter()
        .filter(|n| matches!(n, LineNode::ManagedComment { .. }))
        .count();
    assert_eq!(managed_count, 2, "Expected 2 managed comments");
}

// ═══════════════════════════════════════════════════════════════
// Serialization Tests (Modified Nodes)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_serialize_modified_export_double_quote() {
    let node = LineNode::EnvExport {
        line_number: 0,
        original_text: "export FOO=\"old\"".to_string(),
        key: "FOO".to_string(),
        value: "new_value".to_string(),
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        inline_comment: None,
    };
    let serialized = node.serialize(true);
    assert_eq!(serialized, r#"export FOO="new_value""#);
}

#[test]
fn test_serialize_modified_export_single_quote() {
    let node = LineNode::EnvExport {
        line_number: 0,
        original_text: "export FOO='old'".to_string(),
        key: "FOO".to_string(),
        value: "new_value".to_string(),
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Single,
        inline_comment: None,
    };
    let serialized = node.serialize(true);
    assert_eq!(serialized, "export FOO='new_value'");
}

#[test]
fn test_serialize_modified_bare_no_quote() {
    let node = LineNode::EnvExport {
        line_number: 0,
        original_text: "MY_VAR=old".to_string(),
        key: "MY_VAR".to_string(),
        value: "new_value".to_string(),
        export_style: ExportStyle::Bare,
        quote_style: QuoteStyle::None,
        inline_comment: None,
    };
    let serialized = node.serialize(true);
    assert_eq!(serialized, "MY_VAR=new_value");
}

#[test]
fn test_serialize_modified_with_inline_comment() {
    let node = LineNode::EnvExport {
        line_number: 0,
        original_text: r#"export FOO="old" # comment"#.to_string(),
        key: "FOO".to_string(),
        value: "new_value".to_string(),
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        inline_comment: Some(" # comment".to_string()),
    };
    let serialized = node.serialize(true);
    assert_eq!(
        serialized, r#"export FOO="new_value" # comment"#,
        "inline comment spacing preserved"
    );
}

#[test]
fn test_serialize_unmodified_preserves_original() {
    let original = "  export   WEIRD_SPACING=\"value\"  ";
    let node = LineNode::EnvExport {
        line_number: 0,
        original_text: original.to_string(),
        key: "WEIRD_SPACING".to_string(),
        value: "value".to_string(),
        export_style: ExportStyle::Export,
        quote_style: QuoteStyle::Double,
        inline_comment: None,
    };
    let serialized = node.serialize(false);
    assert_eq!(
        serialized, original,
        "Unmodified node should preserve original text"
    );
}
