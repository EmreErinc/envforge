# EnvForge v0.4.3 Release Notes

## What's New

### Unified Check (`envforge check`)

One command to catch all environment issues:

```bash
envforge check                    # Run all categories
envforge check --only doctor,scan # Selective
envforge check --json             # CI-friendly
```

Runs doctor, validate, scan, age, and drift checks in sequence. Each failure shows a fix hint. Exit 1 on errors.

### Encrypted Sync

Sync snapshots are now age-encrypted at rest in the git repo:

- Transparent encrypt on `sync push`, auto-decrypt on `sync pull`
- Backward compatible with unencrypted repos
- Uses existing age keypair — no new key management
- Clear error on key mismatch

### Environment Snapshots (`envforge snapshot`)

Backup and restore your active profile state:

```bash
envforge snapshot create before-upgrade
envforge snapshot list
envforge snapshot diff --last
envforge snapshot restore --last
envforge snapshot delete before-upgrade
```

Auto-prunes to 20 snapshots. Auto-backup before restore.

### Key Explain (`envforge explain`)

X-ray view of any environment variable:

```bash
envforge explain DATABASE_URL
```

Shows source file/line, profile context, schema info, encryption status, secret reference, sync marking, and age tracking.

### Secret Rotation (`envforge rotate`)

Guided interactive rotation with provider integration:

```bash
envforge rotate API_KEY           # Single key
envforge rotate --stale           # All stale secrets (>90 days)
envforge rotate API_KEY --dry-run # Preview
```

3-step flow: show masked → prompt new → confirm. Resets age, logs rotation, offers provider/sync push.

## Quality

- 444 total tests (was 398), all passing
- 46 new tests across 5 features
- No new crate dependencies
- Clean compilation, zero warnings

## New Commands Summary

| Command | Description |
|---------|-------------|
| `envforge check [--only] [--json]` | Unified health check |
| `envforge explain KEY [--json]` | Key X-ray |
| `envforge snapshot create\|list\|restore\|diff\|delete` | Env backup/restore |
| `envforge rotate KEY [--dry-run] [--stale]` | Secret rotation |

Sync encryption is automatic — no new commands needed.

## Upgrade

```bash
cargo install env-forge-tui
```

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete details.
