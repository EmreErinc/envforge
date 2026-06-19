# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
cargo test --lib               # Library tests only
cargo test --test parser_tests # Run specific test file
cargo test -- <pattern>        # Run tests matching pattern
cargo fmt                      # Auto-format code
cargo fmt --check              # Check formatting without changes
cargo clippy -- -D warnings    # Lint (warnings as errors)
```

**Pre-commit verification:**
```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

## Project Overview

EnvForge is a Rust CLI + TUI tool for managing environment variables in shell configuration files. It provides AI safety tools, secret provider integrations (13 providers), encrypted sync, and runs on Linux/macOS.

### Architecture

```
src/
├── main.rs   # Entry point — routes to TUI or CLI
├── lib.rs    # Module declarations
├── model/    # Data types (LineNode, ShellFile, ExportStyle, errors)
├── parser/   # Shell file parser & writer (byte-for-byte round-trip safe)
├── config/   # App config, backup, atomic writes
├── ops/      # Core operations (40+ modules incl. data-driven fence registry) — pure business logic
│   ├── sync/     # Git-based cross-machine sync
│   └── secrets/  # 13 secret provider integrations
├── ui/       # Ratatui TUI (app state, rendering, dialogs)
├── cli/      # Clap CLI (80+ subcommands)
├── lsp/      # Language Server Protocol for IDE extensions
└── mcp/      # MCP server (`envforge mcp serve`) — read-safe, gated on `mcp-server` feature
```

### Running the Application

```bash
envforge              # Launch TUI
envforge <cmd>        # Run CLI command (80+ subcommands)
envforge lsp          # Start LSP server for IDE extensions
```

## Key Design Principles

1. **Parser is the foundation** — all other modules depend on it. Parse → serialize must be byte-identical.
2. **ops/ contains pure business logic** — no I/O decisions, no UI.
3. **ui/ and cli/ are thin layers** that call ops/.
4. **Never delete** — Soft-delete with tags, nothing physically removed.
5. **Atomic writes** — Every write uses tempfile + rename.
6. **Schema-driven** — `.env.schema` as single source of truth for project ENV requirements.
7. **AI-safe by default** — Volatile mode, redaction, fencing built into core workflows.
8. **Offset safety** — Protected zones (conda, Amazon Q blocks) never modified.
9. **Zero trust** — Secrets encrypted at rest, in transit, and in memory.

## Dependencies

- Minimum Rust: 1.75
- Key crates: ratatui (0.30), crossterm (0.28), clap (4), tokio, age, serde, insta

## Testing Conventions

- **All tests** live in `tests/` directory (no in-module tests)
- **Test file naming:** `{feature}_tests.rs` (e.g., `parser_tests.rs`)
- **Test function naming:** `test_{what_is_being_tested}_{condition}`
- Use `insta` for snapshot testing
- Use `tempfile` for filesystem operations

Run single test with output:
```bash
cargo test test_name -- --nocapture --test-threads=1
```

## Error Handling

- Use `thiserror::Error` for all custom error types (defined in `src/model/error.rs`)
- Error types include context (e.g., file paths)
- Propagate errors with `?` operator; avoid `.unwrap()` in library code

## Adding a New Feature

1. Define operation in `src/ops/` as pure function (no CLI/UI knowledge)
2. Add CLI integration in `src/cli/mod.rs` (subcommand) and `src/cli/commands.rs` (handler)
3. Add TUI integration in `src/ui/app.rs` (key handler) and `src/ui/dialogs.rs` (if popup needed)
4. Add tests in `tests/{feature}_tests.rs`
5. Update help text in `src/ui/dialogs.rs` if user-facing

## AI Safety Context

This tool is specifically designed to protect secrets from AI agents. Key features:
- **Volatile mode** — secrets auto-expire from memory
- **Fence** — blocks AI from reading env vars
- **AI-guard** — detects and blocks prompt injection
- **MCP scan** — hardens MCP server configs
- **Canary tokens** — detect secret exfiltration
- **Redaction** — masks secrets in logs/output

When modifying security features, consider attack vectors from AI systems.