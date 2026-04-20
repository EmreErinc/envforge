# EnvForge v0.5.4 — Advanced AI Safety

22 AI safety tools across 6 security layers. 683 tests. The most comprehensive AI-agent secret protection in any CLI tool.

## What's New in v0.5.4

### Canary Secrets (Honeypot Credentials)

Plant fake credentials that alert when an AI agent exfiltrates them:

```bash
envforge canary create AWS_SECRET_KEY --pattern aws_key
# Canary created: AWS_SECRET_KEY
#   Fake value: AKIANU18FMT07ELSZ6DK
#   Add to your .env — if this value appears anywhere, an agent leaked it.

envforge canary list                   # Show all canaries with trigger status
envforge canary check                  # Check for triggered canaries
envforge canary delete AWS_SECRET_KEY  # Remove a canary
```

Patterns: `aws_key`, `github_token`, `stripe_key`, `slack_token`, `gitlab_token`, `generic`. Integrated into AI guard — canary values in tool output trigger instant alerts.

### Zero-Access Approval Flow

Human must approve each secret access request:

```bash
envforge proxy --port 8100 --require-approval

# When agent requests a secret:
# 🔒 Secret access request: DB_PASSWORD from 127.0.0.1
#    Approve? [y/N]: _
```

Denied requests return 403 and are logged. Combinable with `--require-lease` for layered security.

### Secret Dependency Mapping

Answer: "If I rotate this secret, what breaks?"

```bash
envforge deps DB_PASSWORD --source
```

```
Dependencies for DB_PASSWORD

EnvForge Managed:
  ~/.env_managed.dev:15        export DB_PASSWORD="..."

Project .env Files:
  .env.production:12           DB_PASSWORD=...

Source Code:
  src/db.rs:45                 let pass = env::var("DB_PASSWORD")?;
  src/config.py:12             db_pass = os.environ["DB_PASSWORD"]

Config Files:
  docker-compose.yml:23        - DB_PASSWORD=${DB_PASSWORD}

Total: 5 references across 5 files
```

Scans 9 languages (JS, Python, Rust, Go, Java, Ruby, PHP, C, Shell) + config files (Docker, Terraform, K8s, GitHub Actions).

### External Scanner Hook

Delegate secret detection to ggshield's 500+ detector engine:

```bash
export ENVFORGE_EXTERNAL_SCANNER="ggshield secret scan"
```

AI guard automatically calls the external scanner on tool inputs/outputs. Falls back to built-in pattern matching if not set.

## Complete AI Safety Suite (22 Tools)

| Layer | Tool | Command |
|-------|------|---------|
| **Prevention** | Secret Fence | `envforge fence` |
| **Prevention** | Pre-Commit Hook | `envforge scan --install-hook` |
| **Prevention** | 3-Stage AI Guard | `envforge ai-guard pre-tool\|post-tool` |
| **Prevention** | AI Hooks | `envforge ai-hook install claude-code` |
| **Prevention** | File Alerts | Built into AI guard |
| **Runtime** | Volatile Mode | `envforge run --volatile` |
| **Runtime** | Log Redaction | `envforge run --redact` |
| **Runtime** | Credential Proxy | `envforge proxy --port 8100` |
| **Runtime** | Session Leases | `envforge lease create --ttl 1h` |
| **Runtime** | Killswitch | `envforge revoke --all` |
| **Context** | AI-Safe Schema | `envforge schema emit-ai` |
| **Context** | Safe Export | `envforge export --safe` |
| **Context** | Ignore File | `.envforgeignore` |
| **Remediation** | MCP Scan | `envforge scan --mcp` |
| **Remediation** | MCP Harden | `envforge mcp harden` |
| **Remediation** | Prompt Sanitizer | `envforge sanitize FILE` |
| **Detection** | Canary Secrets | `envforge canary create KEY` |
| **Detection** | AI Leak Audit | `envforge audit --ai-leaks` |
| **Detection** | Access Audit | `envforge audit --access` |
| **Governance** | Approval Flow | `envforge proxy --require-approval` |
| **Governance** | Dependency Map | `envforge deps KEY --source` |
| **Governance** | External Scanner | `ENVFORGE_EXTERNAL_SCANNER=ggshield` |

## Quick Setup

```bash
# 1. Create AI ignore rules for all tools
envforge fence

# 2. Generate AI-safe context file
envforge schema emit-ai --infer --output .env.ai.md

# 3. Install pre-commit hook + AI guard hooks
envforge scan --install-hook
envforge ai-hook install claude-code

# 4. Scan and harden AI tool configs
envforge scan --mcp && envforge mcp harden

# 5. Plant canary secrets
envforge canary create INTERNAL_API_KEY --pattern generic

# 6. Run with maximum protection
envforge lease create --ttl 8h
envforge proxy --port 8100 --require-lease --require-approval
```

## Competitive Landscape

| Capability | EnvForge | ggshield | dotenvx AS2 | 1Password | AgentSecrets | AgentKey |
|-----------|----------|----------|-------------|-----------|-------------|----------|
| Pre-tool scanning | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Post-tool scanning | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Canary secrets | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Approval flow | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Session leases + killswitch | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Dependency mapping | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Volatile mode (no disk) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Log redaction | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| MCP config harden | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Secret fence (multi-tool) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| External scanner hook | ✅ | N/A | ❌ | ❌ | ❌ | ❌ |
| Domain allowlist | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| Full env management | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ |
| 7 secret providers | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Open source CLI | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |

## Stats

- **683 tests**, 0 failures
- **22 AI safety tools** across 6 layers
- **90+ CLI subcommands**
- **41 features** implemented in this release cycle
- No new crate dependencies

## Version History

| Version | Highlights |
|---------|-----------|
| v0.5.4 | Canary secrets, approval flow, dependency mapping, external scanner |
| v0.5.3 | 3-stage AI guard, session leases, killswitch, proxy audit |
| v0.5.2 | MCP harden, fence, sanitizer, AI hooks, credential proxy |
| v0.5.1 | AI schema emit, MCP scan, docker-secrets, log redaction, URI refs |
| v0.5.0 | Pre-commit hook, shell hook, volatile, share, rotate propagate, audit |
| v0.4.3 | Unified check, encrypted sync, snapshots, explain, rotation |
| v0.4.2 | Multi-format export, secret age, provider diff |
| v0.4.1 | GitHub Action (5 modes) |

## Upgrade

```bash
cargo install env-forge-tui
```
