# EnvForge v0.1.0 — Initial Release

A powerful terminal-based environment variable manager for Linux and macOS. Safely manage ENV variables in your shell config files with a TUI and CLI interface.

## Highlights

- **Zero-corruption guarantee** — Line-level AST parser with byte-for-byte round-trip fidelity. Your `.zshrc` is never broken.
- **Never deletes anything** — All operations use soft-delete with reversible tags.
- **TUI + CLI** — Interactive terminal UI with vim-style navigation, or scriptable CLI with JSON output.

## Core Features

### Management
- Parse `.zshrc`, `.bashrc`, `.bash_profile`, `.profile`, `.zprofile`
- Add, edit, soft-delete, restore ENV variables
- Reference file strategy (`~/.env_managed` with source injection)
- Protected zone detection (conda, Amazon Q, nvm blocks — never modified)
- Atomic writes (tempfile + rename) with automatic backup

### TUI Interface
- Table view with keyboard navigation (j/k) and mouse support
- Fuzzy search with match highlighting — type "dbh" to find `DB_HOST`
- Collapsible ENV grouping (auto-detected prefixes + user-defined)
- Active/passive toggle with Space key
- Value masking for sensitive keys
- Full in-session undo history
- Import/export from TUI

### Environment Profiles
- Multiple profiles: dev, staging, prod
- Shared ENVs across all profiles
- Profile switching with `P` key or `envforge profile switch`
- Last used profile remembered

### Security
- ENV encryption at rest (age/X25519)
- Secret scanning — detect leaked values in source code
- Git staged file scanning for pre-commit hooks
- Value masking in TUI and changelog

### Developer Tools
- `.env` file import/export
- Duplicate key detection with resolution
- ENV validation rules (url, number, bool, email, regex)
- Automatic change log with CLI viewer
- Shell completions (zsh, bash, fish)
- JSON output and `--dry-run` for all commands

## Installation

```bash
cargo install env-forge-tui
```

## Quick Start

```bash
# Launch TUI (first run triggers setup wizard)
envforge

# Or use CLI
envforge list
envforge set DATABASE_URL="postgres://localhost/mydb"
envforge profile create dev
envforge scan --staged
```

## Supported Platforms

| Platform | Architecture |
|----------|-------------|
| Linux | x86_64, aarch64 |
| macOS | x86_64, Apple Silicon |

## Links

- [Documentation](https://github.com/emreerinc/envforge#readme)
- [Contributing Guide](https://github.com/emreerinc/envforge/blob/main/CONTRIBUTING.md)
- [crates.io](https://crates.io/crates/env-forge-tui)
