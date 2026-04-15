# EnvForge v0.2.0 — Remote Sync

Sync your environment variables across machines using Git.

## Highlights

- **Git-based sync** — Push/pull ENV configs between machines via any Git remote (GitHub, GitLab, Bitbucket, self-hosted).
- **Selective sync** — Choose exactly which keys sync and which stay local, with glob pattern support.
- **Machine overrides** — Same shared config, different values per machine (e.g. `DB_HOST=localhost` on dev, `DB_HOST=prod-db` on server).
- **Offline-first** — Everything works without a network. Remote is optional.

## New Commands

```bash
# Setup
envforge sync init                              # Local-only sync repo
envforge sync init --remote git@github.com:u/r  # Sync with remote

# Choose what to sync
envforge sync mark --all --sync                 # Sync everything
envforge sync mark "AWS_*" --sync               # Sync by pattern
envforge sync mark SECRET_KEY --local           # Keep local-only
envforge sync list-keys                         # View sync status

# Push & pull
envforge sync push                              # Push to remote
envforge sync pull                              # Pull from remote
envforge sync status                            # What changed?

# Per-machine overrides
envforge sync override DB_HOST localhost        # This machine only
envforge sync override --list dummy             # View overrides

# History & safety
envforge sync history                           # Snapshot history
envforge sync rollback --last                   # Undo last push
envforge sync log                               # Operation log
envforge sync machine                           # Machine info
```

## How It Works

```
~/.envforge/sync/           <- Separate Git repo (never touches your config)
├── snapshot.toml           <- Shared ENV snapshot
├── sync-config.toml        <- Sync settings + manifest
├── overrides/
│   └── macbook-pro-a3f1.toml  <- This machine's overrides
└── .git/
```

1. **Mark** which keys to sync (`--sync`) or keep local (`--local`)
2. **Push** exports marked keys to `snapshot.toml`, commits, and pushes
3. **Pull** fetches the latest snapshot and applies changes
4. **Override** lets each machine customize shared values
5. **Rollback** restores any previous snapshot from Git history

All data stays in `~/.envforge/sync/`. Your existing shell config is never touched by sync operations.

## Conflict Resolution

When both local and remote change the same key:

- `conflict_strategy = "ask"` (default) — Shows conflicts for manual resolution
- `conflict_strategy = "keep-local"` — Auto-resolve with local values
- `conflict_strategy = "keep-remote"` — Auto-resolve with remote values

## Quality

- 115 sync-specific tests (82 unit + 33 integration)
- 240 total project tests
- Clippy clean, rustfmt clean
- All existing features unchanged and passing

## Upgrade

```bash
cargo install env-forge-tui
```

No breaking changes. Sync is a new opt-in feature — existing workflows are unaffected.

## Links

- [Full Changelog](CHANGELOG.md)
- [Documentation](https://github.com/emreerinc/envforge#readme)
- [crates.io](https://crates.io/crates/env-forge-tui)
