# EnvForge v0.3.0 — Secret Manager Integration

Pull and push environment variables from external secret managers — Vault, AWS SSM, 1Password, Doppler, Infisical, GCP, and Azure.

## Highlights

- **7 secret managers** supported out of the box via CLI wrappers (no SDK dependencies)
- **Three modes**: Pull (import), Reference (lazy resolve with cache), Push (export)
- **Encrypted credentials** stored locally with age encryption
- **TUI integration** — secrets menu, reference key icons, provider status

## New Commands

```bash
# Configure credentials (encrypted)
envforge secrets config vault --set addr=https://vault.example.com
envforge secrets config vault --set token=hvs.xxx
envforge secrets config aws-ssm --set profile=production

# Pull secrets
envforge secrets pull --from vault --path secret/myapp
envforge secrets pull --from aws-ssm --path /prod/ --filter "DB_*"

# Push secrets
envforge secrets push --to doppler --all
envforge secrets push --to vault --path secret/myapp --keys DB_URL,API_KEY

# Reference mode (value never stored locally)
envforge secrets ref DB_URL --from vault --path secret/myapp/DB_URL
envforge secrets resolve                   # Fetch all references
envforge secrets unref DB_URL              # Remove reference

# Provider management
envforge secrets providers                 # Show all 7 providers + status
envforge secrets status                    # Configured providers
```

## Supported Providers

| Provider | CLI | Auth Methods |
|----------|-----|-------------|
| HashiCorp Vault | `vault` | Token, AppRole |
| AWS SSM | `aws` | Access Key, Profile, IAM Role |
| 1Password | `op` | Service Account Token |
| Doppler | `doppler` | Service Token |
| Infisical | `infisical` | Token, Machine Identity |
| GCP Secret Manager | `gcloud` | gcloud auth |
| Azure Key Vault | `az` | az login |

## Architecture

```
src/ops/secrets/
├── provider.rs          ← SecretProvider trait + ProviderRegistry
├── credentials.rs       ← Age-encrypted credential store
├── cache.rs             ← TTL-based reference cache + SecretRef
├── modes.rs             ← Pull/Push/Resolve orchestration
└── providers/           ← 7 provider implementations (CLI wrappers)
```

Adding a new provider requires only implementing the `SecretProvider` trait and registering it in `create_default_registry()`.

## Quality

- 256 total tests, 0 failures
- Clippy clean, rustfmt clean
- No new crate dependencies (reuses existing age, serde_json, chrono, tempfile)

## Upgrade

```bash
cargo install env-forge-tui
```

No breaking changes. Secret manager integration is fully opt-in.

**Full Changelog**: https://github.com/emreerinc/envforge/compare/v0.2.2...v0.3.0
