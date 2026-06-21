---
stepsCompleted: ['step-01-init', 'step-02-discovery', 'step-02b-vision', 'step-02c-executive-summary', 'step-03-success', 'step-04-journeys', 'step-05-domain', 'step-06-innovation', 'step-07-project-type', 'step-08-scoping', 'step-09-functional', 'step-10-nonfunctional', 'step-11-polish', 'step-12-complete']
inputDocuments: ['docs/ide-behavior-contract.md', 'docs/lsp-clients.md', 'docs/integration-matrix.md']
workflowType: 'prd'
classification:
  projectType: 'Developer tool (CLI + TUI + LSP language server)'
  domain: 'Developer tooling / DevSecOps (env-var & secrets management, AI safety)'
  complexity: 'medium-high'
  projectContext: 'brownfield'
releaseMode: 'phased'
---

# Product Requirements Document - EnvForge

**Author:** Emre
**Date:** 2026-06-20

## Executive Summary

EnvForge today understands shell rc files and `.env` files, and exposes IDE-grade language features over them through its own LSP server (`src/lsp/`): hover, completions, go-to-definition, find-references, document/semantic highlighting, diagnostics, and rename. But in real polyglot repositories, the bulk of application configuration does not live in `.env` — it lives in framework config files: Spring Boot / Quarkus `application.properties` and `application.yml`, with profile variants like `application-prod.yml`. EnvForge is blind to these surfaces today, which means the keys developers most often read, complete, and jump to are exactly the ones the tool cannot help with — and exactly the ones most likely to leak secrets to AI agents.

This release extends EnvForge's existing LSP and parser machinery to **Java/JVM framework config files**. Phase 1 delivers full Spring Boot coverage — `application.properties`, `application.yml`, and their `application-{profile}.*` profile variants — plus Quarkus / MicroProfile `application.properties`, and formalizes the `.env` cascade convention that already covers Node, Python, PHP, and Go. Language features (hover, completions, go-to-definition, find-references, highlight, diagnostics) work across all targeted formats. YAML is supported **read-only** for language features in Phase 1: hover/completion/go-to-def/highlight require only reading the document, while byte-for-byte round-trip-safe YAML *writes* are deferred until a comment-preserving Rust YAML solution is validated.

The same AI-safety guarantees that protect `.env` today — fencing, redaction, exposure tracking, canary detection — extend to the new config surfaces, so secrets in `application-prod.yml` get the same protection as secrets in `.env`.

### What Makes This Special

The core insight: **developers don't think in file formats — they think in configuration.** A `DATABASE_URL` is the same logical concern whether it lives in `.env`, `application.yml`, or `application-prod.properties`. EnvForge is the only env/secrets tool that combines (a) IDE-grade language intelligence with (b) byte-for-byte round-trip safety and (c) AI-safety by default. Extending that triad across framework config files makes EnvForge the single tool that gives a developer hover, completion, go-to-definition, highlight, and secret-protection over *every* configuration surface in a polyglot repo — not just the `.env` slice. Competing dotenv plugins and per-language config extensions each see one format; EnvForge sees the whole configuration graph, and treats every surface as a place secrets must be guarded from AI agents.

This release also establishes a reusable seam — a format-agnostic config-document abstraction — so adding the next format (TOML, `appsettings.json`, full multi-framework) becomes incremental rather than a rewrite.

## Project Classification

- **Project Type:** Developer tool — CLI + TUI + LSP language server (Rust).
- **Domain:** Developer tooling / DevSecOps — environment-variable & secrets management with AI-safety guarantees.
- **Complexity:** Medium-high. Drivers: multi-format parsing under a byte-for-byte round-trip invariant, LSP correctness across formats, profile/cascade resolution semantics, and the open YAML comment-preservation problem in the Rust ecosystem.
- **Project Context:** Brownfield. Extends existing `src/lsp/` (tower-lsp 0.20), `src/parser/`, and `src/ops/` schema modules; reuses position→symbol resolution and the schema-line map already powering `.env` features.

## Success Criteria

### User Success

- A Java developer opens `application.yml` / `application.properties` in a supported IDE (VS Code, IntelliJ, Neovim via EnvForge LSP) and **hover** on a key shows the resolved value, provenance, and any schema/sensitivity metadata — same fidelity they get on `.env` today.
- **Completion** suggests known keys (from `.env.schema` and from sibling profile files) and `${VAR}` references while editing `application*.properties/yml`.
- **Go-to-definition** on a `${SOME_VAR}` placeholder or a profile key jumps to where that variable is defined (schema entry, base `application.yml`, or `.env`), across files.
- **Find-references / document highlight** shows every place a key is used across base + profile config files.
- Secrets sitting in `application-prod.yml` receive the **same AI-safety treatment** as `.env` secrets: fence respected, values redacted in hover/labels, exposure tracked, canary detection active.
- The "aha" moment: a developer in a polyglot repo stops switching tools/plugins per config format — one LSP answers hover/completion/go-to-def/highlight everywhere config lives.

### Business Success

- EnvForge becomes relevant to the large Spring Boot / JVM developer population, not only `.env`-centric stacks — measurable as adoption in repos that contain `application.*` files (telemetry-free proxy: documented in integration matrix, referenced in IDE extension marketplaces).
- The format-agnostic seam shipped here reduces the cost of the *next* format (TOML, `appsettings.json`) to incremental work, shortening time-to-coverage for subsequent releases.
- Zero regressions to existing `.env`/shell LSP behavior — protects the current install base.

### Technical Success

- **Round-trip invariant upheld:** any format EnvForge *writes* parses and re-serializes byte-for-byte identically. Properties and `.env` cascade use the existing byte-preserving line parser. YAML is **read-only for writes** in Phase 1 — language features operate on a read model; no YAML write path ships until a comment-preserving solution is validated.
- All language features (hover, completion, go-to-def, references, highlight, diagnostics) function on each Phase-1 format, validated by `tests/{feature}_tests.rs` per the project's no-in-module-tests convention.
- File-type routing for new formats added without regressing existing `is_env_file` / `is_schema_file` dispatch; new extensions added to the LSP security allowlist (`src/lsp/security.rs`).
- Profile resolution (`application-{profile}.*` layered over `application.*`) and `${VAR:default}` interpolation implemented as shared, format-independent engines.

### Measurable Outcomes

- 100% of Phase-1 LSP capabilities (7: hover, completion, go-to-def, references, highlight/semantic-tokens, diagnostics, rename-where-writable) work on `application.properties` and `application-{profile}.properties`.
- Hover/completion/go-to-def/highlight (read-only set) work on `application.yml` and `application-{profile}.yml`.
- Parser fuzz/round-trip test corpus for `.properties` and `.env` cascade passes byte-for-byte on a corpus of ≥50 real-world Spring Boot config files.
- LSP response latency for hover/completion on a config file stays within the existing per-handler rate-limit budget (no new perceptible lag vs `.env`).

## Scope at a Glance

This is a **phased** release. Phase 1 (this PRD) ships Java/JVM framework config support — `application.properties`/`.yml`/`.yaml` + `application-{profile}.*` + Quarkus/MicroProfile `.properties`, plus a formalized `.env` cascade — with language features per the [LSP Capability Matrix](#lsp-capability-matrix-phase-1) (YAML read-only) and AI-safety parity. Later phases add YAML writes, TOML, `.NET appsettings.json`, and cross-format schema unification. The full phase breakdown, MVP strategy, and resource/risk analysis live in [Project Scoping & Phased Development](#project-scoping--phased-development).

**Permanently out of scope:** executable config (`settings.py`, `*.exs`, `*.config.js`, `config/*.php`, Rails `*.rb`) and opaque/encrypted config (`credentials.yml.enc`) — cannot be statically parsed safely. ERB-in-YAML and Helm/HCL templates handled only on a best-effort, non-authoritative basis.

## User Journeys

### Journey 1 — Priya, Spring Boot backend dev (primary, happy path)

**Opening scene.** Priya maintains a Spring Boot service. Config is split across `application.yml` (base) and `application-prod.yml` (overrides). She already uses EnvForge for the repo's `.env`, but in `application.yml` her editor is dumb: no hover, no completion, and `${DATABASE_URL}` placeholders are just text.

**Rising action.** She updates EnvForge; the LSP now claims `application*.yml/properties`. She hovers `spring.datasource.url: ${DATABASE_URL}` — a popover shows the resolved value, that `DATABASE_URL` is declared in `.env.schema` and marked sensitive (value redacted), and which profile layer wins.

**Climax.** Typing a new key, completion offers keys already present in the base file and sibling profiles, and offers `${...}` references from the schema. She invokes go-to-definition on `${DATABASE_URL}` and lands on its schema entry.

**Resolution.** One tool answers hover/completion/go-to-def across `.env` and `application.*`. She stops guessing key names and stops grepping for where a placeholder resolves.

*Reveals:* read model for YAML+properties, profile-layer resolution in hover, schema-backed completion, cross-file go-to-def, redaction parity.

### Journey 2 — Marco, polyglot dev (primary, edge case / error recovery)

**Opening scene.** Marco's repo has `.env`, `.env.local`, Quarkus `application.properties`, and a malformed `application-staging.properties` (duplicate key, dangling `${`).

**Rising action.** EnvForge surfaces **diagnostics**: duplicate key flagged, unterminated interpolation flagged — without crashing the LSP or corrupting the file. Hover on a key that exists in `.env.local` but is overridden in `.env` shows the cascade order and the effective value.

**Climax.** He edits the YAML file and triggers a save. EnvForge does **not** attempt a round-trip-unsafe YAML write — language features stay read-only; he edits text directly and the read model refreshes. No silent reformat, no lost comments.

**Resolution.** Even with broken/mixed config, the tool degrades gracefully and never damages files — preserving the byte-for-byte trust contract.

*Reveals:* robust parsing/error recovery, `.env` cascade ordering, explicit YAML read-only write boundary, diagnostics.

### Journey 3 — Aisha, IntelliJ + Neovim user (IDE-variance / integration)

**Opening scene.** Aisha runs IntelliJ at work and Neovim at home, both wired to the EnvForge LSP. She expects identical behavior on `application.yml`.

**Rising action.** Because the features live in the EnvForge LSP server (not per-IDE plugins), hover/completion/go-to-def/highlight behave identically in both clients. Semantic-token highlighting marks keys, values, and sensitive values consistently.

**Resolution.** Config intelligence is editor-agnostic; the integration matrix documents which clients are validated for the new file types.

*Reveals:* capability registration for new document selectors, semantic tokens across formats, `lsp-clients.md` / `integration-matrix.md` updates.

### Journey 4 — Sam, security-conscious dev with an AI agent in the repo (secondary, AI-safety)

**Opening scene.** Sam runs an AI coding agent that can read repo files. `application-prod.yml` contains a real secret. Today EnvForge fences `.env` but ignores `application-prod.yml` — a blind spot.

**Rising action.** With this release, `application-prod.yml` is a recognized config surface: it is covered by the fence, values are redacted in any LSP-exposed labels/hover, exposure tracking counts it, and canary detection applies.

**Climax.** The agent attempts to read the fenced config; EnvForge's existing AI-safety guarantees now apply uniformly to the framework config file, not just `.env`.

**Resolution.** No config surface is a secret blind spot. Protection is defined by "where secrets live," not "which format."

*Reveals:* fence registry extension to new file types, redaction/exposure/canary parity, security allowlist additions.

### Journey Requirements Summary

The journeys above converge on these capability areas:

- **Format recognition & routing:** detect `application.properties/.yml`, profile variants, Quarkus/MicroProfile files, and the `.env` cascade; route to the right read/parse path (Journeys 1–4).
- **Read model + language features:** hover, completion, go-to-def, references, highlight/semantic tokens, diagnostics over the read model (Journeys 1–3).
- **Resolution engines:** profile-layer precedence and `${VAR:default}` interpolation, format-independent (Journeys 1–2).
- **Write-safety boundary:** properties/`.env` writable byte-safe; YAML read-only for language features in Phase 1 (Journey 2).
- **AI-safety parity:** fence, redaction, exposure tracking, canary detection extended to new surfaces (Journey 4).
- **Editor-agnostic delivery + docs:** capability registration, semantic tokens, integration-matrix / lsp-clients updates (Journey 3).
- **Foundational abstraction:** a `ConfigFormat`-style seam so handlers dispatch by format (all journeys; enables next-format growth).

## Domain-Specific Requirements

Domain: developer tooling / DevSecOps. No external regulatory regime applies, but EnvForge's own design principles act as hard constraints that this feature must not violate.

### Technical Constraints (non-negotiable invariants)

- **Byte-for-byte round-trip safety (CLAUDE.md principle #1).** Any format EnvForge *writes* must parse→serialize identically. Properties and `.env` cascade ride the existing byte-preserving line parser. YAML has **no mature comment-preserving Rust crate** (serde_yaml unmaintained; yaml-rust2/saphyr do not preserve comments today), so YAML is **read-only for writes** in Phase 1 — this constraint, not a scoping preference, is why YAML writes are deferred.
- **Atomic writes (principle #5).** Any new write path reuses the tempfile + rename machinery; no partial writes.
- **Offset / protected-zone safety (principle #8).** New parsers must not disturb protected blocks; never modify fenced files.
- **ops/ purity (principle #2/#3).** Format parsing and resolution engines live in `ops/` (or `parser/`) as pure logic; `lsp/` and `cli/` stay thin.

### Security Constraints (AI-safety domain)

- **Zero trust (principle #9) + AI-safe by default (principle #7).** New config surfaces inherit fence, redaction, exposure tracking, and canary detection. A secret in `application-prod.yml` must never be exposed in hover/labels/logs more readily than one in `.env`.
- **LSP executeCommand stays disabled** (existing security boundary) — adding formats must not introduce a command-execution surface.
- **Extension allowlist discipline.** New file extensions (`.properties`, `.yml`/`.yaml`) added explicitly to `src/lsp/security.rs`; no broad/glob widening of what the LSP will read.

### Correctness & Integration Constraints

- **LSP protocol correctness across clients.** Positions are UTF-16; multi-format handlers must keep position→symbol mapping correct for nested YAML keys. Behavior must be identical across VS Code, IntelliJ, and Neovim (validated in `integration-matrix.md` / `lsp-clients.md`).
- **No telemetry / local-only.** Consistent with existing posture: no network calls introduced for config parsing.

### Risk Mitigations

Constraint-level mitigation unique to this section: **profile/cascade ambiguity** → resolution engine has an explicit, documented precedence order, covered by tests. The YAML-round-trip and `.env`-LSP-regression risks (and their mitigations) are consolidated in [Project Scoping → Risk Mitigation Strategy](#project-scoping--phased-development).

## Innovation & Novel Patterns

### Detected Innovation Areas

The novelty is not a single breakthrough but a **novel combination** at a layer no existing tool occupies:

- **Format-agnostic configuration graph.** Existing tools see one format: dotenv plugins see `.env`; Spring/IntelliJ tooling sees `application.*`; per-language extensions see their own config. EnvForge models env vars as logical concerns and resolves them *across* `.env`, properties, and YAML — including profile layering and `${VAR}` references that cross files. The unit of intelligence is the configuration key, not the file.
- **AI-safety as a format-independent property.** Fencing/redaction/exposure/canary follow "where secrets live," not "which format." Extending those guarantees to `application-prod.yml` treats AI-safety as a cross-cutting property of all config surfaces — a posture no competing config tool takes.
- **Round-trip safety as a first-class boundary that *shapes scope*.** Rather than reformatting YAML (what most tools do), EnvForge refuses to write what it can't write losslessly and ships those features read-only. The invariant is a product decision, not just an implementation detail.

### Market Context & Competitive Landscape

- **dotenv IDE plugins** (VS Code, JetBrains): single-format, no cross-file resolution, no secret protection.
- **Spring Boot tooling / IntelliJ Ultimate**: deep `application.*` intelligence but Spring-only, IDE-locked, and no AI-safety/secret model.
- **Secret managers (Vault, Doppler, 1Password)**: manage secret *storage*, not in-editor config intelligence across formats.
- **Gap EnvForge fills:** editor-agnostic, multi-format config intelligence + byte-safe writes + AI-safety, in one local tool.

### Validation Approach

- Build the `ConfigFormat` seam + properties/`.env` path first (lowest risk, mature crates) and prove cross-file resolution and AI-safety parity end-to-end before adding YAML read support.
- Validate against a corpus of real Spring Boot repos (profiles + interpolation) and confirm identical behavior across the three supported IDE clients.

### Risk Mitigation

- **If YAML read model proves harder than expected** → ship properties + `.env` cascade as a standalone valuable release; YAML follows.
- **If the format abstraction over-generalizes** → keep the trait minimal (parse→entries, resolve, write-capability flag); avoid speculative generality for formats not yet scoped.

## Developer-Tool Specific Requirements

### Project-Type Overview

EnvForge is a Rust CLI + TUI + **LSP language server** (tower-lsp 0.20) consumed by VS Code, IntelliJ, and Neovim. This feature is almost entirely an LSP + parser extension: it adds new document types to an existing server whose per-handler features (hover, completion, definition, references, semantic tokens, diagnostics) are already modular but currently routed by hard-coded `.env`/`.env.schema` file predicates.

### Technical Architecture Considerations

Grounded in the current codebase seams:

- **File-type routing** (`src/lsp/server.rs`, the `is_env_file` / `is_schema_file` / `is_mcp_config_file` predicates): add `is_jvm_config_file(uri)` recognizing `application.properties`, `application.yml`/`.yaml`, `application-*.{properties,yml,yaml}`, and `microprofile-config.properties`. Add `.env` cascade recognition for `.env.local` / `.env.{environment}`. Routing added **alongside** existing predicates — no replacement.
- **Document model** (`src/lsp/document.rs`, `parse_env_document() -> Vec<EnvDocEntry>`): introduce a format-agnostic entry model (or generalize `EnvDocEntry`) carrying key, value, position range, and source-layer. Properties map nearly 1:1 to the existing flat key/value entry; YAML needs nested-key flattening (`spring.datasource.url`) with correct UTF-16 position spans.
- **`ConfigFormat` seam:** a minimal trait — `parse(content) -> entries`, `resolve(entries, profiles) -> effective`, `write_capability() -> {ReadWrite | ReadOnly}`. Handlers dispatch by format instead of branching on `.env`. Properties/`.env` = ReadWrite; YAML = ReadOnly (Phase 1).
- **Parser** (`src/parser/`): the existing hand-written byte-preserving line parser covers properties and `.env` cascade. YAML read model uses a read-only parser (e.g. yaml-rust2/saphyr) — **not** wired to any write path.
- **Resolution engines** (pure, in `src/ops/`): (1) profile-layer precedence (`application.yml` < `application-{profile}.yml`; `.env` < `.env.local` < `.env.{env}`); (2) `${VAR}` / `${VAR:default}` interpolation. Format-independent, fully unit-tested.
- **Schema linkage** (`src/ops/` schema modules, `schema_line_map`): reuse the existing `HashMap<key, line>` so go-to-definition from a config key/placeholder targets the `.env.schema` entry, identical to `.env` today.
- **Security allowlist** (`src/lsp/security.rs`): add `.properties`, `.yml`, `.yaml` to the read allowlist. executeCommand stays disabled.
- **AI-safety hooks** (fence registry in `src/ops/`, redaction, exposure, canary): register the new file types so existing guards apply without per-feature rework.
- **Managed-var loader** (`src/lsp/server.rs` `load_managed_vars`): currently shell-only via `parse_shell_file`; extend to surface JVM-config keys as managed vars where appropriate.

### LSP Capability Matrix (Phase 1)

| Capability | `.properties` (+profiles) | `application.yml` (+profiles) | `.env` cascade |
|---|---|---|---|
| Hover | ✅ | ✅ (read) | ✅ |
| Completion | ✅ | ✅ (read) | ✅ |
| Go-to-definition | ✅ | ✅ (read) | ✅ |
| Find references / highlight | ✅ | ✅ (read) | ✅ |
| Semantic tokens | ✅ | ✅ (read) | ✅ |
| Diagnostics | ✅ | ✅ (read) | ✅ |
| Rename / format (write) | ✅ | ⛔ deferred (no lossless YAML write) | ✅ |

### Implementation Considerations

- **Testing** (`tests/{feature}_tests.rs`, no in-module tests, `insta` snapshots, `tempfile`): per-format round-trip corpus for properties/`.env`; LSP feature tests per format; profile-resolution and interpolation unit tests; explicit test asserting YAML write path is **not** reachable.
- **Feature addition workflow** (per CLAUDE.md): define resolution/parse ops in `src/ops/`+`src/parser/`; wire LSP handlers in `src/lsp/`; CLI/TUI stay thin; update help/docs.
- **Docs to update:** `docs/integration-matrix.md`, `docs/lsp-clients.md`, `docs/ide-behavior-contract.md`, CHANGELOG + README test counts (per docs-sync convention).
- **No new network dependency; local-only parsing.**

## Project Scoping & Phased Development

### MVP Strategy & Philosophy

**MVP Approach:** Problem-solving MVP. Ship the highest-value, lowest-risk slice that makes a Java/Spring developer say "EnvForge now understands my real config files." Sequence by parser risk: properties + `.env` cascade (mature, byte-safe) first, then YAML read model.

**Resource Requirements:** Solo Rust developer (existing maintainer). No new external services. New crates: `java-properties` (or hand-written properties line parser to match existing byte-preserving style) and a read-only YAML parser (`yaml-rust2`/`saphyr`).

### MVP Feature Set (Phase 1 — this PRD)

**Core User Journeys Supported:** Journeys 1–4 (Priya happy path, Marco edge/error recovery, Aisha multi-IDE, Sam AI-safety).

**Must-Have Capabilities:**
- `is_jvm_config_file` routing for `application.properties`/`.yml`/`.yaml` + `application-{profile}.*` + Quarkus/MicroProfile `.properties`.
- `.env` cascade recognition (`.env`, `.env.local`, `.env.{environment}`).
- `ConfigFormat` seam + format-agnostic entry model.
- Language features per the Phase-1 Capability Matrix (read-only set on YAML; full set on properties/`.env`).
- Profile-layer resolution + `${VAR:default}` interpolation engines (pure, tested).
- AI-safety parity (fence/redaction/exposure/canary) on new file types.
- Security allowlist additions; executeCommand stays disabled.
- Round-trip test corpus (properties/`.env`); test asserting YAML write path unreachable.
- Docs updated (integration-matrix, lsp-clients, ide-behavior-contract, CHANGELOG/README counts).

### Post-MVP Features

**Phase 2 (Growth):**
- Round-trip-safe **YAML writes** (rename/format on `.yml`) once a comment-preserving Rust approach is validated.
- **TOML family** via `toml_edit` (Cargo.toml, pyproject.toml, config.toml).
- Cross-format **schema unification** (one `.env.schema` across all config files).

**Phase 3 (Expansion / Vision):**
- **.NET** `appsettings.json` + environment variants (lossless JSONC CST).
- Whole-repo **configuration graph** view; uniform AI-agent guarding across every config surface.

### Risk Mitigation Strategy

- **Technical Risks:** YAML comment-preserving writes (no mature crate) → ship YAML read-only, defer writes. Properties/`.env` carry the MVP value alone if YAML read slips. Nested-key UTF-16 position correctness → covered by targeted LSP tests.
- **Market Risks:** Uncertain JVM-developer adoption → MVP is independently useful to existing users (formalized `.env` cascade) even before Java traction; validate against real Spring Boot repos.
- **Resource Risks:** Solo maintainer → strict phasing; Phase 1 is shippable on its own, each later phase is independently valuable and gated on its parser prerequisite.

## Functional Requirements

> Capability contract for Phase 1. Each FR is a testable, implementation-agnostic capability. "Config file" below = the Phase-1 set: `application.properties`/`.yml`/`.yaml`, their `application-{profile}.*` variants, Quarkus/MicroProfile `.properties`, and the `.env` cascade.

### Config File Recognition

- **FR1:** EnvForge can recognize `application.properties`, `application.yml`, and `application.yaml` as supported config files.
- **FR2:** EnvForge can recognize profile-variant files matching `application-{profile}.{properties,yml,yaml}`.
- **FR3:** EnvForge can recognize Quarkus/MicroProfile `application.properties` and `microprofile-config.properties`.
- **FR4:** EnvForge can recognize the `.env` cascade (`.env`, `.env.local`, `.env.{environment}`) as a layered set of config files.
- **FR5:** EnvForge can classify each recognized file's write-capability as read-write (properties, `.env`) or read-only (YAML).

### Configuration Intelligence (Language Features)

- **FR6:** A developer can hover a config key and see its effective value, the file/layer that value resolves from, and any schema/sensitivity metadata.
- **FR7:** A developer receives completion suggestions for config keys drawn from `.env.schema`, sibling profile files, and the same file.
- **FR8:** A developer receives completion suggestions for `${VAR}` references while editing a value.
- **FR9:** A developer can go-to-definition from a config key or a `${VAR}` placeholder to where that variable is defined (schema entry, base config file, or `.env`), across files.
- **FR10:** A developer can find all references to a config key across base and profile config files.
- **FR11:** A developer sees document/semantic-token highlighting that distinguishes keys, values, and sensitive values in config files.
- **FR12:** A developer sees diagnostics for config-file problems (e.g. duplicate key, unterminated `${` interpolation, unknown key vs schema).
- **FR13:** A developer can rename a key and reformat the document for write-capable formats (properties, `.env`); these write actions are unavailable on read-only (YAML) files.
- **FR14:** EnvForge does not modify a read-only-format file through any language feature.

### Resolution & Interpolation

- **FR15:** EnvForge can resolve a config key's effective value across profile layers using a documented precedence (base < profile; `.env` < `.env.local` < `.env.{environment}`).
- **FR16:** EnvForge can resolve `${VAR}` and `${VAR:default}` interpolation references when computing a key's effective value.
- **FR17:** EnvForge can flatten nested YAML keys to dotted paths (e.g. `spring.datasource.url`) for hover, completion, and reference resolution.

### AI-Safety Coverage

- **FR18:** EnvForge can apply fence protection to recognized config files so fenced files are guarded from AI-agent reads.
- **FR19:** EnvForge redacts sensitive values in any hover, label, or output it produces for a config file.
- **FR20:** EnvForge counts recognized config files in its AI-exposure tracking.
- **FR21:** EnvForge can apply canary-token detection to recognized config files.

### Editor Integration

- **FR22:** EnvForge exposes the config-file language features identically across its supported LSP clients (VS Code, IntelliJ, Neovim).
- **FR23:** EnvForge registers the new config-file document types without altering or regressing existing `.env`/`.env.schema`/shell behavior.

### Extensibility Foundation

- **FR24:** EnvForge dispatches language-feature handlers by config-format via a format-agnostic abstraction rather than per-file-name branching.
- **FR25:** EnvForge can add a new config format by implementing the format abstraction (parse, resolve, write-capability) without modifying existing format handlers.

## Non-Functional Requirements

> This is a brownfield-extension PRD. NFRs reference existing implementation anchors (config-file size guard, LSP capability flags, toolchain floor, module layout) **intentionally** — they constrain the extension to the established system. Pure how-level detail is kept out of requirement text; see the Developer-Tool Architecture section for symbol-level specifics.

### Performance

- **NFR1:** Hover and completion on a config file return at p95 ≤ 50 ms on a config file of up to 500 keys, measured by LSP handler timing in the test harness — no perceptible added latency versus `.env` files of comparable size.
- **NFR2:** Config-file parsing handles files up to the existing 10 MiB size guard without blocking the LSP event loop.
- **NFR3:** Profile/cascade resolution reuses cached sibling-file parses: on a keystroke that changes one open document, at most that one file is re-parsed (asserted by a parse-count test); sibling caches invalidate only on their own change/save.

### Security (AI-Safety — primary quality attribute)

- **NFR4:** A secret in a recognized config file is never exposed in hover, labels, logs, or any LSP output more readily than the same secret in `.env` — redaction parity is mandatory.
- **NFR5:** Recognized config files are subject to fence enforcement; a fenced config file is not read/served by the LSP to a fenced consumer.
- **NFR6:** No new LSP command-execution surface is introduced; `executeCommandProvider` remains disabled.
- **NFR7:** Only the explicitly allow-listed extensions (`.properties`, `.yml`, `.yaml` added to the existing allowlist) are read; no glob/broad widening.
- **NFR8:** Config parsing performs no network I/O; all resolution is local.

### Reliability & Correctness

- **NFR9:** Round-trip invariant: any write-capable format (properties, `.env`) parses→serializes byte-for-byte identically across the test corpus; a malformed file is never silently rewritten.
- **NFR10:** YAML files are never modified by any write path in Phase 1; an automated test asserts the YAML write path is unreachable.
- **NFR11:** Malformed config files (syntax errors, duplicate keys, dangling interpolation) produce diagnostics, never LSP crashes or panics; all parse/resolution errors are surfaced as recoverable errors, not process aborts.
- **NFR12:** Adding the new formats causes zero regressions to existing `.env`/`.env.schema`/shell LSP behavior — the full existing test suite stays green.

### Compatibility & Integration

- **NFR13:** Config-file language features behave identically across VS Code, IntelliJ, and Neovim LSP clients; supported clients/file-types are recorded in `docs/integration-matrix.md` and `docs/lsp-clients.md`.
- **NFR14:** LSP `Position` handling is UTF-16-correct for nested YAML keys and multi-byte values.
- **NFR15:** Minimum Rust 1.75 and existing crate baseline are not regressed; any new crate is mature and maintained.

### Maintainability

- **NFR16:** Format parsing and resolution live in `src/ops/`/`src/parser/` as pure logic; `src/lsp/` and `src/cli/` remain thin per the project's layering principles.
- **NFR17:** All tests live in `tests/{feature}_tests.rs` (no in-module tests), using `insta` snapshots and `tempfile` per project conventions; CHANGELOG and README test counts updated on completion.
