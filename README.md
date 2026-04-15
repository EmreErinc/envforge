# EnvForge

A powerful terminal-based environment variable manager with TUI and CLI interfaces for Linux and macOS.

EnvForge safely manages environment variables in your shell configuration files (`.zshrc`, `.bashrc`, etc.) with byte-for-byte round-trip fidelity, meaning it never corrupts your existing config.

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-blue.svg)

## Features

### Core
- **Safe parsing** — Line-level AST preserves every byte. Parse-serialize round-trip is always identical.
- **Soft delete** — Nothing is ever physically deleted. Entries are commented with tags and can be restored.
- **Atomic writes** — All file writes use tempfile + rename. Crash-safe by design.
- **Auto backup** — Backup created before every write. Last 10 retained.

### TUI Interface
- **Table view** — KEY / VALUE / LOCATION columns with sorting and filtering
- **Keyboard-driven** — Full vim-style navigation (j/k, search with /, etc.)
- **Mouse support** — Click to select, scroll to navigate
- **Value masking** — Sensitive values (SECRET, TOKEN, PASSWORD) masked by default
- **Grouping** — Auto-detected prefix groups (DB_*, AWS_*) + user-defined groups, collapsible
- **Active/passive toggle** — Space key to enable/disable ENV entries
- **Fuzzy search** — Type "dbh" to find DB_HOST, with match highlighting
- **Undo history** — Full in-session undo with `u` key
- **Profile switching** — Press `P` to switch between dev/staging/prod
- **Import/Export** — `I` to import from .env, `E` to export

### CLI Interface
```
envforge list [--json]              # List all variables
envforge get KEY [--json]           # Get a value
envforge set KEY=VALUE              # Set or create
envforge delete KEY                 # Soft-delete
envforge copy KEY                   # Copy to clipboard
envforge move KEY                   # Move to reference file
envforge import file.env [--force]  # Import from .env
envforge export [path]              # Export to .env
envforge duplicates                 # Find duplicate keys
envforge scan [path] [--staged]     # Scan for leaked secrets
envforge validate                   # Check validation rules
envforge encrypt KEY                # Encrypt a value (age)
envforge decrypt KEY                # Decrypt a value
envforge profile list|switch|create|delete
envforge sync <subcommand>          # Sync across machines (see below)
envforge secrets <subcommand>       # Secret manager integration (see below)
envforge log [KEY] [-n N]           # View change history
envforge completions zsh|bash|fish  # Shell completions
envforge config                     # Show config
envforge diff                       # Show pending changes
envforge --dry-run <command>        # Preview without writing
```

### Remote Sync

Sync your environment variables across machines using Git:

```bash
# Initialize sync repository
envforge sync init
envforge sync init --remote git@github.com:user/envforge-sync.git

# Choose which keys to sync
envforge sync mark --all --sync                 # Sync all keys
envforge sync mark DB_HOST --sync               # Sync a single key
envforge sync mark "AWS_*" --sync               # Sync by glob pattern
envforge sync mark SECRET_KEY --local           # Keep a key local-only
envforge sync list-keys                         # View sync/local status

# Push and pull
envforge sync push                              # Export & push to remote
envforge sync push -m "updated DB config"       # Custom commit message
envforge sync push --dry-run                    # Preview without pushing
envforge sync pull                              # Pull latest from remote
envforge sync pull --dry-run                    # Preview incoming changes
envforge sync status                            # Show local vs remote diff

# Machine-specific overrides
envforge sync override DB_HOST localhost        # Override for this machine
envforge sync override DB_HOST --remove         # Remove override
envforge sync override --list dummy             # List all overrides

# History and rollback
envforge sync history                           # View snapshot history
envforge sync rollback --last                   # Rollback to previous
envforge sync rollback abc1234                  # Rollback to specific commit
envforge sync log                               # View operation log

# Machine info
envforge sync machine                           # Show machine ID
```

**How it works:**
- Sync data lives in `~/.envforge/sync/` — a separate Git repo that never touches your existing config.
- You choose which keys to sync (`--sync`) and which stay local (`--local`).
- Each machine has a unique ID and can set overrides that take precedence over shared values.
- Offline-first: everything works locally, remote sync is optional.
- All commands support `--json` for scripting and `--dry-run` for preview.

### Secret Manager Integration

Pull and push secrets from external secret managers:

```bash
# Configure provider credentials (encrypted with age)
envforge secrets config vault --set addr=https://vault.example.com
envforge secrets config vault --set token=hvs.xxx

# Pull secrets from a provider
envforge secrets pull --from vault --path secret/myapp
envforge secrets pull --from aws-ssm --path /myapp/prod --filter "DB_*"

# Push secrets to a provider
envforge secrets push --to vault --path secret/myapp --keys DB_URL,API_KEY
envforge secrets push --to doppler --all

# Reference mode (lazy resolve, cached)
envforge secrets ref DB_URL --from vault --path secret/myapp/DB_URL
envforge secrets resolve

# Manage providers
envforge secrets providers                       # List all (7 supported)
envforge secrets status                          # Show configured providers
```

**Supported providers**: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault

**Three modes**: Pull (import once), Reference (lazy resolve with cache), Push (export to manager)

### Profiles
Manage different environment sets for dev/staging/prod:
```
~/.env_managed.shared    # Always loaded (common ENVs)
~/.env_managed.dev       # Dev profile
~/.env_managed.prod      # Prod profile
```
Profile-specific values override shared values. Last used profile remembered.

### Remote Sync
- **Git-based sync** — Sync ENV variables across machines using any Git remote (GitHub, GitLab, etc.)
- **Selective sync** — Choose which keys to sync and which stay local-only, with glob pattern support
- **Machine overrides** — Per-machine values that override shared config (e.g. different DB_HOST per machine)
- **Conflict resolution** — Three-way merge with keep-local, keep-remote, or manual edit strategies
- **Offline-first** — Everything works locally, remote is optional. Full history via Git.
- **Rollback** — Restore any previous snapshot with automatic backup

### Security
- **ENV encryption** — Encrypt sensitive values at rest with `age` (X25519)
- **Secret scanning** — Detect leaked secrets in source code (`envforge scan --staged` as pre-commit hook)
- **Value masking** — Sensitive keys never displayed in plain text by default

## Installation

### From source (Rust 1.75+)
```bash
git clone https://github.com/emreerinc/envforge.git
cd envforge
cargo install --path .
```

### Cargo
```bash
cargo install env-forge-tui
```

### Shell Completions
```bash
# Zsh
envforge completions zsh > ~/.zsh/completions/_envforge

# Bash
envforge completions bash > /etc/bash_completion.d/envforge

# Fish
envforge completions fish > ~/.config/fish/completions/envforge.fish
```

## Quick Start

```bash
# First run — setup wizard
envforge

# Or skip wizard with defaults
envforge config

# List your ENV variables
envforge list

# Search (fuzzy)
envforge list --filter dbh

# Import from .env file
envforge import .env

# Create profiles
envforge profile create dev
envforge profile create prod
envforge profile switch dev

# Encrypt sensitive values
envforge encrypt API_KEY

# Scan for secrets before commit
envforge scan --staged
```

## Configuration

Config file: `~/.config/envforge/config.toml`

```toml
[general]
default_shell = "zsh"

[files]
primary = "~/.zshrc"
reference = "~/.env_managed"
use_reference_file = true

[offsets]
header_protected_lines = 0
footer_protected_lines = 0

[protected_blocks]
markers = []

[groups]
database = ["DB_*", "DATABASE_*", "PG_*"]
aws = ["AWS_*"]

[profiles]
active = "dev"
shared_file = "~/.env_managed.shared"

[profiles.dev]
file = "~/.env_managed.dev"

[profiles.prod]
file = "~/.env_managed.prod"

[validation]
DATABASE_URL = "url"
PORT = "number"
DEBUG = "bool"
API_KEY = "nonempty"
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j/k` or `↑/↓` | Navigate |
| `Space` | Toggle active/passive |
| `e` | Edit value |
| `a` | Add new variable |
| `d` | Delete (soft) |
| `r` | Restore deleted |
| `u` | Undo last operation |
| `c` / `C` | Copy value / KEY=VALUE |
| `m` | Move to reference file |
| `v` | Toggle value mask |
| `/` | Fuzzy search |
| `g` | Toggle grouping |
| `→/Enter` | Expand group |
| `←` | Collapse group |
| `P` | Switch profile |
| `I` | Import from .env |
| `E` | Export to .env |
| `S` / `Ctrl+S` | Save changes |
| `?` | Help |
| `q` | Quit |

## Architecture

```
src/
├── main.rs          # Entry point — routes to TUI or CLI
├── lib.rs           # Module declarations
├── model/           # Data types (LineNode, ShellFile, ExportStyle)
├── parser/          # Shell file parser & writer (round-trip safe)
├── config/          # App config, backup, atomic writes
├── ops/             # Operations (CRUD, profiles, groups, encryption, etc.)
│   └── sync/        # Remote sync (Git wrapper, push/pull, conflict, machine overrides)
├── ui/              # Ratatui TUI (app state, rendering, dialogs)
└── cli/             # Clap CLI (subcommands, sync commands, wizard)
```

### Design Principles

1. **Never delete** — EnvForge never physically removes content. Soft-delete with tags.
2. **Atomic writes** — Every write uses tempfile + rename. No partial files on crash.
3. **Order preservation** — Line-level AST preserves exact line order and formatting.
4. **Offset safety** — Protected zones (conda, Amazon Q blocks) are never modified.
5. **Backup first** — Automatic backup before every write operation.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License. See [LICENSE](LICENSE) for details.
