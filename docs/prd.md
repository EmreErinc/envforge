---
stepsCompleted:
  - step-01-init
  - step-02-discovery
  - step-02b-vision
  - step-02c-executive-summary
  - step-03-success
  - step-04-journeys
  - step-05-domain
  - step-06-innovation
  - step-07-project-type
  - step-08-scoping
  - step-09-functional
  - step-10-nonfunctional
  - step-11-polish
  - step-12-complete
releaseMode: phased
classification:
  projectType: developer_tool
  domain: developer_tooling_devsecops
  complexity: medium
  projectContext: brownfield
inputDocuments:
  - docs/ide-behavior-contract.md
  - docs/lsp-clients.md
  - docs/integration-matrix.md
  - docs/api-reference.md
workflowType: 'prd'
---

# Product Requirements Document - EnvForge

**Author:** Emre
**Date:** 2026-06-23

## Executive Summary

EnvForge already ships a Language Server (`envforge lsp`) and first-party VS Code, IntelliJ, Neovim, and Zed clients that surface schema-aware completion, hover with provenance, diagnostics, and `.env`→schema navigation. As of 0.8.4 the LSP attaches **only to EnvForge's own files** (`.env*`, `.env.schema*`, MCP config) — it no longer hijacks whole source languages. This PRD builds on that foundation to deliver **clean, reliable, environment-aware `.env` intelligence in the IDE**.

The IDE must (1) reliably recognize a project's `.env` files — including environment variants `.env.development`, `.env.stage`, `.env.production`, etc. — (2) match each key against the project's known env-var set across environments, and (3) suggest keys and values as the user types. The set of recognized env files and their profile relationships is declared in a single project manifest, **`.envforge.project.toml`**, which becomes the source of truth: the IDE recognizes exactly the declared files (no guessing, no over-attachment) and treats their keys as one logical key-set with per-environment values.

Target users: developers working in multi-environment projects (Spring/Node/Python/etc. with `.env.{profile}` conventions) and the AI coding agents operating alongside them, who need accurate, redaction-safe env context without the LSP degrading the editor.

### What Makes This Special

Manifest-driven, environment-aware `.env` recognition. Instead of per-file guessing, `.envforge.project.toml` declares the project's env-file set and profiles, so the IDE knows precisely which `.env.*` files belong to the project and how `.env.development` / `.env.stage` / `.env.production` relate as variants of one key-set. Completion and key/value matching then operate across environments — surfacing "this key is set in `production` but missing in `stage`," offering known keys/values as you type — while staying AI-safe by default (sensitive values redacted in labels, fence/exposure parity) and strictly scoped to EnvForge's own files so native code intelligence is never disturbed.

The core insight: env vars across environment-specific files are **one key-set with per-environment values**, not independent files. The IDE should know that explicitly from a manifest rather than infer it.

## Project Classification

- **Project Type:** Developer tool — IDE/LSP integration layer over an existing Rust CLI.
- **Domain:** Developer tooling / DevSecOps (environment-variable and secret safety).
- **Complexity:** Medium — LSP wire-protocol correctness across 4 first-party clients plus a new `.envforge.project.toml` schema and recognition layer; bounded scope, no regulatory compliance burden.
- **Project Context:** Brownfield — extends and cleans up the existing `envforge lsp` server and editor plugins; anchored on the 0.8.4 "attach only to EnvForge's own files" decision.

## Success Criteria

### User Success

- A developer opens any `.env.*` file declared in `.envforge.project.toml` and the IDE recognizes it immediately — exposure overlays, hover, and completion fire without per-file configuration.
- Typing a key in an env file surfaces the project's known keys (drawn from the union of all declared environments); typing `=` surfaces known/prior values for that key.
- The developer can see, at a glance, "this key is set in `production` but missing in `stage`" — cross-environment gaps are visible, not discovered at deploy time.
- Native code intelligence (goto, completion in `.ts`/`.rs`/etc.) is **never** disturbed — the env feature is invisible until the developer is actually in an EnvForge-owned file.

### Business Success

- The four first-party clients (VS Code, IntelliJ, Neovim, Zed) deliver the recognition + completion behavior with no editor-degradation regressions — protecting EnvForge's "AI-safe, never-in-the-way" positioning.
- Reduced support/issue volume around "LSP attached to my source files" and "IDE doesn't see my `.env.staging`" — the two failure modes this PRD closes.

### Technical Success

- LSP recognizes **exactly** the env-file set resolved from `.envforge.project.toml` (declared files + profile variants) and attaches to nothing outside EnvForge's own files.
- Completion, hover, and diagnostics operate over a single logical key-set with per-environment values; the manifest is parsed round-trip-safe and reloaded on change.
- AI-safety parity holds on every recognized variant: sensitive values redacted in completion/hover labels, fence + exposure classification applied identically across `.env.development` / `.env.stage` / `.env.production`.

### Measurable Outcomes

- **100%** of files declared in `.envforge.project.toml` are recognized in all 4 first-party clients; **0** attachments to non-declared / non-EnvForge files.
- Key completion returns the full project key-set with **0** false omissions for keys present in any declared environment.
- Cross-environment "missing key" diagnostic fires for **every** key present in ≥1 declared env and absent in another.
- **0** regressions in native source-language IDE features (verified per client).
- Manifest parse + env-set resolution under a fixed latency budget (target: completion/hover round-trip < 100 ms on a typical project).

## Product Scope

### MVP - Minimum Viable Product

- `.envforge.project.toml` manifest: schema + round-trip-safe parser declaring the project's env files and profile variants (e.g. `base = ".env"`, `profiles = { development = ".env.development", stage = ".env.stage", production = ".env.production" }`).
- LSP recognizes the resolved env-file set (including variant filenames) and attaches only to those + existing EnvForge-owned files.
- Key completion sourced from the union project key-set as the user types a key.
- Value completion offering known/per-environment values for a key as the user types after `=` (your explicit "suggest values" ask).
- Hover shows the key's per-environment values (redacted when sensitive) + provenance.
- Cross-environment "key missing in this environment" diagnostic.
- Delivered in VS Code + the generic LSP path first; manifest reload on change.

### Growth Features (Post-MVP)

- Quick-fix "add missing key to all declared environments"; rename a key across every variant atomically.
- Full visual parity (exposure heatmap / badges / signs) across all variants in IntelliJ, Neovim, Zed.
- Profile-aware exposure + fence reporting per environment.

### Vision (Future)

- Auto-detect existing `.env.*` files and scaffold `.envforge.project.toml` (init/wizard).
- Unify with `.env.schema` as the typed contract layered over the manifest's file set.
- Secret-provider-aware value hints (reference completion to provider keys).
- Multi-root / monorepo workspaces with several manifests.

## User Journeys

### Journey 1 — Backend developer, multi-environment project (primary, happy path)

**Maya** maintains a Spring service with `.env`, `.env.development`, `.env.stage`, and `.env.production`. Today her IDE treats each file as an opaque blob; she copy-pastes keys between them and discovers missing config only when staging boots and crashes.

She adds `.envforge.project.toml` declaring her four files. The moment she opens `.env.stage`, the IDE recognizes it (exposure overlay appears), and as she types `DATABASE_` it completes to `DATABASE_URL` — a key it knows from the project key-set because it exists in `.env.production`. Hover on `DATABASE_URL` shows its value in each environment (production redacted as sensitive). A diagnostic flags `STRIPE_WEBHOOK_SECRET` as "set in production, missing here." She fixes the gap before committing. **New reality:** the four files are one coherent key-set she edits with confidence, not four blobs she reconciles by hand.

### Journey 2 — Adding a new environment / malformed manifest (primary, edge case + recovery)

Maya introduces `.env.qa`. Until she adds it to `.envforge.project.toml`, the IDE does **not** treat it specially — no false attachment. She declares `qa = ".env.qa"`; on save the LSP reloads the manifest and `.env.qa` lights up immediately. Later she fat-fingers the TOML (duplicate profile key). The IDE surfaces a clear manifest diagnostic pointing at the offending line and falls back to the last-good env-set rather than silently dropping recognition. **Recovery path is explicit, never silent.**

### Journey 3 — Developer working alongside an AI coding agent (secondary)

**Sam** codes with an AI agent active. He needs env intelligence in his `.env.*` files but cannot tolerate the LSP hijacking his `.ts`/`.go` buffers (the pre-0.8.4 failure that broke native goto/completion). With manifest-scoped recognition, EnvForge fires only inside declared env files; Sam's source-language intelligence and his agent's behavior are untouched. Sensitive values stay redacted in hover/completion labels, so the agent never sees raw secrets through the editor surface. **The feature is present where wanted, invisible everywhere else.**

### Journey 4 — Tech lead onboarding the team / managing the manifest (operations)

**Dev**, the tech lead, owns `.envforge.project.toml`. He declares the canonical env-file set once and commits it; every teammate's IDE then recognizes the same files identically across VS Code, IntelliJ, Neovim, and Zed — no per-developer setup. When he adds `.env.preview`, one manifest edit propagates recognition to the whole team. **The manifest is the single, version-controlled source of truth for "what env files this project has."**

### Journey Requirements Summary

- **Manifest layer:** parse `.envforge.project.toml` (files + profile map), resolve env-file set, reload on change, diagnose malformed manifest with last-good fallback. *(Journeys 1, 2, 4)*
- **Recognition layer:** LSP attaches to exactly the resolved env-file set + existing EnvForge-owned files; nothing else. *(Journeys 1, 2, 3)*
- **Key-set intelligence:** union project key-set powering key completion; per-environment value model for hover. *(Journey 1)*
- **Cross-environment diagnostics:** detect keys present in ≥1 env and missing in another. *(Journey 1)*
- **AI-safety parity:** redaction in labels + fence/exposure across all variants; strict scoping so native + agent intelligence is undisturbed. *(Journey 3)*
- **Cross-client parity:** identical recognition + behavior on all four first-party clients from one manifest. *(Journey 4)*

## Domain-Specific Requirements

No external regulatory regime applies, but this DevSecOps + LSP domain imposes hard constraints:

### Security & AI-Safety

- Sensitive values must be redacted in every IDE surface (completion `label`, hover, inlay) for all recognized variants — raw values flow only through `text_edit.new_text` where the editor requires them, per existing redaction policy.
- Fence + exposure classification must apply identically across `.env`, `.env.development`, `.env.stage`, `.env.production` — no environment may be a redaction blind spot.
- Recognition driven by `.envforge.project.toml` must never widen the LSP's attach surface beyond EnvForge-owned files. Declaring an env file grants env-intelligence only; it must not turn the LSP into a general file handler.

### Technical Constraints

- **Byte-round-trip safety:** the manifest parser and any env-file edits (rename, quick-fix) must round-trip byte-identically (project Parser principle) — no reflowing of unrelated lines.
- **LSP protocol correctness:** UTF-16 position encoding for ranges; deterministic feature output with no client-capability branching (same input ⇒ same output across clients).
- **Manifest reload:** changes to `.envforge.project.toml` must re-resolve the env-set live (on save/change) without an LSP restart.
- **Graceful failure:** a malformed manifest yields a clear diagnostic and falls back to the last-good env-set; it must not crash the server or silently drop recognition.

### Integration Requirements

- Must compose with the existing `.env.schema` / `.env.schema.toml` typed contract and the `envforge/*` custom requests (exposure map, fence) — the manifest layer sits beneath schema, declaring the file set schema validates against.
- Must reach all four first-party clients via their existing attach mechanisms (VS Code `documentSelector` patterns, IntelliJ `languageMapping` `fileNamePattern`, Neovim filename-gated attach) without reintroducing source-language attach.

### Risk Mitigations

- **Risk: manifest filename/pattern explosion re-opens the over-attachment wound.** Mitigation: attach strictly to the resolved concrete file set, by filename/pattern, never by blanket language.
- **Risk: per-environment value model leaks secrets cross-file.** Mitigation: redaction applied at the label layer before any cross-env value is surfaced.
- **Risk: divergent behavior across clients.** Mitigation: all logic server-side; clients only attach + render.

## Innovation & Novel Patterns

### Detected Innovation Areas

- **Manifest as the env-file source of truth.** Generic dotenv IDE plugins treat each `.env*` file independently and lean on language/glob heuristics (which over-attach). Declaring the project's env-file set + profiles in `.envforge.project.toml` makes recognition explicit and authoritative — the IDE knows the exact, intended file set, not a guess.
- **Environment as a first-class dimension.** Keys are modeled as one logical key-set with per-environment values, enabling cross-environment intelligence (gap detection, per-env hover) that per-file plugins structurally cannot offer.
- **Scoped, AI-safe by construction.** Distinctive precisely because it refuses the easy path (attach broadly): recognition is concrete-file-scoped and redaction-parity is mandatory, so the feature adds intelligence without degrading the editor or leaking secrets to AI agents.

### Market Context & Competitive Landscape

Mainstream dotenv extensions (syntax highlighting + simple key hints) and framework-config plugins assume language/file-type ownership. EnvForge's angle is the opposite: a project-declared file set + environment-aware key-set + strict scoping. Incremental relative to "an LSP for `.env`," but the manifest + cross-environment model is the differentiator.

### Validation Approach

- Dogfood on a real multi-env project; confirm recognition matches the manifest exactly and cross-env diagnostics catch real gaps.
- Cross-client conformance suite: same manifest ⇒ identical recognized set + feature output on all four clients.
- Negative tests: undeclared `.env`-like files and source files receive **no** attachment.

### Risk Mitigation

- If the manifest model proves too heavyweight for small projects, fall back to recognizing the conventional `.env*` set with the manifest as an optional override (degrade gracefully, never block).

## Developer-Tool Specific Requirements

### Project-Type Overview

An LSP-backed IDE integration layer over the existing `envforge` CLI. All intelligence lives server-side in `envforge lsp`; the four first-party clients (VS Code, IntelliJ, Neovim, Zed) only attach to the resolved file set and render. The new surface area is a project manifest (`.envforge.project.toml`) plus the recognition + env-set-resolution layer that feeds existing LSP features (completion, hover, diagnostics).

### Technical Architecture Considerations

Pipeline: `parse .envforge.project.toml` → `resolve env-file set (base + profile variants)` → `recognition predicate (is this URI a declared env file?)` → `existing LSP feature dispatch over a unified key-set`. The manifest layer sits **beneath** `.env.schema` (schema types the keys; manifest enumerates the files). Server holds the resolved set in the existing `RwLock<HashMap>` doc/state model and re-resolves on manifest change. No client-capability branching — deterministic output across clients.

### IDE Integration & File-Support Matrix

| Concern | Approach |
|---|---|
| Recognized files | Exactly the set resolved from the manifest (`base` + each `profiles.*` value) + existing EnvForge-owned files (`.env*` defaults, `.env.schema*`, MCP config) |
| VS Code attach | `documentSelector` filename/pattern entries for the resolved files (no blanket `language:`) |
| IntelliJ attach | lsp4ij `languageMapping` with `fileNamePattern` per resolved file |
| Neovim attach | filename-gated attach (native filetype preserved; LSP start list widened, not remapped) |
| Zed attach | by language name where unavoidable (documented limitation); functionally safe (server ignores non-targets) |
| Forbidden | attaching to whole source languages or arbitrary TOML/YAML/JSON (the 0.8.4 regression class) |

### API Surface

- **Manifest schema** (`.envforge.project.toml`): `[project]` metadata; `base` (string path); `profiles` (table of `name → path`); optional `extra_files` (array) for non-profile env files. Versioned (`schema_version`).
- **LSP methods extended** (existing, now operating over the unified key-set): `textDocument/completion` (key + value), `textDocument/hover` (per-environment values), `textDocument/publishDiagnostics` (cross-environment gaps), plus exposure/fence parity via `envforge/exposureMap`.
- No new generic `executeCommand`; the disabled-by-default policy stands.

### Installation & Distribution

- Ships inside the existing first-party plugins + `envforge lsp` — no new install path. The manifest is an opt-in file a developer commits to the repo; absent a manifest, the LSP recognizes the conventional `.env*` set (backward compatible).

### Migration Guide

- Projects with only `.env.schema` keep working; adding `.envforge.project.toml` is additive (declares the file set the schema validates against).
- Projects relying on implicit `.env*` recognition keep working unchanged; the manifest overrides/extends the recognized set when present.

### Implementation Considerations

- Reuse the byte-round-trip parser discipline for both manifest and env-file edits.
- Manifest parse errors surface as diagnostics on `.envforge.project.toml` with last-good fallback.
- Conformance test: one manifest fixture ⇒ identical resolved set + feature output asserted across all client attach configs.

## Project Scoping & Phased Development

> Phasing below is a **recommended roadmap**, not a fixed commitment — it can collapse to a single release. **Every requirement you stated explicitly** (recognize `.env` + environment variants, match keys against the known env-var set, suggest **keys and values** while typing, `.envforge.project.toml` as source of truth) is in **MVP** — none deferred.

### MVP Strategy & Philosophy

**MVP Approach:** Problem-solving MVP — make multi-environment `.env` files coherent and authoritative in the IDE, scoped and AI-safe. **Resource Requirements:** Rust LSP work (manifest parser + env-set resolution + recognition predicate + key-set/value model) plus thin per-client attach config; primarily one systems engineer familiar with the existing `envforge lsp`.

### MVP Feature Set (Phase 1)

**Core User Journeys Supported:** Journey 1 (multi-env recognition + key/value suggestion + cross-env gaps), Journey 2 (add variant / malformed-manifest recovery), Journey 3 (scoped, undisturbed source intelligence), Journey 4 (one manifest → team-wide recognition).

**Must-Have Capabilities:**
- `.envforge.project.toml` schema + round-trip-safe parser (`base`, `profiles`, optional `extra_files`, `schema_version`).
- LSP resolves the declared env-file set and recognizes exactly those + existing EnvForge-owned files; manifest reload on change; malformed-manifest diagnostic with last-good fallback.
- **Key completion** from the union project key-set as the user types a key.
- **Value completion** offering known values for a key (incl. per-environment values) as the user types after `=` — *(your explicit "suggest values" ask; promoted from Growth to MVP)*.
- Hover showing per-environment values (sensitive redacted) + provenance.
- Cross-environment "key missing in this environment" diagnostic.
- AI-safety parity (redaction + fence/exposure) across all recognized variants.
- Delivered on VS Code + generic LSP first; the other three first-party clients attach via their existing mechanisms.

### Post-MVP Features

**Phase 2 (Growth):**
- Quick-fix "add missing key to all declared environments"; atomic rename of a key across every variant.
- Full visual parity (heatmap / badges / signs) for variants across IntelliJ, Neovim, Zed.
- Profile-aware exposure + fence reporting per environment.

**Phase 3 (Vision):**
- Auto-detect existing `.env.*` and scaffold `.envforge.project.toml` (init/wizard).
- Manifest ↔ `.env.schema` unification as the typed contract over the declared file set.
- Secret-provider-aware value hints; multi-root / monorepo manifests.

### Risk Mitigation Strategy

- **Technical Risks:** re-opening the over-attachment regression → attach strictly to the concrete resolved file set, never by language; conformance + negative tests gate it. Per-environment value model leaking secrets → redaction at the label layer before any cross-env value is surfaced.
- **Market/Adoption Risks:** manifest friction on small projects → manifest optional; absent it, fall back to conventional `.env*` recognition (backward compatible).
- **Resource Risks:** if capacity is tight → ship MVP key+value completion + recognition on VS Code/generic LSP first; defer full multi-client visual parity to Growth without dropping any core capability.

## Functional Requirements

> Capability contract. Each FR is a testable, implementation-agnostic capability. Actors: **Developer**, **Tech Lead**, **IDE/LSP** (system), **AI agent** (bystander).

### Project Manifest

- FR1: A Tech Lead can declare the project's env-file set in a `.envforge.project.toml` manifest, specifying a base env file and a set of named profile variants.
- FR2: A Tech Lead can declare additional non-profile env files in the manifest (beyond base + profiles).
- FR3: The IDE/LSP can parse the manifest and resolve it into a concrete set of recognized env-file paths.
- FR4: The IDE/LSP can re-resolve the recognized set when the manifest changes, without requiring a server/editor restart.
- FR5: The IDE/LSP can report a malformed manifest as a diagnostic on `.envforge.project.toml` and continue serving the last successfully resolved set.
- FR6: When no manifest is present, the IDE/LSP can fall back to recognizing the conventional `.env*` file set (backward compatible).

### Env-File Recognition & Attach Scoping

- FR7: The IDE/LSP can recognize each file in the resolved set — including environment variants (`.env.development`, `.env.stage`, `.env.production`, etc.) — as an EnvForge-owned env file.
- FR8: The IDE/LSP can attach only to the resolved set plus pre-existing EnvForge-owned files (`.env*` defaults, `.env.schema*`, MCP config), and to nothing else.
- FR9: The IDE/LSP can refrain from attaching to whole source languages or to arbitrary TOML/YAML/JSON files.
- FR10: A Developer can have an undeclared `.env`-like file remain unrecognized until it is added to the manifest.

### Key & Value Suggestion

- FR11: A Developer can receive key completions drawn from the project's known key-set (the union of keys across all recognized environments) while typing a key.
- FR12: A Developer can receive value completions for a key while typing its value, including values already set for that key in other environments.
- FR13: A Developer can trigger these completions through normal typing (assignment/reference trigger characters), consistent with existing LSP completion behavior.
- FR14: A Developer can have schema-declared keys/values (from `.env.schema`) included in completions when a schema is present.

### Cross-Environment Intelligence

- FR15: A Developer can hover a key and see its value in each recognized environment, with provenance (which file/environment defines it).
- FR16: The IDE/LSP can model keys as one logical key-set with per-environment values across the recognized files.
- FR17: A Developer can see a diagnostic when a key is present in at least one recognized environment but missing from the current one.
- FR18: A Developer can navigate from a key occurrence to its definitions across the recognized env files (and `.env.schema` when present).

### AI-Safety & Redaction Parity

- FR19: The IDE/LSP can redact sensitive values in all surfaced labels (completion, hover, inlay) for every recognized variant.
- FR20: The IDE/LSP can apply fence and exposure classification identically across all recognized environments (no environment is a redaction blind spot).
- FR21: An AI agent operating in the editor can be prevented from observing raw sensitive values through EnvForge's IDE surfaces.

### Cross-Client Consistency

- FR22: A Developer can get identical recognition and feature behavior from one manifest across VS Code, IntelliJ, Neovim, and Zed (subject to each client's documented overlay limits).
- FR23: A Tech Lead can commit one manifest and have every teammate's IDE recognize the same env-file set without per-developer configuration.
- FR24: The IDE/LSP can produce deterministic feature output independent of client capabilities (same input ⇒ same output across clients).

## Non-Functional Requirements

> Only categories that matter for this product. Scalability and Accessibility are intentionally omitted (no traffic-growth or public-UI dimension).

### Performance

- NFR1: Completion and hover round-trips on a recognized env file complete in < 100 ms on a typical project (≤ ~10 env files, ≤ ~500 keys total).
- NFR2: Manifest parse + env-set re-resolution after a `.envforge.project.toml` change completes in < 200 ms and never blocks unrelated LSP requests.
- NFR3: Recognition adds no measurable latency to editing files outside the recognized set (the LSP is never consulted for them).

### Security

- NFR4: Sensitive values are redacted in 100% of IDE-surfaced labels (completion, hover, inlay) across every recognized environment; raw values traverse only `text_edit.new_text` where the editor requires insertion.
- NFR5: Fence and exposure classification are applied with identical rules to every recognized variant — no environment is exempt.
- NFR6: Declaring a file in the manifest grants env-intelligence only; it must never widen the LSP's attach surface to non-EnvForge file handling.
- NFR7: No EnvForge IDE surface exposes a raw sensitive value to an AI agent reading the editor.

### Reliability

- NFR8: A malformed `.envforge.project.toml` never crashes the LSP; the server emits a manifest diagnostic and serves the last successfully resolved set.
- NFR9: Manifest parsing and any env-file edit (rename, quick-fix) round-trip byte-identically — no reflow of unrelated lines/comments.
- NFR10: Recognition and feature behavior cause zero regressions in native source-language IDE features (verified per first-party client).

### Compatibility & Integration

- NFR11: Feature output is deterministic across clients — no branching on client capabilities (same input ⇒ same output on VS Code, IntelliJ, Neovim, Zed, and generic LSP clients).
- NFR12: LSP positions use correct UTF-16 encoding for all ranges in recognized files.
- NFR13: The manifest layer composes with the existing `.env.schema` / `.env.schema.toml` contract and the `envforge/*` custom requests without breaking them.
- NFR14: Behavior degrades gracefully where a client lacks a surface (e.g. Zed overlays) — language features still serve; missing overlays are documented, not silent failures.
