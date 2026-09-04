# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in EnvForge, please report it responsibly:

1. **Do NOT** open a public GitHub issue
2. Email: emre_erinc@hotmail.com (or use [GitHub Security Advisories](https://github.com/emreerinc/envforge/security/advisories/new))
3. Include: description, reproduction steps, potential impact
4. We aim to respond within 48 hours

## Security Model

### What EnvForge Protects

- **File integrity** — Atomic writes prevent corruption. SHA-256 hash verification.
- **No data loss** — Soft-delete only. Original content preserved as comments.
- **Secret masking** — Sensitive values never displayed in plain text by default.
- **Encryption at rest** — Age X25519 encryption for sensitive values. Mandatory by construction for all 13 secret providers.
- **Credential encryption policy** — Every provider declares its encryption posture via the `SecretProvider` trait (compile-time enforced). No provider returns plaintext credentials to disk without explicit, audited justification.
- **Credential isolation** — All provider credentials passed via environment variables or stdin pipes (never CLI flags) to prevent `/proc/PID/cmdline` leakage.
- **Error sanitization** — CLI error output is sanitized to redact credential patterns before logging or display.
- **Recovery key** — A second age keypair is generated on first run. Store it offline to recover encrypted credentials if the primary key is lost.

### What EnvForge Does NOT Protect

- **Runtime memory** — Decrypted values exist in memory during session
- **Terminal history** — CLI commands with values may appear in shell history
- **Clipboard** — Copied values are in system clipboard (not cleared automatically)
- **CLI binary integrity** — EnvForge does not verify GPG signatures of provider CLIs. A compromised binary in PATH could exfiltrate secrets. Verify binary integrity yourself.
- **Cache on disk** — Secret cache files (`~/.config/envforge/secrets-cache/*.cache`) are TOML whose `value` fields are age-encrypted (`ENC[age:...]`) with 0600 permissions. Legacy plaintext caches are treated as a miss and removed. Anyone with the age key can decrypt them, same as `credentials.toml`.
- **ARGV bypass in debug builds** — `ENVFORGE_UNSAFE_ARGV=*` disables secret detection in debug builds only. This path is audited at `Critical` severity and rejected in release builds. The old `ENVFORGE_UNSAFE_ARGV=1` format is blocked entirely.

### Encryption Details

- Algorithm: X25519 (via `age` crate, `plugin` feature disabled)
- Key storage: `~/.config/envforge/age.key` with `0600` permissions (auto-corrected if permissive). Override via `ENVFORGE_AGE_KEY` (inline) or `ENVFORGE_AGE_KEY_FILE` (custom path).
- Recovery key: `~/.config/envforge/age-recovery.key` — generated on first run, store offline.
- Encrypted format: `ENC[age:base64data]` stored in shell files, credentials, and the secrets cache (`value` field)
- Key generation: Automatic on first `encrypt` command; auto-generated keypair with explicit permission hardening
- Credential encryption: Mandatory by construction for all providers except those with explicit `NotSupported` justification
- RUSTSEC-2024-0433 mitigation: `age` crate compiled without `plugin` feature; arbitrary code execution vector eliminated

### File Permissions

| File | Permissions | Contents |
|------|------------|----------|
| `~/.config/envforge/config.toml` | User default | Configuration (no secrets) |
| `~/.config/envforge/age.key` | `0600` | Age secret key (primary) |
| `~/.config/envforge/age-recovery.key` | `0600` | Age secret key (recovery — store offline) |
| `~/.config/envforge/credentials.toml` | `0600` | Encrypted provider credentials |
| `~/.config/envforge/secrets-cache/` | `0600` per file | Age-encrypted cached secret values (TTL-based) |
| `~/.config/envforge/backups/` | User default | File backups |
| `~/.config/envforge/changelog.log` | User default | Change log (values masked) |

## Supported Versions

| Version | Supported |
|---------|-----------|
| 1.0.x   | ✅ Yes    |
| 0.8.x   | ✅ Yes    |
| < 0.8   | ❌ No     |


## Dependencies

EnvForge automatically audits dependencies daily using `cargo audit` and `cargo deny` via GitHub Actions. Key security-relevant dependencies:

- `age` — Encryption (well-audited, widely used, plugin feature disabled)
- `sha2` — File integrity hashing
- `tempfile` — Atomic write operations
- `serde_norway` — YAML serialization (replaces deprecated `serde_yaml`)

## Automated Security Scanning

We employ several automated tools to maintain a high security standard:

- **Cargo Audit**: Checks for known vulnerabilities in our dependency tree daily.
- **Cargo Deny**: Enforces license compliance and bans problematic crates.
- **Dependabot**: Automatically manages dependency updates to keep us on the latest secure versions.
- **CodeQL**: Performs static analysis to detect potential security vulnerabilities in the codebase.
- **CLI Binary Audit**: Weekly CI check verifying installed versions of the 13 provider CLI binaries against minimum requirements.

## Secret Provider CLI Requirements

All 13 secret providers use external CLI binaries. EnvForge passes credentials via environment variables or stdin (never CLI flags) to prevent credential leakage via `/proc/PID/cmdline`.

### Minimum CLI Versions

| Provider | Binary | Minimum Version | Security Notes |
|----------|--------|----------------|----------------|
| HashiCorp Vault | `vault` | 1.15.0 | [Advisories](https://www.hashicorp.com/security/vulnerabilities) |
| AWS SSM | `aws` | 2.13.0 | [Advisories](https://aws.amazon.com/security/security-bulletins/) |
| Azure Key Vault | `az` | 2.50.0 | [Advisories](https://msrc.microsoft.com/) |
| GCP Secret Manager | `gcloud` | 450.0.0 | [Advisories](https://cloud.google.com/sdk/docs/release-notes) |
| 1Password | `op` | 2.25.0 | [Advisories](https://developer.1password.com/docs/cli/release-notes/) |
| Doppler | `doppler` | 3.50.0 | [Docs](https://docs.doppler.com/docs/cli-changelog) |
| Infisical | `infisical` | 0.14.0 | [Releases](https://github.com/Infisical/infisical-cli/releases) |
| Akeyless | `akeyless` | 1.50.0 | [Docs](https://docs.akeyless.io/docs/cli) |
| Bitwarden | `bws` | 0.10.0 | [Releases](https://github.com/bitwarden/sdk/releases) |
| CyberArk Conjur | `conjur` | 1.0.0 | [Releases](https://github.com/cyberark/conjur-cli-go/releases) |
| Keeper | `ksm` | 1.0.0 | [Docs](https://docs.keeper.io/secrets-manager/secrets-manager-cli) |
| Mozilla SOPS | `sops` | 3.8.0 | [Releases](https://github.com/getsops/sops/releases) |
| pass/gopass | `pass`/`gopass` | 1.7.0 | [Releases](https://www.passwordstore.org/) |

### Credential Passing Methods

| Provider | Method | Environment Variables |
|----------|--------|----------------------|
| HashiCorp Vault | Env vars | `VAULT_ADDR`, `VAULT_TOKEN` |
| AWS SSM | Env vars | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_PROFILE`, `AWS_DEFAULT_REGION` |
| Azure Key Vault | Azure CLI auth | (managed by `az login`) |
| GCP Secret Manager | gcloud auth | (managed by `gcloud auth`) |
| 1Password | Env var | `OP_SERVICE_ACCOUNT_TOKEN` |
| Doppler | Env var | `DOPPLER_TOKEN` |
| Infisical | Env var | `INFISICAL_TOKEN` |
| Akeyless | Env var | `AKEYLESS_ACCESS_ID`, `AKEYLESS_ACCESS_KEY` |
| Bitwarden | Env var | `BWS_ACCESS_TOKEN` |
| CyberArk Conjur | Stdin pipe | `CONJUR_APPLIANCE_URL`, `CONJUR_ACCOUNT`, `CONJUR_AUTHN_LOGIN`, `CONJUR_AUTHN_API_KEY` |
| Keeper | Config file | (managed by `ksm`) |
| Mozilla SOPS | Env var | `SOPS_AGE_KEY_FILE` |
| pass/gopass | Env var | `PASSWORD_STORE_DIR` |
