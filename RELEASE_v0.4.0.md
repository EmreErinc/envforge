# EnvForge v0.4.0 — Run, Schema, AI Safety & Git Merge

The biggest feature release since v0.1.0. Four major capabilities that transform EnvForge from an ENV manager into a complete developer environment platform.

## Highlights

- **`envforge run`** — Run any command with injected ENV vars. Profile switching, secret resolution, and .env file loading — all without touching your shell config.
- **`.env.schema`** — Define types, requirements, and descriptions for your project's ENV vars. Validate in CI, generate docs, detect drift across environments, onboard new developers interactively.
- **AI Agent Safety** — Protect secrets from leaking into AI coding tools. Safe export with redaction, `.envforgeignore` convention, doctor warnings.
- **Git Merge Driver** — Semantic three-way merge for `.env` files. Auto-merges non-conflicting keys, only flags true conflicts.

## New Commands

### Subprocess Runner
```bash
envforge run -- npm start                          # Inject all ENV vars
envforge run --profile prod -- docker compose up   # Use specific profile
envforge run --resolve -- npm start                # Decrypt + resolve secrets
envforge run --env-file .env.local -- npm start    # Load .env file
envforge run --override PORT=9090 -- npm start     # Override single var
envforge run --dry-run -- npm start                # Preview without executing
```

### ENV Schema
```bash
envforge schema generate --output .env.schema      # Generate from current ENV
envforge validate --schema .env.schema             # Validate against schema
envforge validate --schema .env.schema --env .env  # Validate specific file
envforge validate --env .env --environment prod    # Env-specific rules
envforge docs --schema .env.schema                 # Generate Markdown docs
envforge drift --envs .env.dev .env.prod           # Cross-environment diff
envforge init --schema .env.schema                 # Interactive onboarding
```

### AI Safety
```bash
envforge export --safe                             # Redact sensitive values
envforge export --safe --output .env.safe          # Save redacted file
envforge export --env-example                      # Schema-based .env.example
envforge doctor                                    # Warns about AI exposure
```

### Git Merge Driver
```bash
envforge git install-merge-driver                  # One-time setup
envforge git remove-merge-driver                   # Clean removal
# After install, `git merge` handles .env files automatically
```

### Health & Analysis (v0.3.1)
```bash
envforge doctor [--verbose]                        # 10 health checks
envforge secrets resolve [--key KEY]               # Shell init support
envforge profile diff dev prod                     # Profile comparison
```

## Schema Format

```toml
[DATABASE_URL]
type = "url"
required = true
description = "PostgreSQL connection string"
pattern = "^postgres://"

[PORT]
type = "port"
required = true
default = "3000"

[NODE_ENV]
type = "enum"
required = true
values = ["development", "staging", "production"]

[API_KEY]
type = "string"
required = true
sensitive = true

# Environment-specific overrides
[DATABASE_URL.production]
pattern = "^postgres://prod-"
```

Supported types: `string`, `number`, `bool`, `url`, `email`, `enum`, `regex`, `port`

## Bug Fixes (Secret Manager Providers)

All 7 providers validated against official CLI documentation:

| Provider | Fix |
|----------|-----|
| AWS SSM | Pagination for >10 params, `--recursive` flag |
| Doppler | Filter system keys (DOPPLER_PROJECT etc.), batch push |
| Infisical | Correct `export --format=json` command, `KEY=VALUE` syntax |
| Azure | Underscore-to-hyphen name mapping for Key Vault |
| Vault | Parse `data.keys` for `kv list` (was bare array) |
| 1Password | `--fields=label=KEY` syntax |
| GCP | Handle empty list edge case |

## Quality

- **357 tests** (was 256), all passing
- 59 new provider validation tests
- 42 new feature tests (schema, run, safe export, doctor, drift)
- Clippy clean (`-D warnings`), rustfmt clean
- No new crate dependencies

## Architecture

```
New modules:
  src/ops/run.rs       — Subprocess runner (collect, merge, spawn)
  src/ops/schema.rs    — Schema parser, validator, docs, drift, generate
  src/ops/doctor.rs    — 10 health checks with hints
  src/ops/profile_diff.rs — Profile comparison
  
Enhanced:
  src/ops/dotenv.rs    — Safe export, env-example generation
  src/cli/mod.rs       — 13 new subcommands
  src/cli/commands.rs  — Git merge driver, schema tools
```

## Upgrade

```bash
cargo install env-forge-tui
```

No breaking changes. All new features are additive and opt-in.

**Full Changelog**: https://github.com/emreerinc/envforge/compare/v0.3.0...v0.4.0
