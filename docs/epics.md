---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics', 'step-03-create-stories', 'step-04-final-validation']
status: 'complete'
inputDocuments:
  - 'docs/prd.md'
  - 'docs/architecture.md'
  - 'docs/over-engineering-analysis.md'
---

# EnvForge v0.9 "Omnipresence" - Epic Breakdown

## Overview

Complete epic/story breakdown for EnvForge v0.9, decomposing the PRD (FRs/NFRs) and Architecture (D1–D8) into implementable stories. Scope reflects the applied over-engineering trim: MCP = local stdio + 2 tools; Tier-1/OSS tools + `AGENTS.md`; detection folded into `fence --status`; long-tail tools, MCP remote/extra tools, `doctor --ai` standalone, and Emacs/Sublime plugins are Vision (out of v0.9).

## Requirements Inventory

### Functional Requirements

FR1: A developer can fence all detected AI tools with one command, generating each tool's correct ignore and/or rules files in one pass.
FR2: EnvForge can natively fence: Cursor, GitHub Copilot, Claude (Code/Desktop), Windsurf/Codeium, Cline, Aider, Gemini CLI. (Long-tail tools = Vision data.)
FR2a: EnvForge writes/maintains an `AGENTS.md` rules file (cross-tool standard) as a fence target.
FR2b: For tools without an ignore mechanism (Copilot, Claude Code, Amazon Q, Codex, Zed), EnvForge applies the tool's available protection (deny rule / rules file) + `AGENTS.md` + canary, never implying a non-existent ignore file.
FR3: Disabling a fence removes only EnvForge-owned content, preserving user-authored content in shared files.
FR4: A developer can enable/disable fencing per individual tool target.
FR5: Fence targets are data registry entries — a new tool = a new entry, not new control flow.
FR6: Every fence write round-trips byte-safely; protected zones (conda, Amazon Q) never disturbed.
FR7: `fence --status` reports coverage per named tool, not just an aggregate boolean.
FR8: EnvForge surfaces an explicit "installed-but-unfenced" state for detected-but-unfenced tools.
FR9: Fence status is machine-readable JSON with deterministic exit codes for CI gating.
FR10: `fence --status` performs lightweight registry-hint detection of installed tools and reports fenced/unfenced + recommended fix. (Standalone `doctor --ai` = Vision.)
FR11: Aggregate "AI BLOCKED" reads protected only when all detected tools are covered.
FR12: An MCP-capable agent can connect to `envforge mcp` over local stdio.
FR13: Through the MCP server, an agent can `list_keys` (key names) and `describe_schema` (types/required/defaults + redacted previews) — no raw secret value.
FR15: The MCP server exposes no raw-value capability; all responses redacted by construction.
FR16: Every access-class MCP operation is audit-logged without the secret value appearing.
FR17: EnvForge ships ready-to-use MCP config snippets for major clients (Claude Desktop/Code, Cursor, VS Code, Windsurf, Cline).
FR18: EnvForge flags hardcoded credentials in AI-tool/agent config files across an expanded set of path patterns, as editor diagnostics + CLI.
FR19: Config-file credential findings include a quick-fix/recommendation to use an env-var reference.
FR20: A Neovim user can install a first-party plugin (statusline, exposure heatmap, fence-toggle) on top of the LSP.
FR21: A Zed user can use EnvForge via LSP wiring + the `envforge mcp` server registered as a Zed extension (no custom UI).
FR22: All editor clients consume identical LSP behavior; no client re-implements logic; parity test-enforced.
FR23: Verified LSP-only support maintained for Helix, Emacs, Sublime, Lapce, Kakoune. (Fleet dropped.)
FR24: A security lead can gate CI on fence coverage + config-credential lint (unfenced/hardcoded → build fails).
FR25: Each registry entry references the authoritative source for the tool's convention (verifiability).
FR26: EnvForge publishes an up-to-date integration matrix; README/CHANGELOG test counts kept in sync.

### NonFunctional Requirements

NFR-S1: No raw secret value appears in any MCP response under any input (property/fuzz verified).
NFR-S2: All secret-bearing output (CLI/LSP/MCP) passes through the shared redaction routine.
NFR-S3: Every reveal/access-class op emits an audit event whose message excludes the value.
NFR-S4: Fence disable preserves 100% of user-authored content in shared files (round-trip tested).
NFR-S5: Secrets stay encrypted at rest/in transit/in memory (existing invariant), unchanged.
NFR-P1: Full `envforge fence` pass < 500 ms p95 on ≤15 targets (CLI-timed in tests).
NFR-P2: `fence --status` (incl. detection) < 300 ms p95.
NFR-P3: MCP request latency dominated by the underlying `ops/` call; no extra disk walks.
NFR-P4: No regression to existing LSP latencies; lint reuses existing scan heuristics.
NFR-R1: Parse→serialize byte-identical for every file EnvForge writes.
NFR-R2: All writes atomic; crash mid-write never corrupts a config file.
NFR-R3: Fence ops idempotent; re-plant of present canary is a no-op.
NFR-R4: One target's failure doesn't abort others; partial results reported, not swallowed.
NFR-R5: MCP server runs isolated; a fault there can't take down the LSP.
NFR-C1: All new features work identically on Linux + macOS.
NFR-C2: Minimum Rust 1.75 maintained.
NFR-C3: LSP contract backward-compatible; every existing `tests/lsp_*` ID passes unchanged.
NFR-C4: MCP server conforms to spec `2025-11-25` over stdio (v0.9); interop with ≥2 clients verified.
NFR-M1: Adding an AI-tool fence target requires only a registry data entry + tests — no control-flow change.
NFR-M2: Registry entries validated by schema/test at build time; invalid entry fails CI.
NFR-M3: IDE feature parity test-enforced; plugin/LSP divergence is a failing test.

### Additional Requirements

- **No starter template** — brownfield; v0.9 extends the existing Cargo workspace. No project-init story.
- **New dependency:** `rmcp` (official Rust MCP SDK) behind Cargo feature `mcp-server` (gated so fence breadth builds without it).
- **Architecture decisions D1–D8** drive the epics: D1 registry data model (`ops/fence/registry.rs`), D2 FileKind writers (`ops/fence/writers.rs`), D3 coverage/status model, D4 detection folded into status, D5 MCP egress safety, D6 MCP server (`src/mcp/`), D7 LSP linter glob expansion, D8 plugins (`editors/nvim`, `editors/zed`).
- **Shared `ops::redact`** choke point for all sensitive output (relocate/share from `lsp::redact`).
- **Docs:** integration matrix (`docs/integration-matrix.md`), `lsp-clients.md`/`ide-behavior-contract.md` updates, README/CHANGELOG test-count sync.
- **Unverified before code-lock:** exact MCP config paths for Windsurf/Cline + Claude Desktop Linux (PRD appendix flags).

### UX Design Requirements

N/A — CLI/LSP tool, no UX design document. Plugin UI behavior is fixed by `ide-behavior-contract.md` (parity contract), not a UX spec.

### FR Coverage Map

FR1: Epic 1 — one-command multi-tool fence
FR2: Epic 1 — Tier-1/OSS tool fence support
FR2a: Epic 1 — `AGENTS.md` cross-tool target
FR2b: Epic 1 — no-ignore-file fallback (deny/rules)
FR3: Epic 1 — content-preserving disable
FR4: Epic 1 — per-target enable/disable
FR5: Epic 1 — registry as data
FR6: Epic 1 — byte-safe round-trip + protected zones
FR7: Epic 1 — per-tool status
FR8: Epic 1 — installed-but-unfenced state
FR9: Epic 1 — JSON status + exit codes
FR10: Epic 1 — registry-hint detection in status
FR11: Epic 1 — honest aggregate "AI BLOCKED"
FR25: Epic 1 — per-entry source URL (verifiability)
FR12: Epic 2 — MCP connect over stdio
FR13: Epic 2 — `list_keys` + `describe_schema`
FR15: Epic 2 — no raw-value capability
FR16: Epic 2 — audit-logged MCP ops
FR17: Epic 2 — MCP config snippets
FR18: Epic 3 — config-credential lint breadth
FR19: Epic 3 — credential-finding quick-fix
FR20: Epic 4 — Neovim plugin
FR21: Epic 4 — Zed LSP+MCP integration
FR22: Epic 4 — client parity (no re-implemented logic)
FR23: Epic 4 — LSP-only matrix maintained
FR24: Epic 5 — CI gating on coverage + lint
FR26: Epic 5 — integration matrix + test-count sync

## Epic List

### Epic 1: Universal AI-Tool Fencing
A developer fences every AI tool in their stack with one command and sees honest, per-tool coverage — the core "no silent holes" thesis. Refactors fence targets into a data-driven registry (D1) with FileKind-dispatched writers (D2), adds Tier-1/OSS tools + `AGENTS.md`, and folds detection into a per-tool `fence --status` (D3/D4). Standalone: delivers the full fence value with no dependency on later epics. Keystone — everything reuses the registry.
**FRs covered:** FR1, FR2, FR2a, FR2b, FR3, FR4, FR5, FR6, FR7, FR8, FR9, FR10, FR11, FR25
**Key NFRs:** NFR-R1/R2/R3/R4 (round-trip, atomic, idempotent, partial-failure), NFR-S4 (content preserve), NFR-P1/P2 (perf), NFR-M1/M2 (registry-as-data, build-time validation)
**Core files:** `ops/fence.rs`, `ops/fence/registry.rs`, `ops/fence/writers.rs`, `ops/detect.rs`, `cli/commands.rs`, `lsp/commands.rs`

### Epic 2: Safe AI Egress — EnvForge MCP Server
An MCP-capable agent gets env metadata (key names + redacted schema) through a guarded, audited channel instead of reading `.env` raw. New `src/mcp/` on `rmcp` over stdio, 2 read-safe tools, redaction-by-construction + audit (D5/D6). Standalone + feature-gated (`mcp-server`); independent of Epic 1.
**FRs covered:** FR12, FR13, FR15, FR16, FR17
**Key NFRs:** NFR-S1 (no-secret fuzz), NFR-S2/S3 (redact, audit), NFR-R5 (process isolation), NFR-C4 (spec/stdio), NFR-P3
**Core files:** `src/mcp/{mod,server,tools}.rs`, `cli/mcp_serve_cmd.rs`, `ops/redact.rs`, `Cargo.toml` (`mcp-server` feature + `rmcp`)

### Epic 3: AI-Config Leak Linting
A developer (and CI) sees hardcoded credentials in AI-tool/agent config files flagged across more path patterns, with a quick-fix to env-var references. Additive expansion of the existing `envforge-mcp` LSP diagnostic + plugin manifests (D7). Standalone; reuses existing `ops::mcp_scan` heuristics unchanged.
**FRs covered:** FR18, FR19
**Key NFRs:** NFR-P4 (no extra scan passes), NFR-C3 (additive LSP contract)
**Core files:** `lsp/mcp_diagnostics.rs`, `editors/{vscode,intellij}` manifests, `ide-behavior-contract.md` (L14)

### Epic 4: Editor Reach
Developers beyond VS Code/IntelliJ get EnvForge natively: a first-party Neovim plugin (statusline + exposure heatmap + fence toggle) and Zed via LSP + the MCP server, with verified LSP-only support across the rest — all consuming identical LSP behavior (D8, parity). Depends only on the (unchanged) LSP contract → standalone.
**FRs covered:** FR20, FR21, FR22, FR23
**Key NFRs:** NFR-M3 (parity test-enforced), NFR-C1 (Linux+macOS)
**Core files:** `editors/nvim/`, `editors/zed/`, `docs/lsp-clients.md`

### Epic 5: Governance, CI & Coverage Proof
A security lead can prove and enforce coverage: CI gates on `fence --status` JSON + config-credential lint (build fails on unfenced/hardcoded), and EnvForge ships an integration matrix with synced test counts. Cross-cutting capstone consuming Epic 1 (status JSON) + Epic 3 (lint) outputs; runs last.
**FRs covered:** FR24, FR26
**Key NFRs:** NFR-C2 (Rust 1.75), docs-sync discipline
**Core files:** CI config, `docs/integration-matrix.md`, `README.md`/`CHANGELOG.md`

## Epic 1: Universal AI-Tool Fencing

Refactor fence targets into a data-driven registry with FileKind-dispatched writers, add Tier-1/OSS tools + `AGENTS.md`, and deliver honest per-tool `fence --status` with folded-in detection. Keystone epic — everything downstream reuses the registry. Stories ordered D1→D2→data→D3/D4; each builds only on prior stories.

### Story 1.1: Fence target registry data model

As a maintainer,
I want fence targets defined as a compile-time data registry instead of an enum with bespoke writers,
So that adding a tool is a data entry, not new control flow.

**Acceptance Criteria:**

**Given** the current 5 hardcoded `FenceTarget` variants
**When** the registry (`ops/fence/registry.rs`) is introduced as `&'static [FenceTargetSpec]` with `TargetFile { path, kind, ownership }`, `FileKind`, `Ownership`, `detection`, `source_url`, `has_real_ignore`
**Then** the existing 5 targets (`.envforgeignore`, `.cursorignore`, `.cursorrules`, copilot-instructions, `.claude/settings.json`) are represented as registry entries
**And** `FenceTarget`/`FenceTargets`/`resolve_fence_targets`/`fence config` read the registry rather than hardcoded match arms
**And** a build-time test validates every entry (non-empty id/path, valid kind, source_url present) — an invalid entry fails CI (NFR-M2)
**And** all existing fence tests pass unchanged (no behavior change).

### Story 1.2: FileKind-dispatched writers and strippers

As a maintainer,
I want one writer/stripper per `FileKind` instead of per-target functions,
So that new tools reuse a kind and content-preservation is uniform.

**Acceptance Criteria:**

**Given** the registry from Story 1.1
**When** `write_block`/`strip_block` are implemented for `Ignore` (marked line-block), `Rules`/`CrossTool` (marked markdown block), and `DenyRule` (JSON merge into `permissions.deny`)
**Then** enabling a target writes the correct content per its kind, and disabling strips only EnvForge-owned content per `Ownership` (FR3)
**And** parse→serialize is byte-identical and protected zones (conda, Amazon Q) are untouched (FR6, NFR-R1)
**And** writes are atomic (tempfile+rename) and re-running produces no diff (NFR-R2/R3)
**And** each kind has a round-trip + content-preservation test; user-authored content in shared files is preserved 100% (NFR-S4)
**And** the prior per-target `write_*` functions are removed.

### Story 1.3: Tier-1 and OSS tool fence entries

As a developer,
I want EnvForge to fence Windsurf/Codeium, Cline, Aider, and Gemini CLI natively,
So that the high-adoption tools in my stack are covered.

**Acceptance Criteria:**

**Given** the registry + writers from 1.1/1.2
**When** entries are added for Windsurf/Codeium (`.codeiumignore` + `.windsurf/rules/`), Cline (`.clineignore` + `.clinerules`), Aider (`.aiderignore` + `.aider.conf.yml`), Gemini CLI (`.geminiignore` + `GEMINI.md`)
**Then** each uses its verified current convention with a `source_url` (FR2, FR25)
**And** the misnomers `.windsurfignore`/`.claudeignore` are NOT used
**And** each new entry has round-trip + content-preservation tests
**And** `fence` and `fence config` operate on the new targets without code changes (NFR-M1).

### Story 1.4: AGENTS.md target and no-ignore-file fallbacks

As a developer,
I want tools without an ignore file still protected via rules/deny + `AGENTS.md`,
So that there are no silent holes for Copilot, Claude Code, Amazon Q, Codex, or Zed.

**Acceptance Criteria:**

**Given** the writers support `Rules`/`CrossTool`/`DenyRule` kinds
**When** an `AGENTS.md` `CrossTool` entry is added, plus fallback handling for tools with `has_real_ignore=false` (Copilot rules, Claude Code `permissions.deny`, etc.)
**Then** fencing those tools writes the available protection (deny/rules) + `AGENTS.md` + canary rather than a non-existent ignore file (FR2a, FR2b)
**And** `has_real_ignore=false` entries are marked so status never reports a false "fenced" (feeds Story 1.6)
**And** disabling preserves user content in `AGENTS.md`/settings files.

### Story 1.5: One-command fence across all targets + per-target toggle

As a developer,
I want one `envforge fence` to cover every registry target and `fence config` to toggle individuals,
So that I protect my whole AI toolchain in one pass.

**Acceptance Criteria:**

**Given** the populated registry (1.1–1.4)
**When** I run `envforge fence`
**Then** every enabled target is written in one pass, preserving user content, reporting per-target success/skip (FR1)
**And** one target's failure does not abort the others; partial results are reported (NFR-R4)
**And** `envforge fence config --enable/--disable <id>` toggles a single target and `--list` shows per-target enabled state (FR4)
**And** a full fence pass over ≤15 targets completes < 500 ms p95 in an integration timing test (NFR-P1).

### Story 1.6: Per-tool fence status with honest aggregate

As a developer,
I want `fence --status` to name each tool's coverage and only show "AI BLOCKED" when all are covered,
So that I'm never given a false sense of security.

**Acceptance Criteria:**

**Given** a fenced/partially-fenced project
**When** I run `envforge fence --status [--json]`
**Then** output lists each target by tool name with state `covered | fallback | unfenced | not_installed` (FR7)
**And** JSON output + deterministic exit codes are emitted for CI (FR9)
**And** the aggregate "AI BLOCKED" reads protected only when every detected target is `covered` or `fallback` (FR11)
**And** the `resolved_targets` JSON contract from v0.8.3 is extended, not reshaped (plugins keep working)
**And** status returns < 300 ms p95 (NFR-P2).

### Story 1.7: Registry-hint detection (installed-but-unfenced)

As a developer who just installed a new AI tool,
I want `fence --status` to detect installed-but-unfenced tools,
So that a newly added tool can't silently expose my secrets.

**Acceptance Criteria:**

**Given** the registry entries carry `detection` hints
**When** `fence --status` runs
**Then** it matches hints against the workspace + known config locations and marks detected-but-unfenced tools as `unfenced` with a recommended fix (FR8, FR10)
**And** detection adds no separate command (folded into status) and keeps status < 300 ms p95
**And** a tool present and fenced reports `covered`; absent reports `not_installed`.

## Epic 2: Safe AI Egress — EnvForge MCP Server

A local-stdio MCP server (`envforge mcp`) exposing 2 read-safe, redacted, audited tools so agents get env metadata without raw file access. Feature-gated; independent of Epic 1. Stories: skeleton → shared redact → tools → audit/fuzz → config snippets.

### Story 2.1: MCP server skeleton over stdio (feature-gated)

As a maintainer,
I want an `envforge mcp` subcommand that boots an `rmcp` stdio server behind a Cargo feature,
So that fence breadth still builds without MCP and the server runs isolated.

**Acceptance Criteria:**

**Given** the existing Cargo workspace
**When** `rmcp` is added as an optional dep behind feature `mcp-server` and `src/mcp/{mod,server}.rs` + `cli/mcp_serve_cmd.rs` are created
**Then** `envforge mcp` starts an MCP server over stdio that completes the spec `2025-11-25` handshake with a client (FR12, NFR-C4)
**And** the server runs as its own process/transport, isolated from the LSP (NFR-R5)
**And** `cargo build` without the feature excludes `rmcp` and `src/mcp/`
**And** `cargo build --features mcp-server` compiles clean (clippy -D warnings).

### Story 2.2: Shared redaction routine

As a maintainer,
I want one shared `ops::redact` used by CLI, LSP, and MCP,
So that no surface can emit an unredacted sensitive value.

**Acceptance Criteria:**

**Given** redaction currently lives in `lsp::redact::redact_for_label`
**When** it is relocated/shared as `ops::redact` with `lsp::redact` re-exporting it
**Then** all existing call sites compile and behave identically (no test changes)
**And** the function is available to `src/mcp/` without depending on the `lsp` module.

### Story 2.3: list_keys and describe_schema tools

As an AI agent,
I want to list env-var key names and describe the schema with redacted previews,
So that I can scaffold/explain `.env` without reading raw secrets.

**Acceptance Criteria:**

**Given** the MCP server (2.1) and shared redact (2.2)
**When** the `list_keys` and `describe_schema` tools are registered, each delegating to the same `ops/` fn the CLI/LSP use
**Then** `list_keys` returns key names only and `describe_schema` returns types/required/defaults/examples + redacted current-value previews (FR13)
**And** no tool returns a raw secret value, and no `get_value`-class tool exists (FR15)
**And** tool output is byte-identical to the equivalent CLI/LSP path (minus redaction).

### Story 2.4: MCP audit logging + no-secret guarantee test

As a security lead,
I want every MCP call audited and a test proving no secret can leak through MCP,
So that the egress is provably safe.

**Acceptance Criteria:**

**Given** the MCP tools from 2.3
**When** a tool is invoked
**Then** a `RuntimeEvent` is emitted to the monitor bus whose message excludes the secret value (FR16, NFR-S3)
**And** a property/fuzz test asserts that over randomized env/schema state no high-entropy token appears in any MCP tool response (NFR-S1)
**And** the audit message format mirrors the existing C6 pattern (`MCP <tool>: <key>`).

### Story 2.5: MCP client config snippets

As a developer,
I want ready-to-paste config snippets for major MCP clients,
So that I can wire EnvForge's MCP server into my agent in seconds.

**Acceptance Criteria:**

**Given** the working `envforge mcp` server
**When** docs are added
**Then** copy-paste config snippets exist for Claude Desktop (`claude_desktop_config.json`), Claude Code (`.mcp.json`/`~/.claude.json`), Cursor (`.cursor/mcp.json`), VS Code (`.vscode/mcp.json`), Windsurf, Cline (FR17)
**And** each snippet is verified against ≥2 real clients (NFR-C4)
**And** paths flagged unverified in the PRD appendix (Windsurf/Cline, Claude Desktop Linux) are confirmed before publishing.

## Epic 3: AI-Config Leak Linting

Expand the existing `envforge-mcp` LSP diagnostic to more AI-tool/agent config paths with a quick-fix, reusing `ops::mcp_scan` heuristics unchanged. Additive; preserves parity.

### Story 3.1: Expand MCP-config lint coverage

As a developer,
I want hardcoded credentials flagged across more AI-tool config files,
So that secrets pasted into agent configs are caught.

**Acceptance Criteria:**

**Given** the current `envforge-mcp` diagnostic covers 5 path patterns
**When** the `documentSelector` glob set is widened to include `.vscode/mcp.json`, `.cursor/mcp.json`, Claude Code `.mcp.json`/`~/.claude.json`, and Windsurf/Cline MCP configs
**Then** opening/editing those files surfaces credential findings using the existing `ops::mcp_scan` heuristics (FR18)
**And** no new scan logic is added and there is no added scan pass (NFR-P4)
**And** existing `tests/lsp_*` diagnostic IDs pass unchanged (NFR-C3).

### Story 3.2: Credential-finding quick-fix to env-var reference

As a developer,
I want a one-click fix that replaces a hardcoded literal with an env-var reference,
So that I can remediate a flagged credential immediately.

**Acceptance Criteria:**

**Given** a credential finding in a config file
**When** I invoke the code action / CLI recommendation
**Then** the literal is replaced with an `${ENV_VAR}`-style reference managed by EnvForge (FR19)
**And** the action is offered both as an LSP `codeAction` and surfaced via CLI
**And** a test fences the quick-fix edit shape.

### Story 3.3: Plugin manifest + behavior-contract sync

As a maintainer,
I want the VS Code/IntelliJ manifests and the behavior contract to match the new lint coverage,
So that first-party plugins and the LSP stay in parity.

**Acceptance Criteria:**

**Given** the widened lint coverage (3.1)
**When** plugin `documentSelector`/`languageMapping` patterns are extended and `ide-behavior-contract.md` L14 row is updated
**Then** both first-party plugins surface the new findings identically to the LSP (FR22-style parity)
**And** the contract's pinned wording/source tag (`envforge-mcp`) is preserved.

## Epic 4: Editor Reach

First-party Neovim plugin + Zed via LSP+MCP, with the LSP-only matrix verified and parity test-enforced. Depends only on the unchanged LSP contract.

### Story 4.1: Neovim plugin — LSP wiring, statusline, fence toggle

As a Neovim user,
I want a first-party EnvForge plugin with a statusline indicator and fence-toggle command,
So that I get native EnvForge UX in my editor.

**Acceptance Criteria:**

**Given** `envforge lsp` and the `envforge fence` CLI
**When** the `editors/nvim/` Lua plugin is installed
**Then** it auto-registers the LSP for `.env*`/source filetypes, shows a statusline segment (`<N> vars · AI BLOCKED/ALLOWED`) from `envforge fence --status --json`, and provides a `:EnvForgeFenceToggle` command (FR20)
**And** the plugin contains no business logic — it only calls the LSP/CLI (FR22)
**And** it works on Linux + macOS (NFR-C1).

### Story 4.2: Neovim exposure heatmap

As a Neovim user,
I want red/amber/green exposure markers in the sign column for `.env*` lines,
So that I can see per-var AI exposure inline.

**Acceptance Criteria:**

**Given** the Neovim plugin (4.1) and `envforge exposure --file PATH --json`
**When** I open a `.env*` file
**Then** each env-var line gets a sign-column/virtual-text marker colored by exposure level, with a canary shield glyph where applicable (FR20)
**And** markers refresh on save and clear on non-env files
**And** the rendering matches the `ide-behavior-contract.md` exposure data (no divergence).

### Story 4.3: Zed extension — LSP + MCP registration

As a Zed user,
I want EnvForge wired via the LSP plus the MCP server registered as a Zed extension,
So that I get diagnostics/exposure and safe agent egress without a custom-UI plugin.

**Acceptance Criteria:**

**Given** `envforge lsp` and `envforge mcp` (Epic 2)
**When** the `editors/zed/extension.toml` is installed
**Then** Zed runs the EnvForge language server for `.env*` and registers `envforge mcp` as an MCP server extension (FR21)
**And** no custom status-bar/gutter UI is attempted (Zed API limitation documented)
**And** the fence writes the rules-chain files Zed honors (`AGENTS.md` canonical, from Epic 1).

### Story 4.4: LSP-only matrix verification + parity

As a maintainer,
I want verified LSP-only support across the remaining editors and parity tests,
So that any plugin/LSP divergence is caught and Fleet is removed.

**Acceptance Criteria:**

**Given** the LSP contract (unchanged)
**When** setup is verified for Helix, Emacs, Sublime Text, Lapce, Kakoune and Fleet is removed from docs
**Then** `docs/lsp-clients.md` reflects first-party Neovim + LSP-only matrix with Fleet dropped (FR23)
**And** parity tests fence the shared LSP response bodies so first-party plugins cannot diverge (FR22, NFR-M3)
**And** every existing `tests/lsp_*` ID still passes (NFR-C3).

## Epic 5: Governance, CI & Coverage Proof

Capstone: CI gates on coverage + lint, and EnvForge ships a maintained integration matrix with synced test counts. Consumes Epic 1 status JSON + Epic 3 lint.

### Story 5.1: CI gating on fence coverage + credential lint

As a security lead,
I want CI to fail when an AI-tool config is unfenced or contains a hardcoded credential,
So that coverage is enforced, not hoped-for.

**Acceptance Criteria:**

**Given** `fence --status --json` exit codes (Epic 1) and the config lint (Epic 3)
**When** a CI job runs them on a repo
**Then** the build fails if any detected tool is `unfenced` or any config-credential finding exists (FR24)
**And** the job emits human-readable output naming the offending tool/file
**And** a documented CI snippet (GitHub Actions) is provided.

### Story 5.2: Integration matrix documentation

As a developer evaluating EnvForge,
I want a single matrix of tools × editors × capabilities,
So that I can see exactly what's covered.

**Acceptance Criteria:**

**Given** the v0.9 coverage
**When** `docs/integration-matrix.md` is created
**Then** it tabulates each AI tool (fence mechanism + status) and each editor (first-party vs LSP-only vs LSP+MCP) against capabilities (FR26)
**And** it cites each tool-convention `source_url` from the registry
**And** it marks deferred/Vision items explicitly (long-tail tools, MCP remote, Emacs/Sublime).

### Story 5.3: Docs and test-count synchronization

As a maintainer,
I want README/CHANGELOG and LSP docs synced after the v0.9 test additions,
So that published counts and client guidance stay accurate.

**Acceptance Criteria:**

**Given** the new tests from Epics 1–4
**When** v0.9 is finalized
**Then** README and CHANGELOG test counts are updated to the new total (docs-sync discipline) (FR26)
**And** `docs/lsp-clients.md` and `docs/ide-behavior-contract.md` reflect the new clients, lint rows, and dropped Fleet
**And** `cargo fmt && cargo clippy -- -D warnings && cargo test` is green.

## Validation Summary

**FR coverage:** all 26 active v0.9 FRs map to ≥1 story — E1: FR1(1.5),2(1.3),2a(1.4),2b(1.4),3(1.2),4(1.5),5(1.1),6(1.2/1.3),7(1.6),8(1.7),9(1.6),10(1.7),11(1.6),25(1.1/1.3); E2: FR12(2.1),13(2.3),15(2.3),16(2.4),17(2.5); E3: FR18(3.1),19(3.2); E4: FR20(4.1/4.2),21(4.3),22(4.1/4.4),23(4.4); E5: FR24(5.1),26(5.2/5.3). **0 uncovered.** (FR14, FR23a = Vision, intentionally excluded.)

**Architecture compliance:** no starter template (brownfield) → first story is the registry refactor (keystone), not project init. No DB/entities. Registry created once in 1.1. `rmcp`/`mcp-server` feature introduced only in Story 2.1 (where first needed).

**Story quality:** all single-dev-agent sized; Given/When/Then ACs; NFRs woven in (round-trip, p95, fuzz, parity, atomic). Stories 1.1/1.2 are pure refactors ("existing tests pass, no behavior change") — safe keystone landing.

**Epic structure:** user-value oriented (not tech layers). File-churn check — all `ops/fence*` work consolidated into E1 (no churn); E2=`src/mcp/`, E3=`lsp/mcp_diagnostics.rs`, E4=`editors/`, E5=CI/docs → distinct components. Consolidation of registry+status into E1 was explicit.

**Dependencies:** epics independent and individually shippable; build order E1→E2→E3→E4→E5. Within-epic stories build only on prior stories (verified). **One cross-epic note:** Story 4.3 (Zed) registers the Epic 2 MCP server — satisfied because E2 precedes E4; Zed still delivers LSP value if E2 absent.

**Result: PASS — ready for development.** First story to implement: **1.1 (fence registry data model)** — the keystone.
