# EnvForge CLI Reference

> Generated for EnvForge v1.0.2


## Quick Recipes

Jump to a solution:

| I want to... | Command(s) |
|---|---|
| Protect a project from AI agents | `envforge fence` → `envforge ai-hook install claude-code` → `envforge mcp harden` |
| Sync env vars across machines | `envforge sync init` → `envforge sync mark --all --sync` → `envforge sync push` |
| Pull secrets from Vault/AWS | `envforge secrets config vault --set token=x` → `envforge secrets pull --from vault --path secret/myapp` → `envforge run --resolve -- cmd` |
| Validate .env in CI/CD | `envforge validate --schema .env.schema --env .env.production` |
| Find unused secrets | `envforge analytics unused` → `envforge analytics deprecation` |
| Set up a new project | `envforge project init` → `envforge schema generate` → `envforge init` |
| Share secrets with a teammate | `envforge share create --recipient age1... --all --output secrets.age` |
| Rotate stale secrets | `envforge rotate --stale --propagate` |
| Run a health check | `envforge check` / `envforge doctor --verbose` |
| Export for Kubernetes | `envforge export --format k8s --k8s-name app-secrets` |
| Scan for leaked secrets | `envforge scan --staged` / `envforge scan --mcp` |
| Generate shell completions | `envforge completions zsh --install` |
| Monitor secret infrastructure | `envforge monitor status` / `envforge monitor stream` |
| Automate secret lifecycle | `envforge lifecycle rule list` → `envforge lifecycle check` |

Commands support `--json` for machine output and `--dry-run` for preview.

## Global Flags

Every command accepts these flags:

| Flag | Description |
|------|-------------|
| `--json` | Output in JSON format |
| `--dry-run` | Preview changes without writing to disk |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

---

## Security & AI Safety

### envforge fence

Create AI-tool ignore/rules files for all detected AI tools (11 targets incl. Cursor, Copilot, Claude Code, Windsurf, Cline, Aider, Gemini CLI, Amazon Q, the `AGENTS.md` standard). Per-target configurable — see `fence config`. `fence --status` reports honest per-tool coverage.

```
Usage: envforge fence [OPTIONS] [COMMAND]
```

| Flag | Description |
|------|-------------|
| `--status` | Check fence status instead of creating |
| `--disable` | Remove envforge-owned fence content |

| Command | Description |
|---------|-------------|
| `config` | List or change which fence targets are written (see below) |

**Examples:**

```bash
# Enable fence (writes only the enabled targets)
envforge fence

# Check status
envforge fence --status

# Disable fence
envforge fence --disable

# Manage which targets fence writes
envforge fence config --list
envforge fence config --disable copilot
```

---

### envforge exposure

Show which lines in a .env file an AI agent could read (red / amber / green per line).

```
Usage: envforge exposure [OPTIONS] --file <FILE>
```

| Argument | Description |
|----------|-------------|
| `--file <FILE>` | Path to the .env file to classify |

**Examples:**

```bash
envforge exposure --file .env
envforge exposure --file .env.production --json
```

---

### envforge sanitize

Replace secret values in a file with `${KEY}` placeholders so it is safe to share.

```
Usage: envforge sanitize [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | File to sanitize |
| `--output <OUTPUT>` | Output file (default: stdout) |

**Examples:**

```bash
# Print a sanitized copy to the screen
envforge sanitize config.yaml

# Write the sanitized copy to a new file
envforge sanitize config.yaml --output config.shareable.yaml
```

---

### envforge ai-hook

Install or remove AI coding tool security hooks.

```
Usage: envforge ai-hook [OPTIONS] <COMMAND>
```

**Commands:**

- `install <TOOL>`: Install hooks (supported: `claude-code`, `cursor`, `copilot`)
- `remove <TOOL>`: Remove hooks
- `status`: Check if hooks are installed

**Examples:**

```bash
envforge ai-hook install claude-code
envforge ai-hook status --json
```

---

### envforge proxy

Start local credential proxy for AI agents.

```
Usage: envforge proxy [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--port <PORT>` | Port to listen on [default: 8100] |
| `--keys <KEYS>` | Only serve these keys (comma-separated) |
| `--require-lease` | Require active lease for access |
| `--require-approval` | Require human approval for each secret access |

**Examples:**

```bash
envforge proxy --port 9000
envforge proxy --require-approval
```

---

## Variable Management

### envforge list

List all environment variables with optional filtering, grouping, and sorting.

```
Usage: envforge list [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--filter <FILTER>` | Filter entries by key pattern (substring match) |
| `--group <GROUP>` | Group entries by prefix or tag (`prefix`, `tag`) |
| `--sort <SORT>` | Sort order: `key`, `value`, `file` [default: key] |
| `--reverse` | Reverse sort order |

**Examples:**

```bash
# List all variables
envforge list

# List matching a pattern
envforge list --filter DB

# Group by prefix
envforge list --group prefix

# Sort by value, descending
envforge list --sort value --reverse

# List as JSON
envforge list --json
```

---

### envforge get

Get the value of a specific variable.

```
Usage: envforge get [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

| Option | Description |
|--------|-------------|
| `--reveal` | Show the full value. Sensitive values (SECRET/TOKEN/PASSWORD/KEY/…) are **masked by default**; pass `--reveal` for cleartext. Applies to text and `--json` output. |
| `--json` | JSON output (includes a `redacted` flag) |

**Examples:**

```bash
envforge get DATABASE_URL                # non-sensitive value shown as-is
envforge get API_KEY                     # sensitive → masked (e.g. sk-***xyz)
envforge get API_KEY --reveal            # cleartext (audit-logged)
envforge get API_KEY --json
```

---

### envforge set

Set a variable (create or update).

```
Usage: envforge set [OPTIONS] <ASSIGNMENT>
```

| Argument | Description |
|----------|-------------|
| `<ASSIGNMENT>` | KEY=VALUE pair |

| Option | Description |
|--------|-------------|
| `--stdin` | Read the value from stdin (keeps secrets off argv / `ps` / `/proc` / shell history) |
| `--dry-run` | Preview the diff without writing. Sensitive values are redacted in the diff. |

> Secrets are detected on the command line (length, known prefixes, and entropy) and refused with a hint to use `--stdin`. On success, `set` prints only `Set <KEY>` — never the value or its length.

**Examples:**

```bash
envforge set NODE_ENV=production
echo "$SECRET" | envforge set API_KEY --stdin   # safe path for secrets
envforge set API_KEY --stdin --dry-run           # preview (value redacted)
```

---

### envforge delete

Soft-delete a variable.

```
Usage: envforge delete [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

**Examples:**

```bash
envforge delete OLD_API_KEY
envforge delete TEMP_VAR --dry-run
```

---

### envforge copy

Copy a variable's value to clipboard.

```
Usage: envforge copy [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

| Flag | Description |
|------|-------------|
| `--key-only` | Copy key name instead of value |

**Examples:**

```bash
envforge copy DATABASE_URL
envforge copy API_KEY --json
envforge copy MY_VAR --key-only
```

---

### envforge move

Move a variable to the reference file, or rename in-place.

```
Usage: envforge move [OPTIONS] <KEY> [NEW_KEY]
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |
| `[NEW_KEY]` | New key name (renames in-place if provided) |

**Examples:**

```bash
# Move a variable to the reference file
envforge move LEGACY_KEY

# Rename a variable in-place
envforge move OLD_NAME NEW_NAME

# Preview the move
envforge move OLD_SECRET --dry-run
```

---

### envforge search

Search environment variables by substring or fuzzy matching.

```
Usage: envforge search [OPTIONS] <QUERY>
```

| Argument | Description |
|----------|-------------|
| `<QUERY>` | Search term (matches keys and values) |

| Flag | Description |
|------|-------------|
| `--fuzzy` | Use fuzzy matching (default is substring match) |

**Examples:**

```bash
# Substring search
envforge search db

# Fuzzy search
envforge search "database" --fuzzy

# JSON output
envforge search api --json
```

Sensitive values are automatically masked in output (e.g., `sk-ab***56`).

---

### envforge duplicates

Detect and list duplicate keys.

```
Usage: envforge duplicates [OPTIONS]
```

**Examples:**

```bash
envforge duplicates
envforge duplicates --json
```

---

### envforge diff

Show pending changes as a diff.

```
Usage: envforge diff [OPTIONS]
```

**Examples:**

```bash
envforge diff
envforge diff --json
```

---

### envforge explain

Show all known info about a single environment variable (schema, history, provider).

```
Usage: envforge explain [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name to explain |

**Examples:**

```bash
envforge explain DATABASE_URL
envforge explain API_KEY --json
```

---

### envforge deps

Show where an environment variable is referenced across your project.

```
Usage: envforge deps [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

| Flag | Description |
|------|-------------|
| `--source` | Include source code scanning (slower) |

**Examples:**

```bash
envforge deps DATABASE_URL
envforge deps API_KEY --source
envforge deps SECRET_KEY --json
```

---

## Import & Export

### envforge import

Import variables from a .env file.

```
Usage: envforge import [OPTIONS] <PATH>
```

| Argument | Description |
|----------|-------------|
| `<PATH>` | Path to .env file |

| Flag | Description |
|------|-------------|
| `--force` | Overwrite existing keys without prompting |

**Examples:**

```bash
envforge import .env.backup
envforge import .env.production --force
envforge import .env.staging --dry-run
```

---

### envforge export

Export variables to various formats.

```
Usage: envforge export [OPTIONS] [PATH]
```

| Argument | Description |
|----------|-------------|
| `[PATH]` | Output file path (stdout if omitted) |

| Flag | Description |
|------|-------------|
| `--exclude-sensitive` | Exclude sensitive keys (SECRET, TOKEN, PASSWORD, etc.) |
| `--safe` | Redact sensitive values as [REDACTED] (safe for AI tools) |
| `--reveal` | For `--format` export: emit sensitive values in **cleartext**. By default `--format` output redacts sensitive values as `[REDACTED]` (prints a note to stderr). |
| `--env-example` | Generate .env.example from schema with placeholder values |
| `--filter <FILTER>` | Only export entries matching this query |
| `--format <FORMAT>` | Output format: `dotenv`, `json`, `yaml`, `toml`, `docker`, `docker-secrets`, `k8s`, `tfvars` (8 formats) |
| `--k8s-name <K8S_NAME>` | Kubernetes Secret name (for k8s format, default: envforge-secrets) |
| `--k8s-namespace <K8S_NAMESPACE>` | Kubernetes namespace (for k8s format, default: default) |

> **Redaction default:** `export --format …` masks sensitive values by default so a stray export can't leak secrets to stdout/scrollback or a committed file. Pass `--reveal` to emit real values (e.g. when generating a deployable secret manifest).

**Examples:**

```bash
# Export to stdout
envforge export

# Export to a file
envforge export .env.backup

# Export as YAML
envforge export --format yaml

# Export as Kubernetes Secret manifest
envforge export --format k8s --k8s-name my-app-secrets --k8s-namespace prod

# Export excluding sensitive values
envforge export --exclude-sensitive

# AWS Terraform tfvars format
envforge export --format tfvars

# Docker compose env_file format (quoted values)
envforge export --format docker

# Generate .env.example with placeholder values
envforge export --env-example

# Export safe version for AI tools
envforge export --safe

# Export only matching keys
envforge export --filter "DB_*"
```

---

## Schema & Validation

### envforge validate

Validate ENV values against config rules and/or .env.schema.

```
Usage: envforge validate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--schema <SCHEMA>` | Path to .env.schema (auto-detected if omitted) |
| `--env <ENV_FILE>` | Validate a specific .env file instead of EnvForge config |
| `--environment <ENVIRONMENT>` | Environment name for schema overrides (e.g., production) |
| `--rules <RULES>` | Additional validation rules as KEY=rule pairs (e.g., PORT=port, EMAIL=email) |

**Examples:**

```bash
# Validate current environment
envforge validate

# Validate against a specific schema
envforge validate --schema ./custom.env.schema

# Validate a specific .env file
envforge validate --env .env.production

# Validate with environment-specific overrides
envforge validate --environment production

# Validate with ad-hoc rules
envforge validate --rules PORT=port --rules HOST=url
```

---

### envforge schema generate

Generate .env.schema from current environment variables.

```
Usage: envforge schema generate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--output <OUTPUT>` | Write output to file instead of stdout |

**Examples:**

```bash
# Generate schema to stdout
envforge schema generate

# Write schema to file
envforge schema generate --output .env.schema
```

---

### envforge schema json-schema

Output JSON Schema for .env.schema format.

```
Usage: envforge schema json-schema [OPTIONS]
```

**Examples:**

```bash
envforge schema json-schema
envforge schema json-schema > env-schema.json
```

---

### envforge schema emit-ai

Generate AI-safe context file (names and types, no values).

```
Usage: envforge schema emit-ai [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--output <OUTPUT>` | Output file path (default: stdout) |
| `--infer` | Infer types from current env vars (when no .env.schema exists) |

**Examples:**

```bash
envforge schema emit-ai
envforge schema emit-ai --output .env.ai-context
envforge schema emit-ai --infer
```

---

### envforge docs

Generate documentation from .env.schema.

```
Usage: envforge docs [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--schema <SCHEMA>` | Path to .env.schema (auto-detected if omitted) |
| `--output <OUTPUT>` | Write output to file instead of stdout |

**Examples:**

```bash
envforge docs
envforge docs --output docs/env-vars.md
envforge docs --schema .env.schema
```

---

### envforge init

Interactive environment setup from .env.schema.

```
Usage: envforge init [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--schema <SCHEMA>` | Path to .env.schema (auto-detected if omitted) |
| `--output <OUTPUT>` | Output .env file path [default: .env] |

**Examples:**

```bash
envforge init
envforge init --schema .env.schema --output .env.local
```

---

### envforge drift

Detect environment variable drift across .env files.

```
Usage: envforge drift [OPTIONS] --envs <ENV_FILES>...
```

| Flag | Description |
|------|-------------|
| `--schema <SCHEMA>` | Path to .env.schema (auto-detected if omitted) |
| `--environment <ENVIRONMENT>` | Environment name for schema overrides |
| `--envs <ENV_FILES>...` | .env files to compare |

**Examples:**

```bash
envforge drift --envs .env.development .env.production
envforge drift --envs .env .env.staging --schema .env.schema
envforge drift --envs .env .env.prod --environment production
```

---

## Runtime

### envforge run

Run a command with EnvForge-managed environment variables.

```
Usage: envforge run [OPTIONS] -- <COMMAND>...
```

| Argument | Description |
|----------|-------------|
| `<COMMAND>...` | Command and arguments to run (after `--`) |

| Flag | Description |
|------|-------------|
| `--profile <PROFILE>` | Profile to use (default: active profile) |
| `--profiles <PROFILES>` | Load and merge multiple profiles (comma-separated, last wins) |
| `--resolve` | Resolve secret references (ref:provider:path) at runtime |
| `--env-file <ENV_FILES>` | Load additional .env file(s) (can be repeated) |
| `--override <OVERRIDES>` | Override a specific variable (KEY=VALUE, can be repeated) |
| `--volatile` | AI-agent-safe mode: resolve secrets in memory only, skip .env disk files |
| `--redact` | Redact known secret values in subprocess output |
| `--no-project` | Skip project config auto-detection |

**Examples:**

```bash
# Run a command with environment variables
envforge run -- node server.js

# Use a specific profile
envforge run --profile production -- npm start

# Merge multiple profiles (last wins)
envforge run --profiles base,staging -- ./deploy.sh

# Resolve secret references at runtime
envforge run --resolve -- python app.py

# Override a variable for this run
envforge run --override PORT=3001 -- npm start

# AI-safe volatile mode (secrets never touch disk)
envforge run --volatile --resolve -- ./my-agent.sh

# Redact secrets from subprocess output
envforge run --redact -- ./scripts/debug.sh

# Combine env files with overrides
envforge run --env-file .env.local --override DEBUG=true -- cargo test

# Skip project config auto-detection
envforge run --no-project -- npm start
```

---

### envforge env

Output environment variables as shell export statements (for `eval`).

```
Usage: envforge env [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--dir <DIR>` | Directory to load from (default: current) |

**Examples:**

```bash
eval "$(envforge env)"
eval "$(envforge env --dir /path/to/project)"
```

---

### envforge hook

Generate shell hook for auto-loading environment variables.

```
Usage: envforge hook [OPTIONS] <SHELL>
```

| Argument | Description |
|----------|-------------|
| `<SHELL>` | Shell type: `zsh`, `bash`, `fish` |

**Examples:**

```bash
# Add to .zshrc
eval "$(envforge hook zsh)"

# Add to .bashrc
eval "$(envforge hook bash)"

# Add to fish config
envforge hook fish | source
```

### envforge env-unload

Unload environment variables from the current shell session (clears exported env vars).

```
Usage: envforge env-unload [OPTIONS] [PATTERN]
```

| Argument | Description |
|----------|-------------|
| `[PATTERN]` | Unload variables matching this glob pattern (e.g., `API_*`) |

**Examples:**

```bash
envforge env-unload
envforge env-unload API_*
```

---

## Profiles

### envforge profile list

List all profiles.

```
Usage: envforge profile list [OPTIONS]
```

**Examples:**

```bash
envforge profile list
envforge profile list --json
```

---

### envforge profile switch

Switch to a profile.

```
Usage: envforge profile switch [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Profile name |

**Examples:**

```bash
envforge profile switch staging
envforge profile switch production --dry-run
```

---

### envforge profile create

Create a new profile.

```
Usage: envforge profile create [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Profile name |

**Examples:**

```bash
envforge profile create staging
envforge profile create testing --dry-run
```

---

### envforge profile delete

Delete a profile.

```
Usage: envforge profile delete [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Profile name |

**Examples:**

```bash
envforge profile delete old-staging
envforge profile delete temp --dry-run
```

---

### envforge profile diff

Compare environment variables between two profiles.

```
Usage: envforge profile diff [OPTIONS] <A> <B>
```

| Argument | Description |
|----------|-------------|
| `<A>` | First profile name |
| `<B>` | Second profile name |

**Examples:**

```bash
envforge profile diff development production
envforge profile diff staging prod --json
```

---

## Project Management

Project-scoped env management with multi-environment support, guided setup, and provider integration. Config file: `.envforge.project.{toml,yaml,json}`.

### envforge project init

Initialize project-scoped env management.

```
Usage: envforge project init [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--format <FORMAT>` | Config format: `toml`, `yaml`, `json` [default: toml] |
| `--force` | Force reinitialize (overwrite existing config) |
| `--fence` | Write AI-tool fence files after scaffolding (default set: detected tools) |
| `--fence-targets <IDS>` | Comma-separated fence target ids to write (e.g. `cursor_ignore,copilot`); implies `--fence` |
| `--no-fence` | Skip AI-tool fencing entirely (overrides the interactive prompt) |
| `--non-interactive` | Skip the interactive fence prompt; use flags/detected defaults |

**Examples:**

```bash
envforge project init                                   # scaffold + interactive fence prompt
envforge project init --format yaml
envforge project init --fence-targets cursor_ignore,copilot,gemini  # fence only these
envforge project init --fence --non-interactive         # fence detected tools, no prompt
envforge project init --no-fence                        # scaffold only, no fencing
```

Creates `.envforge.project.toml` (or `.yaml`/`.json`), a default `.env.development` file, and adds `.env.*` patterns to `.gitignore`.

**AI-tool fencing:** in an interactive terminal, init prompts which AI tools to fence — `[Enter = detected · 'all' · 'none' · or comma-separated ids]`. Detected tools (via the fence registry's detection hints) are the default. Use the flags above for CI/scripts. See [`fence config`](#envforge-fence-config) for per-target management and `fence --status` for honest coverage.

---

### envforge project wizard

Interactive 3-step guided setup: init → schema → key-value entry. Resumable — skips already completed steps.

```
Usage: envforge project wizard [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--force` | Force re-run all steps |

**Examples:**

```bash
envforge project wizard
envforge project wizard --force
```

**Steps:**
1. **Init** — Creates project config (detects existing `.env` and `.env.schema`)
2. **Schema** — Generates `.env.schema` from current `.env` file
3. **Values** — Interactive prompts for each schema key

---

### envforge project env create

Create a new environment.

```
Usage: envforge project env create [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Environment name (lowercase alphanumeric + hyphens) |

| Flag | Description |
|------|-------------|
| `--description <DESC>` | Optional description |

**Examples:**

```bash
envforge project env create staging
envforge project env create production --description "Live production environment"
```

---

### envforge project env list

List all environments with active indicator.

```
Usage: envforge project env list
```

**Examples:**

```bash
envforge project env list
```

---

### envforge project env switch

Switch active environment.

```
Usage: envforge project env switch <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Environment name to activate |

**Examples:**

```bash
envforge project env switch production
envforge project env switch staging
```

---

### envforge project env delete

Delete an environment.

```
Usage: envforge project env delete <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Environment name to delete |

**Examples:**

```bash
envforge project env delete staging
```

---

### envforge project env diff

Compare two environments side-by-side.

```
Usage: envforge project env diff <A> <B>
```

| Argument | Description |
|----------|-------------|
| `<A>` | First environment name |
| `<B>` | Second environment name |

**Examples:**

```bash
envforge project env diff development production
envforge project env diff staging production
```

---

### envforge project validate

Validate project env against schema.

```
Usage: envforge project validate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--environment <NAME>` | Validate a specific environment (default: active) |

**Examples:**

```bash
envforge project validate
envforge project validate --environment production
```

---

### envforge project scan

Scan project for leaked secrets.

```
Usage: envforge project scan [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--staged` | Only scan git staged files |
| `--mcp` | Scan MCP config files |

**Examples:**

```bash
envforge project scan
envforge project scan --staged
envforge project scan --mcp
```

---

### envforge project schema generate

Generate `.env.schema` from project env.

```
Usage: envforge project schema generate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--output <PATH>` | Output file path (default: `.env.schema`) |

**Examples:**

```bash
envforge project schema generate
envforge project schema generate --output custom-schema.toml
```

---

### envforge project schema emit-ai

Generate AI-safe context file (key names and types only, no values).

```
Usage: envforge project schema emit-ai [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--output <PATH>` | Output file path |
| `--infer` | Infer types from current env vars |

**Examples:**

```bash
envforge project schema emit-ai
envforge project schema emit-ai --infer --output ai-context.md
```

---

### envforge project config

Show or edit project configuration.

```
Usage: envforge project config [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--set <KEY=VALUE>` | Set a config value |

**Examples:**

```bash
envforge project config
envforge project config --set name=my-app
envforge project config --set schema_path=.env.schema
```

---

### envforge project status

Show project health overview.

```
Usage: envforge project status
```

**Examples:**

```bash
envforge project status
```

---

### envforge project pull

Pull secrets from provider into project env.

```
Usage: envforge project pull [OPTIONS] --from <FROM>
```

| Flag | Description |
|------|-------------|
| `--from <FROM>` | Provider name (vault, aws-ssm, doppler, etc.) |
| `--path <PATH>` | Secret path in the provider [default: ""] |
| `--filter <FILTER>` | Filter keys by glob pattern |
| `--environment <NAME>` | Target specific environment (default: active) |

**Examples:**

```bash
envforge project pull --from vault --path secret/myapp
envforge project pull --from doppler --environment production
envforge project pull --from aws-ssm --path /myapp/ --filter "DB_*"
```

---

### envforge project push

Push project env to provider.

```
Usage: envforge project push [OPTIONS] --to <TO>
```

| Flag | Description |
|------|-------------|
| `--to <TO>` | Provider name |
| `--path <PATH>` | Secret path in the provider [default: ""] |
| `--keys <KEYS>` | Specific keys to push (comma-separated) |
| `--all` | Push all keys |
| `--filter <FILTER>` | Filter keys by glob pattern |

**Examples:**

```bash
envforge project push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY
envforge project push --to doppler --all
```

---

### envforge project fence

Create AI ignore rules for project.

```
Usage: envforge project fence
```

**Examples:**

```bash
envforge project fence
```

---

### envforge project sanitize

Sanitize file by replacing secret values found in project env.

```
Usage: envforge project sanitize [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | File to sanitize |

| Flag | Description |
|------|-------------|
| `--output <PATH>` | Output file (default: stdout) |

**Examples:**

```bash
envforge project sanitize docker-compose.yml
envforge project sanitize config.yaml --output config.sanitized.yaml
```

---

### envforge project export

Export project env.

```
Usage: envforge project export [OPTIONS] [PATH]
```

| Argument | Description |
|----------|-------------|
| `[PATH]` | Output file path (default: stdout) |

| Flag | Description |
|------|-------------|
| `--safe` | Redact sensitive values |
| `--format <FORMAT>` | Output format (dotenv, json, yaml, etc.) |
| `--filter <FILTER>` | Filter keys by glob pattern |

**Examples:**

```bash
envforge project export
envforge project export --format json
envforge project export --safe --format yaml
envforge project export .env.export --filter "DB_*"
```

---

## Encryption

### envforge encrypt

Encrypt a variable's value.

```
Usage: envforge encrypt [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

**Examples:**

```bash
envforge encrypt DATABASE_PASSWORD
envforge encrypt API_SECRET --dry-run
```

---

### envforge decrypt

Decrypt a variable's value.

```
Usage: envforge decrypt [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

**Examples:**

```bash
envforge decrypt DATABASE_PASSWORD
envforge decrypt API_SECRET --json
```

### ENVFORGE_AGE_KEY (CI / Headless)

EnvForge resolves the age encryption key in this priority order:

| Priority | Source | Usage |
|----------|--------|-------|
| 1 (highest) | `ENVFORGE_AGE_KEY` env var | Raw key content. For CI pipelines and headless environments. |
| 2 | `ENVFORGE_AGE_KEY_FILE` env var | Path to custom key file. |
| 3 (default) | `~/.config/envforge/age.key` | Auto-generated on first `envforge encrypt` call. |

**Recovery key:** `~/.config/envforge/age-recovery.key` is generated on first run. Store offline.

```bash
# CI example
ENVFORGE_AGE_KEY="$(cat ~/.config/envforge/age.key)" envforge encrypt KEY
```

---

## Snapshots & Undo

### envforge snapshot create

Create a snapshot of current environment variables.

```
Usage: envforge snapshot create [OPTIONS] [NAME]
```

| Argument | Description |
|----------|-------------|
| `[NAME]` | Snapshot name (default: auto-generated timestamp) |

**Examples:**

```bash
envforge snapshot create
envforge snapshot create pre-deploy
envforge snapshot create before-migration --dry-run
```

---

### envforge snapshot list

List all snapshots.

```
Usage: envforge snapshot list [OPTIONS]
```

**Examples:**

```bash
envforge snapshot list
envforge snapshot list --json
```

---

### envforge snapshot restore

Restore environment variables from a snapshot.

```
Usage: envforge snapshot restore [OPTIONS] [NAME]
```

| Argument | Description |
|----------|-------------|
| `[NAME]` | Snapshot name (substring match) |

| Flag | Description |
|------|-------------|
| `--last` | Restore the most recent snapshot |

**Examples:**

```bash
envforge snapshot restore pre-deploy
envforge snapshot restore --last
envforge snapshot restore pre-deploy --dry-run
```

---

### envforge snapshot diff

Show diff between a snapshot and current environment.

```
Usage: envforge snapshot diff [OPTIONS] [NAME]
```

| Argument | Description |
|----------|-------------|
| `[NAME]` | Snapshot name (substring match) |

| Flag | Description |
|------|-------------|
| `--last` | Diff against the most recent snapshot |

**Examples:**

```bash
envforge snapshot diff pre-deploy
envforge snapshot diff --last
```

---

### envforge snapshot delete

Delete a snapshot.

```
Usage: envforge snapshot delete [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Snapshot name (substring match) |

**Examples:**

```bash
envforge snapshot delete pre-deploy
envforge snapshot delete old-snapshot --dry-run
```

---

### envforge undo

Undo the last mutation using backup snapshots.

```
Usage: envforge undo [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--list` | List available undo snapshots |

**Examples:**

```bash
# List available undo points
envforge undo --list

# Undo the last mutation
envforge undo

# Preview undo
envforge undo --dry-run
```

---

## Backups

### envforge backup list

List available backups.

```
Usage: envforge backup list [OPTIONS]
```

**Examples:**

```bash
envforge backup list
envforge backup list --json
```

---

### envforge backup restore

Restore from a backup file.

```
Usage: envforge backup restore [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to backup file |

**Examples:**

```bash
envforge backup restore ~/.envforge/backups/2025-01-01.toml
envforge backup restore backup.toml --dry-run
```

---

## Remote Sync

### envforge sync init

Initialize sync repository.

```
Usage: envforge sync init [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--remote <REMOTE>` | Remote git URL to clone from |
| `--machine-id <MACHINE_ID>` | Custom machine ID |
| `--force` | Force reinitialize (backup existing) |
| `--enforce-ssh` | Reject non-SSH (http/https) remotes at clone time; persisted to the sync config so later operations stay SSH-only |

**Examples:**

```bash
envforge sync init
envforge sync init --remote git@github.com:myorg/env-sync.git
envforge sync init --remote git@github.com:myorg/env-sync.git --enforce-ssh
envforge sync init --force --machine-id macbook-work
```

### Sync Encryption Policy

Configure encryption enforcement in `sync-config.toml`:

```toml
[sync]
# Mandatory (default) — reject plaintext snapshots
encryption_policy = "mandatory"

# Migration window — auto-hardens after the given UTC datetime
encryption_policy = "migration-until 2027-01-01T00:00:00Z"
```

Use `--force-migration` to bypass the deadline during migration windows:

```bash
envforge sync pull --force-migration
```

**Warning:** `--force-migration` bypasses encryption enforcement. A WARN-level audit event is emitted. Re-enable Mandatory as soon as migration is complete.

---

### envforge sync push

Push local changes to sync repository.

```
Usage: envforge sync push [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-m, --message <MESSAGE>` | Custom commit message |

**Examples:**

```bash
envforge sync push
envforge sync push -m "Updated API keys for Q2"
envforge sync push --dry-run
```

---

### envforge sync pull

Pull remote changes to local.

```
Usage: envforge sync pull [OPTIONS]
```

**Examples:**

```bash
envforge sync pull
envforge sync pull --dry-run
```

---

### envforge sync status

Show sync status (local vs snapshot diff).

```
Usage: envforge sync status [OPTIONS]
```

**Examples:**

```bash
envforge sync status
envforge sync status --json
```

---

### envforge sync mark

Mark keys for sync or local-only.

```
Usage: envforge sync mark [OPTIONS] [KEY]
```

| Argument | Description |
|----------|-------------|
| `[KEY]` | Key name or glob pattern (optional when `--all` is used) |

| Flag | Description |
|------|-------------|
| `--sync` | Mark as synced |
| `--local` | Mark as local-only |
| `--all` | Apply to all keys |

**Examples:**

```bash
envforge sync mark LOCAL_SECRET --local
envforge sync mark --all --sync
envforge sync mark "DEV_*" --local
```

---

### envforge sync list-keys

List keys with sync/local status.

```
Usage: envforge sync list-keys [OPTIONS]
```

**Examples:**

```bash
envforge sync list-keys
envforge sync list-keys --json
```

---

### envforge sync override

Set a machine-specific override.

```
Usage: envforge sync override [OPTIONS] <KEY> [VALUE]
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Key name |
| `[VALUE]` | Value (omit to remove override) |

| Flag | Description |
|------|-------------|
| `--remove` | Remove override |
| `--list` | List all overrides |

**Examples:**

```bash
envforge sync override DATABASE_HOST localhost
envforge sync override DATABASE_HOST --remove
envforge sync override unused --list
```

---

### envforge sync history

Show sync history.

```
Usage: envforge sync history [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-n, --n <N>` | Number of entries [default: 10] |

**Examples:**

```bash
envforge sync history
envforge sync history -n 20
envforge sync history --json
```

---

### envforge sync rollback

Rollback to a previous snapshot.

```
Usage: envforge sync rollback [OPTIONS] [COMMIT]
```

| Argument | Description |
|----------|-------------|
| `[COMMIT]` | Commit hash to rollback to |

| Flag | Description |
|------|-------------|
| `--last` | Rollback to previous snapshot |

**Examples:**

```bash
envforge sync rollback --last
envforge sync rollback abc1234
envforge sync rollback --last --dry-run
```

---

### envforge sync log

View sync operation log.

```
Usage: envforge sync log [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-n, --n <N>` | Number of entries [default: 10] |

**Examples:**

```bash
envforge sync log
envforge sync log -n 25
```

---

### envforge sync machine

Show machine info.

```
Usage: envforge sync machine [OPTIONS]
```

**Examples:**

```bash
envforge sync machine
envforge sync machine --json
```

---

## Secret Managers

### envforge secrets pull

Pull secrets from a provider.

```
Usage: envforge secrets pull [OPTIONS] --from <FROM>
```

| Flag | Description |
|------|-------------|
| `--from <FROM>` | Provider name (vault, aws-ssm, 1password, doppler, infisical, gcp, azure, bitwarden, akeyless, conjur, sops, pass, keeper) |
| `--path <PATH>` | Secret path in the provider [default: ""] |
| `--filter <FILTER>` | Filter keys by glob pattern |

**Examples:**

```bash
envforge secrets pull --from vault --path secret/myapp
envforge secrets pull --from aws-ssm --path /myapp/ --filter "DB_*"
envforge secrets pull --from 1password --path "My Vault" --dry-run
```

---

### envforge secrets push

Push secrets to a provider.

```
Usage: envforge secrets push [OPTIONS] --to <TO>
```

| Flag | Description |
|------|-------------|
| `--to <TO>` | Provider name |
| `--path <PATH>` | Secret path in the provider [default: ""] |
| `--keys <KEYS>` | Specific keys to push (comma-separated) |
| `--all` | Push all keys |
| `--filter <FILTER>` | Filter keys by glob pattern |

**Examples:**

```bash
envforge secrets push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY
envforge secrets push --to doppler --all
envforge secrets push --to aws-ssm --path /prod/ --filter "PROD_*"
```

---

### envforge secrets ref

Create a reference to a remote secret.

```
Usage: envforge secrets ref [OPTIONS] --from <FROM> --path <PATH> <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | ENV key name |

| Flag | Description |
|------|-------------|
| `--from <FROM>` | Provider name |
| `--path <PATH>` | Full path in the provider (including key name) |

**Examples:**

```bash
envforge secrets ref DATABASE_URL --from vault --path secret/myapp/db_url
envforge secrets ref API_KEY --from aws-ssm --path /prod/api-key
```

---

### envforge secrets unref

Remove a reference (convert back to normal key).

```
Usage: envforge secrets unref [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | ENV key name |

**Examples:**

```bash
envforge secrets unref DATABASE_URL
envforge secrets unref API_KEY --dry-run
```

---

### envforge secrets resolve

Resolve secret references.

```
Usage: envforge secrets resolve [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--key <KEY>` | Specific key to resolve (omit for all) |

**Examples:**

```bash
envforge secrets resolve
envforge secrets resolve --key DATABASE_URL
```

---

### envforge secrets config

Configure provider credentials.

```
Usage: envforge secrets config [OPTIONS] <PROVIDER>
```

| Argument | Description |
|----------|-------------|
| `<PROVIDER>` | Provider name |

| Flag | Description |
|------|-------------|
| `--set <SET>` | Set a credential value (key=value format) |
| `--show` | Show stored credentials |
| `--remove` | Remove all credentials for this provider |
| `--ttl <TTL>` | Set TTL for the credential (e.g., "8h", "24h", "7d", "30d"). Used with --set |

**Examples:**

```bash
envforge secrets config vault --set addr=https://vault.example.com
envforge secrets config vault --set token=hvs.abc123 --ttl 8h
envforge secrets config vault --show
envforge secrets config doppler --remove
```

---

### envforge secrets providers

List available providers and their status.

```
Usage: envforge secrets providers [OPTIONS]
```

**Examples:**

```bash
envforge secrets providers
envforge secrets providers --json
```

---

### envforge secrets status

Show which keys come from which provider.

```
Usage: envforge secrets status [OPTIONS]
```

**Examples:**

```bash
envforge secrets status
envforge secrets status --json
```

---

### envforge secrets age

Show age of tracked secrets, flag stale ones.

```
Usage: envforge secrets age [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--threshold <THRESHOLD>` | Stale threshold in days [default: 90] |
| `--stale-only` | Only show stale secrets |

**Examples:**

```bash
envforge secrets age
envforge secrets age --threshold 30
envforge secrets age --stale-only
```

---

### envforge secrets diff

Compare local ENV vars vs provider state.

```
Usage: envforge secrets diff [OPTIONS] --from <FROM>
```

| Flag | Description |
|------|-------------|
| `--from <FROM>` | Provider name |
| `--path <PATH>` | Secret path in the provider [default: ""] |
| `--filter <FILTER>` | Filter keys by glob pattern |

**Examples:**

```bash
envforge secrets diff --from vault --path secret/myapp
envforge secrets diff --from aws-ssm --path /prod/ --filter "API_*"
```

---

### envforge secrets cache list

List all cached secrets.

```
Usage: envforge secrets cache list [OPTIONS]
```

**Examples:**

```bash
envforge secrets cache list
envforge secrets cache list --json
```

---

### envforge secrets cache clear

Clear cache.

```
Usage: envforge secrets cache clear [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--provider <PROVIDER>` | Only clear cache for a specific provider |

**Examples:**

```bash
envforge secrets cache clear
envforge secrets cache clear --provider vault
```

---

### envforge rotate

Rotate a secret: update value, reset age, optionally push to provider and sync.

```
Usage: envforge rotate [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name to rotate |

| Flag | Description |
|------|-------------|
| `--dry-run` | Preview rotation without making changes |
| `--stale` | Rotate all stale secrets interactively |
| `--propagate` | Auto-push to provider and sync after rotation (no interactive prompts) |

**Examples:**

```bash
envforge rotate API_KEY
envforge rotate DATABASE_PASSWORD --dry-run
envforge rotate unused --stale
envforge rotate STRIPE_KEY --propagate
```

---

### envforge resolve-uri

Resolve secret URIs in a config file (vault://path, aws-ssm://path, etc.).

```
Usage: envforge resolve-uri [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to file with secret URIs |

| Flag | Description |
|------|-------------|
| `--env` | Output as .env format (default: export statements) |
| `--output <OUTPUT>` | Output file (default: stdout) |

**Examples:**

```bash
envforge resolve-uri config/secrets.yml
envforge resolve-uri config/secrets.yml --env
envforge resolve-uri config/secrets.yml --output .env.resolved
```

---

### Provider Quick Reference

| Provider | Name | Binary | Required Fields | Path Used? |
|----------|------|--------|-----------------|------------|
| HashiCorp Vault | `vault` | `vault` | `addr` | Yes — KV mount path |
| AWS SSM | `aws-ssm` | `aws` | _(none)_ | Yes — parameter path prefix |
| 1Password | `1password` | `op` | `service_account_token` | Yes — item name or UUID |
| Doppler | `doppler` | `doppler` | `token`, `project`, `config` | No — ignored |
| Infisical | `infisical` | `infisical` | `token`, `project_id`, `environment` | No — ignored |
| GCP Secret Manager | `gcp` | `gcloud` | `project_id` | No — ignored |
| Azure Key Vault | `azure` | `az` | `vault_name` | No — ignored |
| Bitwarden | `bitwarden` | `bws` | `access_token` | No — ignored |
| Akeyless | `akeyless` | `akeyless` | `access_id`, `access_key` | Yes — item path prefix |
| CyberArk Conjur | `conjur` | `conjur` | `url`, `account`, `login`, `api_key` | Yes — variable path prefix |
| Mozilla SOPS | `sops` | `sops` | `key_file` | Yes — encrypted file path |
| pass/gopass | `pass` | `pass`/`gopass` | _(none)_ | Yes — entry prefix filter |
| Keeper | `keeper` | `ksm` | _(none)_ | No — ignored |

---

## Secret Sharing

### envforge share create

Create an encrypted share file.

```
Usage: envforge share create [OPTIONS] --recipient <RECIPIENT>
```

| Flag | Description |
|------|-------------|
| `--recipient <RECIPIENT>` | Recipient's age public key (age1...) |
| `--keys <KEYS>` | Specific keys to share (comma-separated) |
| `--all` | Share all keys |
| `--filter <FILTER>` | Filter by pattern |
| `--output <OUTPUT>` | Output file path [default: envforge-share.age] |
| `--expire <EXPIRE>` | Expiry in hours |

**Examples:**

```bash
envforge share create --recipient age1abc... --keys API_KEY,DB_URL
envforge share create --recipient age1abc... --all --expire 24
envforge share create --recipient age1abc... --filter "PROD_*" --output prod-secrets.age
```

---

### envforge share receive

Receive and import a share file.

```
Usage: envforge share receive [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to share file |

| Flag | Description |
|------|-------------|
| `--import` | Import keys into EnvForge config |

**Examples:**

```bash
envforge share receive envforge-share.age
envforge share receive prod-secrets.age --import
```

---

## Security Scanning

### envforge scan

Scan files for leaked secrets.

```
Usage: envforge scan [OPTIONS] [PATH]
```

| Argument | Description |
|----------|-------------|
| `[PATH]` | Path to scan (default: current directory) |

| Flag | Description |
|------|-------------|
| `--staged` | Only scan git staged files |
| `--install-hook` | Install git pre-commit hook that runs `envforge scan --staged` |
| `--remove-hook` | Remove the envforge pre-commit hook |
| `--mcp` | Scan MCP config files for hardcoded credentials |

**Examples:**

```bash
envforge scan
envforge scan ./src
envforge scan --staged
envforge scan --install-hook
envforge scan --mcp
```

---

### envforge mcp status

Check if any MCP config files contain plaintext secrets.

```
Usage: envforge mcp status [OPTIONS]
```

**Examples:**

```bash
envforge mcp status
envforge mcp status --json
```

Exits non-zero (2) when a hardcoded credential is found, so it can gate CI (see [`ci-gating.md`](ci-gating.md)).

---

### envforge mcp serve

Run the EnvForge **MCP server** over stdio: a read-safe Model Context Protocol
server that exposes env-var key names and schema metadata (redacted, audited —
**never raw secret values**) to AI agents. Requires a build with the
`mcp-server` feature. Tools: `list_keys`, `describe_schema`. Client config
snippets: [`mcp-server.md`](mcp-server.md).

```
Usage: envforge mcp serve
```

**Example (Claude Code `.mcp.json`):**

```json
{ "mcpServers": { "envforge": { "command": "envforge", "args": ["mcp", "serve"] } } }
```

---

### envforge mcp harden

Replace plaintext secrets with ${VAR} env var references (backs up originals).

```
Usage: envforge mcp harden [OPTIONS]
```

**Examples:**

```bash
envforge mcp harden
envforge mcp harden --dry-run
```

---

### envforge mcp pin

Record the MCP servers you trust in a lockfile, so you are warned if any of them change.

```
Usage: envforge mcp pin [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--strict` | Require a known-good reputation for every server |
| `--inspect` | Connect to each server and record its tool list |
| `--refresh` | Re-check every server and update the lockfile |
| `--accept` | Apply a refresh after reviewing the changes |
| `--lockfile <PATH>` | Use a lockfile other than `.envforge/mcp.lock` |

**Examples:**

```bash
# Pin the servers in your config
envforge mcp pin

# Update the lockfile after adding a server
envforge mcp pin --refresh --accept
```

---

### envforge mcp verify

Check that your MCP servers still match the trusted lockfile. Exits non-zero if anything changed.

```
Usage: envforge mcp verify [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--json` | Output a machine-readable report |
| `--strict` | Also fail on servers with an unknown reputation |
| `--lockfile <PATH>` | Use a lockfile other than `.envforge/mcp.lock` |

**Examples:**

```bash
envforge mcp verify
envforge mcp verify --strict --json
```

---

### envforge mcp diff

Show what changed between the trusted lockfile and your current MCP servers.

```
Usage: envforge mcp diff [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--server <NAME>` | Show changes for one server only |
| `--lockfile <PATH>` | Use a lockfile other than `.envforge/mcp.lock` |

**Examples:**

```bash
envforge mcp diff
envforge mcp diff --server my-server
```

---

### envforge mcp explain

Print the lockfile in a readable, annotated form for review (for example, in a pull request).

```
Usage: envforge mcp explain [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--lock` | Render the lockfile (the only mode today) |
| `--format <FORMAT>` | Output format: `text` or `markdown` (default: `text`) |

**Examples:**

```bash
envforge mcp explain --lock
envforge mcp explain --lock --format markdown
```

---

### envforge mcp trust

Mark a server as trusted by you, even if its reputation is unknown. A reason is required.

```
Usage: envforge mcp trust <NAME> --reason <REASON>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Server name (npm package or alias) |
| `--reason <REASON>` | Why you trust this server (recorded for the audit log) |

**Examples:**

```bash
envforge mcp trust my-internal-server --reason "built and reviewed in-house"
```

---

### envforge mcp untrust

Remove a trust override you previously recorded for a server.

```
Usage: envforge mcp untrust <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Server name |

**Examples:**

```bash
envforge mcp untrust my-internal-server
```

---

### envforge mcp launch

Verify the lockfile and, only if it passes, start your editor. One safe step instead of two.

```
Usage: envforge mcp launch <IDE> [-- <ARGS>...]
```

| Argument | Description |
|----------|-------------|
| `<IDE>` | Editor to start: `claude-code`, `claude`, or `cursor` |
| `[ARGS]` | Extra arguments passed straight to the editor |

**Examples:**

```bash
envforge mcp launch claude-code
envforge mcp launch cursor -- .
```

---

## AI Safety

### envforge fence

Create AI tool ignore rules for all supported tools (Cursor, Copilot, Claude Code), check status, or remove the envforge-owned fence content while preserving user content. By default all five targets are written; use `fence config` to enable/disable individual targets. Configuration is read from the global config (`~/.config/envforge/config.toml`, `[fence.targets]`); when no config is present every target is enabled (identical to prior behavior).

```
Usage: envforge fence [OPTIONS] [COMMAND]
```

| Flag | Description |
|------|-------------|
| `--status` | Check if all *enabled* ignore rules are correctly installed (mutually exclusive with `--disable`). `all_fenced` is computed over the enabled target set. |
| `--disable` | Strip envforge-owned fence content from `.envforgeignore`, `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json` (all known targets, regardless of config). User content is preserved; `.envforgeignore` (fully envforge-owned) is deleted outright. |

**Examples:**

```bash
envforge fence
envforge fence --status
envforge fence --status --json
envforge fence --disable
envforge fence --dry-run
```

### envforge fence config

Inspect or change which fence targets are written. Targets: `envforgeignore`, `cursor_ignore`, `cursor_rules`, `copilot`, `claude_code`. Changes persist to the global config (atomic write). Disabled targets are skipped by `fence` activation across every surface (CLI, TUI, LSP, IDE plugins); `fence --disable` still cleans up all known targets.

```
Usage: envforge fence config [--list] [--enable <TARGET>] [--disable <TARGET>] [--json]
```

| Flag | Description |
|------|-------------|
| `--list` | Show each target with its effective state and source (`default` / `global`) |
| `--enable <TARGET>` | Enable a target, persisting the choice |
| `--disable <TARGET>` | Disable a target, persisting the choice |
| `--json` | Emit the target list as JSON (`[{"target","enabled","source"}]`) for scripts/plugins |

**Examples:**

```bash
envforge fence config --list
envforge fence config --list --json
envforge fence config --disable copilot
envforge fence config --enable copilot
```

### envforge ai-guard

AI agent guard — invoked by AI tool hooks (not for direct use).

```
Usage: envforge ai-guard [OPTIONS] <STAGE> <TOOL_NAME> [TOOL_INPUT]
```

| Argument | Description |
|----------|-------------|
| `<STAGE>` | Hook stage: `pre-tool`, `post-tool` |
| `<TOOL_NAME>` | Tool name |
| `[TOOL_INPUT]` | Tool input (JSON string or path) |

**Examples:**

```bash
envforge ai-guard pre-tool Write '{"file_path": ".env"}'
envforge ai-guard post-tool Read '{"file_path": "config.json"}'
```

---

### envforge proxy

Start local credential proxy for AI agents.

```
Usage: envforge proxy [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--port <PORT>` | Port to listen on [default: 8100] |
| `--keys <KEYS>` | Only serve these keys (comma-separated) |
| `--profile <PROFILE>` | Profile to use |
| `--allow-origins <ALLOW_ORIGINS>` | Allowed origins (comma-separated, default: localhost only) |
| `--require-lease` | Require active lease for access |
| `--require-approval` | Require human approval for each secret access |

**Examples:**

```bash
envforge proxy
envforge proxy --keys API_KEY,DATABASE_URL --port 9000
envforge proxy --require-lease --require-approval
envforge proxy --allow-origins "http://localhost:3000,http://localhost:8080"
```

---

### envforge lease create

Create a new time-bounded secret access lease.

```
Usage: envforge lease create [OPTIONS] --ttl <TTL>
```

| Flag | Description |
|------|-------------|
| `--name <NAME>` | Lease name (default: auto-generated) |
| `--ttl <TTL>` | Time-to-live (e.g., "1h", "30m", "8h", "24h", "7d") |
| `--keys <KEYS>` | Restrict to specific keys (comma-separated) |

**Examples:**

```bash
envforge lease create --ttl 1h
envforge lease create --name deploy-lease --ttl 30m --keys API_KEY,SECRET
envforge lease create --ttl 7d
```

---

### envforge lease list

List all leases.

```
Usage: envforge lease list [OPTIONS]
```

**Examples:**

```bash
envforge lease list
envforge lease list --json
```

---

### envforge lease cleanup

Clean up expired leases.

```
Usage: envforge lease cleanup [OPTIONS]
```

**Examples:**

```bash
envforge lease cleanup
envforge lease cleanup --dry-run
```

---

### envforge lease renew

Extend an active lease's TTL without recreating it. Same code path the IDE plugins call when the user clicks the volatile-countdown status bar item.

```
Usage: envforge lease renew <NAME> --ttl <TTL>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Lease name (as shown by `envforge lease list`) |

| Flag | Description |
|------|-------------|
| `--ttl <TTL>` | New TTL counted from now (e.g. `30m`, `2h`, `1d`). Replaces the remaining time, does not stack. |

**Examples:**

```bash
envforge lease renew deploy-lease --ttl 1h
envforge lease renew deploy-lease --ttl 30m --json
```

Rejects revoked or already-expired leases — create a fresh lease instead.

---

### envforge lease grant

Give one running command short-lived access to a single secret, then auto-expire it. Tied to a specific process, so nothing else can use it.

```
Usage: envforge lease grant <KEY> --pid <PID> --ttl <TTL> [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Secret name (e.g. `STRIPE_KEY`) |
| `--pid <PID>` | The process that is allowed to use the secret |
| `--ttl <TTL>` | How long the access lasts (e.g. `30s`, `5m`, `1h`) |
| `--tool <TOOL>` | Tool or agent name, recorded in the audit log |
| `--multi-redeem` | Allow the secret to be read more than once |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge lease grant STRIPE_KEY --pid 4821 --ttl 30s
envforge lease grant DB_URL --pid 4821 --ttl 5m --tool claude-code --json
```

---

### envforge lease revoke

Cancel an active lease right away by name.

```
Usage: envforge lease revoke <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Lease name |

**Examples:**

```bash
envforge lease revoke debug-session
```

---

### envforge lease status

Show the details of one lease: what it covers, who it is for, and how long is left.

```
Usage: envforge lease status <NAME> [--json]
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Lease name |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge lease status deploy-lease
envforge lease status deploy-lease --json
```

---

### envforge session start

Start a new AI tool session with scoped secret access.

```
Usage: envforge session start [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--tool <TOOL>` | AI tool type: `claude-code`, `cursor`, `copilot` (auto-detected if omitted) |
| `--ttl <TTL>` | Session TTL (e.g., "1h", "30m", "8h", "1d") [default: 1h] |

**Examples:**

```bash
envforge session start
envforge session start --tool claude-code
envforge session start --tool cursor --ttl 30m
```

---

### envforge session stop

Stop (expire) a session.

```
Usage: envforge session stop [OPTIONS] [ID]
```

| Argument | Description |
|----------|-------------|
| `[ID]` | Session ID to stop (current session if omitted) |

**Examples:**

```bash
envforge session stop
envforge session stop 246f86ae-8b41-48bb-b9f7-0da491594b23
```

---

### envforge session list

List active and expired sessions.

```
Usage: envforge session list [OPTIONS]
```

**Examples:**

```bash
envforge session list
envforge session list --json
```

---

### envforge session show

Show details for a specific session.

```
Usage: envforge session show [OPTIONS] <ID>
```

| Argument | Description |
|----------|-------------|
| `<ID>` | Session ID |

**Examples:**

```bash
envforge session show 246f86ae-8b41-48bb-b9f7-0da491594b23
envforge session show 246f86ae-8b41-48bb-b9f7-0da491594b23 --json
```

---

### envforge session cleanup

Clean up expired sessions.

```
Usage: envforge session cleanup [OPTIONS]
```

**Examples:**

```bash
envforge session cleanup
envforge session cleanup --dry-run
```

---

### envforge revoke

Emergency revoke all secret access.

```
Usage: envforge revoke [OPTIONS] [NAME]
```

| Argument | Description |
|----------|-------------|
| `[NAME]` | Specific lease name to revoke |

| Flag | Description |
|------|-------------|
| `--all` | Revoke all active leases (killswitch) |

**Examples:**

```bash
envforge revoke deploy-lease
envforge revoke --all
envforge revoke --all --dry-run
```

---

## Canary Tokens

### envforge canary create

Create a canary secret (honeypot credential that alerts when the fake value is seen in scanned output).

```
Usage: envforge canary create [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Key name (e.g., AWS_SECRET_KEY) |

| Flag | Description |
|------|-------------|
| `--pattern <PATTERN>` | Pattern: `aws_key`, `github_token`, `stripe_key`, `slack_token`, `gitlab_token`, `generic` [default: generic] |

**Examples:**

```bash
envforge canary create HONEYPOT_API_KEY
envforge canary create AWS_SECRET_ACCESS_KEY --pattern aws_key
envforge canary create GITHUB_TOKEN --pattern github_token
```

---

### envforge canary list

List all canary secrets.

```
Usage: envforge canary list [OPTIONS]
```

**Examples:**

```bash
envforge canary list
envforge canary list --json
```

---

### envforge canary check

Check for triggered canaries.

```
Usage: envforge canary check [OPTIONS]
```

**Examples:**

```bash
envforge canary check
envforge canary check --json
```

---

### envforge canary delete

Delete a canary.

```
Usage: envforge canary delete [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Canary key name |

**Examples:**

```bash
envforge canary delete HONEYPOT_API_KEY
envforge canary delete OLD_CANARY --dry-run
```

---

### envforge canary rotate

Rotate canary values (regenerate fake values).

```
Usage: envforge canary rotate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--all` | Rotate all eligible canaries |
| `--key <KEY>` | Rotate a specific canary by key |
| `--dry-run` | Show what would be rotated without making changes |

**Examples:**

```bash
envforge canary rotate --key HONEYPOT_API_KEY
envforge canary rotate --all
envforge canary rotate --all --dry-run
```

---

### envforge canary place

Place a canary line into a file.

```
Usage: envforge canary place [OPTIONS] <KEY> <FILE>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Canary key |
| `<FILE>` | Target file path |

| Flag | Description |
|------|-------------|
| `--position <POSITION>` | Position: `top`, `middle`, `bottom`, `random` [default: bottom] |

**Examples:**

```bash
envforge canary place HONEYPOT_API_KEY ./src/config.ts
envforge canary place AWS_SECRET ./src/index.ts --position top
```

---

### envforge canary mint-v2

Create a trackable fake secret. If it ever leaks, you can decode it to learn which tool and process leaked it.

```
Usage: envforge canary mint-v2 <KEY> [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Canary key (e.g. `STRIPE_KEY`) |
| `--tool <TOOL>` | Tool name to embed (e.g. `claude-code`, `cursor`) |
| `--pid <PID>` | Process ID to embed (defaults to the current process) |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge canary mint-v2 STRIPE_KEY --tool claude-code
envforge canary mint-v2 AWS_SECRET --json
```

---

### envforge canary decode

Read a tracked fake secret (a v2 canary) to find out where and when it leaked.

```
Usage: envforge canary decode <TOKEN> [--json]
```

| Argument | Description |
|----------|-------------|
| `<TOKEN>` | The token string to decode |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge canary decode ef_v2_8a3f...
envforge canary decode ef_v2_8a3f... --json
```

---

### envforge canary scan

Search a file (or piped input) for any tracked fake secrets that leaked into it.

```
Usage: envforge canary scan <INPUT> [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `<INPUT>` | File path, or `-` to read from standard input |
| `--strict` | Exit non-zero if any token is found |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge canary scan ./logs/app.log
cat output.txt | envforge canary scan - --strict
```

---

### envforge canary rotate-key

Change the secret key used to sign new canary tokens. Older keys are kept so existing tokens still decode.

```
Usage: envforge canary rotate-key [--dry-run]
```

| Flag | Description |
|------|-------------|
| `--dry-run` | Show what would happen without making changes |

**Examples:**

```bash
envforge canary rotate-key --dry-run
envforge canary rotate-key
```

---

### envforge canary migrate

Upgrade older (v1) canaries to the trackable v2 format.

```
Usage: envforge canary migrate [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--bulk` | Migrate every eligible v1 canary |
| `--replace <KEY>` | Migrate one specific canary by key |
| `--dry-run` | Show what would be migrated without making changes |

**Examples:**

```bash
envforge canary migrate --dry-run
envforge canary migrate --bulk
```

---

## Adversarial Hardening

### envforge hardening show

Show current hardening configuration.

```
Usage: envforge hardening show [OPTIONS]
```

**Examples:**

```bash
envforge hardening show
```

---

### envforge hardening enable

Enable a hardening layer.

```
Usage: envforge hardening enable [OPTIONS] <LAYER>
```

| Argument | Description |
|----------|-------------|
| `<LAYER>` | Layer name: `control_chars`, `base64_decode`, `split_strings`, `encoding_chain` |

**Examples:**

```bash
envforge hardening enable control_chars
envforge hardening enable base64_decode
```

---

### envforge hardening disable

Disable a hardening layer.

```
Usage: envforge hardening disable [OPTIONS] <LAYER>
```

| Argument | Description |
|----------|-------------|
| `<LAYER>` | Layer name: `control_chars`, `base64_decode`, `split_strings`, `encoding_chain` |

**Examples:**

```bash
envforge hardening disable control_chars
envforge hardening disable encoding_chain
```

---

### envforge scanner list

List configured external scanners.

```
Usage: envforge scanner list [OPTIONS]
```

**Examples:**

```bash
envforge scanner list
```

---

### envforge scanner test

Test a scanner with sample content.

```
Usage: envforge scanner test [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Scanner name |

**Examples:**

```bash
envforge scanner test trufflehog
```

---

### envforge scanner run

Run a scanner against arbitrary content.

```
Usage: envforge scanner run [OPTIONS] <NAME> <CONTENT>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Scanner name |
| `<CONTENT>` | Content to scan |

**Examples:**

```bash
envforge scanner run trufflehog "sk-abc123def456"
```

---

### envforge scanner enable

Enable a scanner.

```
Usage: envforge scanner enable [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Scanner name |

**Examples:**

```bash
envforge scanner enable trufflehog
```

---

### envforge scanner disable

Disable a scanner.

```
Usage: envforge scanner disable [OPTIONS] <NAME>
```

| Argument | Description |
|----------|-------------|
| `<NAME>` | Scanner name |

**Examples:**

```bash
envforge scanner disable trufflehog
```

---

## Audit & Diagnostics

### envforge audit

View change audit trail from sync history.

```
Usage: envforge audit [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--key <KEY>` | Filter by key name |
| `--since <SINCE>` | Filter changes since date (ISO 8601) |
| `--machine <MACHINE>` | Filter by machine ID |
| `-n, --n <N>` | Number of entries to show [default: 50] |
| `--ai-leaks` | Scan git history for secrets leaked in AI-assisted commits |
| `--access` | Show proxy access audit log |

**Examples:**

```bash
envforge audit
envforge audit --key DATABASE_URL
envforge audit --since 2025-01-01
envforge audit --ai-leaks
envforge audit --access
envforge audit --machine macbook-work -n 20
```

---

### envforge audit-trail query

Query audit events with filters.

```
Usage: envforge audit-trail query [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--event-type <EVENT_TYPE>` | Filter by event type |
| `--source <SOURCE>` | Filter by source |
| `--secret-key <SECRET_KEY>` | Filter by secret key |
| `--time <TIME>` | Time range: last_1h, last_24h, last_7d, last_30d, all [default: last_24h] |
| `--limit <LIMIT>` | Limit number of results [default: 50] |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail query --time last_24h
envforge audit-trail query --event-type SECRET_ACCESS --source proxy
envforge audit-trail query --secret-key API_KEY --time last_7d
envforge audit-trail query --time all --limit 100 --json
```

---

### envforge audit-trail report

Generate a compliance report.

```
Usage: envforge audit-trail report [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--report-type <REPORT_TYPE>` | summary, detail, trend, violation, compliance [default: summary] |
| `--group-by <GROUP_BY>` | event_type, source, result, hour, day, week, month, secret_key, tool_type |
| `--time <TIME>` | Time range: last_1h, last_24h, last_7d, last_30d, all [default: last_24h] |
| `--format <FORMAT>` | json, csv, markdown [default: json] |
| `--output <OUTPUT>` | Output file path (stdout if not specified) |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail report --report-type summary
envforge audit-trail report --report-type compliance --format markdown
envforge audit-trail report --report-type trend --group-by day --time last_7d
envforge audit-trail report --report-type detail --output report.csv --format csv
```

---

### envforge audit-trail custody

Show chain of custody for a secret.

```
Usage: envforge audit-trail custody [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--secret-key <SECRET_KEY>` | Secret key to trace |
| `--session <SESSION>` | Session ID to trace |
| `--ownership` | Show ownership report |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail custody --secret-key API_KEY
envforge audit-trail custody --secret-key DB_PASSWORD --ownership
envforge audit-trail custody --session 246f86ae-8b41
```

---

### envforge audit-trail integrity

Verify tamper-evident integrity of audit logs.

```
Usage: envforge audit-trail integrity [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--category <CATEGORY>` | Specific log category: ai-guard, proxy, sync, cli, tui, hook, general |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail integrity
envforge audit-trail integrity --category proxy
envforge audit-trail integrity --category ai-guard --json
```

---

### envforge audit-trail stats

Show audit statistics.

```
Usage: envforge audit-trail stats [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--time <TIME>` | Time range: last_1h, last_24h, last_7d, last_30d, all [default: last_24h] |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail stats
envforge audit-trail stats --time last_7d
envforge audit-trail stats --time all --json
```

---

### envforge audit-trail tail

Tail recent audit events.

```
Usage: envforge audit-trail tail [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--n <N>` | Number of recent events to show [default: 20] |
| `--source <SOURCE>` | Filter by source |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail tail
envforge audit-trail tail --n 50
envforge audit-trail tail --source proxy --json
```

---

### envforge audit-trail retention

Manage audit log retention.

```
Usage: envforge audit-trail retention [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--policy <POLICY>` | Delete events older than: 1d, 7d, 30d, 90d, 365d [default: 90d] |
| `--execute` | Actually perform cleanup (dry-run by default) |
| `--log-dir <LOG_DIR>` | Audit log directory |

**Examples:**

```bash
envforge audit-trail retention
envforge audit-trail retention --policy 30d
envforge audit-trail retention --policy 7d --execute
```

---

### envforge doctor

Run health checks on EnvForge setup.

```
Usage: envforge doctor [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed output for each check |

**Examples:**

```bash
envforge doctor
envforge doctor --verbose
envforge doctor --json
```

---

### envforge check

Run all checks: doctor + validate + scan + age + drift.

```
Usage: envforge check [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--only <ONLY>` | Only run specific categories (comma-separated: `doctor`, `validate`, `scan`, `age`, `drift`) |

**Examples:**

```bash
envforge check
envforge check --only validate,scan
envforge check --json
```

---

### envforge log

View change history.

```
Usage: envforge log [OPTIONS] [KEY]
```

| Argument | Description |
|----------|-------------|
| `[KEY]` | Filter by key name |

| Flag | Description |
|------|-------------|
| `-n, --n <N>` | Number of entries to show [default: 50] |

**Examples:**

```bash
envforge log
envforge log API_KEY
envforge log -n 10
```

---

### envforge config

Show current configuration.

```
Usage: envforge config [OPTIONS]
```

**Examples:**

```bash
envforge config
envforge config --json
```

---

### envforge offset

Show managed zone and protected block offsets.

```
Usage: envforge offset [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--show` | Show detected protected blocks |
| `--suggest` | Suggest header/footer offsets |

**Examples:**

```bash
envforge offset --show
envforge offset --suggest
envforge offset --show --json
```

---

## CI Trust

### envforge ci-trust classify

Classify a CI trigger by trust level (pull_request, push, schedule, workflow_dispatch).

```
Usage: envforge ci-trust classify [OPTIONS] <EVENT>
```

| Argument | Description |
|----------|-------------|
| `<EVENT>` | GitHub Actions event name (e.g., `pull_request`, `push`) |

| Flag | Description |
|------|-------------|
| `--actor <ACTOR>` | GitHub username who triggered the workflow |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge ci-trust classify pull_request --actor dependabot
envforge ci-trust classify push --actor admin --json
```

### envforge ci-trust quarantine

Remove risky variables from an untrusted CI run.

```
Usage: envforge ci-trust quarantine [OPTIONS]
```

**Examples:**

```bash
envforge ci-trust quarantine --output .env.safe
```

### envforge ci-trust summary

Generate a CI Trust summary report in GitHub Step Summary format.

```
Usage: envforge ci-trust summary
```

---

## Secret Lifecycle

### envforge lifecycle check

Evaluate all enabled lifecycle rules against current state.

```
Usage: envforge lifecycle check [OPTIONS]
```

**Examples:**

```bash
envforge lifecycle check
envforge lifecycle check --json
```

Output (text):
```
Fired 2 trigger(s):
  [age] rule=rotate-stale-tokens — Secret API_KEY is 95 days old
  [cron] rule=weekly-health-report — Weekly health check triggered
```

---

### envforge lifecycle rule list

List all configured lifecycle rules.

```
Usage: envforge lifecycle rule list [OPTIONS]
```

**Examples:**

```bash
envforge lifecycle rule list
envforge lifecycle rule list --json
```

---

### envforge lifecycle rule rotate-secret

Rotate a secret via lifecycle orchestrator.

```
Usage: envforge lifecycle rule rotate-secret <KEY> [--strategy STRATEGY]
```

| Argument/Flag | Description |
|---|---|
| `<KEY>` | Secret key to rotate |
| `--strategy <STRATEGY>` | Rotation strategy: `replace`, `dual_write`, `blue_green`, `provider_managed` [default: replace] |

**Examples:**

```bash
envforge lifecycle rule rotate-secret API_KEY --strategy dual_write
envforge lifecycle rule rotate-secret DB_PASSWORD --json
```

---

### envforge lifecycle state

Show lifecycle state for a secret (creating, active, rotating, deprecated, etc.).

```
Usage: envforge lifecycle state <KEY> [OPTIONS]
```

**Examples:**

```bash
envforge lifecycle state API_KEY
envforge lifecycle state DB_PASSWORD --json
```

Output:
```
API_KEY: Active (last rotated: 2026-04-15T12:00:00Z, rotation count: 3)
```

---

### envforge lifecycle snapshot list

List lifecycle snapshots for a key.

```
Usage: envforge lifecycle snapshot list [--key KEY] [OPTIONS]
```

| Flag | Description |
|---|---|
| `--key <KEY>` | Filter by key |

**Examples:**

```bash
envforge lifecycle snapshot list
envforge lifecycle snapshot list --key API_KEY --json
```

---

### envforge lifecycle snapshot delete

Delete a lifecycle snapshot by ID.

```
Usage: envforge lifecycle snapshot delete <ID>
```

**Examples:**

```bash
envforge lifecycle snapshot delete 550e8400-e29b-41d4-a716-446655440000
```

---

## Secret Analytics

Track secret usage patterns, detect dormant secrets, and generate deprecation recommendations.

### envforge analytics unused

Detect secrets with no access in N days.

```
Usage: envforge analytics unused [--threshold DAYS] [OPTIONS]
```

| Flag | Description |
|---|---|
| `--threshold <DAYS>` | Days threshold [default: 90] |

**Examples:**

```bash
envforge analytics unused
envforge analytics unused --threshold 30 --json
```

Output:
```
Unused secrets (no access in 90 days):

  OLD_API_KEY — dormant in all environments (confidence: 95%)
  LEGACY_TOKEN — only referenced in archived configs (confidence: 87%)
```

---

### envforge analytics low-usage

Detect secrets with unusually low access counts.

```
Usage: envforge analytics low-usage [--max-accesses N] [--days DAYS] [OPTIONS]
```

| Flag | Description |
|---|---|
| `--max-accesses <N>` | Maximum access count threshold [default: 5] |
| `--days <DAYS>` | Time window in days [default: 30] |

**Examples:**

```bash
envforge analytics low-usage
envforge analytics low-usage --max-accesses 3 --days 90
```

---

### envforge analytics deprecation

Show deprecation recommendations with timelines.

```
Usage: envforge analytics deprecation [OPTIONS]
```

**Examples:**

```bash
envforge analytics deprecation
envforge analytics deprecation --json
```

Output:
```
Deprecation recommendations:

  OLD_API_KEY — Dormant >90 days, 0 dependents
    Review by: 2026-05-14, Deprecate by: 2026-06-13, Remove by: 2026-08-12
    Confidence: 95%, Dependents: 0
```

---

### envforge analytics summary

Show analytics summary (event count, secret count, config state).

```
Usage: envforge analytics summary [--days DAYS] [OPTIONS]
```

| Flag | Description |
|---|---|
| `--days <DAYS>` | Time window in days [default: 7] |

**Examples:**

```bash
envforge analytics summary
envforge analytics summary --days 30 --json
```

Output:
```
Analytics Summary:

  Total secrets:    42
  Total events:     1847
  Active secrets:   38

  Config:
    enabled:        true
    retention_days: 365
    max_events:     100000
    auto_aggregate: true
```

---

### envforge analytics recompute

Recompute aggregate statistics from raw events.

```
Usage: envforge analytics recompute [OPTIONS]
```

**Examples:**

```bash
envforge analytics recompute
envforge analytics recompute --json
```

---

### envforge analytics retention show

Show current retention settings.

```
Usage: envforge analytics retention show [OPTIONS]
```

**Examples:**

```bash
envforge analytics retention show
envforge analytics retention show --json
```

---

### envforge analytics retention set

Set retention days for raw events.

```
Usage: envforge analytics retention set --days <DAYS>
```

| Flag | Description |
|---|---|
| `--days <DAYS>` | Number of days to retain raw events |

**Examples:**

```bash
envforge analytics retention set --days 90
```

---

### envforge analytics prune

Remove raw events older than a date or retention window.

```
Usage: envforge analytics prune [--before DATE] [OPTIONS]
```

| Flag | Description |
|---|---|
| `--before <DATE>` | Remove events before this date (ISO 8601 or YYYY-MM-DD) |

**Examples:**

```bash
# Prune based on retention_days setting
envforge analytics prune

# Prune before specific date
envforge analytics prune --before 2025-01-01
envforge analytics prune --before 2026-01-01T00:00:00Z --json
```

---

## ENV-BOM (Environment Bill of Materials)

### envforge envbom emit

Generate an ENV-BOM (SHA-256 digest of canonical JSON, not a signature) for reproducible environment attestation.

```
Usage: envforge envbom emit [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--output <FILE>` | Output file path |
| `--reproducible-now <TIMESTAMP>` | Timestamp for reproducible builds |
| `--json` | Output as JSON |

**Examples:**

```bash
envforge envbom emit
envforge envbom emit --output env-bom.toml
envforge envbom emit --reproducible-now "2026-06-03T00:00:00Z"
```

### envforge envbom verify

Verify an ENV-BOM against the current environment.

```
Usage: envforge envbom verify [OPTIONS] <FILE>
```

---

## Real-Time Monitoring

### envforge monitor status

Run health checks on secret infrastructure (providers, canary integrity, fence status, encryption).

```
Usage: envforge monitor status [OPTIONS]
```

**Examples:**

```bash
envforge monitor status
envforge monitor status --json
```

Output (text):
```
Health Check Results:

  ✓ providers     — 13/13 available
  ✓ canary        — 4 canaries intact
  ✓ encryption    — age key accessible
  ✓ fence         — active on 3/3 AI tools
```

---

### envforge monitor stream

Stream real-time secret access events.

```
Usage: envforge monitor stream [OPTIONS]
```

**Examples:**

```bash
envforge monitor stream
```

Output (streaming JSON lines):
```
{"timestamp":"2026-05-07T12:00:00Z","key":"API_KEY","accessor":"claude-code","action":"read","source":"proxy"}
{"timestamp":"2026-05-07T12:00:01Z","key":"DB_URL","accessor":"cursor","action":"reference","source":"lsp"}
```

Press Ctrl+C to stop.

---

## Git Integration

### envforge git install-merge-driver

Install EnvForge as a Git merge driver for .env files.

```
Usage: envforge git install-merge-driver [OPTIONS]
```

**Examples:**

```bash
envforge git install-merge-driver
envforge git install-merge-driver --dry-run
```

---

### envforge git remove-merge-driver

Remove the Git merge driver.

```
Usage: envforge git remove-merge-driver [OPTIONS]
```

**Examples:**

```bash
envforge git remove-merge-driver
```

---

### envforge git merge

Three-way merge for .env files (called by Git, not directly by users).

```
Usage: envforge git merge <BASE> <OURS> <THEIRS>
```

| Argument | Description |
|----------|-------------|
| `<BASE>` | Base file (ancestor) |
| `<OURS>` | Ours file (current branch) |
| `<THEIRS>` | Theirs file (other branch) |

---

## Shell & Editor Integration

### envforge completions

Generate and install shell completion scripts.

```
Usage: envforge completions [OPTIONS] <SHELL>
```

| Argument | Description |
|----------|-------------|
| `<SHELL>` | Shell type: `zsh`, `bash`, `fish`, `elvish`, `powershell`, `fig`, `kiro`, `carapace`, `inshellisense` |

| Flag | Description |
|------|-------------|
| `--install` | Install completion spec to the correct system path |

**Examples:**

```bash
envforge completions zsh > ~/.zsh/completions/_envforge
envforge completions zsh --install
envforge completions bash --install
envforge completions fish --install
envforge completions kiro --install
envforge completions fig --install
envforge completions carapace --install
envforge completions inshellisense --install
```

#### Kiro CLI Setup

Kiro CLI uses a graphical dropdown autocomplete powered by Fig-format specs. To enable EnvForge completions in Kiro:

```bash
envforge completions kiro --install
```

This does three things:
1. Writes the Fig spec to `~/.kiro/specs/envforge.js`
2. Configures `kiro-cli settings autocomplete.devCompletionsFolder` to point to `~/.kiro/specs`
3. Also writes to `~/.fig/autocomplete/build/envforge.js` for backward compatibility

After installation:
- Enable developer mode: `kiro-cli settings autocomplete.developerMode true`
- Restart Kiro: `kiro-cli restart`
- Open a new terminal and type `envforge <TAB>`

> **Note:** The spec file must use plain JavaScript syntax (not TypeScript). The `--install` flag handles this automatically.

#### Carapace Setup

Carapace is a multi-shell completion engine (bash, zsh, fish, elvish, nushell, xonsh, powershell) that uses YAML-format specs.

```bash
envforge completions carapace --install
```

Installs `~/.config/carapace/specs/envforge.yaml`. Restart your terminal to activate.

#### Inshellisense Setup

Inshellisense (Microsoft) provides IDE-style autocomplete in the terminal using Fig-format specs.

```bash
envforge completions inshellisense --install
```

Installs the Fig JS spec to `~/.fig/autocomplete/build/envforge.js` — inshellisense auto-detects specs from this directory. Run `is` to start a new session. No additional configuration needed.

---

### envforge man

Show built-in manual page for a command.

```
Usage: envforge man [OPTIONS] <COMMAND>...
```

| Argument | Description |
|----------|-------------|
| `<COMMAND>...` | Command name (e.g., "list", "sync push", "secrets pull") |

**Examples:**

```bash
envforge man list
envforge man sync push
envforge man secrets pull
envforge man rotate
```

---

### envforge lsp

Start the Language Server Protocol server for IDE integration.

```
Usage: envforge lsp
```

The LSP server communicates over stdio (stdin/stdout) and is launched automatically by IDE extensions.

**Capabilities:**

| Feature | Description |
|---------|-------------|
| Diagnostics | Missing required vars, type validation, secret leak warnings |
| Hover | Type, description, default, example from `.env.schema` |
| Completions | All envforge-managed vars, schema keys, value suggestions |
| Go-to-definition | `.env` key → `.env.schema` section |

**Supported files:** `.env`, `.env.*`, `*.env`, `.env.schema`

**IDE Extensions:**

- **VS Code** — Install from [Marketplace](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager) or `ext install emreerinc.envforge-env-manager`
- **IntelliJ IDEA** — Install from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/31385-envforge)
- **Neovim** — Configure via `nvim-lspconfig` (see `editors/README.md`)
- **Helix** — Add to `languages.toml` (see `editors/README.md`)
- **Sublime Text** — Configure via LSP package (see `editors/README.md`)

**Execution Modes in IDE Extensions:**
- **Standalone Mode (Zero External Dependencies):** VS Code and IntelliJ extensions work out-of-the-box without downloading an external CLI binary for native syntax highlighting, file decorators, and sidebar UI.
- **Full AI Security & LSP Validation Mode:** Auto-downloading via the welcome page or installing the CLI binary (`brew install emreerinc/tap/envforge` or `cargo install env-forge-tui`) unlocks real-time LSP diagnostics, AI Fence & Guard protection, and multi-profile switching.

---

## Provider Integration Guides

### HashiCorp Vault Integration

> Provider: `vault` | Binary: `vault` | [Official Docs](https://developer.hashicorp.com/vault)

#### Prerequisites

1. Install the Vault CLI:
   ```bash
   # macOS
   brew install hashicorp/tap/vault

   # Linux (Ubuntu/Debian)
   sudo apt-get install vault

   # Or download from: https://developer.hashicorp.com/vault/install
   ```

2. Verify installation:
   ```bash
   vault --version
   ```

3. Ensure you have access to a Vault server (self-hosted or HCP Vault).

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `addr` | Vault server URL | `https://vault.example.com:8200` |

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `token` | Vault token (for token auth) | `hvs.CAESIJlW...` |
| `role_id` | AppRole role ID (for AppRole auth) | `db02de05-fa39-...` |
| `secret_id` | AppRole secret ID (for AppRole auth) | `6a174c20-f6de-...` |
| `auth_method` | Auth method: `token` (default) or `approle` | `approle` |

**Setup Commands**

**Token authentication (default):**
```bash
envforge secrets config vault --set addr=https://vault.example.com:8200
envforge secrets config vault --set token=hvs.your-token-here --ttl 8h
```

**AppRole authentication (CI/CD):**
```bash
envforge secrets config vault --set addr=https://vault.example.com:8200
envforge secrets config vault --set auth_method=approle
envforge secrets config vault --set role_id=db02de05-fa39-...
envforge secrets config vault --set secret_id=6a174c20-f6de-...
```

**Verify:**
```bash
envforge secrets config vault --show
```

#### Path Format

The `--path` flag specifies the **KV secret engine mount path and secret name**:

```
secret/data/myapp        ← KV v2 (default in modern Vault)
secret/myapp             ← KV v1
apps/production/api      ← Custom mount paths
```

EnvForge tries KV v2 path (`data.data`) first, falls back to KV v1 (`data`). You don't need to add `/data/` in the path — EnvForge handles both formats.

**Important**: The path points to a single secret object containing multiple key-value pairs, not to individual keys.

#### End-to-End Workflow

**Pull secrets from Vault**
```bash
envforge secrets pull --from vault --path secret/myapp
envforge secrets pull --from vault --path secret/myapp --filter "DB_*"
```

**Push secrets to Vault**
```bash
envforge secrets push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY
envforge secrets push --to vault --path secret/myapp --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from vault --path secret/myapp/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
envforge run --resolve --dry-run -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from vault --path secret/myapp
```

#### Authentication Details

**Token Auth**
Simplest method. Get a token from your Vault admin or generate one:
```bash
vault login -method=userpass username=myuser
vault token create -ttl=8h
```
The token is passed as `VAULT_TOKEN` environment variable to the Vault CLI.

**AppRole Auth (recommended for CI/CD)**
Non-interactive authentication using role_id + secret_id. EnvForge runs `vault write auth/approle/login` to obtain a session token automatically.

```bash
# On your Vault server, create an AppRole:
vault auth enable approle
vault write auth/approle/role/myapp token_ttl=1h token_max_ttl=4h
vault read auth/approle/role/myapp/role-id
vault write -f auth/approle/role/myapp/secret-id
```

#### Common Pitfalls

1. **KV v1 vs v2 path confusion** — In KV v2, Vault API uses `/secret/data/myapp` internally, but EnvForge handles this automatically. Just use `secret/myapp`.

2. **Token expiry** — Vault tokens have TTLs. Use `--ttl 8h` when configuring to get EnvForge reminders before expiry. Refresh with `envforge secrets config vault --set token=<new-token>`.

3. **Permission denied on list** — The `list` capability is separate from `read` in Vault policies. Ensure your token has both:
   ```hcl
   path "secret/data/myapp" {
     capabilities = ["read", "list"]
   }
   ```

4. **Empty path** — Vault requires a path. `envforge secrets pull --from vault` without `--path` will fail.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `connection refused` | Wrong addr or Vault is down | Verify `addr` with `vault status` |
| `permission denied` | Token lacks required capabilities | Check Vault policy with `vault token lookup` |
| `no value found at path` | Wrong path or KV version mismatch | Try with/without `data/` in path |
| `token expired` | TTL exceeded | Re-authenticate: `envforge secrets config vault --set token=<new>` |

---

### AWS SSM Parameter Store Integration

> Provider: `aws-ssm` | Binary: `aws` | [Official Docs](https://docs.aws.amazon.com/systems-manager/latest/userguide/systems-manager-parameter-store.html)

#### Prerequisites

1. Install the AWS CLI v2:
   ```bash
   # macOS
   brew install awscli

   # Linux
   curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
   unzip awscliv2.zip && sudo ./aws/install

   # Or: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html
   ```

2. Verify installation:
   ```bash
   aws --version
   ```

3. Ensure you have IAM permissions for `ssm:GetParameter`, `ssm:GetParametersByPath`, and `ssm:PutParameter`.

#### Configure EnvForge

**Required Fields**

No strictly required fields — AWS CLI handles authentication via its own chain (env vars, profile, IAM role, SSO).

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `access_key` | AWS access key ID | `AKIAIOSFODNN7EXAMPLE` |
| `secret_key` | AWS secret access key | `wJalrXUtnFEMI/K7MDENG/...` |
| `region` | AWS region | `us-east-1` |
| `profile` | Named AWS CLI profile | `production` |

**Setup Commands**

**Explicit credentials:**
```bash
envforge secrets config aws-ssm --set access_key=AKIAIOSFODNN7EXAMPLE
envforge secrets config aws-ssm --set secret_key=wJalrXUtnFEMI/K7MDENG/... --ttl 24h
envforge secrets config aws-ssm --set region=us-east-1
```

**Profile-based (recommended):**
```bash
envforge secrets config aws-ssm --set profile=production
envforge secrets config aws-ssm --set region=eu-west-1
```

**IAM role (EC2/ECS/Lambda):**
No EnvForge config needed — the AWS SDK auto-detects instance credentials. Just set region if not using the default:
```bash
envforge secrets config aws-ssm --set region=us-west-2
```

**Verify:**
```bash
envforge secrets config aws-ssm --show
```

#### Path Format

The `--path` flag specifies a **parameter path prefix**. SSM parameters are organized hierarchically with `/`:

```
/myapp/              ← All parameters under /myapp/
/myapp/prod/         ← Production parameters
/shared/database/    ← Shared database config
```

EnvForge uses `get-parameters-by-path --recursive` to fetch all parameters under the prefix. Parameter names are **stripped of the prefix** when imported:

```
SSM: /myapp/prod/DATABASE_URL  →  EnvForge key: DATABASE_URL
SSM: /myapp/prod/API_KEY       →  EnvForge key: API_KEY
```

When pushing, keys are appended to the path: `push --to aws-ssm --path /myapp/prod/ --keys DATABASE_URL` creates `/myapp/prod/DATABASE_URL`.

#### End-to-End Workflow

**Pull secrets from SSM**
```bash
envforge secrets pull --from aws-ssm --path /myapp/prod/
envforge secrets pull --from aws-ssm --path /myapp/ --filter "DB_*"
```

**Push secrets to SSM**
```bash
envforge secrets push --to aws-ssm --path /myapp/prod/ --keys DATABASE_URL,API_KEY
envforge secrets push --to aws-ssm --path /myapp/prod/ --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from aws-ssm --path /myapp/prod/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from aws-ssm --path /myapp/prod/
```

#### Authentication Details

AWS CLI uses a credential resolution chain (in order):

1. **Environment variables**: `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` (set by EnvForge when `access_key`/`secret_key` are configured)
2. **Named profile**: `AWS_PROFILE` (set when `profile` is configured)
3. **Instance metadata**: IAM role attached to EC2/ECS/Lambda
4. **SSO**: `aws sso login --profile myprofile`

EnvForge maps its credential fields to AWS environment variables:
- `access_key` → `AWS_ACCESS_KEY_ID`
- `secret_key` → `AWS_SECRET_ACCESS_KEY`
- `profile` → `AWS_PROFILE`
- `region` → `AWS_DEFAULT_REGION`

#### Common Pitfalls

1. **Missing region** — SSM is regional. If you get `Could not connect to the endpoint URL`, set the region:
   ```bash
   envforge secrets config aws-ssm --set region=us-east-1
   ```

2. **Path must end with `/`** — When pulling, the path prefix should end with `/` to avoid matching unrelated parameters (e.g., `/myapp` would also match `/myapp2/...`).

3. **SecureString by default** — All pushed parameters are stored as `SecureString`. Ensure your IAM user/role has `kms:Decrypt` permission.

4. **Pagination** — EnvForge handles SSM pagination automatically. Large parameter sets (100+) work correctly.

5. **Trailing slash in key names** — Push constructs parameter names as `{path}/{key}`. If your path already has a trailing slash, you won't get double slashes — EnvForge trims it.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Could not connect to endpoint` | Wrong or missing region | Set region: `--set region=us-east-1` |
| `AccessDeniedException` | IAM policy missing SSM permissions | Add `ssm:GetParameter*` and `ssm:PutParameter` |
| `ParameterNotFound` | Wrong path prefix or parameter doesn't exist | Verify with `aws ssm get-parameters-by-path --path /your/path/` |
| `KMS access denied` | Missing KMS decrypt permission for SecureString | Add `kms:Decrypt` to IAM policy |
| Empty pull result | No parameters under given path | Check path exists: `aws ssm describe-parameters` |

---

### 1Password Integration

> Provider: `1password` | Binary: `op` | [Official Docs](https://developer.1password.com/docs/cli/)

#### Prerequisites

1. Install the 1Password CLI:
   ```bash
   # macOS
   brew install --cask 1password-cli

   # Linux
   # See: https://developer.1password.com/docs/cli/get-started/
   ```

2. Verify installation:
   ```bash
   op --version
   ```

3. Create a **Service Account** in your 1Password account with access to the desired vault. Service accounts provide non-interactive authentication for automation.

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `service_account_token` | 1Password Service Account token | `ops_eyJzaWduSW5...` |

**Setup Commands**

```bash
envforge secrets config 1password --set service_account_token=ops_eyJzaWduSW5... --ttl 30d
```

**Verify:**
```bash
envforge secrets config 1password --show
```

#### Path Format

The `--path` flag specifies either an **item name** or **item UUID**:

```
"My API Keys"         ← Item name (quote if spaces)
abcd1234-...          ← Item UUID
```

For `list` operations, path is used as the **vault name**:
```bash
envforge secrets pull --from 1password --path "My API Keys"    # Gets fields from one item
```

Item fields are returned as key-value pairs where the **label** is the key and **field value** is the value. Empty labels and empty values are filtered out.

#### End-to-End Workflow

**Pull secrets from 1Password**
```bash
envforge secrets pull --from 1password --path "Backend Secrets"
envforge secrets pull --from 1password --path "Backend Secrets" --filter "DB_*"
```

**Push secrets to 1Password**
```bash
envforge secrets push --to 1password --path "Backend Secrets" --keys DATABASE_URL,API_KEY
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from 1password --path "Backend Secrets"
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from 1password --path "Backend Secrets"
```

#### Authentication Details

1Password uses **Service Account tokens** for automation. The token is passed as `OP_SERVICE_ACCOUNT_TOKEN` environment variable.

**Creating a service account:**
1. Sign in to your 1Password account at https://my.1password.com
2. Go to **Developer Tools** > **Infrastructure Secrets** > **Service Accounts**
3. Create a new service account
4. Grant access to the vaults containing your secrets
5. Copy the token (starts with `ops_`)

Service account tokens do not expire by default, but you can set a TTL in EnvForge to track rotation schedules.

#### Common Pitfalls

1. **Item name vs vault name** — `pull` uses path as an item name/UUID. `list` uses it as a vault name. This is a 1Password CLI behavior.

2. **Empty field values** — Fields with empty values are automatically filtered from pull results.

3. **Special characters in item names** — Quote item names with spaces: `--path "My Secrets"`.

4. **Service account vault access** — Service accounts only see vaults explicitly granted to them. If pull returns empty, verify vault permissions.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `isn't a vault` | Path used as vault but it's an item name | For pull, use item name; for list, use vault name |
| `authentication required` | Invalid or expired service account token | Regenerate token in 1Password dashboard |
| `item not found` | Wrong item name or UUID | Verify with `op item list --vault "VaultName"` |
| Empty results | Fields have no labels or values | Check item structure in 1Password app |

---

### Doppler Integration

> Provider: `doppler` | Binary: `doppler` | [Official Docs](https://docs.doppler.com)

#### Prerequisites

1. Install the Doppler CLI:
   ```bash
   # macOS
   brew install dopplerhq/cli/doppler

   # Linux
   curl -Ls https://cli.doppler.com/install.sh | sh

   # Or: https://docs.doppler.com/docs/install-cli
   ```

2. Verify installation:
   ```bash
   doppler --version
   ```

3. Create a **Service Token** in your Doppler project for the target config (e.g., `dev`, `staging`, `production`).

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `token` | Doppler service token | `dp.st.dev.xxxx...` |
| `project` | Doppler project name | `backend-api` |
| `config` | Doppler config name | `production` |

**Setup Commands**

```bash
envforge secrets config doppler --set token=dp.st.prod.xxxx...
envforge secrets config doppler --set project=backend-api
envforge secrets config doppler --set config=production
```

**Verify:**
```bash
envforge secrets config doppler --show
```

#### Path Format

**The `--path` flag is ignored.** Doppler uses the `project` and `config` from credentials to determine which secrets to access. You can pass any value (or omit it) — it has no effect.

```bash
# These are equivalent:
envforge secrets pull --from doppler
envforge secrets pull --from doppler --path anything
```

#### End-to-End Workflow

**Pull secrets from Doppler**
```bash
envforge secrets pull --from doppler
envforge secrets pull --from doppler --filter "DB_*"
```

**Push secrets to Doppler**
```bash
envforge secrets push --to doppler --keys DATABASE_URL,API_KEY
envforge secrets push --to doppler --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from doppler --path doppler/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from doppler
```

#### Authentication Details

Doppler uses **service tokens** scoped to a project + config combination. The token is passed as `DOPPLER_TOKEN` environment variable.

**Creating a service token:**
1. Go to your project in the Doppler dashboard
2. Select the config (e.g., `production`)
3. Navigate to **Access** > **Service Tokens**
4. Generate a new token
5. Copy the token (starts with `dp.st.`)

Each token is scoped to exactly one project + config. For multiple environments, create separate tokens and switch with:
```bash
envforge secrets config doppler --set config=staging
envforge secrets config doppler --set token=dp.st.stg.yyyy...
```

#### Common Pitfalls

1. **System keys filtered** — Doppler injects `DOPPLER_PROJECT`, `DOPPLER_CONFIG`, and `DOPPLER_ENVIRONMENT` into secret downloads. EnvForge automatically filters these out.

2. **Path is ignored** — Don't rely on `--path` for scoping. Switch project/config via `envforge secrets config doppler --set config=<name>`.

3. **Token scope** — A service token can only access the project+config it was created for. You cannot use a `dev` token to read `production` secrets.

4. **Batch push** — Doppler supports setting multiple secrets in a single API call. EnvForge uses `doppler secrets set KEY1=val1 KEY2=val2` for efficient batch pushes.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `401 Unauthorized` | Invalid or expired service token | Regenerate token in Doppler dashboard |
| `project not found` | Wrong project name in config | Verify with `doppler projects` |
| `config not found` | Wrong config name | Verify with `doppler configs --project <name>` |
| System keys in output | Using Doppler CLI directly | EnvForge filters these; this is normal via raw CLI |

---

### Infisical Integration

> Provider: `infisical` | Binary: `infisical` | [Official Docs](https://infisical.com/docs)

#### Prerequisites

1. Install the Infisical CLI:
   ```bash
   # macOS
   brew install infisical/get-cli/infisical

   # Linux
   curl -1sLf 'https://dl.cloudsmith.io/public/infisical/infisical-cli/setup.deb.sh' | sudo -E bash
   sudo apt-get install infisical

   # Or: https://infisical.com/docs/cli/overview
   ```

2. Verify installation:
   ```bash
   infisical --version
   ```

3. Create a **Service Token** or **Machine Identity** in your Infisical project.

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `token` | Infisical service token | `st.xxxx.yyyy.zzzz` |
| `project_id` | Infisical project ID | `64a1b2c3d4e5f6...` |
| `environment` | Environment slug | `production` |

**Setup Commands**

```bash
envforge secrets config infisical --set token=st.xxxx.yyyy.zzzz
envforge secrets config infisical --set project_id=64a1b2c3d4e5f6...
envforge secrets config infisical --set environment=production
```

**Verify:**
```bash
envforge secrets config infisical --show
```

#### Path Format

**The `--path` flag is ignored.** Infisical uses `project_id` and `environment` from credentials to scope secrets.

```bash
# These are equivalent:
envforge secrets pull --from infisical
envforge secrets pull --from infisical --path anything
```

To switch environments, update the credential:
```bash
envforge secrets config infisical --set environment=staging
```

#### End-to-End Workflow

**Pull secrets from Infisical**
```bash
envforge secrets pull --from infisical
envforge secrets pull --from infisical --filter "DB_*"
```

**Push secrets to Infisical**
```bash
envforge secrets push --to infisical --keys DATABASE_URL,API_KEY
envforge secrets push --to infisical --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from infisical --path infisical/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from infisical
```

#### Authentication Details

Infisical supports two automation auth methods:

1. **Service Tokens** (legacy): Token scoped to project + environment. Passed as `INFISICAL_TOKEN` env var.
2. **Machine Identities** (recommended): More flexible, supports multiple auth methods (Universal Auth, Kubernetes, AWS, GCP, Azure).

EnvForge uses the service token approach via `INFISICAL_TOKEN`.

#### Common Pitfalls

1. **Path is ignored** — Infisical uses project_id + environment from credentials. `--path` has no effect.

2. **Batch push** — Infisical supports setting multiple secrets in one `infisical secrets set KEY1=val1 KEY2=val2` call. EnvForge uses this for efficient pushes.

3. **Environment slug** — Use the slug (e.g., `dev`, `staging`, `production`), not the display name.

4. **Token scope** — Service tokens are scoped to a specific environment within a project. For multiple environments, create separate tokens.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `401 Unauthorized` | Invalid or expired token | Regenerate token in Infisical dashboard |
| `project not found` | Wrong project_id | Copy project ID from project settings page |
| `environment not found` | Wrong environment slug | Use slug format: `dev`, `staging`, `production` |
| Empty results | Token not scoped to environment | Verify token permissions in dashboard |

---

### GCP Secret Manager Integration

> Provider: `gcp` | Binary: `gcloud` | [Official Docs](https://cloud.google.com/secret-manager/docs)

#### Prerequisites

1. Install the Google Cloud SDK:
   ```bash
   # macOS
   brew install google-cloud-sdk

   # Linux
   curl https://sdk.cloud.google.com | bash

   # Or: https://cloud.google.com/sdk/docs/install
   ```

2. Verify installation:
   ```bash
   gcloud --version
   ```

3. Authenticate and enable the Secret Manager API:
   ```bash
   gcloud auth login
   gcloud services enable secretmanager.googleapis.com --project=my-project
   ```

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `project_id` | GCP project ID | `my-project-123` |

**Setup Commands**

```bash
envforge secrets config gcp --set project_id=my-project-123
```

**Verify:**
```bash
envforge secrets config gcp --show
```

#### Path Format

**The `--path` flag is ignored.** GCP Secret Manager lists all secrets in the project. Secrets are identified by name, not by path hierarchy.

```bash
# These are equivalent:
envforge secrets pull --from gcp
envforge secrets pull --from gcp --path anything
```

Use `--filter` to narrow results:
```bash
envforge secrets pull --from gcp --filter "DB_*"
```

#### End-to-End Workflow

**Pull secrets from GCP**
```bash
envforge secrets pull --from gcp
envforge secrets pull --from gcp --filter "PROD_*"
```

**Push secrets to GCP**
```bash
envforge secrets push --to gcp --keys DATABASE_URL,API_KEY
envforge secrets push --to gcp --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from gcp --path gcp/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from gcp
```

#### Authentication Details

GCP uses the standard `gcloud` authentication chain:

1. **User credentials**: `gcloud auth login` (interactive, for development)
2. **Service account**: `gcloud auth activate-service-account --key-file=sa-key.json` (CI/CD)
3. **Application Default Credentials**: `GOOGLE_APPLICATION_CREDENTIALS` env var pointing to a service account key file
4. **Metadata server**: Automatic on GCE/GKE/Cloud Run

**Required IAM roles:**
- `roles/secretmanager.secretAccessor` (for pull/get)
- `roles/secretmanager.admin` (for push/create)

#### Common Pitfalls

1. **Path ignored** — GCP Secret Manager is flat (not hierarchical). Use `--filter` to scope results by naming convention.

2. **Secret creation on push** — EnvForge automatically creates secrets that don't exist when pushing. It tries `gcloud secrets create` first, then adds a version. Existing secrets just get a new version.

3. **Latest version only** — Pull always fetches the `latest` version of each secret. Historical versions are not accessible through EnvForge.

4. **Secret names from resource paths** — GCP returns full resource paths like `projects/123/secrets/MY_SECRET`. EnvForge extracts just the secret name (`MY_SECRET`).

5. **Replication policy** — New secrets are created with `--replication-policy=automatic`. Change this in GCP Console if you need regional replication.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `PERMISSION_DENIED` | Missing IAM role | Grant `secretmanager.secretAccessor` role |
| `API not enabled` | Secret Manager API disabled | Run `gcloud services enable secretmanager.googleapis.com` |
| `project not found` | Wrong project_id | Verify with `gcloud projects list` |
| Empty results | No secrets in project or wrong project | Check with `gcloud secrets list --project=<id>` |
| `already exists` on push | Secret exists (normal) | EnvForge handles this — it adds a new version |

---

### Azure Key Vault Integration

> Provider: `azure` | Binary: `az` | [Official Docs](https://learn.microsoft.com/en-us/azure/key-vault/)

#### Prerequisites

1. Install the Azure CLI:
   ```bash
   # macOS
   brew install azure-cli

   # Linux
   curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash

   # Or: https://learn.microsoft.com/en-us/cli/azure/install-azure-cli
   ```

2. Verify installation:
   ```bash
   az --version
   ```

3. Authenticate:
   ```bash
   az login
   ```

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `vault_name` | Azure Key Vault name | `myapp-secrets` |

**Setup Commands**

```bash
envforge secrets config azure --set vault_name=myapp-secrets
```

**Verify:**
```bash
envforge secrets config azure --show
```

#### Path Format

**The `--path` flag is ignored.** Azure Key Vault lists all secrets in the vault. The vault is specified in the `vault_name` credential field.

```bash
# These are equivalent:
envforge secrets pull --from azure
envforge secrets pull --from azure --path anything
```

#### End-to-End Workflow

**Pull secrets from Azure Key Vault**
```bash
envforge secrets pull --from azure
envforge secrets pull --from azure --filter "DB_*"
```

**Push secrets to Azure Key Vault**
```bash
envforge secrets push --to azure --keys DATABASE_URL,API_KEY
envforge secrets push --to azure --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from azure --path azure/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from azure
```

#### Authentication Details

Azure CLI uses its own authentication chain:

1. **Interactive login**: `az login` (browser-based)
2. **Service principal**: `az login --service-principal -u <app-id> -p <secret> --tenant <tenant-id>` (CI/CD)
3. **Managed identity**: Automatic on Azure VMs, App Service, Functions
4. **Device code**: `az login --use-device-code` (headless environments)

**Required role:**
- `Key Vault Secrets User` (for read/list)
- `Key Vault Secrets Officer` (for read/list/write)

#### Common Pitfalls

1. **Underscore-to-hyphen conversion** — Azure Key Vault only allows `[a-zA-Z0-9-]` in secret names. EnvForge automatically converts:
   - **Push**: `DATABASE_URL` → stored as `DATABASE-URL`
   - **Pull**: `DATABASE-URL` → returned as `DATABASE_URL`

   This is transparent but important to know if you access the same secrets from other tools.

2. **Path is ignored** — All secrets in the vault are accessible. Use `--filter` to narrow results.

3. **Soft-delete** — Azure Key Vault has soft-delete enabled by default. Deleted secrets are retained for the purge protection period.

4. **Secret names are case-insensitive** — `API-KEY` and `api-key` refer to the same secret in Azure.

5. **ID-based listing** — Azure returns secrets as URLs (`https://vault.vault.azure.net/secrets/name`). EnvForge extracts the secret name from the URL path.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `ForbiddenByPolicy` | Missing Key Vault RBAC role | Assign `Key Vault Secrets User` role |
| `VaultNotFound` | Wrong vault name | Verify with `az keyvault list` |
| `SecretNotFound` | Secret name doesn't match (hyphen vs underscore) | Check actual name in portal |
| `Unauthorized` | Azure CLI not logged in | Run `az login` |
| Keys have hyphens | Normal — Azure converts underscores | EnvForge converts back automatically |

---

### Bitwarden Secrets Manager Integration

> Provider: `bitwarden` | Binary: `bws` | [Official Docs](https://bitwarden.com/help/secrets-manager-overview/)

#### Prerequisites

1. Install the Bitwarden Secrets Manager CLI:
   ```bash
   # npm
   npm install -g @bitwarden/cli

   # Or download from: https://bitwarden.com/help/secrets-manager-cli/
   ```

2. Verify installation:
   ```bash
   bws --version
   ```

3. Create a **Machine Account** in your Bitwarden organization and generate an access token.

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `access_token` | Machine account access token | `0.xxxxxxxx-xxxx-...` |

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `project_id` | Bitwarden project UUID (required for push) | `abcdef12-3456-...` |

**Setup Commands**

```bash
envforge secrets config bitwarden --set access_token=0.xxxxxxxx-xxxx-...
envforge secrets config bitwarden --set project_id=abcdef12-3456-...
```

**Verify:**
```bash
envforge secrets config bitwarden --show
```

#### Path Format

**The `--path` flag is ignored.** Bitwarden Secrets Manager lists secrets by machine account access, optionally filtered by project_id.

```bash
# These are equivalent:
envforge secrets pull --from bitwarden
envforge secrets pull --from bitwarden --path anything
```

#### End-to-End Workflow

**Pull secrets from Bitwarden**
```bash
envforge secrets pull --from bitwarden
envforge secrets pull --from bitwarden --filter "DB_*"
```

**Push secrets to Bitwarden**
```bash
envforge secrets push --to bitwarden --keys DATABASE_URL,API_KEY
envforge secrets push --to bitwarden --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from bitwarden --path bitwarden/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from bitwarden
```

#### Authentication Details

Bitwarden uses **machine account access tokens** for automation. The token is passed as `BWS_ACCESS_TOKEN` environment variable.

#### Common Pitfalls

1. **project_id required for push** — Pull works without project_id (lists all accessible secrets), but push requires it to know where to create new secrets.

2. **Update vs create** — EnvForge checks existing secrets by key name. Existing secrets are updated via `bws secret edit`, new ones are created via `bws secret create`.

3. **Secret key field** — Bitwarden secrets have a `key` (name) and `value`. EnvForge maps these directly.

4. **Access scope** — Machine accounts only see projects explicitly granted to them.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `unauthorized` | Invalid access token | Regenerate token in Bitwarden |
| `project_id required` on push | Missing project_id credential | `--set project_id=<uuid>` |
| Empty results | Machine account has no project access | Grant project access in dashboard |
| `not found` on edit | Secret ID changed | EnvForge re-lists before edit; may be a race condition |

---

### Akeyless Vault Integration

> Provider: `akeyless` | Binary: `akeyless` | [Official Docs](https://docs.akeyless.io)

#### Prerequisites

1. Install the Akeyless CLI:
   ```bash
   # macOS
   brew install akeyless

   # Linux
   curl -o akeyless https://akeyless-cli.s3.us-east-2.amazonaws.com/cli/latest/production/cli-linux-amd64
   chmod +x akeyless && sudo mv akeyless /usr/local/bin/

   # Or: https://docs.akeyless.io/docs/cli
   ```

2. Verify installation:
   ```bash
   akeyless --version
   ```

3. Obtain an **Access ID** and **Access Key** from your Akeyless console.

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `access_id` | Akeyless access ID | `p-abcdef123456` |
| `access_key` | Akeyless access key | `aBcDeFgHiJkLmN...` |

**Setup Commands**

```bash
envforge secrets config akeyless --set access_id=p-abcdef123456
envforge secrets config akeyless --set access_key=aBcDeFgHiJkLmN...
```

**Verify:**
```bash
envforge secrets config akeyless --show
```

#### Path Format

The `--path` flag specifies a **folder path** in Akeyless. Secrets are organized hierarchically:

```
/                     ← Root (all secrets)
/myapp/               ← Application secrets
/myapp/production/    ← Environment-specific
```

EnvForge uses a **two-step process**: first lists items at the path, then fetches each secret value individually. Only `STATIC_SECRET` type items are returned.

Key names are extracted from the **last segment** of the full path:
```
/myapp/production/DATABASE_URL  →  EnvForge key: DATABASE_URL
```

#### End-to-End Workflow

**Pull secrets from Akeyless**
```bash
envforge secrets pull --from akeyless --path /myapp/production/
envforge secrets pull --from akeyless --path /
envforge secrets pull --from akeyless --path /myapp/ --filter "DB_*"
```

**Push secrets to Akeyless**
```bash
envforge secrets push --to akeyless --path /myapp/production/ --keys DATABASE_URL,API_KEY
envforge secrets push --to akeyless --path /myapp/production/ --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from akeyless --path /myapp/production/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from akeyless --path /myapp/production/
```

#### Authentication Details

Akeyless uses **access key authentication** passed as CLI flags (`--access-id` and `--access-key`). EnvForge doesn't set environment variables — credentials are passed directly to each CLI call.

Other Akeyless auth methods (SAML, OIDC, AWS IAM) are not directly supported through EnvForge's credential config but can work if you authenticate separately with `akeyless auth`.

#### Common Pitfalls

1. **Two-step pull** — Pull requires list + get for each item. This is slower than single-call providers but handles Akeyless's architecture correctly.

2. **STATIC_SECRET only** — EnvForge only returns items of type `STATIC_SECRET`. Dynamic secrets, rotated secrets, and SSH certificates are filtered out.

3. **Push creates or updates** — EnvForge tries `update-secret-val` first. If the secret doesn't exist, it falls back to `create-secret`.

4. **Empty path defaults to `/`** — If you omit `--path`, EnvForge uses `/` (root), which lists all secrets.

5. **Full path for references** — When creating refs, use the full Akeyless path including the key name.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `unauthorized` | Wrong access_id or access_key | Verify credentials in Akeyless console |
| Empty results | No STATIC_SECRET items at path | Check with `akeyless list-items --path /your/path/` |
| Slow pull | Many items (each requires individual GET) | Use `--filter` to narrow scope |
| `item not found` on push | Path doesn't exist for update | Normal — EnvForge will create it |

---

### CyberArk Conjur Integration

> Provider: `conjur` | Binary: `conjur` | [Official Docs](https://docs.conjur.org)

#### Prerequisites

1. Install the Conjur CLI (Go version):
   ```bash
   # Download from GitHub releases
   # https://github.com/cyberark/conjur-cli-go/releases

   # macOS (example)
   curl -L -o conjur https://github.com/cyberark/conjur-cli-go/releases/latest/download/conjur-darwin-amd64
   chmod +x conjur && sudo mv conjur /usr/local/bin/
   ```

2. Verify installation:
   ```bash
   conjur --version
   ```

3. Obtain your Conjur **account name**, **server URL**, **host login**, and **API key** from your Conjur administrator.

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `url` | Conjur server URL | `https://conjur.example.com` |
| `account` | Conjur account name | `myorg` |
| `login` | Host or user login identity | `host/myapp/production` |
| `api_key` | API key for the login identity | `3xk9d8f...` |

**Setup Commands**

```bash
envforge secrets config conjur --set url=https://conjur.example.com
envforge secrets config conjur --set account=myorg
envforge secrets config conjur --set login=host/myapp/production
envforge secrets config conjur --set api_key=3xk9d8f... --ttl 30d
```

**Verify:**
```bash
envforge secrets config conjur --show
```

#### Path Format

The `--path` flag specifies a **variable path prefix** for filtering. Conjur variables are organized hierarchically:

```
                          ← Empty: all variables
myapp/                    ← Variables starting with myapp/
myapp/production/         ← Environment-specific variables
```

Conjur stores variables as `account:variable:path/to/secret`. EnvForge strips the `account:variable:` prefix and extracts the **last segment** as the key name:
```
myorg:variable:myapp/production/DATABASE_URL  →  EnvForge key: DATABASE_URL
```

#### End-to-End Workflow

**Pull secrets from Conjur**
```bash
envforge secrets pull --from conjur
envforge secrets pull --from conjur --path myapp/production/
envforge secrets pull --from conjur --path myapp/ --filter "DB_*"
```

**Push secrets to Conjur**
```bash
envforge secrets push --to conjur --keys DATABASE_URL,API_KEY
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from conjur --path myapp/production/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from conjur --path myapp/production/
```

#### Authentication Details

Conjur uses a **two-step authentication** process:

1. **`conjur init`** — Initializes the CLI connection with server URL and account name
2. **`conjur login`** — Authenticates with the host identity and API key

EnvForge runs both steps automatically before each operation using the configured credentials. No environment variables are set — all credentials are passed as CLI flags.

**Identity types:**
- `host/myapp/production` — Machine identity (recommended for automation)
- `admin` — Administrative user (not recommended for production)

#### Common Pitfalls

1. **Variables must exist in policy** — Conjur requires variables to be declared in a policy before values can be set. Push fails if the variable doesn't exist. Create variables via Conjur policy:
   ```yaml
   - !variable myapp/production/DATABASE_URL
   - !variable myapp/production/API_KEY
   ```

2. **Init + login per operation** — EnvForge runs `conjur init` and `conjur login` before each operation. This ensures fresh authentication but adds latency.

3. **Account prefix stripping** — List returns `account:variable:path`. EnvForge strips the prefix automatically using the configured `account` value.

4. **No environment variables** — Unlike most providers, Conjur doesn't use env vars for auth. All credentials are passed via CLI flags.

5. **Key name = last path segment** — `myapp/db/HOST` becomes key `HOST`. If you have conflicts (e.g., `app1/HOST` and `app2/HOST`), use path prefix filtering.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `unable to init` | Wrong URL or unreachable server | Verify URL with `curl https://conjur.example.com/info` |
| `401 Unauthorized` | Wrong login or api_key | Verify credentials; rotate API key if needed |
| `variable not found` | Variable not declared in policy | Add variable to Conjur policy first |
| Empty list | No variables match path prefix | Check with `conjur list -k variable` directly |
| `account mismatch` | Account in credential doesn't match server | Verify account name with Conjur admin |

---

### Mozilla SOPS Integration

> Provider: `sops` | Binary: `sops` | [Official Docs](https://github.com/getsops/sops)

#### Prerequisites

1. Install SOPS:
   ```bash
   # macOS
   brew install sops

   # Linux
   # Download from: https://github.com/getsops/sops/releases
   ```

2. Verify installation:
   ```bash
   sops --version
   ```

3. Install an encryption backend. SOPS supports age, PGP, AWS KMS, GCP KMS, and Azure Key Vault. For local use, **age** is recommended:
   ```bash
   # macOS
   brew install age

   # Generate a key pair
   age-keygen -o ~/.config/sops/age/keys.txt
   ```

#### Configure EnvForge

**Required Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `key_file` | Path to age key file (or age public key for encryption) | `~/.config/sops/age/keys.txt` |

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `encryption_type` | Encryption backend: `age` (default) | `age` |

**Setup Commands**

```bash
envforge secrets config sops --set key_file=~/.config/sops/age/keys.txt
```

**Verify:**
```bash
envforge secrets config sops --show
```

#### Path Format

**The `--path` flag is required and must point to a SOPS-encrypted file.** Unlike other providers, SOPS operates on encrypted files, not remote APIs.

```bash
envforge secrets pull --from sops --path secrets.enc.json
envforge secrets pull --from sops --path config/prod.enc.yaml
```

Without `--path`, operations fail with: `path must be a SOPS-encrypted file path`.

#### End-to-End Workflow

**Pull (decrypt) secrets from a SOPS file**
```bash
envforge secrets pull --from sops --path secrets.enc.json
envforge secrets pull --from sops --path secrets.enc.json --filter "DB_*"
```

**Push (encrypt) secrets to a SOPS file**
```bash
envforge secrets push --to sops --path secrets.enc.json --keys DATABASE_URL,API_KEY
envforge secrets push --to sops --path secrets.enc.json --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from sops --path secrets.enc.json
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from sops --path secrets.enc.json
```

#### Authentication Details

SOPS delegates encryption/decryption to an underlying backend:

- **age**: Uses `SOPS_AGE_KEY_FILE` env var (set by EnvForge from `key_file` credential) for decryption. For encryption, the public key (recipient) is passed via `--age` flag.
- **PGP**: Uses GPG keyring
- **AWS KMS**: Uses AWS credentials (see AWS SSM provider for auth methods)
- **GCP KMS**: Uses `gcloud` auth
- **Azure Key Vault**: Uses `az` auth

EnvForge currently sets `SOPS_AGE_KEY_FILE` for age-based decryption.

#### Common Pitfalls

1. **File-based, not API-based** — SOPS is fundamentally different from other providers. It encrypts/decrypts files, not remote key-value stores. The file must exist locally.

2. **Path is required** — Unlike most providers, `--path` is mandatory and must point to an actual file.

3. **`sops` metadata key filtered** — Decrypted SOPS JSON contains a `sops` metadata key. EnvForge filters this out automatically.

4. **Push merges, doesn't replace** — Push decrypts the existing file, merges new key-value pairs, writes to a temp file, encrypts, then copies back to the original path. Existing keys not in the push set are preserved.

5. **Encryption requires recipient** — For push (encryption), the `key_file` credential is used as the `--age` recipient flag. Ensure it contains or points to the public key.

6. **JSON output only** — Pull always uses `--output-type json` for parsing. Source files can be YAML, JSON, or other SOPS-supported formats.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `path must be a SOPS-encrypted file` | Missing `--path` flag | Specify the encrypted file path |
| `could not decrypt` | Wrong key file or missing private key | Verify `key_file` points to the private key |
| `sops` key in results | EnvForge filtering failed | Report as bug — should be filtered automatically |
| `file not found` | Encrypted file doesn't exist at path | Create with `sops encrypt --age <pubkey> input.json > secrets.enc.json` |
| `no data key found` | File encrypted with different key | Re-encrypt with your key or get the matching private key |

---

### pass/gopass Integration

> Provider: `pass` | Binary: `pass` or `gopass` | [pass](https://www.passwordstore.org/) | [gopass](https://github.com/gopasspw/gopass)

#### Prerequisites

**Option A: pass (standard)**
```bash
# macOS
brew install pass

# Linux (Ubuntu/Debian)
sudo apt-get install pass

# Initialize with your GPG key
pass init <GPG_KEY_ID>
```

**Option B: gopass (recommended)**
```bash
# macOS
brew install gopass

# Linux
# See: https://github.com/gopasspw/gopass#installation

# Initialize
gopass init
```

Verify installation:
```bash
pass --version   # or
gopass --version
```

#### Configure EnvForge

**Required Fields**

No required fields — pass/gopass uses the default password store (`~/.password-store`) and auto-detects the binary.

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `store_path` | Custom password store directory | `/home/user/.team-store` |
| `binary` | Force binary: `pass` or `gopass` | `gopass` |

**Setup Commands**

```bash
# Only needed for non-default configuration
envforge secrets config pass --set store_path=/path/to/store
envforge secrets config pass --set binary=gopass
```

**Verify:**
```bash
envforge secrets config pass --show
```

#### Path Format

The `--path` flag specifies an **entry prefix filter**. Entries are organized as files in the password store:

```
                          ← Empty: all entries
myapp/                    ← Entries starting with myapp/
myapp/production/         ← Production entries
```

Password store structure maps to file paths:
```
~/.password-store/
  myapp/
    production/
      DATABASE_URL.gpg    ← Entry: myapp/production/DATABASE_URL
      API_KEY.gpg          ← Entry: myapp/production/API_KEY
    staging/
      DATABASE_URL.gpg    ← Entry: myapp/staging/DATABASE_URL
```

EnvForge scans for `.gpg` files, strips the extension, and uses the relative path as the key name.

#### End-to-End Workflow

**Pull secrets from password store**
```bash
envforge secrets pull --from pass
envforge secrets pull --from pass --path myapp/production/
envforge secrets pull --from pass --path myapp/ --filter "*DATABASE*"
```

**Push secrets to password store**
```bash
envforge secrets push --to pass --keys DATABASE_URL,API_KEY
envforge secrets push --to pass --all
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from pass --path myapp/production/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from pass --path myapp/production/
```

#### Authentication Details

pass/gopass uses **GPG encryption**. No tokens or API keys needed — your GPG private key is the authentication.

- **Decryption (pull)**: Requires the GPG private key in your keyring
- **Encryption (push)**: Requires the GPG public key(s) configured in the store

EnvForge sets `PASSWORD_STORE_DIR` if `store_path` is configured.

**Binary auto-detection:**
1. If `binary` is set in credentials → use that
2. If `gopass` is found on PATH → use gopass
3. Otherwise → use pass

**gopass vs pass differences:**
- gopass uses `-o` flag for `show` (output only, no clipboard)
- gopass uses `-f` for `insert` (force, no confirmation)
- pass uses `-e` for `insert` (echo mode)
- EnvForge handles these differences transparently

#### Common Pitfalls

1. **First line is the value** — Both pass and gopass store multiline entries. EnvForge only reads the **first line** as the secret value. Additional lines (metadata, URLs) are ignored.

2. **Key name = relative path** — Entry `myapp/production/DATABASE_URL` has key name `myapp/production/DATABASE_URL`, not just `DATABASE_URL`. Use path prefix filtering to scope.

3. **GPG agent required** — If your GPG key has a passphrase, `gpg-agent` must be running and unlocked. In CI/CD, use a passphrase-less key or configure `gpg-preset-passphrase`.

4. **Hidden files/dirs skipped** — Entries starting with `.` are filtered out (e.g., `.git`, `.gpg-id`).

5. **Push writes via stdin** — EnvForge pipes the value to `pass insert -e` or `gopass insert -f` via stdin. This avoids interactive prompts.

6. **Store must be initialized** — `pass init` or `gopass init` must be run before EnvForge can access the store.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `command not found` | pass/gopass not installed | Install via package manager |
| `gpg: decryption failed` | GPG key not available or locked | Check `gpg --list-secret-keys`; unlock agent |
| Empty results | Wrong store path or empty store | Verify `PASSWORD_STORE_DIR` or `--set store_path` |
| `Error: password store is empty` | Store not initialized | Run `pass init <GPG_KEY_ID>` |
| Key names include full path | Normal behavior | Use `--path` prefix to scope, or rename entries |

---

### Keeper Secrets Manager Integration

> Provider: `keeper` | Binary: `ksm` | [Official Docs](https://docs.keeper.io/en/keeperpam/secrets-manager/)

#### Prerequisites

1. Install the Keeper Secrets Manager CLI:
   ```bash
   pip3 install keeper-secrets-manager-cli
   ```

2. Verify installation:
   ```bash
   ksm --version
   ```

3. Initialize with a **one-time access token** from Keeper:
   ```bash
   ksm profile init --token <ONE_TIME_TOKEN>
   ```
   This creates a device configuration stored in the OS keyring.

#### Configure EnvForge

**Required Fields**

No required fields — Keeper uses its device configuration created during `ksm profile init`.

**Optional Fields**

| Field | Description | Example |
|-------|-------------|---------|
| `profile` | KSM profile name (if using multiple) | `production` |

**Setup Commands**

```bash
# Only needed if using non-default profile
envforge secrets config keeper --set profile=production
```

**Verify:**
```bash
envforge secrets config keeper --show
```

#### Path Format

**The `--path` flag is ignored.** Keeper lists all secrets accessible to the device configuration.

```bash
# These are equivalent:
envforge secrets pull --from keeper
envforge secrets pull --from keeper --path anything
```

#### End-to-End Workflow

**Pull secrets from Keeper**
```bash
envforge secrets pull --from keeper
envforge secrets pull --from keeper --filter "DB_*"
```

**Push (update) secrets in Keeper**
```bash
# Update existing secrets only — cannot create new records
envforge secrets push --to keeper --keys DATABASE_URL,API_KEY
```

**Create references**
```bash
envforge secrets ref DATABASE_URL --from keeper --path keeper/DATABASE_URL
```

**Resolve and run**
```bash
envforge run --resolve -- npm start
```

**Compare local vs remote**
```bash
envforge secrets diff --from keeper
```

#### Authentication Details

Keeper uses a **device-based authentication** model:

1. An admin generates a one-time access token in the Keeper Secrets Manager dashboard
2. `ksm profile init --token <TOKEN>` registers the device and stores credentials in the OS keyring
3. Subsequent operations use the stored device configuration automatically

**Multiple profiles:**
```bash
ksm profile init --profile production --token <TOKEN_1>
ksm profile init --profile staging --token <TOKEN_2>
envforge secrets config keeper --set profile=production
```

#### Common Pitfalls

1. **Push = update only** — Keeper push can only **update existing records**. It cannot create new secrets. If a key doesn't exist as a Keeper record title, push fails with an error.

2. **Record title = key name** — EnvForge uses the Keeper record **title** as the secret key name. Ensure your record titles match your ENV variable names.

3. **Password field extraction** — Pull extracts values from record fields in priority order: `password` > `login` > `secret` > `text`. Only the first non-empty field value is used.

4. **One-time token** — The initialization token can only be used once. If you need to re-initialize, generate a new token.

5. **Python 3.10+ required** — The KSM CLI requires Python 3.10 or later.

6. **List then get** — Pull first lists all records (getting UIDs and titles), then fetches each record individually. This is slower for large vaults.

#### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `not initialized` | Device not registered | Run `ksm profile init --token <TOKEN>` |
| `cannot create new secret` on push | Keeper push only updates | Create records in Keeper UI/app first |
| `no extractable value` | Record has no password/login/secret field | Add a password field to the Keeper record |
| `token already used` | One-time token consumed | Generate a new token in Keeper dashboard |
| `ModuleNotFoundError` | Python dependency missing | `pip3 install keeper-secrets-manager-cli` |

---

## Lifecycle & Governance

### envforge lifecycle check

Evaluate all enabled lifecycle rules against current state.

```
Usage: envforge lifecycle check [OPTIONS]
```

**Examples:**

```bash
envforge lifecycle check
envforge lifecycle check --json
```

---

### envforge lifecycle rule

Manage lifecycle rules (rotation, decommissioning, notifications).

```
Usage: envforge lifecycle rule [OPTIONS] <COMMAND>
```

**Commands:**

- `list`: List all configured lifecycle rules
- `show <ID>`: Show detailed rule configuration
- `enable <ID>`: Enable a specific rule
- `disable <ID>`: Disable a specific rule
- `create`: Interactive rule creation wizard

**Examples:**

```bash
envforge lifecycle rule list
envforge lifecycle rule disable 550e8400-e29b-41d4-a716-446655440000
```

---

### envforge lifecycle state

Manage lifecycle state for a specific secret.

```
Usage: envforge lifecycle state [OPTIONS] <KEY>
```

**Examples:**

```bash
# Show state history for a secret
envforge lifecycle state API_KEY

# Manually transition state
envforge lifecycle state DB_PASSWORD --set deprecated
```

---

## Analytics & Monitoring

### envforge monitor

Real-time monitoring of secret infrastructure and access events.

```
Usage: envforge monitor [OPTIONS] <COMMAND>
```

**Commands:**

- `status`: Run health checks on all configured secret providers
- `stream`: Stream real-time secret access events (audit stream)

**Examples:**

```bash
# Check provider health
envforge monitor status

# Watch secret access in real-time
envforge monitor stream
```

---

### envforge analytics

Secret usage analytics, unused detection, and retention policy management.

```
Usage: envforge analytics [OPTIONS] <COMMAND>
```

**Commands:**

- `unused`: Detect secrets with no access in N days (default 90)
- `low-usage`: Detect secrets with usage below threshold
- `summary`: Show overall analytics summary (event and secret counts)
- `deprecation`: Show secrets recommended for deprecation based on usage
- `retention`: Show or set analytics data retention policy

**Examples:**

```bash
# Find secrets unused for 30 days
envforge analytics unused --days 30

# Show usage summary
envforge analytics summary --json
```

---

### envforge audit

View change audit trail from sync history and proxy access logs.

```
Usage: envforge audit [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--key <KEY>` | Filter by key name |
| `--since <SINCE>` | Filter changes since date (ISO 8601) |
| `--machine <MACHINE>` | Filter by machine ID |
| `-n <N>` | Number of entries to show [default: 50] |
| `--ai-leaks` | Scan git history for secrets leaked in AI-assisted commits |
| `--access` | Show proxy access audit log |

**Examples:**

```bash
# View last 20 changes to a specific key
envforge audit --key API_KEY -n 20

# Scan for AI-assisted leaks in history
envforge audit --ai-leaks

# View proxy access logs
envforge audit --access
```

---

## Advanced AI Protection

### envforge lease

Manage secret access leases (time-bounded, revocable).

```
Usage: envforge lease [OPTIONS] <COMMAND>
```

**Commands:**

- `create <NAME> --ttl <TTL>`: Create a new named access lease
- `list`: List all active and expired leases
- `revoke <NAME>`: Revoke an active lease immediately
- `renew <NAME> --ttl <TTL>`: Extend an existing lease's TTL
- `grant`: Give one running command short-lived access to a secret, then auto-expire it

**Examples:**

```bash
# Create a 2-hour lease for debugging
envforge lease create "debug-session" --ttl 2h

# List all leases
envforge lease list --json

# Revoke a lease
envforge lease revoke "old-session"
```

---

### envforge session

Manage AI tool sessions with scoped secret access.

```
Usage: envforge session [OPTIONS] <COMMAND>
```

**Commands:**

- `start --tool <TOOL>`: Start a new scoped AI session (supported: `claude-code`, `cursor`)
- `stop`: Stop and expire the current AI session
- `list`: List active and historical sessions
- `cleanup`: Remove expired session data

**Examples:**

```bash
# Start a session for Claude Code
envforge session start --tool claude-code

# End current session
envforge session stop
```

---

### envforge canary

Manage canary secrets (honeypot credentials that alert when the fake value is seen in scanned output).

```
Usage: envforge canary [OPTIONS] <COMMAND>
```

**Commands:**

- `create <KEY>`: Create a new canary secret with a fake value
- `list`: List all registered canary secrets
- `check`: Check if any canary tokens have been triggered (fake value seen in scanned output)
- `scan --input <FILE>`: Scan a log, file, or stdin for known canary tokens
- `mint-v2`: Mint a forensic v2 canary token (HMAC-encoded)

**Examples:**

```bash
# Create a honeypot AWS key
envforge canary create AWS_SECRET_ACCESS_KEY

# Check if any tripwires fired
envforge canary check

# Scan a production log for leaked canaries
envforge canary scan --input /var/log/app.log
```

---

### envforge hardening

Turn on extra checks that catch hidden or disguised secrets in AI agent input.

```
Usage: envforge hardening [OPTIONS] <COMMAND>
```

**Commands:**

- `status`: Show status of hardening layers
- `apply <LAYER>`: Apply a specific hardening configuration
- `test`: Run adversarial test suite against current configuration

**Examples:**

```bash
envforge hardening status
envforge hardening apply prompt-injection-shield
```
