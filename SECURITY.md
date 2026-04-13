# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in EnvForge, please report it responsibly:

1. **Do NOT** open a public GitHub issue
2. Email: security@envforge.dev (or use GitHub Security Advisories)
3. Include: description, reproduction steps, potential impact
4. We aim to respond within 48 hours

## Security Model

### What EnvForge Protects

- **File integrity** — Atomic writes prevent corruption. SHA-256 hash verification.
- **No data loss** — Soft-delete only. Original content preserved as comments.
- **Secret masking** — Sensitive values never displayed in plain text by default.
- **Encryption at rest** — Optional age encryption for sensitive values.

### What EnvForge Does NOT Protect

- **Runtime memory** — Decrypted values exist in memory during session
- **Terminal history** — CLI commands with values may appear in shell history
- **Clipboard** — Copied values are in system clipboard (not cleared automatically)
- **Age key security** — The age key file (`~/.config/envforge/age.key`) is protected by file permissions only

### Encryption Details

- Algorithm: X25519 (via `age` crate)
- Key storage: `~/.config/envforge/age.key` with `0600` permissions
- Encrypted format: `ENC[age:base64data]` stored in shell files
- Key generation: Automatic on first `encrypt` command

### File Permissions

| File | Permissions | Contents |
|------|------------|----------|
| `~/.config/envforge/config.toml` | User default | Configuration (no secrets) |
| `~/.config/envforge/age.key` | `0600` | Age secret key |
| `~/.config/envforge/backups/` | User default | File backups |
| `~/.config/envforge/changelog.log` | User default | Change log (values masked) |

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x | Yes |

## Dependencies

EnvForge audits dependencies with `cargo audit`. Key security-relevant dependencies:

- `age` — Encryption (well-audited, widely used)
- `sha2` — File integrity hashing
- `tempfile` — Atomic write operations
