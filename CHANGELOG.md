# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-20

### Added

#### Pre-Commit Hook Integration
- `envforge scan --install-hook` — Auto-install git pre-commit hook running `envforge scan --staged`
- Appends to existing pre-commit hooks (doesn't overwrite)
- `envforge scan --remove-hook` — Clean removal of EnvForge hook lines
- Hook blocks commits when secrets detected (non-zero exit)

#### Shell Auto-Load Hook
- `eval "$(envforge hook zsh)"` — direnv-style auto-load on directory change
- Supports zsh (chpwd), bash (PROMPT_COMMAND), and fish (--on-variable PWD)
- Auto-detects `.envforge.toml` or `.env.schema` in directory
- Auto-unloads when leaving directory (restores previous env)
- `envforge env [--dir PATH]` — Output shell export statements for eval
- `.envforge.toml` project config with `profile` field

#### Volatile Mode (AI Agent Safety)
- `envforge run --volatile` — Secrets resolved in memory only, no disk I/O for secret values
- Forces `--resolve` mode automatically
- Ignores `--env-file` flags (no .env disk reads in volatile mode)
- `--dry-run` masks sensitive values as `****`
- Protects against AI agent file scanning (Claude Code, Cursor, Copilot)

#### Secure Secret Sharing
- `envforge share create --recipient <AGE_PUBKEY> [--keys|--all|--filter]` — Create encrypted share file
- `envforge share receive FILE [--import]` — Decrypt and import shared secrets
- Encrypted with recipient's age public key (not sender's)
- `--expire HOURS` — Soft expiry metadata (warns on receive after expiry)
- Self-contained `.age` file format with sender metadata

#### JSON Schema for `.env.schema`
- `envforge schema json-schema` — Output JSON Schema (Draft 2020-12) for `.env.schema` format
- Covers all fields: type, required, default, description, example, sensitive, pattern, values, min, max
- Environment override sub-objects supported
- Enables VSCode/JetBrains autocomplete and validation

#### Rotate with Propagation
- `envforge rotate KEY --propagate` — Auto-push to provider AND sync after local rotation
- No interactive prompts in propagate mode
- Graceful failure: provider/sync errors don't roll back local change
- Summary: `Rotated: local ✓, vault ✓, sync ✓`
- Works with `--stale --propagate` for bulk rotation

#### Git Author Audit Trail
- `envforge audit` — Per-variable change history with author attribution from sync git history
- `--key NAME` — Filter by variable name
- `--since DATE` — Filter changes after date
- `--machine ID` — Filter by machine
- `--json` — Machine-readable output
- Shows action type: added, modified, removed

#### Token TTL (Credential Expiry)
- `envforge secrets config PROVIDER --set key=value --ttl 8h` — Set credential expiry
- Duration formats: `8h`, `24h`, `7d`, `30d`
- Expired credentials rejected with clear message and renewal hint
- `envforge secrets status` shows TTL remaining per provider
- `envforge doctor` warns about credentials expiring within 24h
- TTL metadata stored in `_meta` sections of `credentials.toml`

#### Offline Fallback & Cache Management
- `envforge run --resolve` now falls back to stale cached values when provider unreachable
- Warning on stderr: "Using cached value for KEY (provider unreachable)"
- `envforge secrets cache list` — Show all cached secrets with provider, age, fresh/expired status
- `envforge secrets cache clear [--provider NAME]` — Clear cache (all or per-provider)

#### Multi-Profile Merge
- `envforge run --profiles dev,staging,custom` — Load and merge multiple profiles
- Left-to-right precedence (last profile wins on conflicts)
- Each profile overlaid on shared vars
- Compatible with `--env-file` and `--override` (highest precedence)
- Error if both `--profile` and `--profiles` used

### Quality
- 485 total tests (was 444), all passing
- 41 new tests across 10 features
- New modules: `hook.rs`, `share.rs`, `schema_json.rs`, `audit.rs`
- No new crate dependencies
- Clippy: 0 errors, 11 style warnings

## [0.4.3] - 2026-04-20

### Added

#### Unified Check (`envforge check`)
- `envforge check` — Single command running all health/safety checks: doctor, validate, scan, age, drift
- Grouped output by category with colored pass/fail/warning indicators
- Interactive fix hints per failure (`→ Run: envforge <fix command>`)
- `--only doctor,scan,age` — Run subset of categories
- `--json` — Machine-readable output for CI/CD
- Graceful skip when prerequisites missing (no schema → skip validate/drift)
- Non-zero exit on errors, zero on warnings-only

#### Encrypted Sync
- Sync snapshots now encrypted with age (X25519) before git commit
- Transparent auto-decrypt on `sync pull` — no extra flags needed
- Backward compatible: `read_snapshot` auto-detects encrypted vs plaintext
- `encrypted` config field in sync settings (default: `true` for new repos)
- Old unencrypted repos work without migration — next push encrypts automatically
- Clear error on key mismatch: "Cannot decrypt sync data. Key mismatch or corrupted."

#### Environment Snapshots (`envforge snapshot`)
- `envforge snapshot create [NAME]` — Capture active profile env state
- `envforge snapshot list` — Show all snapshots with name, date, variable count
- `envforge snapshot restore [NAME|--last]` — Restore with auto-backup before write
- `envforge snapshot diff [NAME|--last]` — Color-coded diff (added/removed/changed)
- `envforge snapshot delete NAME` — Remove snapshot with confirmation
- Auto-prune: keeps last 20 snapshots, oldest pruned on create
- Snapshots stored in `~/.config/envforge/snapshots/` as TOML

#### Key Explain (`envforge explain`)
- `envforge explain KEY` — Unified X-ray showing all known info about a key
- **Source**: file path, line number, export style, active/commented status
- **Profile**: which profile defines it (shared/profile-specific)
- **Schema**: type, required, description, sensitive flag (if `.env.schema` exists)
- **Encryption**: plaintext or encrypted status
- **Reference**: secret reference provider/path (if `ref:` value)
- **Sync**: synced/local/untracked status
- **Age**: days since last pull, stale flag
- `--json` — Full structured JSON output
- Similar key suggestions when key not found (substring + Levenshtein matching)

#### Secret Rotation (`envforge rotate`)
- `envforge rotate KEY` — Interactive 3-step flow: show masked current → prompt new → confirm
- Masked value display (`sk-ab****56` format)
- Atomic local update with backup before write
- Auto-resets secret age to 0 after rotation
- Logs rotation event to changelog
- Optional provider push: `Push to vault? [y/N]`
- Optional sync push: `Push to sync? [y/N]`
- `--dry-run` — Preview rotation plan without changes
- `--stale` — Bulk rotate all secrets older than 90 days (rotate/skip/quit per key)
- Handles encrypted values transparently (re-encrypts new value)

### Quality
- 444 total tests (was 398), all passing
- 46 new tests across 5 features
- No new crate dependencies
- Clippy clean

## [0.4.2] - 2026-04-20

### Added

#### Multi-Format Export
- `envforge export --format <fmt>` — Export env vars in 7 formats: `dotenv`, `json`, `yaml`, `toml`, `docker`, `k8s`, `tfvars`
- **JSON** — `{"KEY": "VALUE"}` object, valid JSON
- **YAML** — Properly quoted booleans (`true`/`false`), numbers, and YAML-special values (`null`, `~`, `yes`, `no`)
- **TOML** — All values quoted, backslash and quote escaping
- **Docker** — Bare `KEY=VALUE` for `--env-file`, no quotes or comments
- **Kubernetes Secret** — Full manifest with base64-encoded `data:` section
  - `--k8s-name NAME` — Set Secret name (default: `envforge-secrets`)
  - `--k8s-namespace NS` — Set namespace (default: `default`)
- **Terraform tfvars** — Lowercase keys, quoted values
- Works with existing `--filter` flag

#### Secret Age Tracking
- `envforge secrets age [--threshold DAYS] [--stale-only]` — Show age of all tracked secrets
- Automatically tracks when secrets are pulled from providers
- Flags stale secrets exceeding threshold (default: 90 days)
- Color-coded output: green ✓ for fresh, red ⚠ for stale
- `--json` output with age_days, stale flag, provider info
- Persistent tracking in `~/.config/envforge/secret-sources.toml`

#### Provider Diff
- `envforge secrets diff --from <provider> --path <path> [--filter PATTERN]` — Compare local ENV vs provider state
- Shows 4 categories: same, changed, only-local, only-remote
- Color-coded diff output with truncated values
- `--json` output for CI/CD integration
- Works with all 7 secret providers

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
- 398 total tests (was 357), all passing
- 27 new Tier 1 feature tests (export formats, secret age, edge cases)
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
