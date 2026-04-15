# EnvForge v0.2.2 — Remote Sync

Sync your environment variables across machines using Git. This is the first feature release since v0.1.0.

## What's New (since v0.1.0)

### Remote Sync

The headline feature — share ENV variables across machines via any Git remote.

```bash
# Initialize
envforge sync init --remote git@github.com:user/envforge-sync.git

# Choose what to sync
envforge sync mark --all --sync                 # Sync all keys
envforge sync mark "AWS_*" --sync               # Sync by glob pattern
envforge sync mark SECRET_KEY --local           # Keep local-only

# Push & pull
envforge sync push                              # Push to remote
envforge sync pull                              # Pull from remote
envforge sync status                            # Show diff

# Per-machine overrides
envforge sync override DB_HOST localhost        # Override for this machine only

# History & safety
envforge sync history                           # View snapshot history
envforge sync rollback --last                   # Rollback to previous snapshot
```

**Key design decisions:**

- **Separate sync repo** — Lives in `~/.envforge/sync/`, never touches your existing shell config
- **Selective sync** — You choose which keys sync (`--sync`) and which stay local (`--local`), with glob pattern support (`AWS_*`, `DB_?`)
- **Machine identity** — Each machine gets a unique ID (`{hostname}-{hex}`), with per-machine overrides that take precedence over shared values
- **Offline-first** — Everything works locally, remote is optional
- **Three-way conflict detection** — When both sides modify the same key, choose keep-local, keep-remote, or manual edit
- **Git-based history** — Rollback to any previous snapshot, automatic backup before destructive operations
- **JSON output** — All sync commands support `--json` for scripting
- **Dry-run** — Preview push/pull changes with `--dry-run`

### New CLI Commands

| Command | Description |
|---------|-------------|
| `envforge sync init` | Initialize sync repository (local or from remote) |
| `envforge sync push` | Export marked keys, commit, and push |
| `envforge sync pull` | Pull remote snapshot and apply changes |
| `envforge sync status` | Show local vs snapshot diff |
| `envforge sync mark` | Mark keys as sync or local-only |
| `envforge sync list-keys` | View all keys with sync/local status |
| `envforge sync override` | Machine-specific value overrides |
| `envforge sync history` | View snapshot commit history |
| `envforge sync rollback` | Restore previous snapshot |
| `envforge sync log` | View sync operation log |
| `envforge sync machine` | Show machine info |

### Architecture

```
~/.envforge/sync/                    ← Separate Git repo
├── snapshot.toml                    ← Shared ENV snapshot (TOML)
├── sync-config.toml                 ← Settings + manifest (which keys sync)
├── overrides/
│   └── macbook-pro-a3f1.toml       ← This machine's overrides
└── .git/
```

```
src/ops/sync/
├── model.rs      ← Types: SyncSnapshot, SyncConfig, SyncDiff, ConflictEntry, errors
├── git.rs        ← GitOps trait + GitCommandRunner (thin wrapper over git binary)
├── init.rs       ← Repo init, machine ID, snapshot/config I/O, atomic writes
├── marking.rs    ← Selective sync marking with glob support
├── diff.rs       ← Diff computation between local state and snapshot
├── conflict.rs   ← Three-way conflict detection and resolution
├── push.rs       ← Snapshot export and push workflow
├── pull.rs       ← Pull, backup, and apply workflow
├── machine.rs    ← Machine overrides CRUD and merge logic
└── history.rs    ← Git history, rollback, sync operation log
```

### Bug Fixes (v0.2.1, v0.2.2)

- Fixed CI failures: git `user.name`/`user.email` now auto-configured for sync repos (no longer requires global git config)
- Fixed `envforge sync mark` UX: `--sync` or `--local` flag is now required (no silent defaults), `--all` no longer requires a key argument

## Quality

- **115 sync-specific tests** (82 unit + 33 integration)
- **240 total project tests**, all passing
- Clippy clean (`-D warnings`), rustfmt clean
- All v0.1.0 features unchanged and passing

## Install / Upgrade

```bash
cargo install env-forge-tui
```

No breaking changes. Remote sync is a new opt-in feature — existing workflows are completely unaffected.

## New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `hostname` | 0.4 | Machine ID generation |
| `rand` | 0.9 | Random hex suffix for machine ID |

**Full Changelog**: https://github.com/emreerinc/envforge/compare/v0.1.0...v0.2.2
