# EnvForge

The AI-safe environment variable manager. Protect your secrets from AI coding agents while managing env vars across machines, providers, and profiles.

EnvForge is a Rust CLI + TUI tool that safely manages environment variables in shell configuration files (`.zshrc`, `.bashrc`, etc.) with **22 AI safety tools**, 13 secret provider integrations, encrypted sync, and 90+ commands.

![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-blue.svg)
![AI Safe](https://img.shields.io/badge/AI--Safe-Secrets%20Protected-brightgreen.svg)

> **Why AI Safety?** GitGuardian's 2026 report found AI-assisted commits leak secrets at **2x the baseline rate**. 24,000+ credentials were found in MCP config files. EnvForge is the first env management tool built to protect secrets FROM AI agents.

## AI Safety Suite

EnvForge provides the most comprehensive AI-agent secret protection of any CLI tool:

| Layer | Tool | Command | What It Does |
|-------|------|---------|-------------|
| **Prevention** | Secret Fence | `envforge fence` | Create ignore rules for Cursor, Copilot, Claude Code |
| **Prevention** | Fence Status | `envforge fence --status` | Verify if ignore rules are active |
| **Prevention** | Pre-Commit Hook | `envforge scan --install-hook` | Block commits containing secrets |
| **Prevention** | 3-Stage AI Guard | `envforge ai-guard pre-tool` | Scan before/after AI tool execution |
| **Prevention** | AI Hooks | `envforge ai-hook install <tool>` | PreToolUse + PostToolUse hooks |
| **Prevention** | Hook Status | `envforge ai-hook status` | Check which tools have active hooks |
| **Prevention** | File Alerts | *(built into ai-guard)* | Warn on `.env`, `.pem`, `.ssh/` access |
| **Runtime** | Volatile Mode | `envforge run --volatile` | Secrets in memory only — never on disk |
| **Runtime** | Log Redaction | `envforge run --redact` | Mask secrets in subprocess output |
| **Runtime** | Credential Proxy | `envforge proxy --port 8100` | HTTP API with domain allowlist + audit |
| **Runtime** | Session Leases | `envforge lease create --ttl 1h` | Time-bounded secret access |
| **Runtime** | Killswitch | `envforge revoke --all` | Instantly revoke all active leases |
| **Context** | AI-Safe Schema | `envforge schema emit-ai` | Types/names without values for AI |
| **Context** | Safe Export | `envforge export --safe` | Redacted `[REDACTED]` values |
| **Context** | Ignore File | `.envforgeignore` | Mark files AI tools should skip |
| **Remediation** | MCP Scan | `envforge mcp status` | Find creds in AI tool configs |
| **Remediation** | MCP Harden | `envforge mcp harden` | Auto-replace with `${VAR}` references |
| **Remediation** | Prompt Sanitizer | `envforge sanitize FILE` | Strip secrets from any file |
| **Detection** | Canary Secrets | `envforge canary create KEY` | Honeypot credentials — alert on exfiltration |
| **Detection** | AI Leak Audit | `envforge audit --ai-leaks` | Scan git for AI-assisted leaks |
| **Detection** | Access Audit | `envforge audit --access` | JSONL log of proxy access |
| **Governance** | Approval Flow | `envforge proxy --require-approval` | Human approves each secret access |
| **Governance** | Dependency Map | `envforge deps KEY --source` | What breaks if this secret rotates? |
| **Governance** | External Scanner | `ENVFORGE_EXTERNAL_SCANNER=ggshield` | Delegate to 500+ detector engine |

### Quick Setup: Protect a Project in 30 Seconds

```bash
# 1. Create AI ignore rules for all tools
envforge fence

# 2. Verify rules are active
envforge fence --status

# 3. Install hooks for Claude Code or Cursor
envforge ai-hook install claude-code

# 4. Generate AI-safe context file
envforge schema emit-ai --infer --output .env.ai.md

# 5. Scan AI tool configs for leaked credentials
envforge mcp status

# 6. Auto-fix MCP configs
envforge mcp harden
```

Done. Cursor, Copilot, and Claude Code now respect your secret boundaries. AI agents get context from `.env.ai.md` instead of reading `.env`.

### Run with Maximum Protection

```bash
# Secrets in memory only + log redaction (nothing leaks)
envforge run --volatile --redact -- npm start

# Time-bounded access with credential proxy
envforge lease create --ttl 1h --keys DB_URL,API_KEY
envforge proxy --port 8100 --require-lease --allow-origins localhost

# Emergency: revoke all access instantly
envforge revoke --all
```

---

## All Features

| Category | Highlights |
|----------|-----------|
| **AI Safety** | 22 tools: canary, approval flow, guard, leases, killswitch, proxy, fence, MCP harden, deps |
| **Check** | `envforge check` — unified health check (doctor + validate + scan + age + drift) |
| **Snapshots** | Backup/restore active profile state, diff, auto-prune |
| **Explain** | `envforge explain KEY` — X-ray view across all subsystems |
| **Rotation** | `envforge rotate KEY --propagate` — guided rotation with multi-target push |
| **Shell Hook** | direnv-style auto-load: `eval "$(envforge hook zsh)"` |
| **Secure Share** | `envforge share` — age-encrypted secret sharing for team onboarding |
| **Audit Trail** | `envforge audit` + `envforge audit-trail` — change history, SOC2 reports, chain of custody, tamper-evident integrity |
| **CI/CD** | GitHub Action with 5 modes: validate, secrets-pull, export, run, drift |
| **Core** | Safe parsing, soft-delete, atomic writes, auto backups, SHA-256 verification |
| **TUI** | Vim-style navigation, fuzzy search, grouping, value masking, mouse support |
| **Export** | 8 formats: dotenv, JSON, YAML, TOML, Docker, Docker Secrets, K8s, tfvars |
| **CLI** | 80+ subcommands, `--json` output, `--dry-run` preview, shell completions |
| **Run** | Subprocess injection with volatile, redact, multi-profile, resolve |
| **Schema** | `.env.schema` — type validation, JSON Schema, onboarding wizard, drift detection |
| **Projects** | Multi-env project config, wizard setup, `envforge project env diff`, provider pull/push per env |
| **Profiles** | Dev/staging/prod, shared files, profile diff, `--profiles` multi-merge |
| **Encryption** | Age (X25519) encryption at rest, encrypted sync, per-value encrypt/decrypt |
| **Remote Sync** | Git-based cross-machine sync, age-encrypted, selective keys, rollback |
| **Secret Managers** | 13 providers, URI refs (`vault://`), TTL, offline fallback, cache — [Provider Guides](docs/cli-reference.md#provider-integration-guides) |
| **IDE Extensions** | VS Code + IntelliJ — LSP diagnostics, hover, completions, go-to-definition |
| **Git Merge** | Custom merge driver for `.env` files — semantic three-way merge |
| **Health Check** | `envforge doctor` + `envforge check` — 15+ checks with fix suggestions |
| **Security** | Secret scanning, pre-commit hooks, value masking, credential TTL |

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

# Project-scoped setup (multi-env)
envforge project init
envforge project env create production
envforge project env switch production

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
envforge search QUERY [--json]           # Fuzzy search keys/values
envforge get KEY [--json]                # Get a value
envforge set KEY=VALUE                   # Set or create
envforge delete KEY                      # Soft-delete
envforge copy KEY                        # Copy to clipboard
envforge move KEY [NEW_KEY]              # Move to reference or rename

# Import / Export
envforge import file.env [--force]       # Import from .env
envforge export [path]                   # Export to .env format
  --exclude-sensitive                    # Skip SECRET, TOKEN, PASSWORD keys
  --safe                                 # Redact sensitive values as [REDACTED]
  --env-example                          # Generate .env.example from schema
  --filter PATTERN                       # Only matching keys
  --format FMT                           # json, yaml, toml, docker, k8s, tfvars
  --k8s-name NAME                        # K8s Secret name (default: envforge-secrets)
  --k8s-namespace NS                     # K8s namespace (default: default)

# Run
envforge run [flags] -- <cmd> [args]     # Run command with injected ENV
  --profile NAME                         # Use specific profile
  --resolve                              # Resolve secrets and decrypt
  --env-file PATH                        # Load .env file (repeatable)
  --override KEY=VALUE                   # Override a value (repeatable)
  --volatile                             # AI-safe: secrets in memory only
  --redact                               # Mask secrets in subprocess output
  --profiles dev,staging                 # Merge multiple profiles (last wins)

# Schema & validation
envforge validate                        # Validate against config rules
  --schema PATH                          # Use .env.schema
  --env PATH                             # Validate specific .env file
  --environment NAME                     # Apply env-specific schema overrides
envforge schema generate [--output PATH] # Generate schema from current ENV
envforge docs --schema PATH              # Generate Markdown docs
envforge drift --envs FILE1 FILE2 ...    # Compare environments
envforge init --schema PATH              # Interactive onboarding

# Unified check
envforge check                           # Run all checks (doctor+validate+scan+age+drift)
  --only doctor,scan                     # Run specific categories
envforge explain KEY                     # Show all info about a key

# Snapshots
envforge snapshot create [NAME]          # Backup active profile state
envforge snapshot list                   # List all snapshots
envforge snapshot restore [NAME|--last]  # Restore from snapshot
envforge snapshot diff [NAME|--last]     # Compare current vs snapshot
envforge snapshot delete NAME            # Remove a snapshot

# Secret rotation
envforge rotate KEY                      # Interactive secret rotation
  --dry-run                              # Preview without changes
  --stale                                # Rotate all stale secrets (>90 days)
  --propagate                            # Auto-push to provider + sync

# Shell hook
envforge hook zsh|bash|fish              # Generate auto-load hook (for eval)
envforge env [--dir PATH]                # Output export statements

# Secure sharing
envforge share create --recipient KEY    # Create encrypted share file
  --keys K1,K2 | --all | --filter PAT   # Select keys
  --expire HOURS                         # Optional expiry
envforge share receive FILE [--import]   # Decrypt and import

# Audit
envforge audit [--key K] [--since DATE]  # Change audit trail from sync
  --machine ID                           # Filter by machine
  --ai-leaks                             # Scan git for AI-assisted secret leaks
  --access                               # View proxy access audit log
envforge audit-trail query               # Query audit events (filter by type, source, key, time)
  --event-type TYPE --source SRC         # Filter events
  --secret-key KEY --time last_7d        # Scope by key and time
envforge audit-trail report              # Generate SOC2 compliance reports
  --report-type compliance --format md   # Export as markdown report
envforge audit-trail custody             # Trace chain of custody for a secret
  --secret-key KEY --ownership           # Ownership lineage report
envforge audit-trail integrity           # Verify tamper-evident log integrity
envforge audit-trail stats               # Aggregate audit statistics
envforge audit-trail tail                # Tail recent audit events
envforge audit-trail retention           # Manage log retention policy

# Analysis
envforge duplicates                      # Find duplicate keys
envforge scan [path] [--staged]          # Scan for leaked secrets
  --install-hook                         # Install git pre-commit hook
  --remove-hook                          # Remove pre-commit hook
envforge mcp status [--json]             # Scan AI tool configs for secrets
envforge mcp harden                      # Replace MCP secrets with ${VAR} refs
envforge fence [--status]                # Create AI ignore rules or check status
envforge sanitize FILE                   # Strip secrets from any file
envforge ai-hook status [--json]         # Check AI tool security hooks
envforge ai-hook install <tool>          # Install hooks (claude-code, cursor)
envforge proxy [--port 8100] [--keys K]  # Local credential proxy for AI agents
envforge audit --ai-leaks                # Scan git for AI-assisted secret leaks
envforge audit --access [--json]         # View proxy access audit log
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
envforge completions zsh|bash|fish|kiro|fig  # Generate shell completions
envforge completions kiro --install          # Install to correct system path
envforge man [COMMAND]                   # Built-in man pages (offline)
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
- **Encrypted at rest**: snapshots are age-encrypted before git commit; auto-decrypted on pull.
- You choose which keys to sync (`--sync`) and which stay local (`--local`).
- Each machine has a unique ID and can set overrides that take precedence over shared values.
- Offline-first: everything works locally, remote sync is optional.
- Backward compatible: unencrypted repos auto-encrypt on next push.

### Secret Manager Integration

Pull, push, and reference secrets from 13 providers:

| Provider | Binary | Auth Method |
|----------|--------|-------------|
| HashiCorp Vault | `vault` | Token, AppRole |
| AWS SSM Parameter Store | `aws` | Access key, profile, IAM role |
| 1Password | `op` | Service account token |
| Doppler | `doppler` | Service token |
| Infisical | `infisical` | Token, machine identity |
| GCP Secret Manager | `gcloud` | Application default credentials |
| Azure Key Vault | `az` | Azure CLI login |
| Bitwarden Secrets Manager | `bws` | Machine account access token |
| Akeyless Vault | `akeyless` | Access ID + Access Key |
| CyberArk Conjur | `conjur` | API key + account |
| Mozilla SOPS | `sops` | age/PGP/KMS key file |
| pass/gopass | `pass`/`gopass` | GPG keyring |
| Keeper Secrets Manager | `ksm` | Device config (one-time token) |

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
envforge secrets providers                      # List all 13 with status
envforge secrets status                         # Show configured providers
envforge secrets config vault --show            # Show stored credentials
envforge secrets age [--threshold N]             # Show secret ages, flag stale
envforge secrets age --stale-only                # Only stale secrets
envforge secrets diff --from vault --path PATH   # Compare local vs provider
envforge secrets diff --from aws-ssm --filter P  # Diff with filter

# Credential TTL
envforge secrets config vault --set token=x --ttl 8h  # Expire in 8 hours
envforge secrets config aws --set key=x --ttl 30d     # Expire in 30 days

# Schema tools
envforge schema generate [--output PATH] # Generate from current ENV
envforge schema json-schema              # Output JSON Schema for editors
envforge schema emit-ai [--infer]        # AI-safe context (no values)
envforge resolve-uri FILE [--env]        # Resolve vault://, aws-ssm:// URIs
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
✓ Providers          — 0/13 binaries found
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

### Unified Check (`envforge check`)

Run all health and safety checks in one command:

```bash
# Run all checks
envforge check

# Run specific categories only
envforge check --only doctor,scan,age

# JSON output for CI/CD
envforge check --json
```

Output:
```
── Doctor ──────────────────────────────────────────
✓ Config              — loaded OK
⚠ Encryption key      — no age key yet
                        → Run: envforge encrypt <KEY>
✓ Shell files         — 3 file(s) parsed, 112 entries

── Validate ────────────────────────────────────────
✓ Schema validation   — 12 variables checked, all valid

── Scan ────────────────────────────────────────────
✓ Secret scan         — no secrets found in source

── Age (skipped) ───────────────────────────────────
  No tracked secrets. Pull secrets to start tracking.

════════════════════════════════════════════════════
  5 categories: 3 run, 2 skipped
  8 checks: 7 ok, 1 warning(s), 0 error(s)
```

Categories: `doctor`, `validate`, `scan`, `age`, `drift`. Missing prerequisites (no schema, no age data) skip gracefully. Exit code 1 on errors.

### Environment Snapshots

Backup and restore your active profile environment state:

```bash
# Create a snapshot before making changes
envforge snapshot create before-upgrade

# List all snapshots
envforge snapshot list

# See what changed since a snapshot
envforge snapshot diff --last

# Restore to a previous state (auto-backup before restore)
envforge snapshot restore before-upgrade

# Delete a snapshot
envforge snapshot delete before-upgrade
```

Snapshots capture all variables from the active profile (shared + profile-specific). Stored in `~/.config/envforge/snapshots/`. Auto-prunes to 20 most recent.

### Key Explain

Deep dive into everything known about a single key:

```bash
envforge explain DATABASE_URL
envforge explain API_KEY --json
```

Output:
```
── KEY: DATABASE_URL ──────────────────────────────

Source
  File:    ~/.env_managed.dev:15
  Style:   export "..."
  Status:  active

Profile
  Profile: dev (profile-specific)

Schema
  Type:    url
  Required: yes
  Description: PostgreSQL connection string

Encryption: plaintext
Reference:  none
Sync:       synced
Age:        45 days (✓ ok)
```

Shows: source file/line, profile context, schema info, encryption status, secret reference, sync marking, and age tracking. Graceful N/A for missing subsystems.

### Secret Rotation

Guided interactive secret rotation:

```bash
# Rotate a single secret
envforge rotate API_KEY

# Preview without changes
envforge rotate API_KEY --dry-run

# Rotate all stale secrets (>90 days)
envforge rotate --stale
```

Flow:
1. Shows masked current value (`sk-ab****56`)
2. Prompts for new value
3. Confirms before applying
4. Updates locally with atomic write + backup
5. Resets secret age to 0
6. Offers to push to provider and sync

Handles encrypted values transparently (re-encrypts new value if original was encrypted).

### AI-Safe Schema Emission

Generate context for AI coding tools without exposing secret values:

```bash
# From .env.schema
envforge schema emit-ai

# Infer types from current env (no schema needed)
envforge schema emit-ai --infer

# Save for AI tools
envforge schema emit-ai --output .env.ai.md
```

Output:
```markdown
## DATABASE_URL
- **Type**: url
- **Required**: yes
- **Description**: PostgreSQL connection string
- **Sensitive**: YES — do not hardcode or log

## PORT
- **Type**: port
- **Default**: 3000
- **Sensitive**: no
```

AI agents see names, types, descriptions — never actual values. Addresses the 2x secret leak rate with AI-assisted development (GitGuardian 2026).

### MCP Configuration Scanning

Scan AI tool configs for hardcoded credentials:

```bash
envforge scan --mcp
```

Output:
```
MCP Configuration Secret Scan

~/.claude/claude_desktop_config.json
  ⚠ mcpServers.slack.env.SLACK_TOKEN = xoxb-****5678
    → Replace with: ${SLACK_TOKEN}

~/.config/github-copilot/apps.json
  ⚠ github.com.oauth_token = gho_****QD9
    → Replace with: ${oauth_token}

2 file(s) scanned, 2 credential(s) found
```

Scans Claude Desktop, Cursor, GitHub Copilot configs. Detects 23+ API key patterns.

### URI-Based Secret References

Reference secrets by provider URI in config files:

```bash
# secrets.env
DATABASE_URL=vault://secret/myapp/DB_URL
API_KEY=aws-ssm:///prod/api-key
STRIPE_KEY=1password://vault/stripe/key
PORT=3000

# Resolve all URIs to actual values
envforge resolve-uri secrets.env

# Output as .env format
envforge resolve-uri secrets.env --env --output .env
```

Supported schemes: `vault://`, `aws-ssm://`, `1password://`, `doppler://`, `infisical://`, `gcp://`, `azure://`. Regular URLs (`https://`) are not treated as secret URIs.

### Runtime Log Redaction

Prevent secrets from leaking in subprocess output:

```bash
# Automatically mask known secrets in stdout/stderr
envforge run --redact -- npm start

# Combine with volatile mode for maximum protection
envforge run --volatile --redact -- npm start
```

Known sensitive values are replaced with `[REDACTED:KEY_NAME]` in real-time. Parallel stdout/stderr processing with no performance penalty.

### MCP Config Hardening

Auto-fix hardcoded credentials in AI tool configurations:

```bash
# Preview what would change
envforge mcp harden --dry-run

# Auto-replace secrets with ${VAR} references (backs up originals)
envforge mcp harden
```

Rewrites Claude Desktop, Cursor, GitHub Copilot configs. Replaces `"sk-live-abc123"` with `"${API_KEY}"`.

### Secret Fence

One command to create AI tool ignore rules for all tools:

```bash
envforge fence
```

Creates: `.envforgeignore`, `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json`. AI tools will respect secret boundaries.

### Prompt Sanitizer

Strip secret values from any file:

```bash
envforge sanitize debug-output.log --output clean.log
envforge sanitize config.json
```

Replaces known secret values with `${KEY}` placeholders. Longest-match-first for accuracy.

### AI Coding Tool Hooks

Install security hooks in AI coding tools:

```bash
envforge ai-hook install claude-code   # PostToolUse hook scanning for secrets
envforge ai-hook install cursor        # Security rules in .cursorrules
envforge ai-hook remove claude-code    # Remove hooks
```

### Agent Credential Proxy

Local HTTP API for AI agents — secrets served via API, never on disk:

```bash
# Start proxy (blocks until Ctrl+C)
envforge proxy --port 8100

# Scope to specific keys only
envforge proxy --port 8100 --keys DB_URL,API_KEY --profile prod
```

Endpoints:
- `GET /env` — all variables (JSON)
- `GET /env/KEY_NAME` — single variable
- `GET /health` — health check

### AI Leak Report

Scan git history for secrets leaked in AI-assisted commits:

```bash
envforge audit --ai-leaks
```

Detects commits co-authored by Claude, Copilot, Cursor. Scans diffs for API keys, connection strings, tokens.

Pair with `envforge audit-trail query --event-type SECRET_EXPOSURE` for real-time exposure monitoring.

### Pre-Commit Hook

Auto-install a git pre-commit hook to catch leaked secrets before they reach your repo:

```bash
# Install hook
envforge scan --install-hook

# Remove hook
envforge scan --remove-hook
```

The hook runs `envforge scan --staged` before each commit. If secrets are found, the commit is blocked. Appends to existing pre-commit hooks (doesn't overwrite).

### Shell Auto-Load (direnv-style)

Auto-load environment variables when entering a project directory:

```bash
# Add to ~/.zshrc
eval "$(envforge hook zsh)"

# Add to ~/.bashrc
eval "$(envforge hook bash)"

# Add to ~/.config/fish/config.fish
envforge hook fish | source
```

Create `.envforge.toml` in your project root:
```toml
profile = "dev"
```

When you `cd` into a directory with `.envforge.toml` or `.env.schema`, your profile auto-loads. When you leave, it auto-unloads and restores previous values.

### Volatile Mode (AI Agent Safety)

Protect secrets from AI coding agents that scan files on disk:

```bash
# Secrets resolved in memory only — never written to .env on disk
envforge run --volatile -- npm start

# Combine with profile
envforge run --volatile --profile prod -- npm start

# Preview (sensitive values masked as ****)
envforge run --volatile --dry-run -- npm start
```

In volatile mode:
- `--resolve` is forced on (refs and encryption resolved in memory)
- `--env-file` flags are ignored (no disk .env reads)
- Dry-run masks sensitive values as `****`

### Secure Secret Sharing

Share secrets with team members using age public-key encryption:

```bash
# Sender: create encrypted share file
envforge share create --recipient age1abc... --all --output secrets.age

# Share specific keys with expiry
envforge share create --recipient age1abc... --keys DB_URL,API_KEY --expire 24

# Recipient: decrypt and view
envforge share receive secrets.age

# Recipient: decrypt and import into EnvForge
envforge share receive secrets.age --import
```

Share files are encrypted with the recipient's age public key — only they can decrypt. Self-contained `.age` files with sender metadata and optional expiry.

### Git Author Audit Trail

Track who changed what variable on which machine:

```bash
envforge audit                          # Full audit trail
envforge audit --key DB_URL             # Filter by key
envforge audit --since 2026-04-01       # Changes since date
envforge audit --machine macbook-a1b2   # Filter by machine
envforge audit --json                   # Machine-readable
```

Output:
```
TIMESTAMP                 MACHINE      ACTION   KEY          COMMIT
2026-04-20T17:00:00Z      macbook-a1   modified DB_HOST      abc1234
2026-04-19T10:30:00Z      workstation  added    API_KEY      def5678
```

Reads from sync git history — no separate audit store needed.

### AI Audit Trail (Compliance & Forensics)

Tamper-evident audit logs with SOC2-compliant reporting:

```bash
# Query all audit events
envforge audit-trail query --time last_24h

# Filter by event type and source
envforge audit-trail query --event-type SECRET_ACCESS --source proxy

# Generate SOC2 compliance report
envforge audit-trail report --report-type compliance --format markdown

# Show trend report grouped by day
envforge audit-trail report --report-type trend --group-by day --time last_7d

# Trace chain of custody for a specific secret
envforge audit-trail custody --secret-key API_KEY --ownership

# Trace all events in a session
envforge audit-trail custody --session 246f86ae-8b41

# Verify tamper-evident integrity
envforge audit-trail integrity

# Show aggregate statistics
envforge audit-trail stats --time last_30d

# Tail recent events (live view)
envforge audit-trail tail --n 50 --source proxy

# Manage retention policy
envforge audit-trail retention --policy 30d --execute
```

Audit logs are stored in `~/.local/share/envforge/audit/`. Each log is tamper-evident with cryptographic chain hashing. Covers: AI Guard events, Proxy access, Sync operations, CLI commands, TUI sessions, and Hook invocations.

### Token TTL (Credential Expiry)

Set expiry on provider credentials for security hygiene:

```bash
# Store with 8-hour TTL
envforge secrets config vault --set token=hvs.xxx --ttl 8h

# Supported durations: 8h, 24h, 7d, 30d
envforge secrets config aws-ssm --set access_key=AKIA... --ttl 30d

# Check TTL status
envforge secrets status
```

Expired credentials are rejected with a clear message. `envforge doctor` warns about credentials expiring within 24 hours.

### Offline Fallback & Cache

Secrets work even when providers are unreachable:

```bash
# Automatic: falls back to cached values when provider down
envforge run --resolve -- npm start
# stderr: ⚠ Using cached value for DB_URL (provider unreachable)

# Manage cache
envforge secrets cache list              # Show all cached secrets
envforge secrets cache clear             # Clear all
envforge secrets cache clear --provider vault  # Clear specific provider
```

### Multi-Profile Merge

Load and merge multiple profiles in a single process:

```bash
# Merge dev + custom profiles (custom wins on conflicts)
envforge run --profiles dev,custom -- npm start

# Merge three profiles with overrides
envforge run --profiles base,staging,hotfix --override DEBUG=true -- npm test
```

Precedence: shared → profiles (left-to-right, last wins) → env-files → overrides.

### Multi-Format Export

Export your environment variables in any format your infrastructure needs:

```bash
# JSON (for config files, APIs)
envforge export --format json

# YAML (for docker-compose, Ansible)
envforge export --format yaml

# TOML (for Rust, Python config)
envforge export --format toml

# Docker env-file (bare KEY=VALUE)
envforge export --format docker

# Kubernetes Secret manifest (base64-encoded)
envforge export --format k8s
envforge export --format k8s --k8s-name my-secrets --k8s-namespace production

# Terraform tfvars
envforge export --format tfvars

# Combine with filter
envforge export --format json --filter "DB_*"

# Save to file
envforge export --format k8s --k8s-name app-secrets -o secrets.yaml
```

**Supported formats:**

| Format | Alias | Output |
|--------|-------|--------|
| `dotenv` | `env`, `.env` | `KEY=VALUE` with quoting |
| `json` | — | `{"KEY": "VALUE"}` |
| `yaml` | `yml` | YAML with proper quoting |
| `toml` | — | `KEY = "VALUE"` |
| `docker` | `docker-env` | Bare `KEY=VALUE` |
| `k8s` | `kubernetes` | K8s Secret manifest |
| `tfvars` | `terraform`, `tf` | Terraform variables |

### Secret Age Tracking

Track when secrets were last pulled and flag stale ones:

```bash
# Show age of all tracked secrets
envforge secrets age

# Flag secrets older than 30 days
envforge secrets age --threshold 30

# Only show stale secrets
envforge secrets age --stale-only

# JSON output for CI
envforge secrets age --json
```

Output:
```
KEY                            PROVIDER     AGE      STATUS
-----------------------------------------------------------------
API_KEY                        vault        120 days ⚠ STALE
DB_PASSWORD                    aws-ssm      45 days  ✓ ok
STRIPE_KEY                     doppler      2 days   ✓ ok

1 secret(s) older than 90 days. Consider rotating them.
```

### Provider Diff

Compare your local environment against a secret provider to find drift:

```bash
# Compare local vs Vault
envforge secrets diff --from vault --path secret/myapp

# Compare with filter
envforge secrets diff --from aws-ssm --path /prod --filter "DB_*"

# JSON output
envforge secrets diff --from doppler --json
```

Output:
```
Diff: local vs vault (secret/myapp)

~~~ Changed: 2 key(s)
  ~ DB_HOST
    - local:  localhost
    + remote: prod-db.internal

--- Only local: 1 key(s)
  - DEBUG

+++ Only remote: 1 key(s)
  + NEW_FEATURE_FLAG

Summary: 5 same, 2 changed, 1 only local, 1 only remote
```

### GitHub Action (CI/CD)

Use EnvForge in your CI/CD pipelines with the official GitHub Action:

```yaml
# Validate .env against schema on every PR
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env

# Pull secrets from any provider into your workflow
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production

# Run tests with process-scoped secrets (never leak to other steps)
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: npm test
    resolve-secrets: 'true'

# Detect drift across environment files
- uses: emreerinc/envforge/action@v1
  with:
    mode: drift
    drift-envs: |
      .env.development
      .env.staging
      .env.production
```

**5 modes:**

| Mode | Description |
|------|-------------|
| `validate` | Check `.env` against `.env.schema`, fail on errors |
| `secrets-pull` | Pull from 13 providers → `GITHUB_ENV` (masked by default) |
| `export` | Export EnvForge-managed vars into workflow |
| `run` | Process-scoped injection — secrets don't persist |
| `drift` | Compare `.env` files, detect config drift |

**Security features:**
- Values masked in Actions logs by default (`mask-values: true`)
- `run` mode = process-scoped, no `GITHUB_ENV` leak
- `export-env: false` option to prevent secrets leaking to subsequent steps

See [action/README.md](action/README.md) for full documentation (all inputs, outputs, and examples).

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

#### AI Agent Protection
- **Volatile mode** — `envforge run --volatile` — secrets resolved in memory, never written to disk
- **Log redaction** — `envforge run --redact` — mask secrets in subprocess output in real-time
- **Secret fence** — `envforge fence` — create ignore rules for Cursor, Copilot, Claude Code
- **MCP scanning** — `envforge scan --mcp` — find hardcoded creds in AI tool configs
- **MCP hardening** — `envforge mcp harden` — auto-replace secrets with `${VAR}` references
- **AI context** — `envforge schema emit-ai` — give AI agents context without values
- **AI hooks** — `envforge ai-hook install claude-code` — security hooks in AI tools
- **Credential proxy** — `envforge proxy` — HTTP API for AI agents, secrets never on disk
- **Prompt sanitizer** — `envforge sanitize FILE` — strip secrets from any file
- **AI leak audit** — `envforge audit --ai-leaks` — find secrets in AI-assisted commits

#### Encryption & Access Control
- **Encryption at rest** — Encrypt individual values with age (X25519): `envforge encrypt API_KEY`
- **Encrypted sync** — Sync snapshots age-encrypted before git push; auto-decrypt on pull
- **Credential TTL** — `--ttl 8h` on provider credentials, auto-expire
- **Process-scoped secrets** — `envforge run --resolve` injects secrets only into the child process
- **Credential storage** — Provider credentials encrypted with age in `~/.config/envforge/credentials.toml`

#### Scanning & Prevention
- **Secret scanning** — Detect leaked secrets in source code: `envforge scan --staged`
- **Pre-commit hook** — `envforge scan --install-hook` blocks commits containing secrets
- **MCP config scanning** — Find credentials in Claude, Cursor, Copilot configs
- **Value masking** — Keys containing SECRET, TOKEN, PASSWORD masked in TUI
- **Schema-based sensitive marking** — Schema `sensitive = true` flag

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

Tab completion for all 90+ commands and flags. Use `--install` for automatic setup:

```bash
# Zsh — auto-install to ~/.zfunc/_envforge
envforge completions zsh --install

# Bash — auto-install to ~/.local/share/bash-completion/completions/envforge
envforge completions bash --install

# Fish — auto-install to ~/.config/fish/completions/envforge.fish
envforge completions fish --install

# Kiro CLI — installs Fig spec + configures devCompletionsFolder + developerMode
envforge completions kiro --install
kiro-cli restart

# Fig / Amazon Q
envforge completions fig --install
```

<details>
<summary>Manual setup (without --install)</summary>

```bash
# Zsh (Oh-My-Zsh)
mkdir -p ~/.zsh/completions
envforge completions zsh > ~/.zsh/completions/_envforge
# Add to ~/.zshrc BEFORE source $ZSH/oh-my-zsh.sh:
#   fpath=(~/.zsh/completions $fpath)

# Zsh (vanilla)
envforge completions zsh > ~/.zsh/completions/_envforge
echo 'fpath=(~/.zsh/completions $fpath); autoload -Uz compinit; compinit' >> ~/.zshrc

# Bash
envforge completions bash > /etc/bash_completion.d/envforge

# Fish
envforge completions fish > ~/.config/fish/completions/envforge.fish

# Kiro CLI (manual)
mkdir -p ~/.kiro/specs
envforge completions kiro > ~/.kiro/specs/envforge.js
# Remove TypeScript annotation: replace "const completion: Fig.Spec = {" with "const completion = {"
kiro-cli settings autocomplete.devCompletionsFolder "$HOME/.kiro/specs"
kiro-cli settings autocomplete.developerMode true
kiro-cli restart
```
</details>

After installing, restart your shell (`exec zsh` or open new terminal).

### Built-in Man Pages

Detailed manual for every command, available offline:

```bash
# Show full command index
envforge man

# Show man page for a specific command
envforge man list
envforge man sync push
envforge man secrets pull
envforge man fence

# "Did you mean?" suggestions for typos
envforge man fenc
# → Did you mean: fence
```

### IDE Extensions

EnvForge includes a built-in Language Server (`envforge lsp`) and extensions for VS Code and IntelliJ IDEA.

**VS Code** — [Marketplace](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager)

```bash
# Install from VS Code
ext install emreerinc.envforge-env-manager
```

**IntelliJ IDEA** — [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/31385-envforge)

```
Settings > Plugins > Marketplace > Search "EnvForge"
```

**What you get in both IDEs:**

| Feature | Description |
|---------|-------------|
| Diagnostics | Missing required vars, type errors, secret leak warnings — real-time |
| Hover | Type, description, default, example from `.env.schema` |
| Completions | All envforge-managed vars + schema keys + value suggestions |
| Go-to-definition | Click key in `.env` → jumps to `.env.schema` section |
| Variables panel | Grouped by prefix, sensitive values masked, copy key/value |
| Profiles panel | View all profiles, switch with one click |
| Commands | Validate, scan, export, sync, health check, schema generate |
| Status bar | Variable count |

Requires `envforge` CLI installed. Extensions use `envforge lsp` for language features and `envforge <cmd> --json` for commands.

See [`editors/`](editors/) for detailed setup guides, Neovim/Helix/Sublime configs, and feature parity table.

## Architecture

```
src/
├── main.rs              # Entry point — routes to TUI or CLI
├── lib.rs               # Module declarations
├── model/               # Data types (LineNode, ShellFile, ExportStyle)
├── parser/              # Shell file parser & writer (byte-for-byte round-trip safe)
├── config/              # App config, backup, atomic writes
│
├── ops/                 # Core operations (35 modules)
│   │
│   │── ── Core ──────────────────────────
│   ├── crud.rs          # Add, edit, delete, move, toggle
│   ├── run.rs           # Subprocess runner (volatile, redact, multi-profile)
│   ├── profile.rs       # Profile management
│   ├── profile_diff.rs  # Profile comparison
│   ├── encrypt.rs       # Age (X25519) encryption/decryption
│   ├── dotenv.rs        # .env parsing, safe export, env-example
│   ├── listing.rs       # ENV entry collection and filtering
│   ├── validation.rs    # Rule-based value validation
│   ├── changelog.rs     # Change tracking
│   ├── duplicates.rs    # Duplicate key detection
│   ├── snapshot.rs      # Environment state backup/restore
│   ├── rotate.rs        # Secret rotation with propagation
│   ├── explain.rs       # Key X-ray across all subsystems
│   ├── export_format.rs # 8 formats (JSON, YAML, TOML, Docker, K8s, tfvars...)
│   │
│   │── ── Schema & Validation ───────────
│   ├── schema.rs        # .env.schema parser, validation, docs, drift, AI emit
│   ├── schema_json.rs   # JSON Schema (Draft 2020-12) output
│   ├── check.rs         # Unified check runner (doctor+validate+scan+age+drift)
│   ├── doctor.rs        # 15+ health checks with fix suggestions
│   │
│   │── ── AI Safety ────────────────────
│   ├── mcp_scan.rs      # MCP config scanning (23+ credential patterns)
│   ├── fence.rs         # AI tool ignore rules (Cursor, Copilot, Claude Code)
│   ├── sanitize.rs      # Secret value stripping from any file
│   ├── ai_hooks.rs      # AI coding tool hook installation
│   ├── proxy.rs         # Local HTTP credential proxy for AI agents
│   ├── audit.rs         # Git author audit trail + AI leak detection
│   │
│   │── ── Scanning & Security ──────────
│   ├── scanner.rs       # Secret scanning (source code + staged files)
│   ├── uri_resolve.rs   # URI-based secret references (vault://, aws-ssm://)
│   │
│   │── ── Shell Integration ────────────
│   ├── hook.rs          # Shell auto-load hooks (zsh, bash, fish)
│   ├── share.rs         # Age-encrypted secret sharing
│   ├── fuzzy.rs         # Fuzzy search with highlighting
│   ├── grouping.rs      # Variable grouping (prefix-based + custom)
│   ├── clipboard.rs     # Clipboard integration
│   ├── undo.rs          # Undo/redo stack
│   │
│   │── ── Remote Sync ──────────────────
│   ├── sync/            # Git-based cross-machine sync
│   │   ├── init.rs      # Sync initialization
│   │   ├── push.rs      # Push to remote
│   │   ├── pull.rs      # Pull from remote
│   │   ├── encryption.rs # Age-encrypted snapshots
│   │   ├── diff.rs      # Diff calculation
│   │   ├── conflict.rs  # Three-way conflict detection
│   │   ├── marking.rs   # Key sync/local marking
│   │   ├── machine.rs   # Machine identity & overrides
│   │   ├── history.rs   # Snapshot history & rollback
│   │   ├── git.rs       # Git operations
│   │   └── model.rs     # Sync data types
│   │
│   │── ── Secret Managers ──────────────
│   └── secrets/         # 13 provider integrations
│       ├── provider.rs  # SecretProvider trait & registry
│       ├── modes.rs     # Pull/push/reference modes
│       ├── cache.rs     # TTL cache with offline fallback
│       ├── credentials.rs # Encrypted credential store with TTL
│       ├── age.rs       # Secret age tracking
│       └── providers/   # Provider implementations
│           ├── vault.rs     # HashiCorp Vault
│           ├── aws_ssm.rs   # AWS SSM Parameter Store
│           ├── onepassword.rs # 1Password
│           ├── doppler.rs   # Doppler
│           ├── infisical.rs # Infisical
│           ├── gcp.rs       # GCP Secret Manager
│           ├── azure.rs     # Azure Key Vault
│           ├── bitwarden.rs # Bitwarden Secrets Manager
│           ├── akeyless.rs  # Akeyless Vault
│           ├── conjur.rs    # CyberArk Conjur
│           ├── sops.rs      # Mozilla SOPS
│           ├── pass.rs      # pass/gopass
│           └── keeper.rs    # Keeper Secrets Manager
│
├── ui/                  # Ratatui TUI
│   ├── app.rs           # App state (View/Edit/Add/Search modes)
│   ├── render.rs        # Rendering logic
│   ├── dialogs.rs       # Modal dialogs
│   ├── table.rs         # Table rendering
│   ├── input.rs         # Text input handling
│   └── mod.rs           # run_tui() entry
│
├── cli/                 # Clap CLI (80+ subcommands)
│   ├── commands.rs      # Command implementations
│   ├── mod.rs           # CLI struct, Commands enum
│   ├── sync_cmd.rs      # Sync subcommands
│   ├── secrets_cmd.rs   # Secrets subcommands
│   └── wizard.rs        # First-run setup wizard
│
└── action/              # GitHub Action (composite)
    ├── action.yml       # Action definition
    ├── scripts/
    │   ├── install.sh   # Binary installer
    │   └── run.sh       # 5-mode runner
    └── tests/
        └── test_action.sh # 21 tests
```

### Design Principles

1. **AI-safe by default** — Volatile mode, redaction, fencing built into core workflows.
2. **Never delete** — Soft-delete with tags. Nothing physically removed.
3. **Atomic writes** — Every write uses tempfile + rename. No partial files on crash.
4. **Order preservation** — Line-level AST preserves exact byte order and formatting.
5. **Offset safety** — Protected zones (conda, Amazon Q blocks) never modified.
6. **Backup first** — Automatic backup before every write operation.
7. **Schema-driven** — `.env.schema` as single source of truth for project ENV requirements.
8. **Offline-first** — Everything works locally. Remote sync and providers are optional.
9. **Zero trust** — Secrets encrypted at rest, in transit (sync), and in memory (volatile).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License. See [LICENSE](LICENSE) for details.
