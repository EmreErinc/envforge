---
stepsCompleted:
  - step-01-validate-prerequisites
  - step-02-design-epics
  - step-03-create-stories
  - step-04-final-validation
inputDocuments:
  - docs/prd.md
  - docs/implementation-readiness-report-2026-06-23.md
---

# EnvForge — Epic Breakdown

## Overview

This document decomposes the PRD (`docs/prd.md`) — clean, environment-aware `.env` IDE support driven by `.envforge.project.toml` — into implementable epics and stories. No separate Architecture or UX document exists; the four open architecture questions from the readiness report are folded into Epic 1 as explicit design stories.

## Requirements Inventory

### Functional Requirements

**Project Manifest**
- FR1: A Tech Lead can declare the project's env-file set in a `.envforge.project.toml` manifest (base env file + named profile variants).
- FR2: A Tech Lead can declare additional non-profile env files in the manifest.
- FR3: The IDE/LSP can parse the manifest and resolve it into a concrete set of recognized env-file paths.
- FR4: The IDE/LSP can re-resolve the recognized set on manifest change without server/editor restart.
- FR5: The IDE/LSP can report a malformed manifest as a diagnostic on `.envforge.project.toml` and keep serving the last good set.
- FR6: With no manifest present, the IDE/LSP falls back to the conventional `.env*` set (backward compatible).

**Env-File Recognition & Attach Scoping**
- FR7: Recognize each file in the resolved set — including variants `.env.development`, `.env.stage`, `.env.production`, etc. — as an EnvForge-owned env file.
- FR8: Attach only to the resolved set + pre-existing EnvForge-owned files (`.env*`, `.env.schema*`, MCP config); nothing else.
- FR9: Never attach to whole source languages or arbitrary TOML/YAML/JSON.
- FR10: An undeclared `.env`-like file stays unrecognized until added to the manifest.

**Key & Value Suggestion**
- FR11: Key completion drawn from the project's known key-set (union across recognized environments) while typing a key.
- FR12: Value completion for a key while typing its value, including values set for that key in other environments.
- FR13: Completions trigger through normal typing (assignment/reference trigger chars), consistent with existing LSP completion.
- FR14: Schema-declared keys/values (`.env.schema`) included in completions when a schema is present.

**Cross-Environment Intelligence**
- FR15: Hover a key → see its value in each recognized environment, with provenance.
- FR16: Model keys as one logical key-set with per-environment values across recognized files.
- FR17: Diagnostic when a key is present in ≥1 recognized environment but missing from the current one.
- FR18: Navigate from a key occurrence to its definitions across recognized env files (and `.env.schema` when present).

**AI-Safety & Redaction Parity**
- FR19: Redact sensitive values in all surfaced labels (completion, hover, inlay) for every recognized variant.
- FR20: Apply fence + exposure classification identically across all recognized environments.
- FR21: Prevent an AI agent from observing raw sensitive values through EnvForge's IDE surfaces.

**Cross-Client Consistency**
- FR22: Identical recognition + behavior from one manifest across VS Code, IntelliJ, Neovim, Zed (subject to documented overlay limits).
- FR23: One committed manifest → every teammate's IDE recognizes the same set without per-developer config.
- FR24: Deterministic feature output independent of client capabilities.

### NonFunctional Requirements

- NFR1: Completion/hover round-trip < 100 ms on a typical project (≤ ~10 env files, ≤ ~500 keys).
- NFR2: Manifest parse + re-resolution < 200 ms; never blocks unrelated LSP requests.
- NFR3: No measurable latency on files outside the recognized set.
- NFR4: 100% redaction of sensitive values in IDE labels; raw values only via `text_edit.new_text`.
- NFR5: Fence/exposure rules identical across every recognized variant.
- NFR6: Manifest declaration grants env-intelligence only — never widens attach surface to general file handling.
- NFR7: No IDE surface exposes a raw sensitive value to an AI agent.
- NFR8: Malformed manifest never crashes the LSP; diagnostic + last-good fallback.
- NFR9: Manifest parse + env-file edits round-trip byte-identically.
- NFR10: Zero regressions in native source-language IDE features (verified per client).
- NFR11: Deterministic output across clients (no client-capability branching).
- NFR12: Correct UTF-16 position encoding for all ranges.
- NFR13: Composes with existing `.env.schema*` contract + `envforge/*` requests without breaking them.
- NFR14: Graceful degradation where a client lacks a surface (documented, not silent).

### Additional Requirements

Derived from the readiness report's open design questions (no Architecture doc; resolved as Epic 1 design stories):
- AR1: Formal `.envforge.project.toml` schema — exact keys, types, path-resolution rules, `schema_version` semantics.
- AR2: Concrete per-environment value data-structure (the unified key-set model).
- AR3: Cross-environment "missing key" diagnostic semantics — severity (warning vs hint), message, range.
- AR4: Precedence/conflict rule between `.envforge.project.toml` (declares files) and `.env.schema` (types keys).
- AR5: Reuse existing `envforge lsp` server architecture — all logic server-side; clients only attach + render; build on existing `RwLock<HashMap>` doc/state model.

### UX Design Requirements

None — no UX document. This is an LSP/IDE-protocol feature; the user-facing surface is editor-native (completion, hover, diagnostics, exposure overlays) governed by the existing IDE behavior contract, not custom UI.

### FR Coverage Map

- FR1 → Epic 1 (declare base + profiles in manifest)
- FR2 → Epic 1 (declare extra files)
- FR3 → Epic 1 (parse + resolve to concrete path set)
- FR4 → Epic 1 (reload on change)
- FR5 → Epic 1 (malformed-manifest diagnostic + last-good fallback)
- FR6 → Epic 1 (no-manifest fallback to conventional `.env*`)
- FR7 → Epic 2 (recognize resolved variants as env files)
- FR8 → Epic 2 (attach only to resolved set + EnvForge-owned files)
- FR9 → Epic 2 (never attach to source languages / arbitrary TOML-YAML-JSON)
- FR10 → Epic 2 (undeclared files stay unrecognized)
- FR11 → Epic 3 (key completion from project key-set)
- FR12 → Epic 3 (value completion incl. cross-env values)
- FR13 → Epic 3 (trigger via normal typing)
- FR14 → Epic 3 (schema-declared keys/values in completion)
- FR16 → Epic 3 (unified key-set / per-env value model — foundation for completion)
- FR15 → Epic 4 (hover per-environment values + provenance)
- FR17 → Epic 4 (cross-environment missing-key diagnostic)
- FR18 → Epic 4 (goto definitions across env files + schema)
- FR19 → Epic 5 (redact sensitive values in all labels, all variants)
- FR20 → Epic 5 (fence/exposure parity across environments)
- FR21 → Epic 5 (no raw secret to AI agents via IDE surfaces)
- FR22 → Epic 6 (identical behavior across 4 clients from one manifest)
- FR23 → Epic 6 (one committed manifest → team-wide recognition)
- FR24 → Epic 6 (deterministic output independent of client capabilities)

## Epic List

### Epic 1: Project Manifest Foundation
A Tech Lead can declare the project's env-file set (base + named profile variants + extra files) in `.envforge.project.toml`, and the LSP parses it into a concrete recognized set — reloading on change, diagnosing a malformed manifest with last-good fallback, and falling back to conventional `.env*` recognition when no manifest exists. Establishes the formal manifest schema (AR1), the manifest↔`.env.schema` precedence rule (AR4), and reuse of the existing server architecture (AR5).
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6 · **NFRs:** NFR2, NFR8, NFR9, NFR13 · **AR:** AR1, AR4, AR5

### Epic 2: Scoped Env-File Recognition
The IDE recognizes exactly the resolved env-file set — including variants `.env.development`/`.env.stage`/`.env.production` — and attaches only to those plus pre-existing EnvForge-owned files. Undeclared `.env`-like files and all source languages / arbitrary config files remain untouched (closes the 0.8.4 over-attachment regression class).
**FRs covered:** FR7, FR8, FR9, FR10 · **NFRs:** NFR3, NFR6, NFR10

### Epic 3: Key & Value Suggestion
A developer typing in a recognized env file receives key completions from the project's known key-set and value completions for a key (including values set in other environments and schema-declared values), triggered by normal typing. Defines the unified key-set + per-environment value data structure (FR16, AR2) that later epics consume.
**FRs covered:** FR11, FR12, FR13, FR14, FR16 · **NFRs:** NFR1, NFR12 · **AR:** AR2

### Epic 4: Cross-Environment Intelligence
Building on the unified key-set model from Epic 3, a developer can hover a key to see its value in each environment with provenance, gets a diagnostic when a key is set in one environment but missing in another, and can navigate to a key's definitions across the recognized files and `.env.schema`. Specifies the missing-key diagnostic semantics (AR3).
**FRs covered:** FR15, FR17, FR18 · **NFRs:** NFR1, NFR12 · **AR:** AR3

### Epic 5: AI-Safety & Redaction Parity
Sensitive values are redacted in every IDE surface across every recognized variant, fence + exposure classification is applied identically across all environments, and no raw secret reaches an AI agent through an EnvForge IDE surface.
**FRs covered:** FR19, FR20, FR21 · **NFRs:** NFR4, NFR5, NFR7

### Epic 6: Cross-Client Delivery & Determinism
The recognition + intelligence behavior is delivered identically from one manifest across VS Code, IntelliJ, Neovim, and Zed (within documented overlay limits), so one committed manifest gives the whole team the same experience with deterministic, client-capability-independent output.
**FRs covered:** FR22, FR23, FR24 · **NFRs:** NFR10, NFR11, NFR14

## Epic 1: Project Manifest Foundation

A Tech Lead can declare the project's env-file set in `.envforge.project.toml`; the LSP parses and resolves it into a concrete recognized set, reloads on change, fails gracefully on a malformed manifest, and falls back to conventional `.env*` recognition when no manifest exists.

### Story 1.1: Define the `.envforge.project.toml` manifest schema

As a Tech Lead,
I want a formally specified `.envforge.project.toml` schema,
So that I can declare my project's env files unambiguously and tooling can rely on a stable format.

**Acceptance Criteria:**

**Given** the manifest design,
**When** the schema is documented,
**Then** it specifies a `schema_version` field, a `base` string field (path to the base env file), a `profiles` table (`name → path`), and an optional `extra_files` array of paths.
**And** path values resolve relative to the manifest's directory (workspace root) and must stay inside it.
**And** the schema is published in `docs/` and referenced by the implementation as the single source of truth (AR1).

### Story 1.2: Parse and resolve the manifest into a concrete file set

As a Tech Lead,
I want the LSP to read my manifest and turn it into the exact set of env files,
So that the IDE knows precisely which files belong to my project.

**Acceptance Criteria:**

**Given** a valid `.envforge.project.toml` declaring `base`, `profiles`, and optional `extra_files`,
**When** the LSP loads the workspace,
**Then** it resolves a deduplicated set of absolute env-file paths (base + every profile path + every extra file) (FR1, FR2, FR3).
**And** resolution completes in < 200 ms for a typical project (NFR2).
**And** the manifest parse is byte-round-trip safe — no rewrite of the manifest occurs (NFR9).

### Story 1.3: Reload the resolved set when the manifest changes

As a Tech Lead,
I want manifest edits to take effect immediately,
So that adding or removing an environment is reflected without restarting my editor.

**Acceptance Criteria:**

**Given** a resolved manifest in an active LSP session,
**When** `.envforge.project.toml` is changed and saved,
**Then** the LSP re-resolves the env-file set live without a server or editor restart (FR4).
**And** re-resolution does not block unrelated LSP requests (NFR2).

### Story 1.4: Diagnose a malformed manifest with last-good fallback

As a Tech Lead,
I want a clear error when my manifest is broken and no loss of existing recognition,
So that a typo never silently disables the feature.

**Acceptance Criteria:**

**Given** a previously valid resolved set,
**When** `.envforge.project.toml` is edited into an invalid state (e.g. TOML syntax error, duplicate profile key, missing `schema_version`),
**Then** the LSP publishes a diagnostic on `.envforge.project.toml` pointing at the offending location (FR5).
**And** it continues serving the last successfully resolved set rather than dropping recognition.
**And** the LSP does not crash (NFR8).

### Story 1.5: Fall back to conventional `.env*` recognition with no manifest

As a developer on a project without a manifest,
I want EnvForge to still recognize my `.env*` files,
So that the feature is backward compatible and zero-config for simple projects.

**Acceptance Criteria:**

**Given** a workspace with no `.envforge.project.toml`,
**When** the LSP loads,
**Then** it recognizes the conventional `.env*` set as before (FR6).
**And** behavior for existing EnvForge-owned files (`.env.schema*`, MCP config) is unchanged (NFR13).

### Story 1.6: Define manifest ↔ `.env.schema` precedence

As a Tech Lead using both a manifest and a schema,
I want a defined relationship between them,
So that there is no ambiguity about which declares files vs types keys.

**Acceptance Criteria:**

**Given** a project with both `.envforge.project.toml` and `.env.schema`/`.env.schema.toml`,
**When** the LSP resolves recognition and validation,
**Then** the manifest determines the recognized file set and the schema types/validates keys within those files (AR4).
**And** the rule is documented and existing `envforge/*` requests continue to function (NFR13, AR5).

## Epic 2: Scoped Env-File Recognition

The IDE recognizes exactly the resolved env-file set — including environment variants — and attaches only to EnvForge-owned files, never to source languages or arbitrary config files.

### Story 2.1: Recognize resolved env files including variants

As a developer,
I want the IDE to treat `.env.development`, `.env.stage`, `.env.production` (and any declared variant) as EnvForge env files,
So that env intelligence fires in every environment file my project uses.

**Acceptance Criteria:**

**Given** a resolved manifest set including environment variants,
**When** I open any file in that set,
**Then** the LSP recognizes it as an EnvForge-owned env file and env features become available (FR7).
**And** recognition uses the concrete resolved path/filename, not a blanket language match.

### Story 2.2: Attach only to the resolved set plus EnvForge-owned files

As a developer,
I want the LSP to attach strictly to EnvForge's files,
So that the editor is never burdened or degraded by EnvForge on unrelated files.

**Acceptance Criteria:**

**Given** a resolved manifest set,
**When** the LSP computes its attach surface,
**Then** it attaches only to the resolved set plus pre-existing EnvForge-owned files (`.env*` defaults, `.env.schema*`, MCP config) and nothing else (FR8).
**And** declaring a file in the manifest grants env-intelligence only — it never turns the LSP into a general file handler (NFR6).

### Story 2.3: Never attach to source languages or arbitrary config files

As a developer using AI/code tooling,
I want EnvForge to stay out of my source files,
So that native code intelligence and AI agents are never disrupted (the 0.8.4 regression class).

**Acceptance Criteria:**

**Given** any TypeScript/Rust/Python/etc. source file or an arbitrary `.toml`/`.yaml`/`.json` that is not a declared env file,
**When** I open or edit it,
**Then** the EnvForge LSP does not attach and produces no features for it (FR9).
**And** there are zero regressions in native source-language IDE features (NFR10).
**And** no measurable latency is added to editing those files (NFR3).

### Story 2.4: Keep undeclared `.env`-like files unrecognized until declared

As a developer,
I want a new env-like file ignored until I add it to the manifest,
So that recognition is intentional, never accidental.

**Acceptance Criteria:**

**Given** a manifest-based project,
**When** a `.env`-like file exists that is not in the resolved set (and not a conventional default in a no-manifest project),
**Then** the LSP does not recognize or attach to it (FR10).
**And** once it is added to the manifest and saved, it is recognized after reload (ties to Story 1.3).

## Epic 3: Key & Value Suggestion

A developer receives key and value completions drawn from the project's unified key-set as they type in a recognized env file.

### Story 3.1: Build the unified key-set + per-environment value model

As the IDE/LSP,
I want a single in-memory model of all keys with their per-environment values,
So that completion, hover, and diagnostics share one consistent source of truth.

**Acceptance Criteria:**

**Given** a resolved env-file set,
**When** the LSP indexes the files,
**Then** it builds a unified key-set where each key maps to its value per environment (FR16, AR2).
**And** the model is stored in the existing server state model and updated on file/manifest change (AR5).
**And** the model never persists raw sensitive values beyond what redaction policy permits at the surface layer.

### Story 3.2: Key completion from the project key-set

As a developer,
I want known keys suggested while typing a key,
So that I don't mistype or forget a key that exists elsewhere in the project.

**Acceptance Criteria:**

**Given** a recognized env file and the unified key-set,
**When** I begin typing a key at the start of a line,
**Then** the LSP offers completions for keys in the project key-set (FR11).
**And** the completion round-trip completes in < 100 ms (NFR1).
**And** ranges use correct UTF-16 encoding (NFR12).

### Story 3.3: Value completion including cross-environment values

As a developer,
I want suggested values for a key while typing its value,
So that I can reuse a value already set in another environment.

**Acceptance Criteria:**

**Given** a recognized env file and a key with values set in other environments,
**When** I type the value portion after `=`,
**Then** the LSP offers value completions including values that key holds in other environments (FR12).
**And** sensitive values are redacted in the completion `label` while the real value flows only through `text_edit.new_text` (ties to Epic 5, NFR4).

### Story 3.4: Trigger completion via normal typing

As a developer,
I want completions to appear through ordinary typing,
So that I don't need a special command.

**Acceptance Criteria:**

**Given** a recognized env file,
**When** I type and reach assignment/reference trigger characters (consistent with existing LSP completion behavior, e.g. `=`, `$`, `{`),
**Then** the LSP returns context-appropriate key or value completions (FR13).

### Story 3.5: Include schema-declared keys and values in completion

As a developer with an `.env.schema`,
I want schema-declared keys and allowed values offered too,
So that completion reflects the typed contract, not just observed values.

**Acceptance Criteria:**

**Given** a project with `.env.schema`/`.env.schema.toml`,
**When** I trigger key or value completion in a recognized env file,
**Then** schema-declared keys and any schema-declared allowed values are included in the suggestions (FR14).
**And** schema and manifest compose per the Story 1.6 precedence rule (NFR13).

## Epic 4: Cross-Environment Intelligence

Building on the unified key-set, the developer sees per-environment values on hover, gets gap diagnostics, and can navigate to definitions.

### Story 4.1: Hover shows per-environment values with provenance

As a developer,
I want to hover a key and see its value in every environment,
So that I understand a key's configuration across the whole project at a glance.

**Acceptance Criteria:**

**Given** a recognized env file and the unified key-set,
**When** I hover a key,
**Then** the LSP shows the key's value in each recognized environment with provenance (which file/environment defines it) (FR15).
**And** sensitive values are redacted in the hover (NFR4, Epic 5).
**And** the hover round-trip completes in < 100 ms (NFR1).

### Story 4.2: Cross-environment missing-key diagnostic

As a developer,
I want to be warned when a key exists in one environment but not the one I'm editing,
So that I catch missing configuration before deploy.

**Acceptance Criteria:**

**Given** a key present in at least one recognized environment and absent from the current file,
**When** the LSP publishes diagnostics for the current file,
**Then** it emits a diagnostic identifying the missing key and which environment(s) define it (FR17).
**And** the diagnostic severity, message, and range follow the defined semantics (AR3 — default severity: Warning; range: end of file or a stable anchor; message names the source environment).
**And** ranges use correct UTF-16 encoding (NFR12).

### Story 4.3: Go-to-definition across env files and schema

As a developer,
I want to jump from a key to where it's defined,
So that I can navigate a key's definitions across environments and the schema.

**Acceptance Criteria:**

**Given** a key in a recognized env file,
**When** I invoke Go-to-Definition,
**Then** the LSP returns the key's definition location(s) across the recognized env files and `.env.schema` when present (FR18).
**And** it does not attempt source-file goto (consistent with Epic 2 scoping).

## Epic 5: AI-Safety & Redaction Parity

Sensitive values are redacted in every IDE surface across every recognized variant, with fence/exposure parity and no raw-secret leakage to AI agents.

### Story 5.1: Redact sensitive values in all labels across every variant

As a security-conscious developer,
I want sensitive values masked in completion, hover, and inlay across all my env files,
So that secrets are never shown in plaintext in the IDE.

**Acceptance Criteria:**

**Given** recognized env files including variants, with sensitive keys,
**When** any label is surfaced (completion, hover, inlay),
**Then** sensitive values are redacted in 100% of those labels for every recognized variant (FR19, NFR4).
**And** the real value traverses only `text_edit.new_text` where the editor requires insertion.

### Story 5.2: Fence + exposure parity across all environments

As a security-conscious developer,
I want fence and exposure classification applied equally to every environment file,
So that no environment is a redaction or exposure blind spot.

**Acceptance Criteria:**

**Given** a resolved set with multiple environments,
**When** exposure classification and fence state are computed (via `envforge/exposureMap` and fence),
**Then** identical rules apply to `.env`, `.env.development`, `.env.stage`, `.env.production`, and every other recognized variant (FR20, NFR5).
**And** the existing `envforge/*` requests serve every recognized variant (NFR13).

### Story 5.3: No raw secret reaches an AI agent via IDE surfaces

As a developer working alongside an AI agent,
I want EnvForge's IDE surfaces to never expose raw secrets,
So that an agent reading the editor cannot exfiltrate them.

**Acceptance Criteria:**

**Given** an AI agent observing editor content and LSP responses,
**When** EnvForge produces any IDE surface (completion/hover/inlay/diagnostics) for a recognized variant,
**Then** no raw sensitive value is present in that surface (FR21, NFR7).

## Epic 6: Cross-Client Delivery & Determinism

The behavior is delivered identically from one manifest across all four first-party clients, deterministically and without client-capability branching.

### Story 6.1: Identical recognition + behavior across the four clients

As a developer on any supported editor,
I want the same env intelligence from one manifest,
So that my team gets a consistent experience regardless of editor.

**Acceptance Criteria:**

**Given** one `.envforge.project.toml`,
**When** the project is opened in VS Code, IntelliJ, Neovim, and Zed,
**Then** each client attaches to the same resolved set and serves the same features, within each client's documented overlay limits (FR22, NFR14).
**And** each client attaches via its existing mechanism (VS Code `documentSelector` patterns, IntelliJ `languageMapping` `fileNamePattern`, Neovim filename-gated attach) without source-language attach (NFR10).

### Story 6.2: One committed manifest configures the whole team

As a Tech Lead,
I want the committed manifest to configure every teammate's IDE,
So that nobody needs per-developer setup.

**Acceptance Criteria:**

**Given** `.envforge.project.toml` committed to the repo,
**When** a teammate opens the project in any supported client,
**Then** their IDE recognizes the same env-file set with no per-developer configuration (FR23).

### Story 6.3: Deterministic, client-capability-independent output

As a maintainer,
I want server output to be deterministic across clients,
So that behavior is testable and consistent.

**Acceptance Criteria:**

**Given** identical input (same manifest + same files),
**When** any client requests a feature,
**Then** the server produces identical output regardless of client capabilities (no client-capability branching) (FR24, NFR11).
**And** a conformance test asserts one manifest fixture yields the same resolved set + feature output across all client attach configs.
