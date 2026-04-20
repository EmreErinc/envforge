# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-20

### Added

#### GitHub Action (`action/`)
- **Composite GitHub Action** for CI/CD integration — manage ENV vars and secrets in pipelines
- 5 operation modes: `validate`, `secrets-pull`, `export`, `run`, `drift`
- **validate** — Check `.env` files against `.env.schema`, fail step on validation errors
- **secrets-pull** — Pull secrets from any of 7 providers (Vault, AWS SSM, 1Password, Doppler, Infisical, GCP, Azure) into `GITHUB_ENV`
- **export** — Export EnvForge-managed variables into workflow environment
- **run** — Execute commands with process-scoped secret injection (secrets never persist in `GITHUB_ENV`)
- **drift** — Detect configuration drift across `.env` files in PRs
- Automatic binary installation from GitHub Releases (Linux x86_64/aarch64, macOS x86_64/aarch64)
- Version pinning (`version` input) or auto-resolve latest release
- Value masking in GitHub Actions logs by default (`mask-values: true`)
- Multiline-safe `GITHUB_ENV` export using heredoc delimiters
- All 4 outputs wired: `variables`, `count`, `validation-result`, `drift-result`
- Example workflow (`.github/workflows/envforge-example.yml`)
- Comprehensive test suite (`action/tests/test_action.sh`) — 21 tests covering syntax, input validation, integration, structure, and permissions

#### Usage
```yaml
# Validate .env on PRs
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env

# Pull secrets from AWS SSM
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production

# Run tests with injected secrets
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: cargo test
    resolve-secrets: 'true'
```

### Quality
- 21 action tests, all passing
- Shell scripts pass `bash -n` syntax validation
- Action outputs correctly wired to composite step

## [0.4.0] - 2026-04-17

### Added

#### Subprocess Runner (`envforge run`)
- `envforge run [flags] -- <command> [args]` — Run any command with injected ENV vars
- `--profile NAME` — Use a specific profile without switching the active profile
- `--resolve` — Decrypt `ENC[age:...]` values and resolve `ref:` secret references at runtime
- `--env-file PATH` — Load additional .env file(s), repeatable, override order preserved
- `--override KEY=VALUE` — Override specific variables, repeatable
- `--dry-run` — Preview injected ENV vars without executing the command
- Exit code and signal passthrough from child process
- ENV merge order: shell env → shared file → profile file → env-file(s) → overrides

#### ENV Schema (`.env.schema`)
- `.env.schema` TOML format with per-variable type, required, default, description, example, sensitive, pattern, values, min, max
- Type system: `string`, `number`, `bool`, `url`, `email`, `enum`, `regex`, `port`
- Environment-specific overrides: `[VARIABLE.production]` sections
- `envforge validate --schema PATH [--env PATH] [--environment NAME]` — Schema-based validation with CI-friendly exit codes
- Schema and `config.toml` `[validation]` rules merged (schema takes priority)
- `envforge schema generate [--output PATH]` — Auto-generate schema from existing ENV with type heuristics
- `envforge docs --schema PATH [--output PATH]` — Generate Markdown documentation table
- `envforge drift --envs FILE1 FILE2 ...` — Multi-environment drift detection with color-coded matrix
- `envforge init --schema PATH [--output PATH]` — Interactive onboarding wizard for new developers

#### AI Agent Safety
- `envforge export --safe` — Export with sensitive values redacted as `[REDACTED]`
- `envforge export --env-example` — Generate `.env.example` from schema with placeholder values
- `.envforgeignore` convention — List files AI tools should not read (`.gitignore` syntax)
- `envforge doctor` AI safety check — Warns when `.env` exists without `.envforgeignore`

#### Git Merge Driver
- `envforge git install-merge-driver` — Register semantic `.env` merge driver in git config
- Three-way merge with key=value understanding: auto-merges non-conflicting changes
- Per-key conflict markers for true conflicts (same key changed on both sides)
- `envforge git remove-merge-driver` — Clean uninstall

#### Health Check (`envforge doctor`)
- 10 health checks: config, encryption key, shell files, duplicates, validation, references, AI safety, sync, providers, credentials
- Every warning includes an actionable fix suggestion (cyan `→` hint)
- `--verbose` for detailed output, `--json` for machine-readable output

#### Other
- `envforge secrets resolve [--key KEY]` — Output `export KEY='value'` for shell init (`eval "$(envforge secrets resolve)"`)
- `envforge profile diff A B` — Side-by-side profile comparison with color-coded output

### Fixed

#### Secret Manager Providers (all 7 validated against official CLI docs)
- **AWS SSM**: Added pagination loop for `get-parameters-by-path` (>10 results), added `--recursive` flag
- **Doppler**: Filter system keys (`DOPPLER_PROJECT`, `DOPPLER_CONFIG`, `DOPPLER_ENVIRONMENT`) from pull results, batch push into single CLI call
- **Infisical**: Changed pull from invalid `secrets get --plain --format=json` to correct `export --format=json`, fixed set syntax from `set KEY VALUE` to `set KEY=VALUE`
- **Azure Key Vault**: Added underscore-to-hyphen name mapping (`DB_HOST` → `DB-HOST` on push, reverse on pull)
- **Vault**: Fixed `kv list` parsing to read `data.keys` instead of bare array
- **1Password**: Fixed `--fields` flag to `--fields=label=KEY` per official docs
- **GCP**: Handle empty `secrets list` output (empty string instead of `[]`)

### Quality
- 357 total tests (was 256), all passing
- 59 new provider validation tests covering all 7 providers
- 42 new feature tests (schema, run, safe export, doctor, drift, merge)
- Clippy clean, rustfmt clean
- No new crate dependencies

## [0.3.0] - 2026-04-15

### Added

#### Secret Manager Integration
- **Provider pattern** with `SecretProvider` trait — extensible for any secret manager
- 7 built-in providers: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault
- `envforge secrets pull --from <provider>` — Pull secrets from any provider
- `envforge secrets push --to <provider> [--keys|--all|--filter]` — Push secrets to provider
- `envforge secrets ref <KEY> --from <provider> --path <path>` — Create lazy references
- `envforge secrets unref <KEY>` — Remove a reference
- `envforge secrets resolve` — Resolve all references (with TTL cache + offline fallback)
- `envforge secrets config <provider> --set key=value` — Store credentials (age encrypted)
- `envforge secrets providers` — List all providers with binary and config status
- `envforge secrets status` — Show configured providers
- **Credential store** — Provider credentials encrypted with age in `credentials.toml`
- **Reference cache** — TTL-based caching for resolved secrets, offline fallback
- **authenticate()** and **validate_config()** on SecretProvider trait
- **Source tracking** — Pulled keys track which provider they came from
- **Progress indicator** — Shown for large pull/push operations (>50 keys)
- Vault AppRole authentication support
- AWS profile and IAM role authentication support
- "Did you mean?" suggestion for misspelled provider names

### Quality
- 256 total tests, all passing
- Clippy clean, rustfmt clean

## [0.2.0] - 2026-04-15

### Added

#### Remote Sync
- **Git-based sync** to share ENV variables across machines via any Git remote
- `envforge sync init [--remote URL] [--machine-id ID] [--force]` — Initialize sync repository
- `envforge sync push [-m MSG] [--dry-run]` — Export marked keys, commit, and push
- `envforge sync pull [--dry-run]` — Pull remote snapshot and apply changes
- `envforge sync status` — Show diff between local state and sync snapshot
- `envforge sync mark <KEY|PATTERN> --sync|--local [--all]` — Select which keys to sync
- `envforge sync list-keys` — View all keys with sync/local status
- `envforge sync override <KEY> <VALUE> [--remove] [--list]` — Machine-specific overrides
- `envforge sync history [-n N]` — View snapshot commit history
- `envforge sync rollback <COMMIT|--last>` — Restore previous snapshot with auto-backup
- `envforge sync log [-n N]` — View sync operation log
- `envforge sync machine` — Show machine identity and override count
- Selective sync with glob pattern support (`AWS_*`, `DB_?`)
- Auto-generated machine identity (`{hostname}-{hex}`) per machine
- Machine overrides — per-machine values that take precedence over shared snapshot
- Three-way conflict detection when both local and remote modify the same key
- Conflict resolution strategies: `keep-local`, `keep-remote`, `manual-edit`, with configurable default
- Offline-first design — all operations work locally, remote sync is optional
- Sync history and rollback via Git commit history
- Sync operation log (local, append-only, auto-rotated at 100 entries)
- JSON output (`--json`) and dry-run (`--dry-run`) for all sync commands

### Changed

- `envforge sync mark` now requires explicit `--sync` or `--local` flag (no silent defaults)
- `--all` flag no longer requires a key argument

### Dependencies

- Added `hostname` (0.4) for machine ID generation
- Added `rand` (0.9) for random hex suffix

### Quality

- 115 sync-specific tests (82 unit + 33 integration)
- 240 total project tests, all passing
- Clippy clean (`-D warnings`), rustfmt clean

## [0.1.0] - 2026-04-12

### Added

#### Core
- Shell file parser with line-level AST and byte-for-byte round-trip fidelity
- Support for `.zshrc`, `.bashrc`, `.bash_profile`, `.profile`, `.zprofile`
- Auto-detection of shell type from `$SHELL`
- Soft-delete with envforge tags (never physically removes content)
- Atomic file writes (tempfile + rename)
- SHA-256 hash verification before writes
- Automatic backup before every write (last 10 retained)
- Protected zone management (header/footer offsets, conda/Amazon Q block detection)
- Reference file strategy (`~/.env_managed` with source injection)
- Conflict detection (hash-based external change detection)

#### TUI Interface
- Table view with KEY / VALUE / LOCATION columns
- Keyboard navigation (vim-style j/k, search with /)
- Mouse support (click to select, scroll to navigate)
- Value masking for sensitive keys (SECRET, TOKEN, PASSWORD, CREDENTIAL, KEY)
- Edit popup, add dialog, confirm dialogs, diff preview
- Fuzzy search with match character highlighting
- ENV grouping — auto-detected prefixes + user-defined groups in config
- Collapsible group headers (arrow keys to expand/collapse)
- Active/passive toggle with Space key (visual status box)
- Full in-session undo history (`u` key)
- Profile switching with `P` key selector popup
- Profile badge in header
- Import (`I`) and export (`E`) from TUI
- Status bar with unsaved indicator, undo count, notifications
- Help screen (`?`)
- Save with `S` or `Ctrl+S`

#### CLI Interface
- `envforge list` — list all variables (with `--json`)
- `envforge get KEY` — get a value
- `envforge set KEY=VALUE` — create or update
- `envforge delete KEY` — soft-delete
- `envforge copy KEY` — copy to clipboard
- `envforge move KEY` — move to reference file
- `envforge import <file>` — import from .env file
- `envforge export [path]` — export to .env format
- `envforge duplicates` — detect duplicate keys
- `envforge scan [path]` — scan for leaked secrets
- `envforge validate` — validate ENV values against config rules
- `envforge encrypt KEY` — encrypt value with age
- `envforge decrypt KEY` — decrypt value
- `envforge profile list|switch|create|delete` — manage profiles
- `envforge log [KEY]` — view change history
- `envforge completions zsh|bash|fish` — generate shell completions
- `envforge diff` — show pending changes
- `envforge config` — show configuration
- `envforge --dry-run` — preview without writing
- First-run setup wizard

#### Profiles
- Multiple environment profiles (dev/staging/prod)
- Shared ENV file (`~/.env_managed.shared`) always sourced
- Profile-specific files (`~/.env_managed.{name}`)
- Precedence: profile > shared > shell config
- Last used profile remembered across sessions
- Migration from single reference file to profile-based

#### Security
- ENV encryption at rest using age (X25519)
- Auto-generated age keypair with 0600 permissions
- Secret scanning — detect leaked values in source code
- Git staged file scanning (`--staged` flag for pre-commit hooks)
- Sensitive value masking in TUI and changelog

#### Quality
- ENV validation rules in config (url, number, bool, email, nonempty, regex)
- Automatic changelog on save
- Log viewer with key filtering
- 125 automated tests
- CI pipeline (GitHub Actions: Linux + macOS matrix)
- Release pipeline (multi-platform binaries)
