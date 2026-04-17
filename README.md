# EnvForge

A powerful terminal-based environment variable manager with TUI and CLI interfaces for Linux and macOS.

EnvForge safely manages environment variables in your shell configuration files (`.zshrc`, `.bashrc`, etc.) with byte-for-byte round-trip fidelity, meaning it never corrupts your existing config.

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-blue.svg)

## Features at a Glance

| Category | Highlights |
|----------|-----------|
| **Core** | Safe parsing, soft-delete, atomic writes, auto backups, SHA-256 verification |
| **TUI** | Vim-style navigation, fuzzy search, grouping, value masking, mouse support |
| **CLI** | 50+ subcommands, `--json` output, `--dry-run` preview, shell completions |
| **Run** | `envforge run` — subprocess ENV injection with profile, resolve, .env file support |
| **Schema** | `.env.schema` — type validation, onboarding wizard, docs generation, drift detection |
| **Profiles** | Dev/staging/prod environments, shared + profile-specific files, profile diff |
| **Encryption** | Age (X25519) encryption at rest, per-value encrypt/decrypt |
| **Remote Sync** | Git-based cross-machine sync, selective keys, machine overrides, rollback |
| **Secret Managers** | 7 providers (Vault, AWS SSM, 1Password, Doppler, Infisical, GCP, Azure) |
| **AI Safety** | Safe export with redaction, `.envforgeignore`, AI-aware doctor checks |
| **Git Merge** | Custom merge driver for `.env` files — semantic three-way merge |
| **Health Check** | `envforge doctor` with 10 checks and actionable fix suggestions |
| **Security** | Secret scanning, value masking, encrypted credential storage |

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

## Quick Start

```bash
# First run — interactive setup wizard
envforge

# List your ENV variables
envforge list

# Add or update a variable
envforge set DATABASE_URL=postgres://localhost/mydb

# Run a command with managed ENV
envforge run -- npm start

# Run with a specific profile
envforge run --profile prod -- docker compose up

# Check system health
envforge doctor

# Launch the TUI
envforge
```

## Core Features

### Subprocess Runner (`envforge run`)

Run any command with EnvForge-managed environment variables injected at runtime. Variables only live in the child process — nothing is written to your shell config.

```bash
# Basic — inject all ENV vars into a command
envforge run -- npm start
envforge run -- docker compose up
envforge run -- python manage.py runserver

# Profile selection — switch environments instantly
envforge run --profile dev -- npm start
envforge run --profile prod -- npm start

# Secret resolution — decrypt ENC[age:...] and resolve ref: at runtime
envforge run --resolve -- npm start

# Load additional .env files (override order: shell < shared < profile < env-file)
envforge run --env-file .env.local -- npm start
envforge run --env-file .env.base --env-file .env.local -- npm test

# Override specific variables
envforge run --override PORT=9090 --override DEBUG=true -- npm start

# Preview what would be injected (without running the command)
envforge run --dry-run -- npm start
envforge run --dry-run --json -- npm start

# Combine everything
envforge run --profile staging --resolve --env-file .env.ci --override LOG_LEVEL=debug -- npm test
```

**Why use `envforge run`?**
- Secrets never touch shell history or `.zshrc`
- Switch profiles without restarting your shell
- Encrypted values auto-decrypted at runtime
- Secret references resolved with cache (fast startup)
- Exit code and signals passed through transparently

### ENV Schema (`.env.schema`)

Define a schema for your project's environment variables — types, requirements, defaults, and descriptions. Use it for validation, onboarding, documentation, and drift detection.

**Create a `.env.schema` in your project root:**

```toml
[DATABASE_URL]
type = "url"
required = true
description = "PostgreSQL connection string"
pattern = "^postgres://"

[PORT]
type = "port"
required = true
default = "3000"
description = "HTTP server port"

[DEBUG]
type = "bool"
required = false
default = "false"

[NODE_ENV]
type = "enum"
required = true
values = ["development", "staging", "production"]
description = "Application environment"

[API_KEY]
type = "string"
required = true
sensitive = true
pattern = "^sk-[a-zA-Z0-9]{32,}$"

# Environment-specific overrides
[DATABASE_URL.production]
pattern = "^postgres://prod-"

[DEBUG.production]
default = "false"
```

**Supported types:** `string`, `number`, `bool`, `url`, `email`, `enum`, `regex`, `port`

**Use the schema:**

```bash
# Validate current ENV against schema
envforge validate --schema .env.schema

# Validate a specific .env file
envforge validate --schema .env.schema --env .env.production

# Validate with environment-specific rules
envforge validate --schema .env.schema --env .env.production --environment production

# CI/CD pipeline (exits with code 1 on errors)
envforge validate --schema .env.schema --env .env.production || exit 1
```

**Generate a schema from existing ENV:**

```bash
# Auto-detect types from current values
envforge schema generate

# Save to file
envforge schema generate --output .env.schema
```

**Generate documentation:**

```bash
# Markdown table to stdout
envforge docs --schema .env.schema

# Save to file
envforge docs --schema .env.schema --output ENV.md
```

Output:
```
# Environment Variables

| Variable | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| API_KEY [sensitive] | string | Yes | — | |
| DATABASE_URL | url | Yes | — | PostgreSQL connection string |
| NODE_ENV | enum | Yes | — | Application environment |
| PORT | port | Yes | 3000 | HTTP server port |
| DEBUG | bool | No | false | |
```

**Detect drift across environments:**

```bash
envforge drift --envs .env.dev .env.staging .env.production
```

Output:
```
Variable                       .env.dev             .env.staging         .env.production
------------------------------ -------------------- -------------------- --------------------
API_URL                        http://localhos...   https://staging...   https://api.prod...
DB_HOST                        localhost            staging-db           (missing)
DEBUG                          true                 false                false
NODE_ENV                       development          staging              production

0 same, 3 differ, 1 missing across 3 environments
```

**Interactive onboarding for new developers:**

```bash
# Walk through each variable interactively
envforge init --schema .env.schema

# Specify output path
envforge init --schema .env.schema --output .env.local
```

### Safe Shell File Parsing

EnvForge parses shell files into a line-level AST that preserves every byte — comments, blank lines, formatting, and ordering. A parse-serialize round-trip produces a byte-identical file.

- **Atomic writes** — Every file write uses tempfile + rename. No partial files on crash.
- **Auto backup** — A backup is created before every write. Last 10 retained automatically.
- **SHA-256 verification** — Detects external changes between reads and writes.
- **Protected zones** — Conda, Amazon Q, and other managed blocks are never modified.
- **Soft delete** — Nothing is physically removed. Entries are commented with tags and can be restored.

### TUI Interface

Launch with `envforge` (no arguments):

| Key | Action |
|-----|--------|
| `j/k` or arrows | Navigate |
| `Space` | Toggle active/passive |
| `e` | Edit value |
| `a` | Add new variable |
| `d` | Delete (soft) |
| `r` | Restore deleted |
| `u` | Undo last operation |
| `c` / `C` | Copy value / KEY=VALUE |
| `m` | Move to reference file |
| `v` | Toggle value masking |
| `/` | Fuzzy search |
| `g` | Toggle grouping |
| `P` | Switch profile |
| `I` / `E` | Import / Export .env |
| `S` | Save changes |
| `?` | Help |
| `q` | Quit |

Additional TUI features:
- **Mouse support** — Click to select, scroll to navigate
- **Value masking** — Keys containing SECRET, TOKEN, PASSWORD auto-masked
- **Grouping** — Auto-detected prefix groups (DB_\*, AWS_\*) + custom groups, collapsible
- **Fuzzy search** — Type "dbh" to find DB_HOST, with match highlighting

### CLI Reference

All commands support `--json` for machine-readable output and `--dry-run` for preview.

```bash
# Variable management
envforge list [--json]                   # List all variables
envforge get KEY [--json]                # Get a value
envforge set KEY=VALUE                   # Set or create
envforge delete KEY                      # Soft-delete
envforge copy KEY                        # Copy to clipboard
envforge move KEY                        # Move to reference file

# Import / Export
envforge import file.env [--force]       # Import from .env
envforge export [path]                   # Export to .env format
  --exclude-sensitive                    # Skip SECRET, TOKEN, PASSWORD keys
  --safe                                 # Redact sensitive values as [REDACTED]
  --env-example                          # Generate .env.example from schema
  --filter PATTERN                       # Only matching keys

# Run
envforge run [flags] -- <cmd> [args]     # Run command with injected ENV
  --profile NAME                         # Use specific profile
  --resolve                              # Resolve secrets and decrypt
  --env-file PATH                        # Load .env file (repeatable)
  --override KEY=VALUE                   # Override a value (repeatable)

# Schema & validation
envforge validate                        # Validate against config rules
  --schema PATH                          # Use .env.schema
  --env PATH                             # Validate specific .env file
  --environment NAME                     # Apply env-specific schema overrides
envforge schema generate [--output PATH] # Generate schema from current ENV
envforge docs --schema PATH              # Generate Markdown docs
envforge drift --envs FILE1 FILE2 ...    # Compare environments
envforge init --schema PATH              # Interactive onboarding

# Analysis
envforge duplicates                      # Find duplicate keys
envforge scan [path] [--staged]          # Scan for leaked secrets
envforge diff                            # Show pending changes
envforge doctor [--verbose]              # Health check with fix suggestions

# Encryption
envforge encrypt KEY                     # Encrypt a value (age/X25519)
envforge decrypt KEY                     # Decrypt a value

# Profiles
envforge profile list                    # List all profiles
envforge profile switch NAME             # Switch active profile
envforge profile create NAME             # Create a new profile
envforge profile delete NAME             # Delete a profile
envforge profile diff A B               # Compare two profiles side-by-side

# Git merge driver
envforge git install-merge-driver        # Register semantic .env merge driver
envforge git remove-merge-driver         # Unregister merge driver

# History & config
envforge log [KEY] [-n N]                # View change history
envforge config                          # Show current configuration
envforge backup list                     # List available backups
envforge backup restore FILE             # Restore from backup
envforge completions zsh|bash|fish       # Generate shell completions
```

### Profiles

Manage different environment sets for dev/staging/prod:

```
~/.env_managed.shared    # Always loaded (common ENVs)
~/.env_managed.dev       # Dev-specific values
~/.env_managed.prod      # Prod-specific values
```

Profile-specific values override shared values. Switch instantly:

```bash
envforge profile switch prod

# Compare what differs between profiles
envforge profile diff dev prod

# Run with a specific profile without switching
envforge run --profile prod -- npm start
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
envforge sync pull                              # Pull latest from remote
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
envforge sync machine                           # Show machine identity
```

**How it works:**
- Sync data lives in `~/.envforge/sync/` — a separate Git repo that never touches your shell config.
- You choose which keys to sync (`--sync`) and which stay local (`--local`).
- Each machine has a unique ID and can set overrides that take precedence over shared values.
- Offline-first: everything works locally, remote sync is optional.

### Secret Manager Integration

Pull, push, and reference secrets from 7 providers:

| Provider | Binary | Auth Method |
|----------|--------|-------------|
| HashiCorp Vault | `vault` | Token, AppRole |
| AWS SSM Parameter Store | `aws` | Access key, profile, IAM role |
| 1Password | `op` | Service account token |
| Doppler | `doppler` | Service token |
| Infisical | `infisical` | Token, machine identity |
| GCP Secret Manager | `gcloud` | Application default credentials |
| Azure Key Vault | `az` | Azure CLI login |

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

# Reference mode (lazy resolve with cache)
envforge secrets ref DB_URL --from vault --path secret/myapp/DB_URL
envforge secrets resolve                        # Resolve all references
envforge secrets resolve --key DB_URL           # Resolve specific key

# Use in shell init (.zshrc / .bashrc)
eval "$(envforge secrets resolve)"

# Or use envforge run for process-scoped secrets (recommended)
envforge run --resolve -- npm start

# Manage providers
envforge secrets providers                      # List all 7 with status
envforge secrets status                         # Show configured providers
envforge secrets config vault --show            # Show stored credentials
```

**Three modes:**
- **Pull** — Import secrets once as local ENV entries
- **Reference** — Lazy resolve at shell init with TTL cache (`eval "$(envforge secrets resolve)"`)
- **Run** — Process-scoped injection: `envforge run --resolve -- cmd` (secrets never touch disk)

### Health Check

Run `envforge doctor` to check the entire system:

```
$ envforge doctor
✓ Config             — loaded OK
⚠ Encryption key     — no age key yet
                       → Run: envforge encrypt <KEY> to generate a key and encrypt a value
✓ Shell files        — 3 file(s) parsed, 112 entries
⚠ Duplicates         — 1 duplicate key(s) found
                       → Run: envforge duplicates to see details and resolve them
✓ Validation         — no rules configured
✓ References         — 0 reference(s), 0 encrypted
✓ AI safety          — no .env in project root
✓ Sync               — in sync, no local changes
✓ Providers          — 0/7 binaries found
✓ Credentials        — no providers configured

  10 checks: 8 ok, 2 warning(s), 0 error(s)
```

Checks: config, encryption key, shell files, duplicates, validation, references, AI safety, sync, providers, credentials. Every warning includes an actionable fix suggestion.

Use `--verbose` for details, `--json` for machine-readable output.

### AI Agent Safety

Protect your secrets from leaking into AI coding tools (Claude Code, Cursor, Copilot):

```bash
# Export with sensitive values redacted — safe to share with AI
envforge export --safe
# Output:
# NODE_ENV=development
# PORT=3000
# API_KEY=[REDACTED]
# DATABASE_URL=[REDACTED]

# Save to file
envforge export --safe --output .env.safe

# Generate .env.example from schema (placeholders, no real values)
envforge export --env-example
```

Create a `.envforgeignore` file in your project root to mark files AI tools should skip:

```
# .envforgeignore
.env
.env.local
.env.production
credentials.toml
*.key
*.pem
```

`envforge doctor` detects when `.env` exists without `.envforgeignore` and warns you:

```
⚠ AI safety — .env found but no .envforgeignore — secrets may leak to AI tools
  → Create .envforgeignore or run: envforge export --safe for redacted output
```

### Git Merge Driver

EnvForge can act as a custom Git merge driver for `.env` files, understanding key=value semantics:

```bash
# One-time setup
envforge git install-merge-driver

# Now git merge handles .env files intelligently:
# - Different keys added on each side → auto-merged
# - Same key, same value → kept
# - Same key, different values → real conflict (with clear context)

# Example conflict output:
# <<<<<<< ours
# DB_HOST=localhost
# =======
# DB_HOST=staging
# >>>>>>> theirs

# Uninstall when no longer needed
envforge git remove-merge-driver
```

### Security

- **Encryption at rest** — Encrypt individual values with age (X25519): `envforge encrypt API_KEY`
- **Process-scoped secrets** — `envforge run --resolve` injects secrets only into the child process
- **AI-safe export** — `envforge export --safe` redacts sensitive values for AI tool consumption
- **Secret scanning** — Detect leaked secrets in source code: `envforge scan --staged` (use as pre-commit hook)
- **Credential storage** — Provider credentials encrypted with age in `~/.config/envforge/credentials.toml`
- **Value masking** — Keys containing SECRET, TOKEN, PASSWORD, CREDENTIAL are masked in TUI by default
- **No plain-text secrets in history** — Encrypted values stay encrypted in backups and sync
- **Schema-based sensitive marking** — Schema `sensitive = true` flag for custom sensitive key patterns

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
EMAIL = "email"
CUSTOM = "regex:^[A-Z]{3}-\\d{4}$"
```

For project-level validation, create a `.env.schema` file (see [ENV Schema](#env-schema-envschema) section above). Schema rules merge with `config.toml` validation, with schema taking priority.

### Shell Completions

```bash
# Zsh
envforge completions zsh > ~/.zsh/completions/_envforge

# Bash
envforge completions bash > /etc/bash_completion.d/envforge

# Fish
envforge completions fish > ~/.config/fish/completions/envforge.fish
```

## Architecture

```
src/
├── main.rs          # Entry point — routes to TUI or CLI
├── lib.rs           # Module declarations
├── model/           # Data types (LineNode, ShellFile, ExportStyle)
├── parser/          # Shell file parser & writer (round-trip safe)
├── config/          # App config, backup, atomic writes
├── ops/             # Operations
│   ├── crud.rs      # Add, edit, delete, move, toggle
│   ├── run.rs       # Subprocess runner (envforge run)
│   ├── schema.rs    # ENV schema parser, validation, docs, drift
│   ├── profile.rs   # Profile management and diff
│   ├── encrypt.rs   # Age encryption/decryption
│   ├── duplicates.rs # Duplicate key detection
│   ├── validation.rs # Rule-based value validation
│   ├── scanner.rs   # Secret scanning
│   ├── doctor.rs    # System health checks (10 checks including AI safety)
│   ├── dotenv.rs    # .env parsing, safe export, env-example generation
│   ├── sync/        # Remote sync (Git, push/pull, conflict, machine overrides)
│   └── secrets/     # Secret manager integration (7 providers, credentials, cache)
├── ui/              # Ratatui TUI (app state, rendering, dialogs)
└── cli/             # Clap CLI (subcommands, sync, secrets, wizard)
```

### Design Principles

1. **Never delete** — EnvForge never physically removes content. Soft-delete with tags.
2. **Atomic writes** — Every write uses tempfile + rename. No partial files on crash.
3. **Order preservation** — Line-level AST preserves exact line order and formatting.
4. **Offset safety** — Protected zones (conda, Amazon Q blocks) are never modified.
5. **Backup first** — Automatic backup before every write operation.
6. **Schema-driven** — `.env.schema` as single source of truth for project ENV requirements.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License. See [LICENSE](LICENSE) for details.
