---
stepsCompleted: ['step-01-init', 'step-02-discovery', 'step-02b-vision', 'step-02c-executive-summary', 'step-03-success', 'step-04-journeys', 'step-05-domain', 'step-06-innovation', 'step-07-project-type', 'step-08-scoping', 'step-09-functional', 'step-10-nonfunctional', 'step-11-polish', 'step-12-complete']
inputDocuments:
  - 'docs/ide-behavior-contract.md'
  - 'docs/lsp-clients.md'
  - 'README.md'
  - 'CHANGELOG.md'
  - 'CLAUDE.md'
  - 'docs/research/ai-tooling-landscape-2025-2026.md'
workflowType: 'prd'
releaseMode: 'single-release'
project_name: 'EnvForge'
user_name: 'Emre'
date: '2026-06-19'
classification:
  projectType: 'Developer tool (CLI + TUI + LSP + IDE plugins)'
  domain: 'Developer tooling / secrets & env management / AI-safety'
  complexity: 'high'
  projectContext: 'brownfield'
---

# Product Requirements Document - EnvForge

**Author:** Emre
**Date:** 2026-06-19

> **Scope of this PRD:** EnvForge **v0.9 "Omnipresence"** — a release that expands EnvForge's IDE and AI-coding-tool coverage from two first-party plugins (VS Code, IntelliJ) and a generic LSP to a broad, native footprint across every major editor and AI agent in use in 2026. The goal is "over-optimum" coverage: wherever a developer's code and AI agent run, EnvForge's secret-fencing, leak-linting, and exfil-detection are present by default.

## Executive Summary

EnvForge is an AI-safety-first environment-variable and secrets manager for the shell and `.env` ecosystem (Rust CLI + TUI + LSP, v0.8.3). It exists because modern AI coding agents ingest a repository wholesale — `.env` files, secrets, and connection strings included — and exfiltrate or leak them through prompt injection, tool calls, and context windows. EnvForge is the only env manager built adversarially against the AI agent in the editor, not just against careless humans.

This release, **v0.9 "Omnipresence,"** closes the coverage gap. Today EnvForge's protections (fence, MCP-config linting, AI-guard, canary tripwires, AI-exposure heatmap) reach VS Code and IntelliJ richly, every LSP editor partially, and a fixed set of AI-tool config files (Cursor, Copilot, Claude). But the AI-tooling landscape has exploded: Windsurf, Cline, Aider, Continue, Gemini CLI, Amazon Q, Roo Code, Zed AI, and others each ship their own ignore/rules conventions and increasingly speak MCP. Any tool EnvForge does not natively understand is an unguarded hole through which secrets leak. v0.9 makes EnvForge omnipresent: it speaks every major AI tool's native fence dialect, ships first-party editor plugins where LSP-only is insufficient, and exposes its own MCP server so agents can request env metadata through a safe, audited, redacting channel instead of reading files raw.

### What Makes This Special

Competing env tools (dotenv, direnv, Doppler, 1Password CLI) protect secrets *at rest and in transit* — a human-threat model. EnvForge protects them *from the AI agent that is actively reading the workspace*, a threat model none of them address. The core insight driving v0.9: AI-secret-exposure is fragmented across N tool-specific config conventions, and that fragmentation *is* the vulnerability. A developer who fences Cursor but not Windsurf is exposed; one who lints `.mcp.json` but not `.vscode/mcp.json` is exposed. EnvForge's differentiating move is to collapse that fragmentation into one `envforge fence` command and one always-current registry of tool conventions, plus a safe MCP egress so agents never need raw file access at all. The "exactly what I needed" moment: a developer runs one command and every AI tool in their stack — present and future — is fenced, linted, and trip-wired, with a status bar that proves it.

## Project Classification

- **Project Type:** Developer tool — Rust CLI + TUI + LSP server + first-party IDE plugins.
- **Domain:** Developer tooling; secrets & environment management; AI-safety / application security.
- **Complexity:** High — security-critical (zero-trust, byte-for-byte parser safety, redaction in memory), broad integration surface, adversarial threat model.
- **Project Context:** Brownfield — mature product (v0.8.3) with an established LSP behavior contract, 1191+ tests, and a documented IDE parity discipline. This release extends, never breaks, that contract.

## Success Criteria

### User Success

- A developer runs **one command** (`envforge fence`) and every AI coding tool installed in their stack — Cursor, Copilot, Windsurf, Cline, Aider, Continue, Gemini CLI, Amazon Q, Roo Code, Claude Code, and more — is fenced from reading secrets, with no per-tool manual config.
- The "aha" moment: the status bar reads `AI BLOCKED · 9 tools fenced` and a single `envforge fence --status` proves coverage across the entire AI toolchain, including tools the developer forgot they had installed.
- AI agents that support MCP can answer "what env vars does this project need?" by calling EnvForge's MCP server and receiving **schema + redacted metadata** — never raw secret values — so the agent stays useful without the developer disabling safety.
- Users on editors beyond VS Code/IntelliJ (Zed, Neovim) get a **native** experience (status bar, exposure gutter) rather than bare LSP diagnostics.
- Zero false sense of security: when a newly-installed AI tool is *not* yet covered, EnvForge tells the user explicitly rather than silently leaving a hole.

### Business Success

- **Coverage as moat:** EnvForge supports ≥ 12 AI coding tools and ≥ 6 editors at GA — measurably broader AI-tool fence coverage than any competing env/secret manager (most support 0).
- **Adoption:** first-party plugin installs (VS Code Marketplace + JetBrains Marketplace + new Zed/Neovim channels) grow; new editor channels (Zed extension registry, Neovim plugin) each reach published/listed status at GA.
- **Retention signal:** repeated `fence`/`sync` invocations per active project per week (proxy for "EnvForge is in the daily loop").
- **Ecosystem credibility:** EnvForge's MCP server is listed in ≥ 1 public MCP registry; the tool-convention registry is referenceable by third parties.

### Technical Success

- **Single source of truth preserved:** every new IDE/AI feature implemented once in `envforge lsp` / `ops/`, rendered identically by all plugins. No drift; parity tests enforce it (extends the existing `ide-behavior-contract.md` discipline).
- **Fence target registry is data-driven:** adding a new AI tool = adding a registry entry (filename + block format + semantics), not new code paths. Each target round-trips byte-safely and preserves user-authored content on disable.
- **MCP server is read-safe by construction:** no tool exposes raw secret values; all responses pass through `redact::redact_for_label`; every reveal-class call is audit-logged.
- **No regressions:** all existing 1191+ tests pass; new coverage adds parity + registry + MCP tests. `cargo fmt && cargo clippy -D warnings && cargo test` green.
- **Offset/zone safety maintained:** protected zones (conda, Amazon Q blocks) and atomic writes unchanged.

### Measurable Outcomes

| Outcome | Target at GA (v0.9) |
|---|---|
| AI coding tools with native fence support | ≥ 9 (Tier-1 + OSS + `AGENTS.md` cross-tool standard); long-tail added later as registry data |
| AI tool MCP-config files linted by LSP | ≥ 8 path patterns (from 5) |
| Editors with first-party native plugins | 3 (VS Code, IntelliJ, **+ Neovim**); Emacs/Sublime = Vision, not v0.9 |
| Editors reached via LSP + (local) MCP — no custom UI possible | Zed, Helix, Lapce |
| Editors with verified LSP-only support | ≥ 5 (Helix, Emacs, Sublime, Lapce, Kakoune) — **Fleet dropped (discontinued Dec 2025)** |
| EnvForge MCP server tools exposed | 2 core (`list_keys`, `describe_schema`), local **stdio** only, 0 raw-secret tools |
| New tests added | ≥ 50 (registry + parity + MCP-safety + lint patterns) |
| `fence --status` reports per-tool coverage + detection | Yes (named tools + installed-but-unfenced) |

## Product Scope (Priority Tiers)

> Tiers below define build-order priority within the single v0.9 release. See **Project Scoping** for the consolidated single-release decision and risk strategy; the two are reconciled there (tiers = priority bands, not deferred phases).

### MVP — Minimum Viable Product (v0.9 core)

1. **AI-tool fence registry expansion.** Refactor fence targets into a data-driven registry and add native support for the **highest-adoption** AI tools beyond today's set: **Windsurf/Codeium, Cline, Aider, Gemini CLI** (current set: Cursor, Copilot, Claude), **plus the `AGENTS.md` cross-tool rules standard** (Linux-Foundation-governed; consumed by Codex, Cursor, Copilot, Gemini, Zed, Cline, Roo, Junie, VS Code, and more — so it implicitly covers much of the long-tail). Per-tool the correct mechanism is used — and crucially, **not every tool has an ignore file**: Copilot (UI content-exclusion only), Claude Code (`permissions.deny Read()` in `.claude/settings.json`), Amazon Q, Codex CLI, and Zed have no ignore file, so the fence falls back to rules-file + `AGENTS.md` + canary defense for those. Each target: correct filename(s)/mechanism, correct block format, byte-safe round-trip, content-preserving disable, `fence config` per-target toggle. **Long-tail tools (Roo Code, Continue, Augment, Tabnine, Amazon Q) are deferred** — the registry makes them cheap to add post-GA as data, and several are already covered via `AGENTS.md`.
2. **`fence --status` per-tool reporting + detection + MCP-config lint expansion.** Status output names each covered tool, flags **installed-but-unfenced** tools (detection via the registry's per-entry hints — folded into status, no separate `doctor` command for v0.9), and emits JSON + exit codes for CI. LSP MCP-config linter (`envforge-mcp`) gains the additional config-file path patterns the new tools use (`.vscode/mcp.json`, `.cursor/mcp.json`, Claude Code `.mcp.json`/`~/.claude.json`, Windsurf/Cline MCP configs — exact paths in the Research Inputs appendix).
3. **EnvForge MCP server (`envforge mcp`) — local stdio.** EnvForge exposes its own MCP server over **stdio** (the transport every mainstream MCP client uses locally) with **2 core read-safe tools for v0.9**: `list_keys` (key names) and `describe_schema` (types/required/defaults + redacted previews) — both redacted, both audited; **no raw-value tool exists**. Follows the established first-party-MCP-server pattern (Vault, Bitwarden, Infisical); EnvForge's differentiator is redaction-by-default + canary/fence semantics at the MCP boundary. Lets MCP-capable agents (Claude Desktop/Code, Cursor, VS Code, Windsurf) get env metadata through a guarded channel instead of reading files. *Remote (Streamable HTTP + OAuth 2.1) transport and the `exposure_map`/`canary_scan`/`canary_check` tools are deferred to Vision — they add network/auth attack surface and aren't needed for the local-agent use case.*

### Growth Features (Post-MVP, still targeted for v0.9 "over-optimum")

4. **First-party Neovim plugin** — Lua plugin layering native UI (statusline component, exposure virtual-text/sign-column heatmap, fence toggle command) on top of the LSP, beyond the bare `lspconfig` snippet shipped today. Neovim has the strongest CLI-user overlap and a mature UI plugin surface (`sign_column`, extmarks, virtual text).
5. **Zed: LSP + local MCP integration (no custom UI).** Zed's extension API today is WASM-restricted to languages/themes/MCP — custom status bar/gutter UI is an *unshipped Draft RFC*. So the Zed play is: verified LSP-only wiring + register the EnvForge **MCP server** (stdio) as a Zed extension + ensure the fence writes the rules-chain files Zed honors (`AGENTS.md` canonical). Revisit native UI if/when Zed ships its Visual Extension API.

### Vision (Future)

- **Long-tail fence tools as registry data:** Roo Code, Continue, Augment, Tabnine, Amazon Q — community-contributed entries validated by schema (cheap given the data-driven registry; several already covered via `AGENTS.md`).
- **MCP remote transport:** Streamable HTTP + OAuth 2.1 + RFC 8707 resource indicators — only when a real remote-agent consumer exists; deferred to avoid adding a network/auth attack surface to a security tool.
- **Additional MCP tools:** `exposure_map`, `canary_scan`, `canary_check` over MCP (already available via CLI/LSP today).
- **Standalone `envforge doctor --ai`** richer detection report (v0.9 folds the essential detection into `fence --status`).
- **Stretch first-party plugins:** Emacs (Elisp: mode-line + fringe + overlays) and Sublime Text (Python API: status bar + phantoms + gutter).
- EnvForge as an MCP *gateway/proxy* that sits in front of other MCP servers and strips secrets from their traffic (addresses OWASP MCP10 over-sharing).
- Native Zed status bar/gutter once Zed ships its Visual Extension API (currently Draft RFC #53403).
- Auto-PR / CI mode that fails builds when an AI-tool config in the repo is unfenced or contains a hardcoded credential.
- Community-contributed registry entries for long-tail AI tools, shipped as data, validated by schema.

## User Journeys

### Journey 1 — Maya, full-stack dev, multi-tool stack (primary, happy path)

Maya works in a repo with a dozen secrets in `.env`. Her stack drifted over a year: she started in VS Code + Copilot, added Cursor, tried Windsurf, runs Aider in the terminal for refactors, and just installed Claude Code. **Opening scene:** she realizes she has no idea which of these tools can see her `AWS_SECRET_ACCESS_KEY`. **Rising action:** she runs `envforge fence`. EnvForge writes/updates the correct ignore + rules files for *every* tool it detects — `.cursorignore`, `.github/copilot-instructions.md`, Windsurf's rules, `.aiderignore`, `.clinerules`, `.claude/settings.json`, Gemini's `.aiexclude` — in one pass, preserving any content she had authored. **Climax:** the status bar flips to `$(shield) AI BLOCKED · 7 tools fenced`; `envforge fence --status` lists each tool by name with a green check. **Resolution:** every agent in her editor now refuses to read the secret file; she keeps coding with AI assist, secrets dark. *Reveals requirements:* data-driven multi-target fence, per-tool status reporting, content-preserving writes.

### Journey 2 — Maya installs a new tool next week (primary, edge case / recovery)

Two weeks later Maya installs Roo Code. **Pain:** her old fence run knew nothing about it — silent exposure is exactly the failure mode EnvForge exists to prevent. **Rising action:** on next `fence --status` (and via the status-bar refresh), EnvForge's registry-hint detection reports `Roo Code: installed, NOT fenced` in amber and recommends the fix. **Climax:** `envforge fence` covers it (Roo Code via its `AGENTS.md`/rules support); status returns to all-green. **Resolution:** the window of exposure was surfaced loudly, not hidden. *Reveals requirements:* AI-tool detection, "installed-but-unfenced" warning state, no-silent-holes guarantee.

### Journey 3 — Claude Code agent as MCP consumer (integration / non-human "user")

An AI agent (Claude Code) needs to scaffold a `.env` for a teammate. **Pain (old world):** it would read the existing `.env` raw — leaking live secrets into its context. **Rising action:** the project is fenced, so the agent can't read the file; instead it calls EnvForge's MCP server tools `list_keys` and `describe_schema`. **Climax:** it receives the *schema* (key names, types, required flags, examples) and *redacted* current-value previews — enough to generate a correct `.env.example` and explain what's needed, with zero live secret material crossing the wire. Every call is audit-logged. **Resolution:** the agent stays maximally useful without ever touching a real secret. *Reveals requirements:* EnvForge MCP server, read-safe tool set, mandatory redaction, audit logging.

### Journey 4 — Devon, security lead, fleet rollout & verification (admin / ops)

Devon owns AppSec for 40 engineers. **Pain:** he can't trust that every dev fenced every AI tool; "did you run it?" doesn't scale. **Rising action:** he standardizes on `envforge fence` in onboarding and wires `envforge fence --status --json` + MCP-config lint into CI so a PR fails if any AI-tool config in the repo is unfenced or contains a hardcoded credential. He plants canaries on the crown-jewel keys. **Climax:** when an agent on some unmonitored laptop exfiltrates a fake key, the canary trips and `envforge canary check` flags it fleet-wide. **Resolution:** coverage is enforced and verifiable, not hoped-for. *Reveals requirements:* JSON status for automation, CI-friendly exit codes, MCP-config lint breadth, canary integration.

### Journey Requirements Summary

The journeys converge on these capability areas:

- **Data-driven fence registry** covering all major AI tools, with content-preserving read/write/disable per target (J1, J2).
- **Per-tool status + detection** (`fence --status` names tools, detects installed-but-unfenced via registry hints, amber state) (J1, J2).
- **EnvForge MCP server** with a read-safe, redacted, audited tool set so agents get metadata without raw file access (J3).
- **Automation surfaces:** JSON output, deterministic exit codes, MCP-config linting breadth for CI gates (J4).
- **Native editor UX beyond VS Code/IntelliJ** (Zed, Neovim) so status/exposure signals reach more developers (J1, cross-cutting).

## Domain-Specific Requirements

EnvForge operates in the **AI-safety / application-security** domain. There is no single external regulator, but the threat model is adversarial and the correctness bar is security-grade. v0.9's expansion multiplies the integration surface, so domain constraints tighten rather than relax.

### Threat Model (what v0.9 must hold against)

- **Adversary:** an AI coding agent (and the model behind it) with read access to the workspace, possibly steered by prompt injection embedded in repo content, issue text, or dependency code.
- **Asset:** secret values in `.env*` and shell config, plus the *fact* of which keys exist.
- **Attack surface added by this release:** every new AI-tool integration is a new place a hole can open. A wrong filename, a malformed ignore block, or a fence that a given tool silently ignores = an exposed secret. The MCP server is itself a new attack surface: a tool that returns a raw value, or an injection that tricks the server into revealing one, is a critical failure.
- **Trust boundary:** the LSP wire and the MCP wire. Raw values may cross the LSP wire only for an explicit, audited human reveal (existing C6 contract). Raw values must **never** cross the MCP wire.

### Security Requirements

- **Redaction-by-default on every new egress.** All MCP server responses and all new status/detection output pass through `redact::redact_for_label`. Raw secret material is excluded by construction, not by filtering.
- **Audit every access-class operation.** MCP reveal-adjacent calls and CLI reveals emit `RuntimeEvent`s to the monitor bus (message excludes the value). Sec-ops can grep the audit stream fleet-wide.
- **Fence correctness is verifiable, per target.** Each registry target declares the exact file(s) and block format; a parity/round-trip test proves: (a) enable writes the correct fence, (b) disable strips only EnvForge-owned content and preserves user content, (c) the written content matches the tool's documented convention.
- **No silent coverage gaps.** If EnvForge cannot confirm a tool's convention, it must surface "unsupported / unverified" rather than imply protection. "Installed-but-unfenced" is an explicit, visible state.
- **Zero-trust at rest/in transit/in memory** (existing principle) extends unchanged to the new MCP server and registry.

### Technical Constraints

- **Byte-for-byte parser safety** (core invariant) applies to every new fence-target writer: parse → serialize is identical; protected zones (conda, Amazon Q blocks) are never disturbed.
- **Atomic writes** (tempfile + rename) for every registry target.
- **Offset safety** preserved across all new file types touched.
- **Cross-platform:** Linux + macOS parity for all new targets and the MCP server.

### Integration Requirements

- Must interoperate with each AI tool's *actual, current* config convention — verified against authoritative sources, not assumed (see Research Inputs appendix).
- MCP server must conform to the current Model Context Protocol spec and be discoverable by standard MCP clients (Claude Desktop/Code, Cursor, VS Code, Windsurf).
- CI integration: JSON output + deterministic exit codes so `fence --status` and MCP-config lint can gate pull requests.

### Risk Mitigations

| Risk | Mitigation |
|---|---|
| A tool's ignore-file convention changes upstream, silently breaking a fence | Registry is data-driven + versioned; `fence --status` detection flags mismatches; parity tests pin expected format; convention sources cited in-repo for re-verification. |
| A new tool *ignores* its own documented ignore file (tool bug) | Layer defense: fence writes *both* ignore and rules/instructions files where available; AI-guard + canaries provide detection even if prevention fails. |
| MCP server leaks a raw value via a crafted request | Read-safe tool set only (no get-raw tool exists in the server); all responses redacted; fuzz/property test asserting no high-entropy token ever appears in any MCP response. |
| User over-trusts "AI BLOCKED" badge | Status names each covered tool and explicitly lists uncovered/installed-but-unfenced tools; no aggregate green unless all detected tools are covered. |

## Innovation & Novel Patterns

### Detected Innovation Areas

- **Universal AI-fence abstraction.** No other env/secret manager treats "AI tool ignore/rules conventions" as a unified, data-driven target set. The novelty is collapsing N fragmented, tool-specific conventions into one command + one versioned registry — turning a per-tool chore into a single guarantee. This is a new paradigm for the env-management category, which to date models only human threats.
- **Read-safe MCP egress for secrets.** Inverting the usual MCP pattern: instead of giving an agent a tool to *fetch* values, EnvForge's MCP server gives agents everything they need to be useful (schema, types, required flags, redacted previews, exposure classification) while making raw retrieval *structurally impossible*. "Maximally helpful, zero raw secrets" is the novel stance.
- **Coverage as a first-class, provable property.** Reporting *which named tools* are fenced and loudly surfacing *installed-but-unfenced* tools reframes security from a binary toggle into an auditable coverage map — closer to a posture dashboard than a setting.

### Market Context & Competitive Landscape

- **Env/secret managers** (dotenv, direnv, Doppler, Infisical, 1Password CLI, HashiCorp Vault) solve storage, rotation, injection, and team sharing — all human/ops threat models. None ship AI-tool fencing, AI-guard, or canary tripwires. EnvForge's AI-adversarial posture is uncontested in this category.
- **AI-tool config governance** is currently manual and fragmented: each tool documents its own ignore/rules file, and developers wire them by hand. There is no cross-tool "fence everything" utility — the gap EnvForge fills.
- **MCP ecosystem** is expanding fast (Claude, Cursor, VS Code, Windsurf, Cline adopting it); secret-handling within MCP is an emerging concern, positioning a read-safe secrets MCP server as timely.
- *(Authoritative, sourced landscape detail — exact config-file conventions, MCP adoption, real secret-leak incidents — is consolidated in the **Research Inputs** appendix, gathered via live web research for this PRD.)*

### Validation Approach

- **Convention accuracy:** every registry entry's filename + format validated against the tool's official docs (cited in-repo); parity tests pin the expected output; `fence --status` detection re-checks against installed tools.
- **MCP safety:** property/fuzz test asserting no high-entropy token appears in any MCP response across randomized state; manual verification against ≥ 2 real MCP clients.
- **Coverage claims:** integration tests enumerate the supported-tool matrix; `fence --status --json` output is snapshot-tested.
- **Editor UX:** Zed + Neovim plugins smoke-tested against the live LSP, asserting parity with the `ide-behavior-contract.md` data.

### Risk Mitigation

- **If a convention is wrong/stale:** data-driven registry → fast fix without code change; defense-in-depth (ignore *and* rules files, plus AI-guard + canaries) means a single wrong entry is not a total failure.
- **If MCP adoption stalls or spec shifts:** MCP server is additive; core fence value stands alone; pin to a spec version and gate behind a feature flag.
- **If a tool has no ignore convention at all:** surface as "unsupported — use fence's rules-file fallback + canary" rather than implying coverage.

## Developer-Tool Specific Requirements

### Project-Type Overview

EnvForge is a developer tool distributed as a single Rust binary that presents four faces: CLI (80+ subcommands), TUI, LSP server, and first-party IDE plugins. v0.9 adds a fifth face — an MCP server — and broadens the plugin/integration footprint. The architectural discipline is fixed: business logic lives in `ops/`, the LSP is the single behavioral source of truth, and CLI/TUI/plugins are thin layers. Every v0.9 feature must respect this.

### Technical Architecture Considerations

**1. Fence target registry (refactor + expansion).** Today fence targets are an enumerated `FenceTarget::all()` set with bespoke writers for `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json`, `.envforgeignore`. v0.9 generalizes this into a **data-driven registry**, where each entry declares:

- `id` (snake_case), display name, and the AI tool it protects;
- one or more **target files** with kind = `ignore` | `rules`/`instructions` | `deny-rule` (e.g. `permissions.deny Read()` in a JSON settings file) | `cross-tool` (`AGENTS.md`);
- the **block format** per file (line-list vs. marked block vs. JSON-merge for settings files);
- **ownership semantics** (fully-owned → deletable on disable; shared → surgical strip preserving user content);
- a **detection hint** (paths/markers that signal the tool is installed) for `fence --status` detection;
- a **source URL** for the convention (verifiability — FR25), so a stale convention is auditable;
- a **capability flag** marking whether the tool has a real ignore mechanism or only rules/deny fallback (drives honest status reporting — no false "fenced").

Correctness note grounded in research: `.windsurfignore` and `.claudeignore` **do not exist** (use `.codeiumignore` and `permissions.deny` respectively); `.cursorrules`/`.windsurfrules`/`.roorules` are deprecated-but-functional (prefer `.cursor/rules/`, `.windsurf/rules/`, `.roo/rules/`); Aider config is `.aider.conf.yml` (`.yml` only). The registry encodes these so the fence is right by data, not folklore.

Adding a tool becomes a registry entry + tests, not new control flow. `ops::fence` consumes the registry; `fence config` lists/toggles per target; `FenceTarget::all()` ordering remains stable for existing parity.

**2. EnvForge MCP server (`envforge mcp`) — local stdio.** A new subcommand starts an MCP server conforming to MCP spec `2025-11-25` over **stdio** (the transport every mainstream MCP client uses locally). It exposes **2 read-safe tools for v0.9**: `list_keys`, `describe_schema`. It reuses the exact `ops/` functions the LSP already serves, so output is byte-identical across LSP, CLI, and MCP. There is deliberately **no** `get_value` tool. All responses route through the shared redaction routine; reveal-class operations (if any are added later) must be human-gated + audited like C6. Mirrors the established first-party-MCP-server pattern (Vault, Bitwarden, Infisical); EnvForge's edge is redaction + canary/fence at the MCP boundary. Config snippets shipped for Claude Desktop (`claude_desktop_config.json`), Claude Code (`.mcp.json` / `~/.claude.json`), Cursor (`.cursor/mcp.json`), VS Code (`.vscode/mcp.json`, GA since 1.102), Windsurf, Cline. (Exact per-OS paths in the Research Inputs appendix.) **Deferred to Vision:** remote Streamable HTTP + OAuth 2.1 / RFC 8707 transport, and the `exposure_map`/`canary_scan`/`canary_check` tools — they add network/auth attack surface without serving the local-agent use case.

**3. MCP-config linter breadth (LSP).** The existing `envforge-mcp` diagnostic source (L14) extends its `documentSelector` glob set to the additional MCP/agent config paths the new tools use. Detection heuristics (`ops::mcp_scan`) are reused unchanged; only the file-pattern coverage grows.

**4. Plugin distribution & parity.** New first-party plugins (Zed extension, Neovim Lua plugin) are thin LSP clients that MUST NOT re-implement logic — same rule that governs the VS Code/IntelliJ plugins. Each renders the `envforge/exposureMap` data and exposes fence-toggle/volatile/reveal via the named `envforge/*` requests (or `envforge exposure --file` subprocess where the editor's extension API is easier to drive that way, as IntelliJ already does). `ide-behavior-contract.md` gains columns/rows for the new clients; parity tests fence the shared LSP response bodies.

**5. AI-tool detection (folded into `fence --status`).** A lightweight scan matches each registry entry's detection hint against the workspace + known config locations and emits: covered, installed-but-unfenced, not-installed. Lives inside `fence --status` for v0.9 (no separate command); drives the "no silent holes" guarantee. A richer standalone `doctor --ai` report is deferred to Vision.

### Implementation Considerations

- **Extend, don't break.** All changes are additive to the LSP contract; existing test IDs in `tests/lsp_*` stay green. New parity files follow the established naming.
- **Registry as data.** Prefer a declarative table (Rust const data or embedded TOML) so community contributions and corrections are low-risk; validate entries by schema/test at build time.
- **Feature-flag the MCP server** if the spec is still moving, so GA can ship fence breadth even if MCP lands behind a flag.
- **CLI ergonomics:** new subcommand (`mcp`) and `fence --status` enhancements (per-tool reporting + detection) follow existing Clap patterns; all support `--json`.
- **Docs:** `lsp-clients.md`, `ide-behavior-contract.md`, README integration matrix, and CHANGELOG updated; per CLAUDE.md, test counts in README/CHANGELOG refreshed after the test additions.
- **Skipped (not applicable to this project type):** visual/UX design system, mobile/device, multi-tenancy/RBAC, payment flows.

## Project Scoping

### Strategy & Philosophy

**Approach:** Single release (v0.9 "Omnipresence"), shipped as a **complete, "over-optimum" feature set** rather than a thin MVP. Per the product directive, breadth of coverage *is* the value proposition — a partial release that fences only some tools would undercut the core "no silent holes" guarantee. The earlier MVP/Growth/Vision tiers are retained as **priority bands within this single release** (what gets built first if engineering capacity is tight), not as deferred future phases. All bands below are in scope for v0.9 unless engineering explicitly negotiates a cut with the product owner.

**Resource Requirements:** Solo/small Rust team (existing maintainer). The data-driven registry is the leverage point — once built, each additional tool is a low-cost data entry + test, making broad coverage tractable for a small team.

### Complete Feature Set (v0.9)

**Core User Journeys Supported:** all four (Maya happy path, new-tool recovery, AI-agent MCP consumer, security-lead fleet rollout).

**Feature set + priority bands:** see **Product Scope (Priority Tiers)** above — Band 1 (must-have) = the MVP tier (registry, Tier-1/OSS fence + `AGENTS.md`, per-tool `fence --status` with detection folded in, local-stdio `envforge mcp` with 2 tools, MCP-lint breadth, full tests); Band 2 (nice-to-have, cut only under capacity pressure) = the Growth tier (Neovim plugin, Zed LSP+MCP). Long-tail tools, MCP remote/extra tools, standalone `doctor --ai`, and Emacs/Sublime plugins are **Vision** (deferred). This section adds only the single-release decision and risk strategy; it does not restate the tier contents.

**Explicitly Out of Scope for v0.9 (Vision / later):**

- **JetBrains Fleet plugin — permanently dropped** (Fleet discontinued Dec 22 2025, succeeded by "Air"; existing IntelliJ Platform plugin unaffected).
- Native Zed status bar/gutter (blocked on Zed's unshipped Visual Extension API, Draft RFC #53403).
- Sourcegraph Cody fence target (no repo rules file; ignore convention sunset — server-side `cody.contextFilters` only).
- MCP gateway/proxy that filters other servers' traffic.
- CI auto-PR enforcement product (the CLI primitives ship; the packaged CI product does not).

### Risk Mitigation Strategy

- **Technical Risks:** the riskiest assumption is that each tool's ignore convention is correct and stable. Mitigation: cite authoritative sources per registry entry, pin formats with parity tests, and ship defense-in-depth (ignore + rules + canary) so one wrong entry isn't catastrophic. The MCP server's spec volatility is mitigated by a feature flag and version pin.
- **Market Risks:** the bet is that AI-tool sprawl + secret-leak fear is real demand. Validated by the research appendix (adoption data + real incidents); de-risked further by the fact that breadth reuses one engine, so cost of being wrong is low.
- **Resource Risks:** small team. Mitigation: the registry makes Band-1 tool additions cheap; Band-2 plugins are independent and individually cuttable without affecting the core guarantee.

## Functional Requirements

> Capability contract for v0.9. Each FR states WHAT capability exists, implementation-agnostic. Priority band (B1 must-have / B2 nice-to-have) noted per FR; B1 is binding for GA.

### AI-Tool Fence Coverage

- **FR1 (B1):** A developer can fence all detected AI coding tools in a project with a single command, generating each tool's correct ignore and/or rules files in one pass.
- **FR2 (B1):** EnvForge can fence each of the following AI tools natively, using each tool's actual current convention: Cursor, GitHub Copilot, Claude (Code/Desktop), Windsurf/Codeium, Cline, Aider, Continue, Gemini CLI, Roo Code, JetBrains AI/Junie, Augment. (B2: Tabnine, OpenAI Codex CLI.)
- **FR2a (B1):** EnvForge writes and maintains an `AGENTS.md` rules file (the Linux-Foundation-governed cross-tool standard) as a fence target, so tools that honor `AGENTS.md` are covered even when they have no dedicated ignore file.
- **FR2b (B1):** For tools that have no ignore-file mechanism (e.g. GitHub Copilot, Claude Code, Amazon Q, Codex CLI, Zed), EnvForge applies the tool's available protection instead (e.g. a deny rule in `.claude/settings.json`, a rules-file instruction) and falls back to rules + `AGENTS.md` + canary, rather than implying a non-existent ignore file.
- **FR3 (B1):** When disabling a fence, EnvForge removes only EnvForge-owned content and preserves any user-authored content in shared config files.
- **FR4 (B1):** A developer can enable/disable fencing per individual tool target (not only all-or-nothing).
- **FR5 (B1):** EnvForge fence targets are defined as data registry entries such that supporting a new tool requires adding an entry, not changing control flow.
- **FR6 (B1):** Every fence write round-trips byte-safely and never disturbs protected zones (conda, Amazon Q blocks) or unrelated file content.

### Coverage Visibility & Detection

- **FR7 (B1):** A developer can see fence status reported per named tool (covered / not covered), not just an aggregate boolean.
- **FR8 (B1):** EnvForge surfaces an explicit "installed-but-unfenced" state for any AI tool it detects as present but not yet fenced.
- **FR9 (B1):** EnvForge exposes fence status as machine-readable JSON with deterministic exit codes suitable for CI gating.
- **FR10 (B1):** `fence --status` performs lightweight detection (via the registry's per-entry hints) of which AI tools are installed and reports each as fenced / unfenced, with a recommended fix. (A richer standalone `doctor --ai` command is deferred to Vision.)
- **FR11 (B1):** The aggregate "AI BLOCKED / protected" indicator reads protected only when all detected tools are covered.

### Safe AI Egress (MCP Server)

- **FR12 (B1):** An MCP-capable AI agent can connect to an EnvForge-provided MCP server (`envforge mcp`) over **local stdio** transport. (Transport detail in NFR-C4; remote transport deferred to Vision.)
- **FR13 (B1):** Through the MCP server, an agent can list env-var key names (`list_keys`) and describe the schema — types, required flags, defaults/examples, redacted current-value previews (`describe_schema`) — without receiving any raw secret value.
- **FR14 (Vision, deferred):** Additional MCP tools — `exposure_map`, `canary_scan`, `canary_check` — exposed over MCP. Available today via CLI/LSP; deferred from v0.9 to keep the MCP surface minimal.
- **FR15 (B1):** The MCP server exposes no capability to retrieve a raw secret value; all responses are redacted by construction.
- **FR16 (B1):** Every access-class MCP operation is recorded to the audit/monitor stream without the secret value appearing in the log.
- **FR17 (B1):** EnvForge provides ready-to-use MCP server configuration snippets for major MCP clients (Claude Desktop/Code, Cursor, VS Code, Windsurf, Cline).

### Leak Linting Breadth

- **FR18 (B1):** EnvForge flags hardcoded credentials in AI-tool/agent configuration files across an expanded set of config-file path patterns (beyond today's five), as editor diagnostics and via CLI.
- **FR19 (B1):** Credential findings in config files include a quick-fix/recommendation to replace the literal with an env-var reference managed by EnvForge.

### Editor Reach (Native Plugins & LSP)

- **FR20 (B2):** A Neovim user can install a first-party EnvForge plugin providing a statusline indicator, exposure heatmap (sign column / virtual text), and a fence-toggle command on top of the LSP.
- **FR21 (B2):** A Zed user can use EnvForge via verified LSP wiring plus the EnvForge MCP server registered as a Zed extension; native status-bar/gutter UI is explicitly out of scope until Zed ships its Visual Extension API.
- **FR22 (B1):** All editor clients (first-party and LSP-only) consume identical LSP behavior; no client re-implements business logic, and parity is enforced by tests.
- **FR23 (B1):** Verified LSP-only support is maintained for Helix, Emacs, Sublime Text, Lapce, and Kakoune. (JetBrains Fleet is no longer a target — discontinued Dec 2025.)
- **FR23a (Vision, deferred):** First-party native plugins for Emacs and Sublime Text render the status indicator and exposure heatmap via their respective native UI APIs. Out of v0.9.

### Governance & Documentation

- **FR24 (B1):** A security lead can drive fence coverage and config-credential linting in CI such that an unfenced AI-tool config or a hardcoded credential fails the build.
- **FR25 (B1):** Each registry entry references the authoritative source for the tool's config convention so coverage claims are verifiable and re-checkable.
- **FR26 (B1):** EnvForge publishes an up-to-date integration matrix (tools × editors × capabilities) in its docs, and keeps README/CHANGELOG test counts synchronized after test changes.

## Non-Functional Requirements

Only categories that materially matter for EnvForge v0.9 are documented. Scalability (not a multi-user server) and broad accessibility (CLI/dev tool) are intentionally omitted.

### Security

- **NFR-S1:** No raw secret value appears in any MCP server response under any input. Enforced by construction (no raw-get tool) and verified by a property/fuzz test over randomized state asserting no high-entropy token in any response.
- **NFR-S2:** All secret-bearing output across CLI, LSP, and MCP passes through EnvForge's redaction routine; redacted labels never reconstruct the original value. (Implementation reuses the existing `redact::redact_for_label`; see architecture.)
- **NFR-S3:** Every reveal- or access-class operation (LSP reveal, MCP access calls) emits an audit event whose message excludes the secret value.
- **NFR-S4:** Fence disable preserves 100% of user-authored content in shared config files; verified by round-trip tests per registry target.
- **NFR-S5:** Secrets remain encrypted at rest, in transit, and in memory (existing zero-trust invariant), unchanged by the new surfaces.

### Performance

- **NFR-P1:** A full `envforge fence` pass over a project completes in under 500 ms at p95 on a repo with ≤ 15 tool targets (measured by the CLI's own timing in integration tests), so it is friction-free in the daily loop.
- **NFR-P2:** `fence --status` (including registry-hint detection) returns in under 300 ms at p95 (measured in integration tests) for status-bar polling; status-bar refresh cadence unchanged (30 s slow timer, 10 s volatile timer per the existing contract).
- **NFR-P3:** MCP server request latency is dominated by the underlying `ops/` call (same cost as the equivalent LSP/CLI path); no added per-request disk walks beyond what classification already requires.
- **NFR-P4:** No regression to existing LSP response latencies; new diagnostics (expanded MCP-config linting) reuse existing scan heuristics without additional passes.

### Reliability & Correctness

- **NFR-R1:** Parse → serialize is byte-identical for every file type EnvForge writes, including all new fence targets (core parser invariant).
- **NFR-R2:** All writes are atomic; a crash mid-write never corrupts a config file. (Existing tempfile-and-rename discipline; mechanism in architecture.)
- **NFR-R3:** Fence operations are idempotent — re-running `fence` produces no spurious diffs and re-planting a present canary is a no-op.
- **NFR-R4:** A failure in one tool target (e.g., unreadable file) does not abort fencing of other targets; partial results are reported, not silently swallowed.
- **NFR-R5:** The MCP server runs as an isolated process/transport such that a fault there cannot take down the LSP or corrupt state.

### Compatibility & Portability

- **NFR-C1:** All new features work identically on Linux and macOS.
- **NFR-C2:** Minimum Rust 1.75 maintained; no dependency raises the floor without explicit decision.
- **NFR-C3:** The LSP behavior contract is backward-compatible: every existing `tests/lsp_*` test ID passes unchanged; new capabilities are additive.
- **NFR-C4:** The MCP server conforms to a pinned Model Context Protocol spec version (`2025-11-25`) over **stdio** transport for v0.9, and interoperates with ≥ 2 mainstream MCP clients verified manually. (Remote Streamable HTTP + OAuth 2.1 / RFC 8707 deferred to Vision.)

### Maintainability

- **NFR-M1:** Adding a new AI-tool fence target requires only a registry data entry plus tests — no changes to fence control flow — keeping per-tool cost low for a small team.
- **NFR-M2:** Registry entries are validated by schema/test at build time; an invalid or incomplete entry fails CI rather than shipping a broken fence.
- **NFR-M3:** IDE feature parity is test-enforced; any divergence between first-party plugins and the LSP contract is a failing test, not a silent bug.

## Appendix A — Research Inputs (Landscape, 2025–2026)

Gathered via live web research for this PRD (2026-06-19). Full report with full source list: `docs/research/ai-tooling-landscape-2025-2026.md`. Legend: ✅ verified against official docs · ⚠️ unverified/secondary · ❌ does not exist.

### A.1 AI-tool fence conventions (registry source-of-truth)

| Tool | Ignore mechanism | Rules / instructions file |
|---|---|---|
| Cursor | `.cursorignore` ✅ (+`.cursorindexingignore`) | `.cursor/rules/*.mdc` ✅ · `.cursorrules` ⚠️ deprecated · `AGENTS.md` ✅ |
| GitHub Copilot | ❌ none (UI "content exclusion", Business/Enterprise only) | `.github/copilot-instructions.md` ✅ · `.github/instructions/*.instructions.md` ✅ · `AGENTS.md` ✅ |
| Windsurf / Codeium | `.codeiumignore` ✅ (+global `~/.codeium/.codeiumignore`) · `.windsurfignore` ❌ | `.windsurf/rules/*.md` ✅ · `~/.codeium/windsurf/memories/global_rules.md` ✅ · `AGENTS.md` ✅ |
| Cline | `.clineignore` ✅ | `.clinerules` file or `.clinerules/` dir ✅ · `AGENTS.md` ✅ |
| Roo Code | `.rooignore` ✅ | `.roo/rules/` ✅ · `.roorules` ⚠️ legacy · `AGENTS.md` ✅ |
| Aider | `.aiderignore` ✅ | `.aider.conf.yml` ✅ (`.yml` only) · `CONVENTIONS.md` (not auto-loaded) · `AGENTS.md` ✅ opt-in |
| Continue.dev | `.continueignore` ✅ (+global) | `.continue/rules/*.md` ✅ / `config.yaml` rules |
| Claude Code | ❌ no `.claudeignore`; use `permissions.deny Read(...)` in `.claude/settings.json` ✅ | `CLAUDE.md` ✅ (+`CLAUDE.local.md`, `~/.claude/CLAUDE.md`) · `AGENTS.md` ✅ |
| Gemini CLI | `.geminiignore` ✅ | `GEMINI.md` ✅ |
| Gemini Code Assist (IDE) | `.aiexclude` ✅ | — |
| Amazon Q Developer | ❌ none documented | `.amazonq/rules/*.md` ✅ |
| OpenAI Codex CLI | ❌ `.codexignore` absent (requested) | `AGENTS.md` ✅ · `~/.codex/config.toml` |
| Tabnine | `.tabnineignore` ✅ | `.tabnine/guidelines/*.md` ✅ |
| Zed | ⚠️ none AI-specific (`.gitignore` + `private_files`) | rules chain → `AGENTS.md` canonical ✅ |
| JetBrains AI / Junie | `.aiignore` ✅ | `.junie/guidelines.md` ✅ · `AGENTS.md` ✅ |
| Sourcegraph Cody | `.cody/ignore` ⚠️ SUNSET → server-side `cody.contextFilters` only | ❌ none |
| Augment | `.augmentignore` ✅ | `.augment/rules/*.md` ✅ · `AGENTS.md` |

**`AGENTS.md`** — cross-tool rules standard (agents.md), governance under **Linux Foundation / Agentic AI Foundation**; rules-only (no ignore counterpart in spec); adopted by Codex, Copilot, Cursor, Gemini, Cline, Roo, Zed, Junie, VS Code, Aider, Windsurf, Augment, others. **EnvForge fence should always write `AGENTS.md`.**

### A.2 MCP — spec, config paths, precedent

- **Spec:** latest released `2025-11-25` (added async `Tasks`); RC `2026-07-28` in progress. Governance donated to **AAIF / Linux Foundation** (⚠️ exact date). Transport: **Streamable HTTP** (SSE deprecated). Auth: `2025-06-18` baseline OAuth 2.1 — PKCE, RFC 9728, RFC 8414, **RFC 8707 resource indicators** (⚠️ verify RFC list). Official Registry preview since 2025-09-08.
- **Config paths:** Claude Code project `.mcp.json` (root) + `~/.claude.json`; Cursor `.cursor/mcp.json` (+`~/.cursor/mcp.json`); VS Code `.vscode/mcp.json` (GA 1.102, Jul 2025); Claude Desktop `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS; Linux path ⚠️); Windsurf `~/.codeium/windsurf/mcp_config.json` ⚠️; Cline `cline_mcp_settings.json` in VS Code globalStorage ⚠️.
- **Precedent for shipping our own:** HashiCorp Vault ✅, Bitwarden ⚠️, Infisical ⚠️ all ship first-party MCP servers. EnvForge differentiator = redaction-by-default + canary/fence at the boundary.
- **Security mapping:** OWASP MCP Top 10 (2025) — **MCP01 Token Mismanagement & Secret Exposure** and **MCP03 Tool Poisoning** map onto EnvForge's existing MCP-scan; use these category names. Tool-Poisoning-Attack (Invariant Labs, Apr 2025).

### A.3 Editors — UI capability (drives plugin vs LSP-only)

- **First-party native UI viable:** VS Code (shipped), Visual Studio, **Neovim** (Lua: statusline/`sign_column`/extmarks), **Emacs** (mode-line/fringe/overlays), **Sublime** (status bar/phantoms/gutter). Neovim+Emacs = strongest CLI-user overlap.
- **LSP-only (host can't do custom UI):** **Zed** (WASM ext API; Visual Extension API is Draft RFC #53403 — **not shippable**; MCP-server extension *is* supported), **Helix** (plugin system pre-stable), **Lapce** (WASI plugins, small).
- **JetBrains Fleet — DISCONTINUED** (no downloads after Dec 22 2025 → "Air"). **Drop.** Existing IntelliJ Platform plugin unaffected.
- The **LSP server is the highest-leverage investment** — sole surface for Zed/Helix/Lapce, baseline everywhere.

### A.4 Market priority & security validation

- **Adoption (SO 2025 survey, directional):** editors — VS Code 75.9%, Cursor 17.9%, Claude Code 9.7%, Zed 7.3%, Neovim 14.0%. AI assistants — Copilot 67.9%, Claude Code 40.8%. 84% use/plan AI; only **29% trust accuracy**.
- **Integration priority order:** **1) GitHub Copilot** (4.7M paid, +75% YoY) → **2) Cursor** (≈$2B ARR; SpaceX $60B acquisition announced Jun 16 2026 ✅; ">$50B Series E" = rumor) → **3) Claude Code** ($2.5B+ run-rate) → Gemini → Amazon Q → JetBrains AI/Junie → Cline → Zed → Continue → Windsurf → Aider → Tabnine.
- **Fence thesis — strongly validated:**
  - **Claude Code silently auto-loads `.env` into memory** (verified, prod) — knostic.ai.
  - **GitGuardian State of Secrets Sprawl 2026:** 29M new hardcoded secrets on public GitHub 2025 (+34% YoY); **AI-service secret leaks +81% YoY**; **AI-assisted commits leak ~2× more** (3.2% vs 1.5%).
  - Verified exfil-chain PoCs: Supabase MCP (`service_role` token dump), GitHub MCP toxic-agent (private→public PR), "Comment-and-Control" multi-vendor (Anthropic rated one **CVSS 9.4**). CVE-2025-59536 (Claude Code RCE+key exfil, 8.7); Gemini CLI env-var exfil PoC (10.0).
  - **Framing:** every chain starts with an agent reading secrets it never needed. The fence removes **leg #1 of Simon Willison's "lethal trifecta"** (private-data access) — the one structural mitigation that does not depend on the LLM choosing to obey.

### A.5 Open items to firm up before code-lock

Exact Windsurf/Cline MCP config strings; Claude Desktop Linux path; MCP OAuth RFC list; AAIF donation date; primary sources for the `.env` auto-load and `.claudeignore`-not-enforced claims. Treat adoption %s as directional (opt-in surveys).
