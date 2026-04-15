# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
