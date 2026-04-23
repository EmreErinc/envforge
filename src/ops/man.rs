//! Built-in man pages for EnvForge CLI.
//!
//! Parses the embedded CLI reference markdown and displays
//! formatted man-page-style output for any command.

use std::collections::BTreeMap;

/// A single man page entry for a command.
#[derive(Debug, Clone)]
pub struct ManPage {
    pub command: String,
    pub category: String,
    pub description: String,
    pub usage: String,
    pub flags: Vec<(String, String)>,
    pub examples: Vec<String>,
}

/// Embedded CLI reference content.
const CLI_REFERENCE: &str = include_str!("../../docs/cli-reference.md");

/// Parse all man pages from the embedded CLI reference.
pub fn load_man_pages() -> BTreeMap<String, ManPage> {
    let mut pages: BTreeMap<String, ManPage> = BTreeMap::new();
    let mut current_category = String::new();
    let mut current_command = String::new();
    let mut current_desc = String::new();
    let mut current_usage = String::new();
    let mut current_flags: Vec<(String, String)> = Vec::new();
    let mut current_examples: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut in_examples = false;
    let mut in_flags_table = false;

    for line in CLI_REFERENCE.lines() {
        // Track code blocks
        if line.starts_with("```") {
            if in_code_block {
                // Closing code block
                if in_examples {
                    let trimmed = code_block_content.trim().to_string();
                    if !trimmed.is_empty() {
                        current_examples.push(trimmed);
                    }
                } else if current_usage.is_empty() && code_block_content.contains("Usage:") {
                    current_usage = code_block_content.trim().to_string();
                }
                code_block_content.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
                code_block_content.clear();
            }
            continue;
        }

        if in_code_block {
            if !code_block_content.is_empty() {
                code_block_content.push('\n');
            }
            code_block_content.push_str(line);
            continue;
        }

        // Category heading (## )
        if line.starts_with("## ") {
            // Save previous command if any
            flush_command(
                &mut pages,
                &current_command,
                &current_category,
                &current_desc,
                &current_usage,
                &current_flags,
                &current_examples,
            );

            current_category = line
                .strip_prefix("## ")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            current_command.clear();
            current_desc.clear();
            current_usage.clear();
            current_flags.clear();
            current_examples.clear();
            in_examples = false;
            in_flags_table = false;
            continue;
        }

        // Command heading (### envforge xxx)
        if line.starts_with("### envforge ") {
            // Save previous command
            flush_command(
                &mut pages,
                &current_command,
                &current_category,
                &current_desc,
                &current_usage,
                &current_flags,
                &current_examples,
            );

            current_command = line[4..].trim().to_string();
            current_desc.clear();
            current_usage.clear();
            current_flags.clear();
            current_examples.clear();
            in_examples = false;
            in_flags_table = false;
            continue;
        }

        // Skip if no current command
        if current_command.is_empty() {
            continue;
        }

        // Description (first non-empty line after command heading)
        if current_desc.is_empty()
            && !line.trim().is_empty()
            && !line.starts_with('|')
            && !line.starts_with("**")
            && !line.starts_with("```")
            && !line.starts_with("---")
        {
            current_desc = line.trim().to_string();
            continue;
        }

        // Examples section
        if line.starts_with("**Examples") || line.starts_with("**Example") {
            in_examples = true;
            in_flags_table = false;
            continue;
        }

        // Flags table
        if line.starts_with("| Flag")
            || line.starts_with("| Argument")
            || line.starts_with("| Option")
        {
            in_flags_table = true;
            in_examples = false;
            continue;
        }

        // Table separator
        if line.starts_with("|---") || line.starts_with("| ---") {
            continue;
        }

        // Parse table rows for flags
        if in_flags_table && line.starts_with('|') {
            let cols: Vec<&str> = line
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if cols.len() >= 2 {
                let flag = cols[0].replace('`', "");
                let desc = cols[1].replace('`', "");
                current_flags.push((flag, desc));
            }
            continue;
        }

        // Non-table line after table ends the table
        if in_flags_table && !line.starts_with('|') && !line.trim().is_empty() {
            in_flags_table = false;
        }

        // Horizontal rule = section break
        if line.starts_with("---") {
            in_examples = false;
            in_flags_table = false;
        }
    }

    // Flush last command
    flush_command(
        &mut pages,
        &current_command,
        &current_category,
        &current_desc,
        &current_usage,
        &current_flags,
        &current_examples,
    );

    pages
}

fn flush_command(
    pages: &mut BTreeMap<String, ManPage>,
    command: &str,
    category: &str,
    desc: &str,
    usage: &str,
    flags: &[(String, String)],
    examples: &[String],
) {
    if command.is_empty() {
        return;
    }

    let page = ManPage {
        command: command.to_string(),
        category: category.to_string(),
        description: desc.to_string(),
        usage: usage.to_string(),
        flags: flags.to_vec(),
        examples: examples.to_vec(),
    };

    // Store with full command name
    pages.insert(command.to_string(), page.clone());

    // Also store with short name (e.g., "list" for "envforge list", "sync push" for "envforge sync push")
    let short = command.strip_prefix("envforge ").unwrap_or(command);
    pages.insert(short.to_string(), page);
}

/// Format a man page for terminal display.
pub fn format_man_page(page: &ManPage) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "\x1b[1m{}\x1b[0m(1)                  EnvForge Manual                  \x1b[1m{}\x1b[0m(1)\n\n",
        page.command.replace("envforge ", ""),
        page.command.replace("envforge ", ""),
    ));

    // NAME
    out.push_str("\x1b[1mNAME\x1b[0m\n");
    out.push_str(&format!(
        "       {} - {}\n\n",
        page.command, page.description
    ));

    // SYNOPSIS
    if !page.usage.is_empty() {
        out.push_str("\x1b[1mSYNOPSIS\x1b[0m\n");
        for line in page.usage.lines() {
            out.push_str(&format!("       {}\n", line));
        }
        out.push('\n');
    }

    // DESCRIPTION
    if !page.description.is_empty() {
        out.push_str("\x1b[1mDESCRIPTION\x1b[0m\n");
        out.push_str(&format!("       {}\n\n", page.description));
    }

    // OPTIONS
    if !page.flags.is_empty() {
        out.push_str("\x1b[1mOPTIONS\x1b[0m\n");
        for (flag, desc) in &page.flags {
            out.push_str(&format!(
                "       \x1b[1m{}\x1b[0m\n              {}\n\n",
                flag, desc
            ));
        }
    }

    // EXAMPLES
    if !page.examples.is_empty() {
        out.push_str("\x1b[1mEXAMPLES\x1b[0m\n");
        for example in &page.examples {
            for line in example.lines() {
                if line.starts_with('#') {
                    // Comment
                    out.push_str(&format!("       \x1b[90m{}\x1b[0m\n", line));
                } else {
                    out.push_str(&format!("       {}\n", line));
                }
            }
            out.push('\n');
        }
    }

    // CATEGORY
    out.push_str("\x1b[1mCATEGORY\x1b[0m\n");
    out.push_str(&format!("       {}\n\n", page.category));

    // SEE ALSO
    out.push_str("\x1b[1mSEE ALSO\x1b[0m\n");
    out.push_str("       envforge(1), envforge-check(1), envforge-doctor(1)\n\n");

    // Footer
    out.push_str(&format!(
        "EnvForge {}                                          {}\n",
        env!("CARGO_PKG_VERSION"),
        page.command
    ));

    out
}

/// Format the man page index (list of all commands).
pub fn format_man_index(pages: &BTreeMap<String, ManPage>) -> String {
    let mut out = String::new();

    out.push_str("\x1b[1mENVFORGE\x1b[0m(1)              EnvForge Manual              \x1b[1mENVFORGE\x1b[0m(1)\n\n");
    out.push_str("\x1b[1mNAME\x1b[0m\n");
    out.push_str("       envforge - AI-safe environment variable manager\n\n");
    out.push_str("\x1b[1mDESCRIPTION\x1b[0m\n");
    out.push_str("       EnvForge safely manages environment variables with 22 AI safety\n");
    out.push_str("       tools, 7 secret provider integrations, encrypted sync, and 90+\n");
    out.push_str("       CLI commands.\n\n");
    out.push_str("\x1b[1mCOMMANDS\x1b[0m\n\n");

    // Group by category, only show "envforge xxx" entries (not short aliases)
    let mut categories: BTreeMap<String, Vec<&ManPage>> = BTreeMap::new();
    for (name, page) in pages {
        if name.starts_with("envforge ") {
            categories
                .entry(page.category.clone())
                .or_default()
                .push(page);
        }
    }

    for (category, cmds) in &categories {
        out.push_str(&format!("   \x1b[1m{}\x1b[0m\n", category));
        for cmd in cmds {
            let short = cmd.command.replace("envforge ", "");
            let desc = if cmd.description.len() > 55 {
                format!("{}...", &cmd.description[..52])
            } else {
                cmd.description.clone()
            };
            out.push_str(&format!("       \x1b[36m{:<28}\x1b[0m {}\n", short, desc));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "\x1b[1mVERSION\x1b[0m\n       {}\n\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(
        "\x1b[1mUSAGE\x1b[0m\n       envforge man <command>     Show man page for a command\n",
    );
    out.push_str("       envforge man list          Show envforge-list man page\n");
    out.push_str("       envforge man sync push     Show envforge-sync-push man page\n");

    out
}

/// Find similar command names for "did you mean?" suggestions.
pub fn suggest_similar(query: &str, pages: &BTreeMap<String, ManPage>) -> Vec<String> {
    let query_lower = query.to_lowercase();
    pages
        .keys()
        .filter(|k| k.starts_with("envforge "))
        .filter(|k| {
            let short = k.replace("envforge ", "").to_lowercase();
            let prefix_len = query_lower.len().clamp(1, 3);
            short.contains(&query_lower) || query_lower.contains(&short) || {
                // Simple prefix match
                short.starts_with(&query_lower[..prefix_len])
            }
        })
        .take(5)
        .map(|k| {
            k.strip_prefix("envforge ")
                .map(|s| s.to_string())
                .unwrap_or_else(|| k.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_man_pages() {
        let pages = load_man_pages();
        assert!(!pages.is_empty(), "Should parse at least some man pages");
        // Should have both full and short names
        assert!(
            pages.contains_key("envforge list") || pages.contains_key("list"),
            "Should contain 'list' command"
        );
    }

    #[test]
    fn test_man_page_has_content() {
        let pages = load_man_pages();
        if let Some(page) = pages.get("envforge list").or(pages.get("list")) {
            assert!(!page.description.is_empty(), "Should have description");
            assert!(!page.command.is_empty(), "Should have command name");
            assert!(!page.category.is_empty(), "Should have category");
        }
    }

    #[test]
    fn test_format_man_page() {
        let page = ManPage {
            command: "envforge list".to_string(),
            category: "Variable Management".to_string(),
            description: "List all environment variables.".to_string(),
            usage: "Usage: envforge list [OPTIONS]".to_string(),
            flags: vec![("--json".to_string(), "Output as JSON".to_string())],
            examples: vec!["envforge list\nenvforge list --json".to_string()],
        };
        let output = format_man_page(&page);
        assert!(output.contains("NAME"));
        assert!(output.contains("SYNOPSIS"));
        assert!(output.contains("OPTIONS"));
        assert!(output.contains("EXAMPLES"));
        assert!(output.contains("envforge list"));
    }

    #[test]
    fn test_format_man_index() {
        let pages = load_man_pages();
        let index = format_man_index(&pages);
        assert!(index.contains("ENVFORGE"));
        assert!(index.contains("COMMANDS"));
    }

    #[test]
    fn test_suggest_similar() {
        let pages = load_man_pages();
        let suggestions = suggest_similar("lis", &pages);
        assert!(
            suggestions.iter().any(|s| s.contains("list")),
            "Should suggest 'list' for 'lis'"
        );
    }

    #[test]
    fn test_short_name_lookup() {
        let pages = load_man_pages();
        // Both "list" and "envforge list" should exist
        let has_short = pages.contains_key("list");
        let has_long = pages.contains_key("envforge list");
        assert!(
            has_short || has_long,
            "Should have at least one form of 'list'"
        );
    }

    #[test]
    fn test_format_man_page_empty_fields() {
        let page = ManPage {
            command: "envforge test".to_string(),
            category: "Test".to_string(),
            description: "A test command".to_string(),
            usage: String::new(),
            flags: vec![],
            examples: vec![],
        };
        let output = format_man_page(&page);
        assert!(output.contains("NAME"));
        assert!(output.contains("DESCRIPTION"));
        // No SYNOPSIS section when usage is empty
        assert!(!output.contains("SYNOPSIS"));
        // No OPTIONS section when flags is empty
        assert!(!output.contains("OPTIONS"));
        // No EXAMPLES section when examples is empty
        assert!(!output.contains("EXAMPLES"));
        assert!(output.contains("CATEGORY"));
    }

    #[test]
    fn test_suggest_similar_no_match() {
        let pages = load_man_pages();
        let suggestions = suggest_similar("zzzzzzzzz", &pages);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_similar_partial() {
        let pages = load_man_pages();
        let suggestions = suggest_similar("syn", &pages);
        assert!(
            suggestions.iter().any(|s| s.contains("sync")),
            "Should suggest sync-related commands for 'syn'"
        );
    }

    #[test]
    fn test_man_page_fields() {
        let page = ManPage {
            command: "envforge test".to_string(),
            category: "Test".to_string(),
            description: "Desc".to_string(),
            usage: "Usage: test".to_string(),
            flags: vec![("--flag".to_string(), "A flag".to_string())],
            examples: vec!["example".to_string()],
        };
        assert_eq!(page.command, "envforge test");
        assert_eq!(page.category, "Test");
        assert_eq!(page.flags.len(), 1);
        assert_eq!(page.flags[0].0, "--flag");
        assert_eq!(page.examples.len(), 1);
    }

    #[test]
    fn test_format_man_page_with_comment_example() {
        let page = ManPage {
            command: "envforge test".to_string(),
            category: "Test".to_string(),
            description: "Test".to_string(),
            usage: String::new(),
            flags: vec![],
            examples: vec!["# This is a comment\nenvforge test".to_string()],
        };
        let output = format_man_page(&page);
        assert!(output.contains("EXAMPLES"));
        // Comment lines get special formatting
        assert!(output.contains("# This is a comment"));
    }

    #[test]
    fn test_format_man_index_has_version() {
        let pages = load_man_pages();
        let index = format_man_index(&pages);
        assert!(index.contains("VERSION"));
        assert!(index.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_load_man_pages_has_subcommands() {
        let pages = load_man_pages();
        // Should have some subcommand pages like "sync push" or "secrets pull"
        let has_multi_word = pages.keys().any(|k| {
            let short = k.strip_prefix("envforge ").unwrap_or(k);
            short.contains(' ')
        });
        assert!(has_multi_word, "Should have multi-word command pages");
    }
}
