# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build                    # Compile the project
cargo test                     # Run all tests
cargo test -- <pattern>        # Run tests matching pattern
cargo test --test <name>       # Run specific test file
cargo fmt                      # Format code
cargo clippy -- -D warnings    # Lint (strict, warnings as errors)
```

## Project Overview

EnvForge is a Rust CLI + TUI tool for managing environment variables in shell configuration files. It provides AI safety tools, secret provider integrations (13 providers), encrypted sync, and runs on Linux/macOS.

### Architecture

```
src/
├── main.rs        # Entry point — routes to TUI or CLI
├── lib.rs         # Module declarations
├── model/         # Data types (LineNode, ShellFile, ExportStyle)
├── parser/        # Shell file parser & writer (byte-for-byte round-trip safe)
├── config/        # App config, backup, atomic writes
├── ops/           # Core operations (35+ modules) — pure business logic
│   ├── sync/      # Git-based cross-machine sync
│   └── secrets/   # 13 secret provider integrations
├── ui/            # Ratatui TUI (app state, rendering, dialogs)
├── cli/           # Clap CLI (80+ subcommands)
└── lsp/           # Language Server Protocol for IDE extensions
```

### Key Design Principles

1. **parser/ is the foundation** — all other modules depend on it. Parse → serialize must be byte-identical.
2. **ops/ contains pure business logic** — no I/O decisions, no UI
3. **ui/ and cli/ are thin layers** that call ops/
4. **Never delete** — Soft-delete with tags, nothing physically removed
5. **Atomic writes** — Every write uses tempfile + rename
6. **Schema-driven** — `.env.schema` as single source of truth for project ENV requirements

### Dependencies

- Minimum Rust: 1.75
- Key crates: ratatui (0.30), crossterm (0.28), clap (4), tokio, age, serde

### Running the Application

```bash
envforge           # Launch TUI
envforge <cmd>     # Run CLI command (80+ subcommands)
envforge lsp       # Start LSP server for IDE extensions
```

### Key Files

- `Cargo.toml` — Project config, edition 2021, minimum Rust 1.75
- `tests/` — Integration tests (all in root, no in-module tests)
- `docs/` — Additional documentation
- `editors/` — VS Code and IntelliJ extension configs