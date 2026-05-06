# EnvForge CLI Reference

> Generated for EnvForge v0.6.2

## Global Flags

Every command accepts these flags:

| Flag | Description |
|------|-------------|
| `--json` | Output in JSON format |
| `--dry-run` | Preview changes without writing to disk |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

---

## Variable Management

### envforge list

List all environment variables.

```
Usage: envforge list [OPTIONS]
```

**Examples:**

```bash
# List all variables
envforge list

# List in JSON format
envforge list --json

# Preview without side effects
envforge list --dry-run
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

**Examples:**

```bash
# Get a single variable
envforge get DATABASE_URL

# Get as JSON
envforge get API_KEY --json
```

---

### envforge set

Set a variable (create or update).

```
Usage: envforge set [OPTIONS] <ASSIGNMENT>
```

| Argument       | Description |
|----------------|-------------|
| `<ASSIGNMENT>` | KEY=VALUE pair |

**Examples:**

```bash
# Set a variable
envforge set DATABASE_URL=postgres://localhost/mydb

# Preview the change without writing
envforge set API_KEY=sk-abc123 --dry-run

# Set and output result as JSON
envforge set NODE_ENV=production --json
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
# Delete a variable
envforge delete OLD_API_KEY

# Preview deletion
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

**Examples:**

```bash
# Copy value to clipboard
envforge copy DATABASE_URL

# Copy with JSON output confirmation
envforge copy API_KEY --json
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

Search environment variables by fuzzy matching.

```
Usage: envforge search [OPTIONS] <QUERY>
```

| Argument | Description |
|----------|-------------|
| `<QUERY>` | Search term (matches keys and values) |

| Flag | Description |
|------|-------------|
| `--json` | Output in JSON format |

**Examples:**

```bash
# Search for database-related variables
envforge search db

# Search for a specific value
envforge search "postgres://localhost"

# Output matches as JSON
envforge search api --json
```

**Note**: Sensitive values are automatically masked in the output (e.g., `sk-ab***56`).

---

### envforge duplicates

Detect and list duplicate keys.

```
Usage: envforge duplicates [OPTIONS]
```

**Examples:**

```bash
# Find duplicate keys
envforge duplicates

# Output duplicates as JSON
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
# Show pending changes
envforge diff

# Show diff in JSON format
envforge diff --json
```

---

### envforge explain

Show all known info about a single environment variable.

```
Usage: envforge explain [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name to explain |

**Examples:**

```bash
# Explain a variable (schema, history, provider, etc.)
envforge explain DATABASE_URL

# Explain with JSON output
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
# Find references to a variable
envforge deps DATABASE_URL

# Include source code scanning
envforge deps API_KEY --source

# Output as JSON
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
# Import from a file
envforge import .env.backup

# Force overwrite existing keys
envforge import .env.production --force

# Preview what would be imported
envforge import .env.staging --dry-run
```

---

### envforge export

Export variables to .env format.

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
| `--env-example` | Generate .env.example from schema with placeholder values |
| `--filter <FILTER>` | Only export entries matching this query |
| `--format <FORMAT>` | Output format: dotenv, json, yaml, toml, docker, k8s, tfvars |
| `--k8s-name <K8S_NAME>` | Kubernetes Secret name (for k8s format, default: envforge-secrets) |
| `--k8s-namespace <K8S_NAMESPACE>` | Kubernetes namespace (for k8s format, default: default) |

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
# Output the JSON Schema definition
envforge schema json-schema

# Pipe to a file
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
# Emit AI-safe context to stdout
envforge schema emit-ai

# Write to file
envforge schema emit-ai --output .env.ai-context

# Infer types when no schema exists
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
# Generate docs to stdout
envforge docs

# Write docs to a file
envforge docs --output docs/env-vars.md

# Use a specific schema
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
# Interactive setup using auto-detected schema
envforge init

# Use a specific schema and output path
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
# Compare two .env files for drift
envforge drift --envs .env.development .env.production

# Compare with schema context
envforge drift --envs .env .env.staging --schema .env.schema

# Check drift for a specific environment
envforge drift --envs .env .env.prod --environment production
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

**Examples:**

```bash
# Initialize with default TOML format
envforge project init

# Initialize with YAML
envforge project init --format yaml

# Reinitialize (overwrite existing)
envforge project init --force
```

Creates `.envforge.project.toml` (or `.yaml`/`.json`), a default `.env.development` file, and adds `.env.*` patterns to `.gitignore`.

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
# Run interactive wizard
envforge project wizard

# Re-run all steps
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
# Validate active environment
envforge project validate

# Validate specific environment
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
# Full project scan
envforge project scan

# Scan only staged files (pre-commit)
envforge project scan --staged

# Include MCP config files
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
# Show current config
envforge project config

# Edit config
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
# Pull from Vault into active environment
envforge project pull --from vault --path secret/myapp

# Pull into specific environment
envforge project pull --from doppler --environment production

# Pull with filter
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
# Push specific keys
envforge project push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY

# Push all keys
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
# Sanitize to stdout
envforge project sanitize docker-compose.yml

# Sanitize to file
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
# Export as dotenv to stdout
envforge project export

# Export as JSON
envforge project export --format json

# Export with redacted values
envforge project export --safe --format yaml

# Export filtered keys to file
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
# Encrypt a sensitive variable
envforge encrypt DATABASE_PASSWORD

# Preview encryption
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
# Decrypt a variable
envforge decrypt DATABASE_PASSWORD

# Output decrypted value as JSON
envforge decrypt API_SECRET --json
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

**Examples:**

```bash
# Initialize local sync repo
envforge sync init

# Initialize with a remote git repository
envforge sync init --remote git@github.com:myorg/env-sync.git

# Reinitialize with custom machine ID
envforge sync init --force --machine-id macbook-work
```

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
# Push changes
envforge sync push

# Push with a custom message
envforge sync push -m "Updated API keys for Q2"

# Preview what would be pushed
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
# Pull latest changes
envforge sync pull

# Preview pull
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
| `[KEY]` | Key name or glob pattern (optional when --all is used) |

| Flag | Description |
|------|-------------|
| `--sync` | Mark as synced |
| `--local` | Mark as local-only |
| `--all` | Apply to all keys |

**Examples:**

```bash
# Mark a key as local-only
envforge sync mark LOCAL_SECRET --local

# Mark all keys for sync
envforge sync mark --all --sync

# Mark keys matching a pattern as local
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
# Set a machine-specific override
envforge sync override DATABASE_HOST localhost

# Remove an override
envforge sync override DATABASE_HOST --remove

# List all overrides
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
# Rollback to the previous snapshot
envforge sync rollback --last

# Rollback to a specific commit
envforge sync rollback abc1234

# Preview rollback
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
# Pull all secrets from Vault
envforge secrets pull --from vault --path secret/myapp

# Pull from AWS SSM with filter
envforge secrets pull --from aws-ssm --path /myapp/ --filter "DB_*"

# Preview what would be pulled
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
# Push specific keys to Vault
envforge secrets push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY

# Push all keys
envforge secrets push --to doppler --all

# Push filtered keys
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
# Create a reference to a Vault secret
envforge secrets ref DATABASE_URL --from vault --path secret/myapp/db_url

# Reference an AWS SSM parameter
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
# Remove a secret reference
envforge secrets unref DATABASE_URL

# Preview unreferencing
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
# Resolve all secret references
envforge secrets resolve

# Resolve a specific key
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
# Configure Vault credentials
envforge secrets config vault --set addr=https://vault.example.com
envforge secrets config vault --set token=hvs.abc123 --ttl 8h

# Show stored credentials
envforge secrets config vault --show

# Remove provider credentials
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
| `--threshold <THRESHOLD>` | Stale threshold in days (default: 90) [default: 90] |
| `--stale-only` | Only show stale secrets |

**Examples:**

```bash
# Show all secret ages
envforge secrets age

# Flag secrets older than 30 days
envforge secrets age --threshold 30

# Only show stale secrets
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
# Compare local vs Vault
envforge secrets diff --from vault --path secret/myapp

# Compare with filter
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
# Clear all cached secrets
envforge secrets cache clear

# Clear cache for a specific provider
envforge secrets cache clear --provider vault
```

---

### envforge secrets rotate

Rotate a secret: update value, reset age, optionally push.

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
# Rotate a single secret
envforge rotate API_KEY

# Preview rotation
envforge rotate DATABASE_PASSWORD --dry-run

# Rotate all stale secrets
envforge rotate unused --stale

# Rotate and propagate to provider + sync
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
# Resolve URIs in a config file
envforge resolve-uri config/secrets.yml

# Output as .env format
envforge resolve-uri config/secrets.yml --env

# Write to a file
envforge resolve-uri config/secrets.yml --output .env.resolved
```

---

### Provider Quick Reference

| Provider | Name | Binary | Required Fields | Path Used? |
|----------|------|--------|-----------------|------------|
| [HashiCorp Vault](#hashicorp-vault-integration) | `vault` | `vault` | `addr` | Yes — KV mount path |
| [AWS SSM](#aws-ssm-parameter-store-integration) | `aws-ssm` | `aws` | _(none)_ | Yes — parameter path prefix |
| [1Password](#1password-integration) | `1password` | `op` | `service_account_token` | Yes — item name or UUID |
| [Doppler](#doppler-integration) | `doppler` | `doppler` | `token`, `project`, `config` | No — ignored |
| [Infisical](#infisical-integration) | `infisical` | `infisical` | `token`, `project_id`, `environment` | No — ignored |
| [GCP Secret Manager](#gcp-secret-manager-integration) | `gcp` | `gcloud` | `project_id` | No — ignored |
| [Azure Key Vault](#azure-key-vault-integration) | `azure` | `az` | `vault_name` | No — ignored |
| [Bitwarden](#bitwarden-secrets-manager-integration) | `bitwarden` | `bws` | `access_token` | No — ignored |
| [Akeyless](#akeyless-vault-integration) | `akeyless` | `akeyless` | `access_id`, `access_key` | Yes — item path prefix |
| [CyberArk Conjur](#cyberark-conjur-integration) | `conjur` | `conjur` | `url`, `account`, `login`, `api_key` | Yes — variable path prefix |
| [Mozilla SOPS](#mozilla-sops-integration) | `sops` | `sops` | `key_file` | Yes — encrypted file path |
| [pass/gopass](#passgopass-integration) | `pass` | `pass`/`gopass` | _(none)_ | Yes — entry prefix filter |
| [Keeper](#keeper-secrets-manager-integration) | `keeper` | `ksm` | _(none)_ | No — ignored |

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
# Pull all keys from a secret path
envforge secrets pull --from vault --path secret/myapp

# Pull with filter
envforge secrets pull --from vault --path secret/myapp --filter "DB_*"
```

**Push secrets to Vault**
```bash
# Push specific keys
envforge secrets push --to vault --path secret/myapp --keys DATABASE_URL,API_KEY

# Push all keys
envforge secrets push --to vault --path secret/myapp --all
```

**Create references**
```bash
# Reference a specific key within a Vault secret
envforge secrets ref DATABASE_URL --from vault --path secret/myapp/DATABASE_URL
```

**Resolve and run**
```bash
# Resolve all secret references and run your app
envforge run --resolve -- npm start

# Preview what would be injected
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
# Pull all parameters under a path
envforge secrets pull --from aws-ssm --path /myapp/prod/

# Pull with filter
envforge secrets pull --from aws-ssm --path /myapp/ --filter "DB_*"
```

**Push secrets to SSM**
```bash
# Push specific keys (stored as SecureString)
envforge secrets push --to aws-ssm --path /myapp/prod/ --keys DATABASE_URL,API_KEY

# Push all keys
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
# Pull all fields from an item
envforge secrets pull --from 1password --path "Backend Secrets"

# Pull with filter
envforge secrets pull --from 1password --path "Backend Secrets" --filter "DB_*"
```

**Push secrets to 1Password**
```bash
# Update existing fields in an item
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
# Pull all secrets for configured project/config
envforge secrets pull --from doppler

# Pull with filter
envforge secrets pull --from doppler --filter "DB_*"
```

**Push secrets to Doppler**
```bash
# Push specific keys
envforge secrets push --to doppler --keys DATABASE_URL,API_KEY

# Push all keys
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
# Pull all secrets for configured project/environment
envforge secrets pull --from infisical

# Pull with filter
envforge secrets pull --from infisical --filter "DB_*"
```

**Push secrets to Infisical**
```bash
# Push specific keys (batch operation)
envforge secrets push --to infisical --keys DATABASE_URL,API_KEY

# Push all keys
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

**Creating a service token:**
1. Go to your project in the Infisical dashboard
2. Navigate to **Project Settings** > **Service Tokens**
3. Create a token scoped to the target environment
4. Copy the token (starts with `st.`)

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
# Pull all secrets in project
envforge secrets pull --from gcp

# Pull with filter
envforge secrets pull --from gcp --filter "PROD_*"
```

**Push secrets to GCP**
```bash
# Push specific keys (creates secret if not exists, adds new version)
envforge secrets push --to gcp --keys DATABASE_URL,API_KEY

# Push all keys
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

EnvForge doesn't set any GCP environment variables — it relies on the `gcloud` CLI's built-in authentication.

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
# Pull all secrets from vault
envforge secrets pull --from azure

# Pull with filter
envforge secrets pull --from azure --filter "DB_*"
```

**Push secrets to Azure Key Vault**
```bash
# Push specific keys
envforge secrets push --to azure --keys DATABASE_URL,API_KEY

# Push all keys
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

EnvForge doesn't set any Azure environment variables — it relies on the `az` CLI's authentication state.

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
# Pull all accessible secrets
envforge secrets pull --from bitwarden

# Pull with filter
envforge secrets pull --from bitwarden --filter "DB_*"
```

**Push secrets to Bitwarden**
```bash
# Push specific keys (project_id required)
envforge secrets push --to bitwarden --keys DATABASE_URL,API_KEY

# Push all keys
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

**Creating a machine account:**
1. Sign in to your Bitwarden organization
2. Go to **Secrets Manager** > **Machine Accounts**
3. Create a new machine account
4. Grant access to the desired projects
5. Generate an access token

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
# Pull all secrets under a path
envforge secrets pull --from akeyless --path /myapp/production/

# Pull from root
envforge secrets pull --from akeyless --path /

# Pull with filter
envforge secrets pull --from akeyless --path /myapp/ --filter "DB_*"
```

**Push secrets to Akeyless**
```bash
# Push specific keys (updates existing, creates if not found)
envforge secrets push --to akeyless --path /myapp/production/ --keys DATABASE_URL,API_KEY

# Push all keys
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
# Pull all variables
envforge secrets pull --from conjur

# Pull variables under a path prefix
envforge secrets pull --from conjur --path myapp/production/

# Pull with filter
envforge secrets pull --from conjur --path myapp/ --filter "DB_*"
```

**Push secrets to Conjur**
```bash
# Push specific keys (variables must exist in Conjur policy)
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

**Getting an API key:**
```bash
# For a host identity
conjur host rotate-api-key -i myapp/production

# For a user
conjur user rotate-api-key -i myuser
```

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
# Decrypt and import all keys
envforge secrets pull --from sops --path secrets.enc.json

# Pull with filter
envforge secrets pull --from sops --path secrets.enc.json --filter "DB_*"
```

**Push (encrypt) secrets to a SOPS file**
```bash
# Push specific keys (merges with existing file, then re-encrypts)
envforge secrets push --to sops --path secrets.enc.json --keys DATABASE_URL,API_KEY

# Push all keys
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
# Pull all entries
envforge secrets pull --from pass

# Pull entries under a prefix
envforge secrets pull --from pass --path myapp/production/

# Pull with filter
envforge secrets pull --from pass --path myapp/ --filter "*DATABASE*"
```

**Push secrets to password store**
```bash
# Push specific keys
envforge secrets push --to pass --keys DATABASE_URL,API_KEY

# Push all keys
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
# Pull all accessible secrets
envforge secrets pull --from keeper

# Pull with filter
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

## Subprocess Runner

### envforge run

Run a command with EnvForge-managed environment variables.

```
Usage: envforge run [OPTIONS] -- <COMMAND>...
```

| Argument | Description |
|----------|-------------|
| `<COMMAND>...` | Command and arguments to run (after --) |

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

## AI Safety

### envforge fence

Create AI tool ignore rules for all supported tools (Cursor, Copilot, Claude Code), or check status.

```
Usage: envforge fence [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--status` | Check if all ignore rules are correctly installed |

**Examples:**

```bash
# Create ignore rules for all AI tools
envforge fence

# Check if rules are active
envforge fence --status

# Preview what would be created
envforge fence --dry-run
```

---

### envforge ai-hook install

Install hooks for an AI coding tool.

```
Usage: envforge ai-hook install [OPTIONS] <TOOL>
```

| Argument | Description |
|----------|-------------|
| `<TOOL>` | Tool name: claude-code, cursor |

**Examples:**

```bash
# Install hooks for Claude Code
envforge ai-hook install claude-code

# Install hooks for Cursor
envforge ai-hook install cursor
```

---

### envforge ai-hook remove

Remove hooks from an AI coding tool.

```
Usage: envforge ai-hook remove [OPTIONS] <TOOL>
```

| Argument | Description |
|----------|-------------|
| `<TOOL>` | Tool name: claude-code, cursor |

**Examples:**

```bash
envforge ai-hook remove claude-code
envforge ai-hook remove cursor
```

---

### envforge ai-hook status

Check if AI tool hooks are installed.

```
Usage: envforge ai-hook status [OPTIONS]
```

**Examples:**

```bash
# Show installation status for all supported tools
envforge ai-hook status

# Get status in JSON format
envforge ai-hook status --json
```

---

### envforge ai-guard

AI agent guard -- invoked by AI tool hooks (not for direct use).

```
Usage: envforge ai-guard [OPTIONS] <STAGE> <TOOL_NAME> [TOOL_INPUT]
```

| Argument | Description |
|----------|-------------|
| `<STAGE>` | Hook stage: pre-tool, post-tool |
| `<TOOL_NAME>` | Tool name |
| `[TOOL_INPUT]` | Tool input (JSON string or path) |

**Examples:**

```bash
# Typically called by hooks, not directly
envforge ai-guard pre-tool Write '{"file_path": ".env"}'
envforge ai-guard post-tool Read '{"file_path": "config.json"}'
```

---

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
| `--install-hook` | Install git pre-commit hook that runs envforge scan --staged |
| `--remove-hook` | Remove the envforge pre-commit hook |
| `--mcp` | Scan MCP config files for hardcoded credentials (alias for envforge mcp status) |

**Examples:**

```bash
# Scan current directory for leaked secrets
envforge scan

# Scan a specific path
envforge scan ./src

# Scan only staged git files
envforge scan --staged

# Install pre-commit hook
envforge scan --install-hook

# Scan MCP config files for hardcoded credentials
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
# List all hardcoded secrets in MCP configs
envforge mcp status

# Output findings as JSON
envforge mcp status --json
```

---

### envforge mcp harden

Replace plaintext secrets with ${VAR} env var references (backs up originals).

```
Usage: envforge mcp harden [OPTIONS]
```

**Examples:**

```bash
# Harden MCP config files
envforge mcp harden

# Preview changes
envforge mcp harden --dry-run
```

---

### envforge sanitize

Sanitize a file by replacing secret values with ${KEY} placeholders.

```
Usage: envforge sanitize [OPTIONS] <FILE>
```

| Argument | Description |
|----------|-------------|
| `<FILE>` | File to sanitize |

| Flag | Description |
|------|-------------|
| `--output <OUTPUT>` | Output file (default: stdout) |

**Examples:**

```bash
# Sanitize a file and print to stdout
envforge sanitize config.json

# Sanitize to a new file
envforge sanitize docker-compose.yml --output docker-compose.sanitized.yml

# Preview sanitization
envforge sanitize .env --dry-run
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
# Start proxy on default port
envforge proxy

# Start proxy with restricted keys
envforge proxy --keys API_KEY,DATABASE_URL --port 9000

# Start proxy requiring leases and human approval
envforge proxy --require-lease --require-approval

# Start proxy with specific allowed origins
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
# Create a 1-hour lease
envforge lease create --ttl 1h

# Create a named lease for specific keys
envforge lease create --name deploy-lease --ttl 30m --keys API_KEY,SECRET

# Create a 7-day lease
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
# Revoke a specific lease
envforge revoke deploy-lease

# Emergency killswitch: revoke all leases
envforge revoke --all

# Preview revocation
envforge revoke --all --dry-run
```

---

### envforge canary create

Create a canary secret (honeypot credential for exfiltration detection).

```
Usage: envforge canary create [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Key name (e.g., AWS_SECRET_KEY) |

| Flag | Description |
|------|-------------|
| `--pattern <PATTERN>` | Pattern: aws_key, github_token, stripe_key, slack_token, gitlab_token, generic [default: generic] |

**Examples:**

```bash
# Create a generic canary
envforge canary create HONEYPOT_API_KEY

# Create an AWS-style canary
envforge canary create AWS_SECRET_ACCESS_KEY --pattern aws_key

# Create a GitHub token canary
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

## Audit Trail

Query, report, and verify the AI audit trail. Commands read from `~/.local/share/envforge/audit/` by default; use `--log-dir` to override.

### envforge audit-trail query

Query audit events with filters (event type, source, secret key, time range).

```
Usage: envforge audit-trail query [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--event-type` | Filter by event type (e.g. `SecretAccessed`, `SecretExposure`, `SessionStarted`) |
| `--source` | Filter by source (`AiGuard`, `Proxy`, `Sync`, `Cli`, `Tui`, `Hook`) |
| `--secret-key` | Filter by secret key name |
| `--time` | Time range: `last_1h`, `last_24h`, `last_7d`, `last_30d`, `all` (default: `last_24h`) |
| `--limit` | Max results (default: 50) |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Show recent events
envforge audit-trail query

# Show events for a specific secret
envforge audit-trail query --secret-key DB_PASSWORD --time all

# Show denied access events from AI Guard
envforge audit-trail query --source AiGuard --event-type AccessDenied

# JSON output for programmatic use
envforge audit-trail query --limit 100 --json
```

---

### envforge audit-trail tail

Show recent audit events (tail-like).

```
Usage: envforge audit-trail tail [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-n` | Number of recent events (default: 20) |
| `--source` | Filter by source |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Show last 20 events
envforge audit-trail tail

# Show last 50 events from proxy
envforge audit-trail tail -n 50 --source Proxy
```

---

### envforge audit-trail report

Generate a SOC2 compliance report.

```
Usage: envforge audit-trail report [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--report-type` | Report type: `summary`, `detail`, `trend`, `violation`, `compliance` (default: `summary`) |
| `--group-by` | Group results by: `event_type`, `source`, `result`, `hour`, `day`, `week`, `month`, `secret_key`, `tool_type` |
| `--time` | Time range: `last_1h`, `last_24h`, `last_7d`, `last_30d`, `all` (default: `last_24h`) |
| `--format` | Output format: `json`, `csv`, `markdown` (default: `json`) |
| `--output` | Write output to file (stdout if omitted) |
| `--log-dir` | Path to audit log directory |

**Examples:**
```bash
# Generate summary report with compliance score
envforge audit-trail report

# Group violations by source
envforge audit-trail report --report-type violation --group-by source

# Export as markdown
envforge audit-trail report --format markdown --output report.md

# Full compliance audit over last 7 days
envforge audit-trail report --report-type compliance --time last_7d --format csv
```

---

### envforge audit-trail custody

Trace chain of custody for a secret.

```
Usage: envforge audit-trail custody [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--secret-key` | Secret key to trace |
| `--session` | Session ID to trace |
| `--ownership` | Show ownership report (last custodian, sources, gaps) |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Trace lineage of a secret
envforge audit-trail custody --secret-key API_KEY --time all

# Show ownership report
envforge audit-trail custody --secret-key DB_PASSWORD --ownership

# Trace a specific session path
envforge audit-trail custody --session session-uuid-1234
```

---

### envforge audit-trail integrity

Verify tamper-evident integrity of audit logs.

```
Usage: envforge audit-trail integrity [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--category` | Log category: `ai-guard`, `proxy`, `sync`, `cli`, `tui`, `hook`, `general` (check all if omitted) |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Check all audit log files
envforge audit-trail integrity

# Check specific log
envforge audit-trail integrity --category ai-guard

# Machine-readable verification
envforge audit-trail integrity --json
```

---

### envforge audit-trail stats

Show audit event statistics.

```
Usage: envforge audit-trail stats [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--time` | Time range: `last_1h`, `last_24h`, `last_7d`, `last_30d`, `all` (default: `last_24h`) |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Show summary statistics
envforge audit-trail stats

# Weekly breakdown
envforge audit-trail stats --time last_7d --json
```

---

### envforge audit-trail retention

Manage audit log retention policy.

```
Usage: envforge audit-trail retention [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--policy` | Retention period: `1d`, `7d`, `30d`, `90d`, `365d` (default: `90d`) |
| `--execute` | Actually perform cleanup (dry-run by default) |
| `--log-dir` | Path to audit log directory |
| `--json` | JSON output |

**Examples:**
```bash
# Show what would be removed (dry-run)
envforge audit-trail retention --policy 30d

# Execute cleanup
envforge audit-trail retention --policy 90d --execute
```

---

## Diagnostics

### envforge check

Run all checks: doctor + validate + scan + age + drift.

```
Usage: envforge check [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--only <ONLY>` | Only run specific categories (comma-separated: doctor,validate,scan,age,drift) |

**Examples:**

```bash
# Run all checks
envforge check

# Run only validation and scanning
envforge check --only validate,scan

# Run checks with JSON output
envforge check --json
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
# Run health checks
envforge doctor

# Run with verbose output
envforge doctor --verbose

# Run with JSON output
envforge doctor --json
```

---

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
# View full audit trail
envforge audit

# Filter by key
envforge audit --key DATABASE_URL

# Filter by date
envforge audit --since 2025-01-01

# Scan for AI-leaked secrets in git history
envforge audit --ai-leaks

# Show proxy access log
envforge audit --access

# Filter by machine
envforge audit --machine macbook-work -n 20
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
# View recent change history
envforge log

# View history for a specific key
envforge log API_KEY

# Show last 10 entries
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

## Snapshots

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
# Create a snapshot with auto-generated name
envforge snapshot create

# Create a named snapshot
envforge snapshot create pre-deploy

# Preview snapshot creation
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
# Restore by name (substring match)
envforge snapshot restore pre-deploy

# Restore the most recent snapshot
envforge snapshot restore --last

# Preview restore
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
# Diff a named snapshot
envforge snapshot diff pre-deploy

# Diff the most recent snapshot
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

## Sharing

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
# Share specific keys with a teammate
envforge share create --recipient age1abc... --keys API_KEY,DB_URL

# Share all keys with expiry
envforge share create --recipient age1abc... --all --expire 24

# Share filtered keys to a custom file
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
# View shared secrets
envforge share receive envforge-share.age

# Import shared secrets
envforge share receive prod-secrets.age --import
```

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

## Backup

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

## Shell Integration

### envforge completions

Generate and install shell completion scripts.

```
Usage: envforge completions [OPTIONS] <SHELL>
```

| Argument | Description |
|----------|-------------|
| `<SHELL>` | Shell type: `zsh`, `bash`, `fish`, `kiro`, `fig` |

| Flag | Description |
|------|-------------|
| `--install` | Install completion spec to the correct system path |

**Examples:**

```bash
# Generate zsh completions to stdout
envforge completions zsh > ~/.zsh/completions/_envforge

# Install zsh completions to ~/.zfunc/_envforge
envforge completions zsh --install

# Install bash completions
envforge completions bash --install

# Install fish completions
envforge completions fish --install

# Install Kiro CLI autocomplete spec
envforge completions kiro --install

# Install Fig autocomplete spec
envforge completions fig --install
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

---

### envforge hook

Generate shell hook for auto-loading.

```
Usage: envforge hook [OPTIONS] <SHELL>
```

| Argument | Description |
|----------|-------------|
| `<SHELL>` | Shell type (zsh, bash, fish) |

**Examples:**

```bash
# Add to .zshrc
eval "$(envforge hook zsh)"

# Add to .bashrc
eval "$(envforge hook bash)"

# Add to fish config
envforge hook fish | source
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
- **IntelliJ IDEA** — Install from [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/31385-envforge) (requires LSP4IJ plugin)
- **Neovim** — Configure via `nvim-lspconfig` (see `editors/README.md`)
- **Helix** — Add to `languages.toml` (see `editors/README.md`)
- **Sublime Text** — Configure via LSP package (see `editors/README.md`)

---

### envforge env

Output environment variables as shell export statements (for eval).

```
Usage: envforge env [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--dir <DIR>` | Directory to load from (default: current) |

**Examples:**

```bash
# Export all variables into current shell
eval "$(envforge env)"

# Export from a specific directory
eval "$(envforge env --dir /path/to/project)"
```
