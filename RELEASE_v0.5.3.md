# EnvForge v0.5.3 — AI Safety Suite

The most comprehensive AI-agent secret protection in any CLI tool. **18 tools** across 5 security layers.

## Why This Matters

- GitGuardian 2026: AI-assisted commits leak secrets at **2x baseline rate** (3.2% vs 1.5%)
- 24,000+ credentials found hardcoded in MCP configuration files
- Claude Code automatically reads `.env` files without user awareness
- SANDWORM_MODE attack: 19 npm packages installed rogue MCP servers to steal secrets
- **64% of secrets leaked in 2022 are still valid today** — remediation is failing at scale

EnvForge is the first env management tool built to protect secrets FROM AI agents.

## Quick Setup: 30 Seconds to Safety

```bash
envforge fence                              # Block AI tools from reading secrets
envforge schema emit-ai --infer -o .env.ai.md  # Give AI context without values
envforge scan --install-hook                # Block secret commits
envforge scan --mcp && envforge mcp harden  # Fix AI tool configs
envforge ai-hook install claude-code        # Install 3-stage guard hooks
```

## The 18-Tool AI Safety Suite

### Layer 1: Prevention (5 tools)

**Secret Fence** — One command, all AI tools protected:
```bash
envforge fence
```
Creates: `.envforgeignore`, `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json`

**3-Stage AI Guard** — Pre-tool + post-tool scanning:
```bash
# Installed via ai-hook, invoked automatically by Claude Code/Cursor
envforge ai-guard pre-tool Read ".env.production"
# ⚠ EnvForge: AI agent accessing sensitive file: .env.production
```
- **PreToolUse**: Alerts on sensitive file access (`.env`, `.pem`, `.ssh/`, `.aws/`), detects secrets in Bash commands
- **PostToolUse**: Scans tool output for leaked secrets
- Safe file exclusions: `.env.schema`, `.env.example`, `.env.ai.md`

**AI Coding Tool Hooks** — Install guard into Claude Code and Cursor:
```bash
envforge ai-hook install claude-code   # PreToolUse + PostToolUse hooks
envforge ai-hook install cursor        # Security rules
envforge ai-hook remove claude-code    # Clean removal
```

**Pre-Commit Hook** — Block commits containing secrets:
```bash
envforge scan --install-hook
```

### Layer 2: Runtime Protection (5 tools)

**Volatile Mode** — Secrets never touch disk:
```bash
envforge run --volatile -- npm start
```
Forces in-memory resolution. Ignores `.env` disk files. Masks values in dry-run.

**Log Redaction** — Real-time output masking:
```bash
envforge run --redact -- npm start
# stdout: Connecting to [REDACTED:DB_PASSWORD]@host...
```

**Credential Proxy** — HTTP API with access control:
```bash
envforge proxy --port 8100 --keys DB_URL,API_KEY --allow-origins localhost --require-lease
```
- `GET /env` — all variables (JSON)
- `GET /env/KEY` — single variable
- Domain allowlist (default: localhost only)
- Lease enforcement per request
- Full audit trail (JSONL)

**Session Leases** — Time-bounded secret access:
```bash
envforge lease create --ttl 1h --keys DB_URL,API_KEY --name dev-session
envforge lease list
envforge lease cleanup
```
Secrets auto-expire. Proxy checks lease validity per request. Duration formats: `30m`, `1h`, `8h`, `24h`, `7d`.

**Killswitch** — Emergency revocation:
```bash
envforge revoke --all
# 🔴 KILLSWITCH: 3 lease(s) revoked.
```
Instantly invalidates all active leases. Zero-delay revocation.

### Layer 3: Context Control (3 tools)

**AI-Safe Schema** — Types and descriptions, never values:
```bash
envforge schema emit-ai --infer --output .env.ai.md
```
```markdown
## DATABASE_URL
- **Type**: url
- **Required**: yes
- **Description**: PostgreSQL connection string
- **Sensitive**: YES — do not hardcode or log
```
Auto-updates when `envforge set`, `import`, or `secrets pull` modifies variables.

**Safe Export** — Redacted values:
```bash
envforge export --safe
# API_KEY=[REDACTED]
# PORT=3000
```

**Ignore File** — `.envforgeignore` marks files AI tools should skip.

### Layer 4: Remediation (3 tools)

**MCP Scan** — Find credentials in AI tool configs:
```bash
envforge scan --mcp
```
Scans Claude Desktop, Cursor, GitHub Copilot. Detects 23+ API key patterns.

**MCP Harden** — Auto-fix:
```bash
envforge mcp harden
# ✓ ~/.claude/claude_desktop_config.json
#   2 secrets replaced (backup: ...json.bak)
#   → Set: export SLACK_TOKEN=<your-value>
```

**Prompt Sanitizer** — Strip secrets from any file:
```bash
envforge sanitize debug.log --output clean.log
```

### Layer 5: Detection (2 tools)

**AI Leak Audit** — Find secrets in AI-assisted commits:
```bash
envforge audit --ai-leaks
```
Detects commits co-authored by Claude, Copilot, Cursor. Scans diffs for API keys, tokens, connection strings.

**Access Audit** — JSONL proxy access log:
```bash
envforge audit --access
```
Every proxy request logged. Values NEVER recorded — only key names, timestamps, client info.

## Competitive Advantage

| Capability | EnvForge | ggshield | dotenvx AS2 | 1Password | AgentSecrets |
|-----------|----------|----------|-------------|-----------|-------------|
| Pre-tool scanning | ✅ | ✅ | ❌ | ❌ | ❌ |
| Post-tool scanning | ✅ | ✅ | ❌ | ❌ | ❌ |
| Volatile mode (no disk) | ✅ | ❌ | ❌ | ❌ | ❌ |
| Log redaction | ✅ | ❌ | ❌ | ❌ | ✅ |
| Session leases + killswitch | ✅ | ❌ | ❌ | ❌ | ❌ |
| MCP config harden | ✅ | ❌ | ❌ | ❌ | ❌ |
| Credential proxy + audit | ✅ | ❌ | ✅ | ✅ | ✅ |
| Secret fence (multi-tool) | ✅ | ❌ | ❌ | ❌ | ❌ |
| AI-safe schema | ✅ | ❌ | ❌ | ❌ | ❌ |
| Domain allowlist | ✅ | ❌ | ❌ | ❌ | ✅ |
| Env management (full) | ✅ | ❌ | ✅ | ❌ | ❌ |
| 7 secret providers | ✅ | ❌ | ❌ | ✅ | ❌ |
| Open source CLI | ✅ | ✅ | ✅ | ❌ | ✅ |

## Quality

- **664 total tests**, 0 failures
- 18 AI safety tools across 5 layers
- 80+ CLI subcommands
- No new crate dependencies
- Clean compilation

## Upgrade

```bash
cargo install env-forge-tui
```

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for complete details.
