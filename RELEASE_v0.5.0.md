# EnvForge v0.5.0 Release Notes

Major release with 10 new features focused on security, developer experience, and team collaboration.

## New Features

### Pre-Commit Hook (`envforge scan --install-hook`)
One command to protect your repo from secret leaks:
```bash
envforge scan --install-hook   # Blocks commits with exposed secrets
envforge scan --remove-hook    # Clean removal
```

### Shell Auto-Load (`envforge hook`)
direnv-style auto-loading — env profile activates when you `cd` into a project:
```bash
eval "$(envforge hook zsh)"    # Add to ~/.zshrc
```
Create `.envforge.toml` with `profile = "dev"` in your project root.

### Volatile Mode (`envforge run --volatile`)
AI-agent-safe mode — secrets never touch disk:
```bash
envforge run --volatile -- npm start
```
Forces in-memory resolution, ignores .env files, masks values in dry-run.

### Secure Sharing (`envforge share`)
Age-encrypted secret sharing for team onboarding:
```bash
envforge share create --recipient age1abc... --all --output secrets.age
envforge share receive secrets.age --import
```

### JSON Schema (`envforge schema json-schema`)
Editor autocomplete for `.env.schema` files:
```bash
envforge schema json-schema > .env.schema.json
```
Draft 2020-12 compatible — works with VSCode and JetBrains.

### Rotate with Propagation (`--propagate`)
Auto-push rotated secrets to all targets:
```bash
envforge rotate API_KEY --propagate    # local + vault + sync in one shot
envforge rotate --stale --propagate    # Bulk rotate all stale secrets
```

### Git Author Audit (`envforge audit`)
Who changed what, when, from which machine:
```bash
envforge audit --key DB_URL --since 2026-04-01
```

### Token TTL (`--ttl`)
Auto-expire provider credentials:
```bash
envforge secrets config vault --set token=hvs.xxx --ttl 8h
```
Doctor warns about expiring credentials. Expired tokens rejected with renewal hint.

### Offline Fallback & Cache Management
Secrets work when providers are down:
```bash
envforge secrets cache list     # View cached secrets
envforge secrets cache clear    # Clear cache
```
`envforge run --resolve` auto-falls back to stale cached values with warning.

### Multi-Profile Merge (`--profiles`)
Load multiple profiles in one process:
```bash
envforge run --profiles dev,staging,hotfix -- npm test
```
Left-to-right precedence, last wins.

## Stats

- **485 total tests**, 0 failures
- **70+ CLI subcommands**
- **7 secret providers** with TTL, cache, offline fallback
- **3 shell hooks** (zsh, bash, fish)
- No new crate dependencies

## Full Feature Set (v0.5.0)

| Category | Features |
|----------|----------|
| Core | Safe parsing, atomic writes, backups, SHA-256, soft-delete |
| TUI | Vim-style, fuzzy search, grouping, masking, mouse |
| CLI | 70+ commands, --json, --dry-run, completions |
| Run | Subprocess injection, volatile, multi-profile, resolve |
| Export | 7 formats (dotenv, JSON, YAML, TOML, Docker, K8s, tfvars) |
| Schema | Type validation, JSON Schema, wizard, docs, drift |
| Profiles | Multi-env, shared files, diff, multi-merge |
| Encryption | Age X25519, per-value, encrypted sync |
| Sync | Git-based, encrypted, selective, overrides, rollback |
| Secrets | 7 providers, TTL, cache, offline, age tracking, diff |
| Check | Unified check (doctor+validate+scan+age+drift) |
| Snapshots | Create, restore, diff, auto-prune |
| Explain | Key X-ray across all subsystems |
| Rotation | Interactive, propagate, bulk stale |
| Hooks | Pre-commit scan, shell auto-load (zsh/bash/fish) |
| Share | Age-encrypted team sharing with expiry |
| Audit | Git author trail with filters |
| AI Safety | Volatile mode, safe export, .envforgeignore |
| CI/CD | GitHub Action (5 modes) |
| Git | Semantic merge driver |

## Upgrade

```bash
cargo install env-forge-tui
```
