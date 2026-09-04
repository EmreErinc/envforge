//! Documentation-sync guardrails.
//!
//! These tests make the doc-sync effort permanent: they fail the build if any
//! surface drifts from the canonical facts — conflicting counts, a stale
//! version, a blank `--help` description, or an undocumented command. When a
//! canonical number legitimately changes, update the surfaces and these tests
//! will confirm everything agrees again.

use clap::CommandFactory;
use envforge::cli::Cli;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{ROOT}/{rel}"))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// Recursively count `#[test]` / `#[tokio::test]` attributes under a directory.
fn count_test_attrs(dir: &str) -> usize {
    let mut total = 0;
    let path = format!("{ROOT}/{dir}");
    let Ok(entries) = std::fs::read_dir(&path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                if name == "target" || name == "snapshots" || name.starts_with('.') {
                    continue;
                }
                total += count_test_attrs(&format!("{dir}/{name}"));
            }
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(src) = std::fs::read_to_string(&p) {
                for line in src.lines() {
                    let t = line.trim();
                    if t == "#[test]" || t == "#[tokio::test]" || t.starts_with("#[tokio::test(") {
                        total += 1;
                    }
                }
            }
        }
    }
    total
}

/// Collect every visible (non-hidden) command path from the clap tree,
/// as space-joined names rooted at the top level (e.g. "sync push").
fn visible_command_paths() -> Vec<String> {
    fn walk(cmd: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for sub in cmd.get_subcommands() {
            if sub.is_hide_set() {
                continue;
            }
            let name = sub.get_name();
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix} {name}")
            };
            out.push(path.clone());
            walk(sub, &path, out);
        }
    }
    let cmd = Cli::command();
    let mut out = Vec::new();
    walk(&cmd, "", &mut out);
    out
}

// ── Guard 1: no command ships a blank --help description ─────────────────────

#[test]
fn test_no_blank_command_help() {
    fn check(cmd: &clap::Command, prefix: &str, blanks: &mut Vec<String>) {
        for sub in cmd.get_subcommands() {
            if sub.is_hide_set() {
                continue;
            }
            let name = sub.get_name();
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix} {name}")
            };
            let about_ok = sub
                .get_about()
                .map(|s| !s.to_string().trim().is_empty())
                .unwrap_or(false);
            if !about_ok {
                blanks.push(path.clone());
            }
            check(sub, &path, blanks);
        }
    }
    let cmd = Cli::command();
    let mut blanks = Vec::new();
    check(&cmd, "", &mut blanks);
    assert!(
        blanks.is_empty(),
        "these commands have a blank `--help` description (add a `///` doc comment): {blanks:?}"
    );
}

// ── Guard 2: every top-level command is documented in the CLI reference ──────
// (cli-reference.md is the source the built-in man pages parse via include_str!)

#[test]
fn test_cli_reference_covers_top_level_commands() {
    let reference = read("docs/cli-reference.md");
    let mut missing = Vec::new();
    for path in visible_command_paths() {
        // only assert top-level commands have a section; subcommands may be
        // grouped under their parent's section.
        if path.contains(' ') {
            continue;
        }
        let heading = format!("### envforge {path}");
        // some commands are documented as the parent of subcommand sections
        // (e.g. `### envforge sync push`); accept either an exact section or a
        // `### envforge <cmd> ` prefixed section.
        let prefix = format!("### envforge {path} ");
        let covered = reference.contains(&heading) || reference.contains(&prefix);
        if !covered {
            missing.push(path);
        }
    }
    assert!(
        missing.is_empty(),
        "these commands exist in the CLI but are undocumented in docs/cli-reference.md: {missing:?}"
    );
}

// ── Guard 3: canonical counts/tagline — one value everywhere, no stale ones ──

#[test]
fn test_canonical_counts_no_stale_values() {
    let index = read("docs/index.html");
    let readme = read("README.md");
    let man = read("src/ops/man.rs");

    // Canonical values must be present on the public surfaces.
    // Tool count is the named row count in docs/index.html #tools (28), not 30+.
    // Command counts (130+ / 100+) are not advertised — do not reintroduce them.
    assert!(
        index.contains("28 AI safety tools"),
        "landing page lost canonical '28 AI safety tools'"
    );
    assert!(
        readme.contains("28 AI safety tools"),
        "README lost canonical '28 AI safety tools'"
    );
    assert!(
        man.contains("28 AI safety"),
        "man index lost canonical '28 AI safety'"
    );

    // Stale values must never reappear.
    let forbidden: &[(&str, &str)] = &[
        ("docs/index.html", "25 AI safety"),
        ("docs/index.html", "<b>25</b>"),
        ("docs/index.html", "27 tools"),
        ("docs/index.html", "<b>27</b>"),
        ("docs/index.html", "30+ AI safety"),
        ("docs/index.html", "130+"),
        ("docs/index.html", "100+ command"),
        ("README.md", "25 AI safety"),
        ("README.md", "| 25 tools"),
        ("README.md", "30+ AI safety"),
        ("README.md", "130+"),
        ("src/ops/man.rs", "22 AI safety"),
        ("src/ops/man.rs", "30+ AI safety"),
        ("src/ops/man.rs", "and 90+"),
        ("src/ops/man.rs", "130+"),
    ];
    let mut hits = Vec::new();
    for (file, needle) in forbidden {
        let content = match *file {
            "docs/index.html" => &index,
            "README.md" => &readme,
            "src/ops/man.rs" => &man,
            _ => unreachable!(),
        };
        if content.contains(needle) {
            hits.push(format!("{file} still contains stale {needle:?}"));
        }
    }
    assert!(hits.is_empty(), "stale doc values found: {hits:?}");
}

// ── Guard 4: no surface hardcodes a version different from Cargo's ───────────

#[test]
fn test_docs_version_matches_cargo() {
    let version = env!("CARGO_PKG_VERSION");
    // (file, marker) pairs whose embedded version stamp must equal Cargo's.
    // NOTE: historical "introduced-in v0.8.3+" notes (e.g. in
    // ide-behavior-contract.md) are intentionally NOT checked — they record when
    // a behavior landed, not the current version.
    let stamps: &[(&str, &str)] = &[
        ("docs/docs.html", "EnvForge v"),
        ("docs/index.html", "EnvForge v"),
        ("docs/cli-reference.md", "Generated for EnvForge v"),
        ("docs/api-reference.md", "API Reference v"),
    ];
    for (rel, marker) in stamps {
        let content = read(rel);
        let mut idx = 0;
        while let Some(pos) = content[idx..].find(marker) {
            let start = idx + pos + marker.len();
            let found: String = content[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !found.is_empty() {
                assert_eq!(
                    &found, version,
                    "{rel} hardcodes a v{found} stamp but Cargo.toml is {version}"
                );
            }
            idx = start;
        }
    }
}

// ── Guard 5: advertised test-count floor stays true (D3) ─────────────────────

#[test]
fn test_advertised_test_floor_is_true() {
    // Public surfaces advertise a floor; CI verifies the floor is real so the
    // number never has to be hand-maintained to an exact value again.
    const FLOOR: usize = 2800;
    let counted = count_test_attrs("tests") + count_test_attrs("src");
    assert!(
        counted >= FLOOR,
        "advertised '{FLOOR}+ tests' is no longer true: only {counted} test attributes found. \
         Lower the floor in docs/FACTS.md, README, and the landing page (and this test), \
         or add tests."
    );

    let index = read("docs/index.html");
    let readme = read("README.md");
    assert!(
        !index.contains("2,800+"),
        "landing hero should not advertise the test-count floor (README only)"
    );
    assert!(
        readme.contains("2,800+"),
        "README lost the '2,800+ tests' floor claim"
    );
}
