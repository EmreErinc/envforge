# EnvForge CLI Reference

> Generated for EnvForge v0.5.4

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

| Argument | Description |
|----------|-------------|
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

Move a variable to the reference file.

```
Usage: envforge move [OPTIONS] <KEY>
```

| Argument | Description |
|----------|-------------|
| `<KEY>` | Variable name |

**Examples:**

```bash
# Move a variable to the reference file
envforge move LEGACY_KEY

# Preview the move
envforge move OLD_SECRET --dry-run
```

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
| `--from <FROM>` | Provider name (vault, aws-ssm, 1password, doppler, infisical, gcp, azure) |
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
```

---

## AI Safety

### envforge fence

Create AI tool ignore rules for all supported tools (Cursor, Copilot, Claude Code).

```
Usage: envforge fence [OPTIONS]
```

**Examples:**

```bash
# Create ignore rules for all AI tools
envforge fence

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

# Preview installation
envforge ai-hook install claude-code --dry-run
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
| `--mcp` | Scan MCP config files for hardcoded credentials |

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
