# EnvForge Security Model

## Threat Model (STRIDE)

### Trust Boundaries

```
┌─────────────────────────────────────────────────────┐
│                  System Boundary                     │
│  ┌──────────┐     ┌──────────┐     ┌──────────────┐ │
│  │ IDE/LSP   │────▶│ envforge  │────▶│ Provider CLI │ │
│  │ (editor)  │     │ process   │     │ (13 binaries)│ │
│  └──────────┘     └──────────┘     └──────────────┘ │
│                         │                            │
│                    ┌────▼─────┐                       │
│                    │ Filesystem│                      │
│                    │ (~/.config│                      │
│                    │  /envforge│                      │
│                    │ )         │                      │
│                    └──────────┘                       │
└─────────────────────────────────────────────────────┘
```

**Boundary 1: IDE/LSP → envforge process**
- Transport: stdin/stdout (tower-lsp, no network socket)
- Controls: rate limiting, path containment, fence enforcement, input validation
- Threat: Malicious editor/plugin issues executeCommand for `reveal.value` or `fence.disable`

**Boundary 2: envforge process → Provider CLI binaries**
- Transport: Subprocess execution (stdin/tempfile for secrets)
- Controls: Environment clearing (PATH/HOME/USER only), binary path resolution, stderr redaction
- Threat: Trojaned provider binary at PATH, PATH hijacking

**Boundary 3: envforge process → Filesystem**
- Storage: `~/.config/envforge/credentials.toml` (age-encrypted, 0600)
- Storage: `~/.config/envforge/audit.jsonl` (append-only, 0600, 10MB rotation)
- Storage: `~/.config/envforge/age.key` (0600, O_NOFOLLOW read)
- Threat: Disk access by same-uid process, backup exfiltration

---

### S — Spoofing

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| Fake provider binary on PATH | High | `resolve_binary_path()` → `which` → `canonicalize` → `is_file`. SHA-256 hash pinning at load time. `verify_gpg_signature()` validates GOODSIG + fingerprint at registration time. | Partial |
| Malicious LSP client impersonating editor | Medium | stdin/stdout transport only; no network listener. | **Mitigated** |
| Fake fence files injected by attacker | Low | Fence uses marker detection; removal preserves user content. | **Mitigated** |

### T — Tampering

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| Audit log modification | High | 0600 permissions, append-only writes, 10MB rotation, SHA-256 hash chain with `verify_integrity()` (tamper.rs). | **Mitigated** |
| Credential store corruption | Medium | `save_store()` uses tempfile + atomic rename. Hash verification on write. | **Mitigated** |
| Sync snapshot tampering | Medium | age-encrypted snapshots, `SyncEncryptionPolicy` (Mandatory sum type, no runtime bool toggle) blocks downgrade attacks. | **Mitigated** |
| Fence file removal | Medium | Auto-recreated on LSP init. Fence writes ignore/rules for the tools in the [integration matrix](docs/integration-matrix.md) (Cursor, Copilot, Claude Code, Windsurf, Cline, Aider, Gemini CLI, Amazon Q, AGENTS.md, EnvForge). Continue is not a fence target. | **Mitigated** |

### R — Repudiation

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| "I didn't access that secret" | High | All `RuntimeEvent`s written to `audit.jsonl`: credential ops, reveal, decrypt, fence changes, provider calls, canary triggers. | **Mitigated** |
| "I didn't run that command" | Medium | Audit entries include timestamp and operation type. Missing PID/cgroup for OS correlation. | Partial |
| Log entry deletion by attacker | High | Append-only file. No cryptographic chain of custody. | Partial |

### I — Information Disclosure

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| Secrets in shell history (.zsh_history/.bash_history) | Critical | CLI blocks positional secret args. Provider subprocesses get `HISTFILE=/dev/null`. | **Mitigated** |
| Secrets in /proc/PID/cmdline (argv leakage) | Critical | CLI blocks positional secret args. Provider secrets piped via stdin/tempfile. | **Mitigated** |
| Secrets in core dumps | High | `setrlimit(RLIMIT_CORE, 0)` at process start. | **Mitigated** |
| Secrets in swap/pagefile | Medium | Credential `VolatileMode` defaults to On (5-min TTL) for in-memory provider creds. That is not `envforge run --volatile`, which you must pass explicitly. Secret values zeroized after use. `CredentialEncryptionPolicy::Mandatory` keeps creds encrypted at rest. No `mlock`/`MADV_DONTDUMP` yet. | Partial |
| LSP autocomplete leaking secret values | High | Sensitive vars show "(sensitive)" in completions and hover. | **Mitigated** |
| LSP methods exposing secret key names | Medium | `redact_secrets_in_message()` centralized utility available to all LSP handlers. Sorts secrets by length descending, skips sub-8-char strings. | Partial |
| Clipboard leakage when copying secrets | Low | `ClipboardConfig` with `warn_on_secret` and `enabled` toggle. | **Mitigated** |
| Provider stderr containing credentials | Medium | `sanitize_error_output()` redacts credential keywords in error output. | **Mitigated** |
| Canary token values in alert output | Low | Canary triggers redact alert message fields. | **Mitigated** |

### D — Denial of Service

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| Document size bomb via LSP | Medium | 1 MiB cap on `didOpen`/`didChange`, 10 MiB on shell file parse. | **Mitigated** |
| LSP document flood | Medium | `max_tracked_documents = 256`, oldest evicted. | **Mitigated** |
| Rate-limit abuse (rapid-firing commands) | Medium | Token bucket per LSP handler; workspace-scoped reveal/fence limits. | **Mitigated** |
| Credential store write race | Low | `save_store()` uses tempfile + atomic rename. Concurrent writers may lose updates. | Partial |
| Audit log disk exhaustion | Low | 10MB rotation, old file kept as `.jsonl.old`. | **Mitigated** |

### E — Elevation of Privilege

| Threat | Risk | Mitigation | Status |
|--------|------|------------|--------|
| same-uid process reading credential file | Low | 0600 permissions. `load_store()` warns and fixes bad perms. | **Mitigated** |
| Root/sudo bypassing all controls | Accepted | Filesystem permissions can't defend against root. Defense-in-depth via encryption. | Open |
| `sudo envforge` leaking creds to root's history | Low | Same CLI argv blocking applies regardless of user. | **Mitigated** |

---

## What EnvForge Protects Against

1. **AI tools reading .env files** (Fence: ignore/deny rules for Claude, Cursor, Copilot)
2. **AI prompt injection leaking secrets** (AI Guard: pre/post-tool scanning with adversarial hardening)
3. **Secret exfiltration by AI agents** (Canary tokens: honey-token detection with forensic payloads)
4. **Shell history exposure** (Argv blocking + HISTFILE suppression in provider subprocesses)
5. **Process listing exposure** (Secrets piped via stdin/tempfile, not argv)
6. **Core dump exposure** (Core dumps disabled at process start)
7. **Credential storage theft** (age-encrypted at rest, 0600, zeroize on drop — mandatory by construction via `CredentialEncryptionPolicy`)
8. **Sync interception** (age-encrypted snapshots, `SyncEncryptionPolicy` sum-type downgrade prevention)
9. **MCP config credential leaks** (Inline diagnostics for hardcoded tokens)
10. **Long-lived credential risk** (credential `VolatileMode` defaults to On with a 5-min TTL; `envforge run --volatile` is still opt-in)

## What EnvForge Does NOT Protect Against

1. **Kernel-level attacks** (root, kernel modules, physical memory access)
2. **Hardware attacks** (DMA, cold-boot, JTAG)
3. **Compromised provider binaries** (trojaned `aws`, `vault` binaries — partial mitigation with hash verification)
4. **Side-channel attacks** (timing, power analysis — not in threat model scope)
5. **Compromised editor extensions** (malicious LSP client with same-uid access can send executeCommand)
6. **Swap/pagefile forensics** (no `mlock` — secrets may page to disk; mitigated by credential `VolatileMode` default On and `CredentialEncryptionPolicy::Mandatory`. `envforge run --volatile` is a separate opt-in flag.)
7. **CI/CD pipeline attacks using `ENVFORGE_UNSAFE_ARGV`** — debug-only, per-provider gated, `Critical` audited
8. **Social engineering** (phishing, shoulder-surfing terminal output)
9. **Shell built-in history (`fc -l` in zsh) if attacker has shell access**
10. **Provider credential weakness** (weak/expired keys detected via `provider_audit()` and TTL; enforcement via `CredentialEncryptionPolicy::Mandatory`)

## Residual Risks (Accepted)

1. **Fence is tool-specific.** Coverage follows the [integration matrix](docs/integration-matrix.md). Tools not in the registry must be configured manually. Continue is deferred (not a fence target). Mitigation: document the matrix; expand on request.
2. **Audit log integrity is hash-chain-verified, not HMAC.** SHA-256 chain via `tamper.rs::verify_integrity()`. Root can still tamper (physics). (Resolved: was "no cryptographic chain.")
3. **Provider credential encryption is now mandatory by construction.** `CredentialEncryptionPolicy::Mandatory` is the only non-deprecated construction path for all 13 providers. `NotSupported` requires explicit justification, reviewer name, and auto-re-evaluation timer. (Resolved: was `[enc:0/3]` reporting-only.)
4. **LSP redaction is centralized, not per-method.** `redact_secrets_in_message()` utility available to all LSP handlers. Individual handlers must opt in to calling it. Mitigation: wire to hover/completion/definition paths in first patch.
5. **`ENVFORGE_UNSAFE_ARGV` bypass.** Now gated per-provider (`ENVFORGE_UNSAFE_ARGV=vault,aws-ssm`) or all (`ENVFORGE_UNSAFE_ARGV=*`). Only available in debug builds. Old `=1` format rejected. Audited at `Critical` severity. Documented as unsafe.

## Vulnerability Reporting

Report security vulnerabilities to the project maintainer. Do not open public issues.
Responsible disclosure process:
1. Contact: [GitHub issues](https://github.com/emreerinc/envforge/issues) (use a private security advisory if the report should not be public)
2. Expected response: within 48 hours
3. Disclosure timeline: coordinated after fix is shipped and users have upgrade window

---

*Last updated: 2026-09-01. Covers EnvForge v1.0.2.*
