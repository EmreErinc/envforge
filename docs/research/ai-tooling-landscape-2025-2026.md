# AI Coding Tooling Landscape (2025–2026) — EnvForge Feature-Planning Research

Research date: 2026-06-19. Feeds EnvForge PRD (AI-fence, MCP linter, AI-guard, canary, heatmap).

Sourcing legend: **VERIFIED** = official docs / official disclosure / tier-1 reporting · **ESTIMATE** = third-party tracker / secondary · **UNVERIFIED / RUMOR** = single weak source or pre-close talk · **❌** = does not exist · **⚠️** = deprecated / caveated.

---

## 1. AI Coding Tools — Ignore & Rules File Conventions

Legend per cell: ✅ verified against official docs · ⚠️ unverified/deprecated · ❌ does not exist.

| Tool | Ignore file (exact path) | Rules / instructions file (exact path) |
|---|---|---|
| **Cursor** | `.cursorignore` ✅ (blocks AI access + index) · `.cursorindexingignore` ✅ (index-only) — both repo root | `.cursor/rules/*.mdc` ✅ (nested dirs ok) · `.cursorrules` ⚠️ deprecated but works · `AGENTS.md` ✅ |
| **GitHub Copilot** | ❌ No ignore file. "Content exclusion" ✅ = repo/org/enterprise UI setting (Business/Enterprise only) | `.github/copilot-instructions.md` ✅ · `.github/instructions/*.instructions.md` ✅ (`applyTo` glob) · `AGENTS.md` ✅ |
| **Windsurf / Codeium** | `.codeiumignore` ✅ (root) · `~/.codeium/.codeiumignore` ✅ (global) · `.windsurfignore` ❌ (not real) | `.windsurf/rules/*.md` ✅ · global `~/.codeium/windsurf/memories/global_rules.md` ✅ · `AGENTS.md` ✅ · `.windsurfrules` ⚠️ legacy |
| **Cline** | `.clineignore` ✅ (root) | `.clinerules` file OR `.clinerules/` dir ✅ (both valid) · global `~/Documents/Cline/Rules` · `AGENTS.md` ✅ |
| **Roo Code** | `.rooignore` ✅ (root) | `.roo/rules/` dir ✅ preferred · `.roorules` ⚠️ legacy fallback · global `~/.roo/rules/` · `AGENTS.md` ✅ |
| **Aider** | `.aiderignore` ✅ (repo root) | `.aider.conf.yml` ✅ (**`.yml` only**) · `CONVENTIONS.md` ✅ but NOT auto-loaded (wire via `read:`/`--read`) · `AGENTS.md` ✅ opt-in |
| **Continue.dev** | `.continueignore` ✅ (root) · `~/.continue/.continueignore` ✅ (global) | `.continue/rules/*.md` ✅ OR `rules` blocks in `config.yaml`; config at `~/.continue/config.yaml` · `.continuerc.json` = config override (not rules) · AGENTS.md ⚠️ not confirmed native |
| **Claude Code** | ❌ No `.claudeignore`. Exclusion via `permissions.deny` `Read(...)` in `.claude/settings.json` ✅ | `CLAUDE.md` ✅ · `CLAUDE.local.md` ✅ · `~/.claude/CLAUDE.md` ✅ · `AGENTS.md` ✅ |
| **Gemini CLI** | `.geminiignore` ✅ (root, +`.gitignore`) | `GEMINI.md` ✅ · `~/.gemini/GEMINI.md` ✅ · AGENTS.md opt-in via `context.fileName` |
| **Gemini Code Assist** (IDE) | `.aiexclude` ✅ (overrides `.gitignore` on conflict) | (no `.md` convention in scope) |
| **Amazon Q Developer** | ⚠️ none documented (`.amazonqignore` not found — treat as nonexistent) | `.amazonq/rules/*.md` ✅ |
| **OpenAI Codex CLI** | ❌ `.codexignore` absent (open feature request) | `AGENTS.md` ✅ (root+nested, `~/.codex/AGENTS.md`) · config `~/.codex/config.toml` ✅ |
| **Tabnine** | `.tabnineignore` ✅ (+`.ignore`,`.gitignore`; local index only) | `.tabnine/guidelines/*.md` ✅ |
| **Zed** | ⚠️ No AI-specific ignore — `.gitignore` + `file_scan_exclusions`/`private_files` settings (known leak issues) | First-match chain: `.rules` → `.cursorrules` → `.windsurfrules` → `.clinerules` → `.github/copilot-instructions.md` → `AGENT.md` → `AGENTS.md` → `CLAUDE.md` → `GEMINI.md` ✅ (`AGENTS.md` canonical post-v1.4.0) |
| **JetBrains AI Assistant / Junie** | `.aiignore` ✅ (project root; also honors `.gitignore`, `.cursorignore`) | `.junie/guidelines.md` ✅ (Junie) · `AGENTS.md` ✅ |
| **Sourcegraph Cody** | `.cody/ignore` ⚠️ SUNSET/removed. Current: `cody.contextFilters` ✅ = server-side site config (repo-name regex), NOT a repo file | ❌ none confirmed |
| **Augment Code** | `.augmentignore` ✅ (workspace root) | `.augment/rules/*.md` ✅ · legacy `.augment-guidelines` ✅ · `AGENTS.md`/`CLAUDE.md` |

### AGENTS.md cross-tool standard
- Canonical home: https://agents.md (repo github.com/openai/agents.md). Plain Markdown, no schema.
- Governance now **Linux Foundation / Agentic AI Foundation** — not solely OpenAI.
- Path: `AGENTS.md` at repo root; nested per-directory files allowed, closest-file-wins.
- **Verified adopters:** Codex CLI, Copilot coding agent, Cursor, Gemini CLI (opt-in), Jules, Aider (opt-in), Zed, Factory/Droid, Roo Code, Cline, Amp, Warp, Windsurf, Augment, VS Code, Junie.
- **AGENTS.md is rules-only** — there is no "agents ignore" file in the spec (`.agentignore` is only a community proposal).

### Flags for the PRD
- **No ignore file exists for:** GitHub Copilot (UI setting), Claude Code (`permissions.deny`), Amazon Q (none found), OpenAI Codex (requested, unimplemented), Zed (settings/`.gitignore`).
- **Misnomers to avoid:** `.windsurfignore` (use `.codeiumignore`), `.claudeignore` (use settings deny).
- **Deprecated-but-functional:** `.cursorrules`, `.windsurfrules`, `.roorules`.
- **Removed:** Cody `.cody/ignore`.
- **Extension gotchas:** Aider config is `.yml` not `.yaml`; `CONVENTIONS.md` not auto-loaded.

---

## 2. Model Context Protocol (MCP) — State, Config Paths, EnvForge Opportunity

### 2.1 Client support (all VERIFIED unless noted)
| Client | MCP? | Notes / Source |
|---|---|---|
| Claude Desktop | Yes | Reference client; local stdio. modelcontextprotocol.io/docs/develop/connect-local-servers |
| Claude Code | Yes | 3 scopes (local/project/user); `claude mcp add`. code.claude.com/docs/en/mcp |
| Cursor | Yes | Project + global. cursor.com/docs/mcp |
| VS Code / Copilot | Yes — **GA in 1.102, Jul 2025** | github.blog/changelog/2025-07-14-...; code.visualstudio.com/docs/agent-customization/mcp-servers |
| Windsurf (Cascade) | Yes | `mcp_config.json`. docs.windsurf.com/windsurf/cascade/mcp |
| Cline | Yes | `cline_mcp_settings.json` (VS Code globalStorage) |
| Zed | Partial / ACP-first | Zed pushing **ACP** (Agent Client Protocol) w/ JetBrains; MCP still used underneath. zed.dev/acp |
| JetBrains (IntelliJ) | Yes | Bundled MCP Server plugin since 2025.2; also ACP partner. jetbrains.com/help/idea/mcp-server.html |
| OpenAI | Yes | Agents SDK (stdio/SSE/Streamable HTTP); ChatGPT connectors; Apps SDK |
| Google Gemini | Yes | Gemini CLI MCP; official MCP endpoints for Google services |

**Takeaway:** MCP is cross-vendor (Anthropic, OpenAI, Google, Microsoft/GitHub, JetBrains). Zed/JetBrains additionally back **ACP** — a sibling agent↔editor protocol, not an MCP replacement.

### 2.2 EXACT config file locations
- **Claude Desktop** `claude_desktop_config.json`
  - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
  - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
  - Linux: `~/.config/Claude/...` ⚠️ UNVERIFIED (no official Linux build; third-party guides only)
- **Claude Code** — project `.mcp.json` (repo root, committable); local/user in `~/.claude.json`. `claude mcp add [--scope local|project|user]`
- **Cursor** — project `.cursor/mcp.json`; global `~/.cursor/mcp.json` (project wins)
- **VS Code** — workspace `.vscode/mcp.json` (GA since 1.102 / Jul 2025)
- **Windsurf** — `mcp_config.json`; macOS/Linux `~/.codeium/windsurf/mcp_config.json`, Windows `%USERPROFILE%\.codeium\windsurf\mcp_config.json` ⚠️ exact strings from third-party guide, verify
- **Cline** — `cline_mcp_settings.json` at `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/...` ⚠️ UNVERIFIED, varies by VS Code variant/OS

> VS Code, Cursor, Claude Code share the same conceptual `mcp.json` schema (`mcpServers`/`servers` map). Template per-client.

### 2.3 Spec status & governance
- **Latest released revision: `2025-11-25`** (one-year anniversary). blog.modelcontextprotocol.io/posts/2025-11-25-first-mcp-anniversary/
- A **`2026-07-28` release candidate** exists (next version in progress).
- **Governance:** Anthropic donated MCP to the **Agentic AI Foundation (AAIF)** under the **Linux Foundation** (co-founded w/ Block + OpenAI; backed by Google/Microsoft/AWS/Cloudflare). anthropic.com/news/donating-the-model-context-protocol... ⚠️ exact day unverified.
- **Transport:** Streamable HTTP (introduced `2025-03-26`) replaced HTTP+SSE; SSE deprecated.
- **Auth:** `2025-06-18` made OAuth 2.1 baseline — PKCE mandatory, RFC 9728 PRM, RFC 8414 AS metadata, **RFC 8707 Resource Indicators** (token bound to a specific server). ⚠️ verify exact RFC list against official auth spec.
- **Tasks:** `2025-11-25` added async `Tasks` abstraction for long-running ops.
- **Registry:** official MCP Registry preview launched **Sept 8 2025** at registry.modelcontextprotocol.io.

### 2.4 Security posture (directly relevant to EnvForge)
- **OWASP MCP Top 10 (2025)** — canonical: owasp.org/www-project-mcp-top-10/
  - **MCP01 Token Mismanagement & Secret Exposure** — secrets in config/env/prompt templates persisting in model memory. Maps directly to EnvForge threat model.
  - **MCP03 Tool Poisoning** — malicious instructions in tool descriptions (rug pulls, schema poisoning, tool shadowing).
  - **MCP09 Shadow MCP Servers**; **MCP10 Context Injection & Over-Sharing**.
- **Tool Poisoning Attack (TPA)** — Invariant Labs, Apr 2025. simonwillison.net/2025/Apr/9/mcp-prompt-injection/
- Defensive tooling: `MCP-Scan` (Invariant Labs); OWASP MCP Security Cheat Sheet.
- **PRD note:** EnvForge's existing "MCP scan" + fence map cleanly to OWASP MCP01/MCP03 — use those category names. Any EnvForge-published MCP server should target Streamable HTTP + OAuth 2.1 resource indicators.

### 2.5 Pattern: ship your own MCP server (recognized, established)
Vendors ship a first-party MCP server exposing a curated, **safe** surface instead of handing agents raw keys/shell:
- **HashiCorp Vault** — official MCP server (stdio + Streamable HTTP; KV/PKI). developer.hashicorp.com/vault/docs/mcp-server/overview
- **Bitwarden** — official MCP server (30+ tools) ⚠️ counts secondary.
- **Infisical** — official MCP server ⚠️ secondary source.
- 1Password/Doppler ⚠️ community/dormant, unverified.

**PRD implication:** An `envforge mcp` server exposing safe, redacted ops (list var **names**, schema-compliance check — never raw values) is squarely in line with Vault/Bitwarden positioning. **Differentiator:** redaction-by-default + canary/fence semantics enforced at the MCP boundary.

---

## 3. Editors / IDEs — LSP Support & Extension UI Capability

Key question for EnvForge (LSP server + custom UI: status-bar secret indicators, gutter decorations for fenced/volatile vars, side panel): does the host API allow that UI, or is it LSP-only?

| Editor | LSP client | Extension API (2025/26) | Custom UI (status bar / gutter / panel)? | EnvForge signal |
|---|---|---|---|---|
| **VS Code** | Full (reference) | Mature, stable | Yes — status bar, `setDecorations` gutter/inline, TreeView, webviews | **First-party plugin** (already shipped) |
| **Visual Studio** | Yes | Mature VSIX/SDK | Yes — tool windows, margins/glyphs, status bar | Viable, lower priority (Windows/.NET) |
| **Neovim** | Built-in `vim.lsp` | Mature Lua ecosystem | Yes — statusline, `sign_column`, extmarks/virtual text, floats | **Plugin warranted** (strong CLI-user overlap) |
| **Emacs** | `eglot` (built-in) + `lsp-mode` | Very mature Elisp | Yes — mode-line, fringe, side windows, overlays | Plugin warranted |
| **Sublime Text** | `sublimelsp/LSP` (mature) | Stable Python API | Yes — status bar, phantoms, gutter; more limited | Feasible, medium effort |
| **Zed** | Built-in | WASM (Rust, `wasm32-wasip2`) | **No** — only languages/themes/icons/snippets/debuggers/MCP. Visual Extension API is **Draft RFC only** (#53403) | **LSP-only**; better play = MCP server |
| **Helix** | Built-in (first-class) | Plugin system **pre-stable** (Steel/Scheme source-build only) | Immature/unknown | **LSP-only** |
| **Lapce** | Built-in | WASI plugins (Rust/C/AS), small ecosystem | Partial; plugins LSP/proxy-oriented | **LSP-only**, low priority |
| **JetBrains Fleet** | n/a | **DISCONTINUED — no downloads after Dec 22, 2025**; evolved into "Air" | n/a | **Do NOT ship** (dead) |

### Key findings
1. **Fleet is dead** (Dec 22 2025 → "Air"). Drop Fleet plans. Existing IntelliJ Platform plugin unaffected.
2. **Zed cannot do custom UI today** — WASM API restricted; status bar/gutter/panels are an unshipped Draft RFC. Zed = LSP-only; the MCP-server angle is the better Zed integration (Zed supports MCP server extensions natively).
3. **Helix & Lapce = LSP-only** — no stable UI plugin target.
4. **First-party plugin warranted (rich UI achievable):** VS Code (have it), Visual Studio, Neovim (Lua), Emacs (Elisp), Sublime (Python). Neovim + Emacs have strongest CLI-tool-user overlap.
5. **The LSP server is the highest-leverage investment** — sole integration surface for Zed/Helix/Lapce, strong baseline everywhere.

Sources: zed.dev/docs/extensions/developing-extensions · github.com/zed-industries/zed/discussions/53403 · blog.jetbrains.com/fleet/2025/12/the-future-of-fleet/ · neovim.io/doc/user/lsp/ · github.com/helix-editor/helix/pull/8675 · github.com/lapce/lapce · lsp.sublimetext.io/features/ · gnu.org/software/emacs/manual/html_mono/eglot.html
⚠️ Verify at PRD time: Zed Visual Extension API ship status (fast-moving), Helix Steel stable status, Lapce custom-UI limit (inferred).

---

## 4. Market Signals & Security Incidents

### 4.1 Cross-tool yardstick — Stack Overflow 2025 Dev Survey (VERIFIED, n≈26K editors)
survey.stackoverflow.co/2025/technology · /2025/ai
- Editors: VS Code **75.9%** · Visual Studio 29.0% · IntelliJ 27.1% · Vim 24.3% · **Cursor 17.9%** (new; highest "admired" 74.4%) · **Claude Code 9.7%** · **Zed 7.3%** · Windsurf 4.9% · Neovim 14.0%.
- AI assistants (among AI users): ChatGPT 81.7% · **Copilot 67.9%** · Gemini 47.4% · **Claude Code 40.8%**.
- 84% use/plan AI tools (up from 76%); trust collapsing — only **29% trust accuracy** (was 40%); 66% spend more time fixing "almost-right" AI code.

### 4.2 Hard numbers
- **Cursor / Anysphere:** Series D $2.3B @ **$29.3B** (Nov 2025). ARR ~$2B (Feb 2026). **SpaceX to acquire for $60B all-stock, announced Jun 16 2026** (SEC filing, closing Q3) — VERIFIED. ">$50B Series E" = RUMOR.
- **Windsurf saga:** OpenAI ~$3B talks (Apr 2025) **collapsed Jul 11 2025** (MS IP rights); **Google reverse-acquihire $2.4B** (hired CEO + R&D into DeepMind); **Cognition (Devin) acquired the remainder** Jul 14 2025. Now smaller.
- **GitHub Copilot:** **4.7M paid (+~75% YoY, Q2 FY26 Jan 2026)**; 20M+ all-time (Jul 2025). FLAG: press conflates all-time vs paid.
- **Claude Code:** GA May 2025; run-rate **$1B (Nov 2025)** → **$2.5B+ (Feb 2026)**. FLAG: run-rate ≠ ARR.
- **Gemini Code Assist / Amazon Q:** opaque, no standalone metrics. Gemini folding into "Antigravity"; Q narrative drifting to Kiro.
- **Cline:** $32M raised, ~5M installs. **Continue:** ~4.6M installs, **acquired by Cursor ~Jun 2026**. **Aider:** OSS, ~46K stars, runs the polyglot benchmark. **Tabnine:** fading (layoffs, enterprise pivot). **Zed:** $32M Series B (Sequoia), >150K active devs, Zed 1.0 ~Apr 2026.

### 4.3 Prioritization ranking (for plugin/integration investment)
1. **GitHub Copilot** — largest base, MS distribution, VS Code default. Incumbent.
2. **Cursor** — fastest-growing standalone, category-definer.
3. **Claude Code** — fastest revenue ramp, agentic CLI, 40% of AI users.
4. **Gemini Code Assist** — huge bundling reach, opaque.
5. **Amazon Q** — AWS bundle, opaque.
6. **JetBrains AI / Junie** — captive IDE audience (Junie GA Jun 17 2026).
7. **Cline** — leading OSS agentic extension, BYO-model.
8. **Zed** — fastest-rising new editor, Rust-native, open protocol.
9. **Continue** — popular OSS, future uncertain post-Cursor.
10. **Windsurf** — gutted by 2025 saga.
11. **Aider** — beloved OSS CLI + benchmark standard.
12. **Tabnine** — fading.

### 4.4 Security incidents validating the AI-fence thesis
- **Claude Code silently auto-loads `.env` into memory — VERIFIED (prod).** knostic.ai/blog/claude-cursor-env-file-secret-leakage. Same source: Cursor swept an API key into a cloud upload (blocked); Claude Code committed a Gemini key to GitHub.
- **`.claudeignore` / deny rules not reliably enforced** — agents are probabilistic (Register, Jan 2026) ⚠️ pull primary.
- **Supabase MCP / Cursor — VERIFIED PoC.** generalanalysis.com/blog/supabase-mcp-blog. Support-ticket injection → MCP agent with RLS-bypassing `service_role` dumps `integration_tokens` back into the ticket. Supabase response "Defense in Depth for MCP" framed as layers, not a fix.
- **GitHub MCP "toxic agent flow" — VERIFIED PoC.** invariantlabs.ai/blog/mcp-github-vulnerability. Public-repo issue coerces agent into leaking PRIVATE-repo data via auto-created public PR.
- **"Comment and Control" — VERIFIED PoC, multi-vendor.** Hit Claude Code Security Review, Gemini CLI Action, Copilot Agent; stole CI secrets (`ANTHROPIC_API_KEY`, `GITHUB_TOKEN`, `GEMINI_API_KEY`). **Anthropic rated one CVSS 9.4 Critical.** securityweek.com/claude-code-gemini-cli-github-copilot-agents-vulnerable-to-prompt-injection-via-comments/
- **"Lethal trifecta" — Simon Willison, VERIFIED canonical.** simonwillison.net/2025/Jun/16/the-lethal-trifecta/ — (1) private-data access, (2) untrusted content, (3) exfiltration. **The fence removes leg #1 — the only leg a local tool can structurally remove, not dependent on the LLM choosing to obey.**
- **CVEs:** CVE-2025-59536 (Claude Code RCE + API-key exfil, CVSS 8.7, fixed 1.0.111+) · CVE-2026-21852 (`ANTHROPIC_BASE_URL` redirect, CVSS 5.3, fixed 2.0.65+) · Gemini CLI prompt-injection env-var exfil (CVSS 10.0 PoC).
- **Secrets in training data:** 11,908 live secrets in Common Crawl (Truffle Security); DeepSeek ClickHouse DB exposed chat history + secret keys (Wiz, Jan 2025 ⚠️ secondary).
- **GitGuardian State of Secrets Sprawl 2026** (VERIFIED): **29M new hardcoded secrets** on public GitHub in 2025 (+34% YoY); **AI-service secret leaks 1,275,105 (+81% YoY)**; **AI-assisted commits leak ~2× more (3.2% vs 1.5% baseline)**. blog.gitguardian.com/the-state-of-secrets-sprawl-2026/

### PRD takeaway
Every documented exfiltration chain starts with an agent reading secrets/private data it never needed. AI-assisted coding ~doubles secret-leak rate; AI-service leaks +81% YoY. An AI fence blocking `.env`/env-var reads cuts the chain at leg #1 of the lethal trifecta — the one structural mitigation that does not depend on the LLM reliably choosing to obey.

### Gaps to firm up before publishing
- Pull primary Wiz DeepSeek writeup + original Register `.claudeignore` article.
- Cursor post-Feb-2026 ARR and ">$50B Series E" are RUMOR; only the $60B SpaceX acquisition is VERIFIED-recent.
- Adoption % are opt-in survey samples — directional, not census.
- Verify exact Windsurf/Cline MCP paths, Claude Desktop Linux path, MCP OAuth RFC list, AAIF donation date.
