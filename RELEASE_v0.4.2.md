# EnvForge v0.4.2 Release Notes

## What's New

### Multi-Format Export

Export your environment variables in 7 formats for any infrastructure:

```bash
envforge export --format json         # {"KEY": "VALUE"}
envforge export --format yaml         # YAML with proper quoting
envforge export --format toml         # KEY = "VALUE"
envforge export --format docker       # Bare KEY=VALUE for --env-file
envforge export --format k8s          # Kubernetes Secret manifest
envforge export --format tfvars       # Terraform variables
```

Kubernetes exports include full manifest with base64-encoded data:

```bash
envforge export --format k8s --k8s-name app-secrets --k8s-namespace production
```

YAML export correctly quotes booleans (`true`/`false`), numbers, and YAML-special values (`null`, `~`, `yes`, `no`, `on`, `off`) to prevent type coercion.

### Secret Age Tracking

Track when secrets were last pulled and flag stale ones for rotation:

```bash
envforge secrets age                    # Show all secret ages
envforge secrets age --threshold 30     # Custom stale threshold (days)
envforge secrets age --stale-only       # Only show stale secrets
envforge secrets age --json             # Machine-readable output
```

```
KEY                            PROVIDER     AGE      STATUS
-----------------------------------------------------------------
API_KEY                        vault        120 days ⚠ STALE
DB_PASSWORD                    aws-ssm      45 days  ✓ ok
STRIPE_KEY                     doppler      2 days   ✓ ok

1 secret(s) older than 90 days. Consider rotating them.
```

Ages are automatically tracked on every `envforge secrets pull` and persisted in `~/.config/envforge/secret-sources.toml`.

### Provider Diff

Compare local environment variables against any secret provider to detect drift:

```bash
envforge secrets diff --from vault --path secret/myapp
envforge secrets diff --from aws-ssm --path /prod --filter "DB_*"
envforge secrets diff --from doppler --json
```

```
Diff: local vs vault (secret/myapp)

~~~ Changed: 2 key(s)
  ~ DB_HOST
    - local:  localhost
    + remote: prod-db.internal

--- Only local: 1 key(s)
  - DEBUG

+++ Only remote: 1 key(s)
  + NEW_FEATURE_FLAG

Summary: 5 same, 2 changed, 1 only local, 1 only remote
```

### GitHub Action

Official GitHub Action for CI/CD integration with 5 modes:

```yaml
# Validate .env against schema on PRs
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env

# Pull secrets from any of 7 providers
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production

# Run tests with process-scoped secrets
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: cargo test
    resolve-secrets: 'true'

# Detect env drift across files
- uses: emreerinc/envforge/action@v1
  with:
    mode: drift
    drift-envs: |
      .env.development
      .env.staging
      .env.production
```

Features:
- Automatic binary installation from GitHub Releases
- Value masking in Actions logs by default
- Process-scoped secret injection (run mode)
- 4 outputs: `variables`, `count`, `validation-result`, `drift-result`
- See [action/README.md](action/README.md) for full documentation

## Quality

- 398 total tests (was 357), all passing
- 27 new feature tests (export formats, secret age, edge cases)
- 21 GitHub Action tests
- No new crate dependencies
- Clippy clean

## Upgrade

```bash
cargo install env-forge-tui
```

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete details.
