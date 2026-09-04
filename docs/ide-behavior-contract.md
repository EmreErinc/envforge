# EnvForge IDE Behavior Contract

Every IDE feature is implemented once in `envforge lsp` and rendered identically by both first-party plugins. Any drift between the VS Code and IntelliJ columns is a bug.

This file is the single source of truth for triggers, wording, icons, and keybindings. Plugins MUST match. Tests in `tests/lsp_phase1_tests.rs` (and successor parity files) enforce the LSP response body.

**Third-party LSP clients** (Helix, Emacs, Sublime Text, Kakoune, Lapce, … — Neovim and Zed now have first-party integrations under `editors/`) consume the same `textDocument/*` capabilities plus the named `envforge/*` custom requests (`envforge/exposureMap`, `envforge/fenceStatus`, `envforge/revealValue`, …) — they just don't get the native status-bar / gutter-heatmap UI without a dedicated plugin. The generic `workspace/executeCommand` provider is **disabled**; security operations go through the named requests instead. See [`lsp-clients.md`](lsp-clients.md) for per-editor setup snippets.

> **Note (v0.8.3):** Sections below that describe security commands via `workspace/executeCommand` reflect the prior wiring. The operations are unchanged, but they are now invoked as named `envforge/*` JSON-RPC requests (same `{ok, result|error}` payloads); the generic `executeCommand` endpoint returns `MethodNotFound`.

## Conventions

- **LSP method**: textDocument/* or workspace/executeCommand command ID
- **Trigger**: the user gesture that fires the feature
- **Wording**: exact strings the user sees (markdown bodies, menu titles, status bar text). Pinned by parity tests.
- **VS Code keybind / IntelliJ keybind**: when a feature is bound, both columns must match by purpose. Default-handler features (hover, completion) inherit each IDE's built-in keybind and are marked `default`.
- **Test ID**: name of the parity test that fences this row.

## Feature rows

### L3 — Hover with provenance

| Field | Value |
|---|---|
| LSP method | `textDocument/hover` |
| Trigger | Cursor on env-var key in `.env*` file |
| VS Code keybind | default hover (`Ctrl+K Ctrl+I` / mouseover) |
| IntelliJ keybind | default hover (`Ctrl+Q` / mouseover) |
| Returned content type | Markdown |
| Mandatory sections | `**KEY**` heading; schema block (if schema entry exists); `---` separator; `**Provenance**` block |
| Provenance lines | `- Defined by: \`<schema \| schema + local \| local (managed by envforge)>\``  ·  `- Current value: \`<value \| redacted preview \| not set \| not managed>\``  ·  `- Source file: \`<basename>\`` (only when managed) |
| Sensitivity rule | If schema marks `sensitive: true` OR key matches `is_sensitive_key`, the current value is rendered via `redact::redact_for_label` and suffixed with `(redacted)`. Raw value never appears in markdown for sensitive keys. |
| Test ID | `test_hover_*` in `tests/lsp_phase1_tests.rs` |

### L2 — Schema diagnostics (unknown-key + quick-fix)

| Field | Value |
|---|---|
| LSP method | `textDocument/publishDiagnostics` (push) + `textDocument/codeAction` |
| Trigger | Open or edit `.env*` while `.env.schema.toml` (or `.env.schema`) defines a variable set |
| Severity | `Warning` (non-blocking) |
| Message format | `Unknown key '<KEY>' (not in schema)` |
| Range | The key's `key_range` (highlights only the key, not the value) |
| Quick-fix title | `Add <KEY> to schema` |
| Quick-fix effect | Appends `\n[<KEY>]\ntype = "string"\n` to the schema file at the line indicated by `schema_line_count` (server-tracked end-of-schema-file marker) |
| VS Code render | Yellow squiggle under key in editor + lightbulb code action |
| IntelliJ render | Yellow inspection under key + Alt-Enter quick-fix list |
| Test IDs | `test_diagnostic_unknown_key_warning`, `test_diagnostic_no_unknown_when_schema_absent`, `test_code_action_add_to_schema`, `test_code_action_add_to_schema_skipped_when_no_schema_uri` |

### L7 — Inlay hints

| Field | Value |
|---|---|
| LSP method | `textDocument/inlayHint` |
| Capability | `inlay_hint_provider: true` (declared in `initialize`) |
| Trigger | Editor requests inlay hints for a visible range in a `.env*` file |
| Position | End of `value_range` for each `EnvVar` entry inside the requested range |
| Hint kind | `InlayHintKind::TYPE` |
| Rule precedence (first match wins) | 1. Empty value + schema → `(<type>)` (e.g. `(string)`, `(port)`) · 2. Value matches schema `default` → `(default)` · 3. Value is `${REF}` and resolves via managed vars → `→ <redacted-if-sensitive>` · 4. Value is `${REF}` and unresolved → `→ ?` · 5. Sensitive key with literal value → `(<redacted>)` · 6. Otherwise → no hint |
| Sensitivity rule | Sensitivity determined by `schema.sensitive == true` OR `is_sensitive_key(key)`. Sensitive resolutions/values rendered via `redact::redact_for_label`. |
| Padding | `padding_left: true` (visual gap from value) |
| VS Code render | Ghost text after value via inlay-hint client |
| IntelliJ render | Inlay chip via lsp4ij inlay client |
| Test IDs | `test_inlay_hint_default_marker`, `test_inlay_hint_type_for_empty_value`, `test_inlay_hint_ref_resolution_redacted_for_sensitive`, `test_inlay_hint_ref_unresolved`, `test_inlay_hint_skips_comments_and_blanks`, `test_inlay_hint_sensitive_value_redacted`, `test_inlay_hint_respects_range_window` |

### L4 — Go-to-definition (source code → schema) — server capability, opt-in only

> **Not delivered by first-party clients.** The VS Code, IntelliJ, and Neovim
> clients no longer attach the LSP to source languages — they attach only to
> EnvForge's own files (`.env*`, `.env.schema*`, MCP config). The server still
> implements this capability for generic LSP clients that opt in by adding their
> own source-language document selectors.

| Field | Value |
|---|---|
| LSP method | `textDocument/definition` |
| Trigger | Cursor on an env-var identifier inside a source file (TS/JS/Python/Rust/Go/Java/Kotlin/Ruby/PHP/C#/Shell), invoke "Go to Definition" |
| Identifier rule | Walk outward from cursor over `[A-Z0-9_]`. Reject if shorter than 2 chars, all digits/underscores, or has no ASCII uppercase letter. This is what gates the feature from firing on ordinary local variables. |
| Match rule | Identifier must exist as a top-level key in `.env.schema.toml` (or `.env.schema`) — looked up via `schema_line_map`. |
| Result | `Location` pointing at the schema file URI, range `(line=N, char=0) .. (line=N, char=0)` where `N` is the schema entry line. |
| Server file ingestion | Server reads the source file from disk, NOT from `did_open` state. Path must canonicalize successfully and stay inside the canonicalized workspace root. Allowed extensions: `.ts .tsx .js .jsx .mjs .cjs .py .rs .go .java .kt .rb .php .cs .sh`. Files larger than `MAX_DOCUMENT_BYTES` (1 MiB) are rejected. |
| First-party client wiring | **None.** The VS Code `documentSelector`, IntelliJ `languageMapping`, and Neovim filetype list no longer include source languages — first-party clients attach only to EnvForge's own files, so this lookup never fires there. A generic LSP client must add the source-language selectors itself to use it. |
| `.env` → schema dispatch | Always available: when the URI is a `.env*` file, the server routes through the `documents` map + `goto_definition` (key in `.env` → `.env.schema` section). This is the goto-definition first-party clients do deliver. |
| Test IDs | `test_source_goto_def_typescript_process_env_dot`, `test_source_goto_def_typescript_bracket_access`, `test_source_goto_def_python_os_environ`, `test_source_goto_def_rust_env_var`, `test_source_goto_def_go_getenv`, `test_source_goto_def_returns_none_on_lowercase_identifier`, `test_source_goto_def_returns_none_when_identifier_missing_from_schema`, `test_source_goto_def_returns_none_when_no_schema_uri`, `test_source_goto_def_clamped_cursor_past_eol`, `test_source_goto_def_unicode_line_safe` |

### L8 — Rename symbol

| Field | Value |
|---|---|
| LSP method | `textDocument/rename` |
| Capability | `rename_provider: true` declared in `initialize` |
| Trigger (env file) | Cursor inside `key_range` of an `EnvDocEntry` in a `.env*` file → invoke "Rename Symbol" |
| Trigger (source file) | Cursor on an `UPPER_SNAKE_CASE` identifier in an allow-listed source file → invoke "Rename Symbol". **Server capability only — first-party clients no longer attach to source languages, so this does not fire there; opt-in for generic clients.** |
| Identifier extraction | Reuses `definition::extract_upper_snake_identifier` (same gates as L4) |
| New name validation | Must match `^[A-Za-z_][A-Za-z0-9_]*$`. Empty / invalid characters / leading-digit / no-op `NEW == OLD` → returns `None` (client surfaces error, no edit applied) |
| Workspace edit scope | Schema file table header `[OLD]` → `[NEW]` on the line tracked in `schema_line_map`. Plus every currently-open `.env*` `documents` entry whose key equals `OLD`: `key_range` rewritten to `NEW`. |
| Out of scope (first cut) | Source-file textual references. Clients can run their own refactor across code references; envforge owns truth for schema + env files. |
| Untracked env files | `.env*` files the client has never opened are not edited. Open them in the editor before invoking rename to include them. |
| VS Code render | Default rename input box + multi-file diff preview if "preview" enabled |
| IntelliJ render | Default rename dialog via lsp4ij — applies WorkspaceEdit on accept |
| Test IDs | `test_rename_propagates_to_schema_and_env_docs`, `test_rename_rejects_invalid_identifier`, `test_rename_noop_returns_none`, `test_rename_returns_none_when_no_match_anywhere`, `test_rename_without_schema_still_edits_open_env_docs`, `test_rename_accepts_leading_underscore_identifier` |

### L14 — MCP config linter

| Field | Value |
|---|---|
| LSP method | `textDocument/publishDiagnostics` (push on `did_open` + `did_change`) |
| Trigger | Open or edit one of: `**/mcp.json` (covers `.cursor/`, `.vscode/`), `**/.mcp.json`, `**/.claude/settings.json`, `**/claude_desktop_config.json`, `**/.claude.json` (Claude Code), `**/mcp_config.json` (Windsurf), `**/cline_mcp_settings.json` (Cline) — widened in Story 3.1 (FR18) |
| Severity | `Warning` |
| Source tag | `envforge-mcp` (distinct from `envforge` on `.env` diagnostics so clients can filter independently) |
| Detection rules | Reuses `ops::mcp_scan` heuristics: known-prefix tokens (AWS, GitHub, Stripe, Slack, SendGrid, JWT, …), connection strings with embedded credentials, sensitive-key-name + secret-looking value combinations |
| Message format | `Hardcoded credential in MCP config: <pattern> at \`<json.path>\` (value \`<masked>\`). Replace with \`${ENV_VAR}\` and load via envforge.` |
| Range | First occurrence of the offending JSON value's string contents in the source line; falls back to (0,0)–(0,0) if not locatable. |
| Ignored | Values starting with `${` or `$`, values shorter than 4 chars, valid JSON parse failures (file silently skipped). |
| Quick-fix (Story 3.2 / FR19) | `textDocument/codeAction` on an `envforge-mcp` diagnostic offers `Replace hardcoded credential with ${VAR}` (QUICKFIX, `is_preferred`): replaces the value range with `${VAR}` where `VAR` is the camelCase-aware SCREAMING_SNAKE of the JSON key. CLI batch equivalent: `envforge mcp harden`. |
| VS Code wiring | `documentSelector` extended with 8 MCP config glob patterns (Story 3.1). |
| IntelliJ wiring | 7 `languageMapping` entries with `fileNamePattern` for JSON-typed MCP/agent config filenames (Story 3.1). |
| Test IDs | `test_mcp_diagnostic_flags_aws_access_key`, `test_mcp_diagnostic_flags_github_pat_with_range`, `test_mcp_diagnostic_ignores_env_var_references`, `test_mcp_diagnostic_flags_postgres_connection_string`, `test_mcp_diagnostic_skips_invalid_json`, `test_mcp_diagnostic_flags_multiple_findings` |

### L13 — Save-time AI-guard diagnostics

| Field | Value |
|---|---|
| LSP method | `textDocument/publishDiagnostics` (push on `did_save` only) |
| Trigger | Save (`Ctrl+S` / `Cmd+S`) of a `.env*` file |
| Why save-only | Prompt-injection content tends to be pasted whole and finalised on save; running mid-typing would flicker without catching anything earlier. The scanner is also heavier than schema diagnostics. |
| Severity mapping | Pattern intrinsic severity → LSP severity: `Critical`/`High` → `Error`, `Medium` → `Warning`, `Low` → `Information` |
| Source tag | `envforge-aiguard` (distinct from `envforge` and `envforge-mcp`) |
| Patterns detected | Reuses `DescriptionScanner` from `ops::mcp_poison`: `ignore_previous`, `disregard_synonyms`, `new_instructions`, role markers, Claude meta tags, tool-call injection, Unicode tag smuggling, zero-width characters, bidi overrides, exfil keyword combos (`curl/webhook/fetch + key/token/secret`) |
| Range | Exact byte span of each pattern match converted to LSP line/col |
| Diagnostic union | On `did_save`, both schema diagnostics (`envforge`) AND ai-guard diagnostics (`envforge-aiguard`) are published in a single batch so neither set clobbers the other. |
| VS Code render | Native diagnostic squiggles; appears only after save |
| IntelliJ render | Native inspection markers via lsp4ij; appears only after save |
| Test IDs | `test_ai_guard_flags_ignore_previous_instructions`, `test_ai_guard_clean_env_produces_no_findings`, `test_ai_guard_flags_exfil_keyword_combo`, `test_ai_guard_finding_range_within_offending_line` |

### L5 — Find references

| Field | Value |
|---|---|
| LSP method | `textDocument/references` |
| Capability | `references_provider: true` declared in `initialize` |
| Trigger (env file) | Cursor inside `key_range` of a `.env*` entry → invoke "Find Usages" / "Find All References" |
| Trigger (source file) | Cursor on UPPER_SNAKE identifier in source file → invoke "Find Usages". **Server capability only — first-party clients no longer attach to source languages, so this does not fire there; opt-in for generic clients.** |
| Identifier extraction | Mirrors L4: `definition::extract_upper_snake_identifier` |
| `includeDeclaration` honored | When `true` (default), schema header is included as declaration. When `false`, only `.env*` entry references are returned. |
| Locations returned | Schema header line for the key (if schema declares it) PLUS every `key_range` in any currently-open `.env*` document whose entry key equals the target |
| Out of scope (first cut) | Source-file textual references. Workspace-walk would be heavy on every request; deferred until usage telemetry justifies. |
| Untracked env files | Not searched — same scope rule as L8 rename |
| VS Code render | Native "References" peek + side panel via `vscode-languageclient` |
| IntelliJ render | "Usages" tool window via lsp4ij |
| Test IDs | `test_references_includes_schema_and_open_env_docs`, `test_references_excludes_declaration_when_requested`, `test_references_returns_empty_when_no_match`, `test_references_without_schema_still_finds_env_doc_matches`, `test_references_multiple_entries_same_doc` |

### L10 — Document formatting

| Field | Value |
|---|---|
| LSP method | `textDocument/formatting` |
| Capability | `document_formatting_provider: true` declared in `initialize` |
| Trigger | Editor "Format Document" command on a `.env*` file |
| Scope | `.env*` files only. Schema files and MCP configs return `None`. |
| Edit shape | Single full-document `TextEdit` (or empty vec when content is already canonical, so unchanged buffers do not get dirtied). |
| Rule 1 | Trim trailing whitespace from every line. |
| Rule 2 | Normalize whitespace around `=`: `KEY = value`, `KEY  =value`, `KEY=  value` → `KEY=value`. |
| Rule 3 | Normalize `export   FOO=…` → `export FOO=…` (single space after `export`). |
| Rule 4 | Collapse runs of 3+ consecutive blank lines down to 2. |
| Rule 5 | Ensure exactly one trailing newline at end of file. |
| Preserved verbatim | Comments, key ordering, quoted-value internals (including trailing whitespace inside quotes), unparseable / non-env lines. |
| Idempotent | Yes — `format(format(x)) == format(x)`. |
| VS Code render | "Format Document" (`Shift+Alt+F`) applies edits via `vscode-languageclient` |
| IntelliJ render | "Reformat Code" (`Ctrl+Alt+L` / `Cmd+Opt+L`) applies edits via lsp4ij |
| Test IDs | `test_format_normalizes_whitespace_around_equals`, `test_format_trims_trailing_whitespace`, `test_format_preserves_quoted_value_internals`, `test_format_collapses_blank_line_runs`, `test_format_preserves_comments`, `test_format_normalizes_export_prefix_spacing`, `test_format_ensures_trailing_newline`, `test_format_is_idempotent`, `test_format_returns_empty_edits_when_already_canonical`, `test_format_emits_single_full_replace_edit`, `test_format_does_not_touch_non_env_lines` |

### L11 — Semantic tokens

| Field | Value |
|---|---|
| LSP method | `textDocument/semanticTokens/full` |
| Capability | `semantic_tokens_provider` declared in `initialize` with full legend |
| Scope | `.env*` files only (non-env URIs return `None`) |
| Token-type legend (index → SemanticTokenType) | 0 → `VARIABLE` · 1 → `STRING` · 2 → `COMMENT` |
| Token-modifier legend (bit → SemanticTokenModifier) | bit 0 → `READONLY` (used as the "sensitive / secret" marker — every default VS Code + JetBrains theme tints `readonly` distinctly, which is exactly the visual cue secrets need) |
| Token emission rules | `EnvVar` entry → key token (VARIABLE) + value token (STRING). `Comment` entry → comment token spanning whole line. `Blank` / `Other` → no tokens. |
| Sensitivity rule | If `schema.sensitive == true` OR `is_sensitive_key(key)`, both the key and value tokens carry the `READONLY` modifier bit. |
| Encoding | LSP delta encoding: first token absolute (delta_line=0, delta_start=0); subsequent tokens deltas vs previous; new line resets `delta_start` to absolute. |
| Sort order | `(line, start)` ascending — required by LSP spec. |
| Empty document | Returns `None` rather than empty tokens to skip the client roundtrip. |
| VS Code render | Tints via active theme's semantic highlighting (`editor.semanticHighlighting.enabled`) |
| IntelliJ render | Tints via lsp4ij semantic highlighting bridge to JetBrains color scheme |
| Test IDs | `test_semantic_tokens_emits_key_value_comment`, `test_semantic_tokens_marks_sensitive_keys_readonly`, `test_semantic_tokens_delta_encoding_first_token_absolute`, `test_semantic_tokens_delta_encoding_subsequent_token_same_line`, `test_semantic_tokens_delta_encoding_new_line_resets_start`, `test_semantic_tokens_uses_schema_sensitive_flag`, `test_semantic_tokens_skip_blank_and_other_lines` |

### C3 — Workspace executeCommand (fence enable / status / config)

| Field | Value |
|---|---|
| LSP method | `workspace/executeCommand` |
| Capability | `execute_command_provider` advertises `SUPPORTED_COMMANDS` legend in `initialize` |
| Command set | `envforge.fence.enable` (calls `ops::fence::create_fence`), `envforge.fence.disable` (calls `ops::fence::remove_fence` — strips envforge-owned content, preserves user content), `envforge.fence.toggle` (probes status, flips direction; returns `{"action": "enabled"\|"disabled"}`), `envforge.fence.status` (calls `ops::fence::check_fence_status`), `envforge.fence.config` (returns resolved per-target config as `[{target, enabled, source}]`) |
| Return shape | `{ "ok": true, "result": <payload> }` on success, `{ "ok": false, "error": "<msg>" }` on failure. Stable JSON contract so plugins don't depend on internal Rust types. |
| `envforge.fence.config` result | Array of 5 objects: `[{ "target": "<snake_case_id>", "enabled": <bool>, "source": "default"\|"global" }]`. Ordered by `FenceTarget::all()` canonical order. `workspace_root` required; returns error if absent. |
| `envforge.fence.status` result (v0.8.3+) | Includes `resolved_targets` field alongside existing `files`, `all_fenced`, `completeness` — same shape as `fence.config` result. `all_fenced` is now relative to the *enabled* target set only (behavior change from v0.8.2 and earlier). |
| Workspace root | Derived from `Backend.workspace_root` (set in `initialize`); commands that touch the filesystem fail with `"workspace root not available"` if absent. |
| Unknown command | Returns `{ "ok": false, "error": "unknown command: <id>" }` rather than throwing — keeps clients robust. |
| Test IDs | `test_command_dispatch_unknown_command_returns_error`, `test_command_dispatch_fence_enable_requires_workspace_root`, `test_command_dispatch_fence_status_requires_workspace_root`, `test_command_dispatch_fence_enable_writes_fence_files`, `test_command_dispatch_fence_status_reflects_freshly_enabled_state`, `test_command_dispatch_fence_status_clean_dir_not_all_fenced`, `test_command_dispatch_fence_config_requires_workspace_root`, `test_command_dispatch_fence_config_returns_target_array`, `test_command_dispatch_fence_config_default_all_enabled`, `test_command_dispatch_fence_status_includes_resolved_targets`, `test_command_dispatch_fence_config_parity_with_fence_status`, `test_command_dispatch_unknown_fence_variant_still_rejected` |

### P1+P2 — Status bar fence indicator + toggle

| Field | Value |
|---|---|
| Trigger | Always-on status bar item; click fires `envforge.fence.toggle` (VS Code) or "Tools > EnvForge > Toggle Fence" (IntelliJ) |
| Data source | Both plugins shell out to `envforge fence --status --json` and read `all_fenced` boolean. Plugins cache + refresh every 30 s. `all_fenced` is relative to the *enabled* target set (v0.8.3+): a disabled target's stale file does not flip `all_fenced` to `false`. |
| Tooltip (v0.8.3+) | Tooltip now lists the active fence targets from `resolved_targets` in the `fence.status` response (e.g. "cursor_ignore, copilot (2/5)"). Plugins should read `result.resolved_targets` from `envforge.fence.status` or call `envforge.fence.config` to populate the tooltip. |
| Render — fenced | `$(shield) AI BLOCKED` (VS Code) / `… · AI BLOCKED` (IntelliJ widget text). VS Code uses warning-tinted background. |
| Render — unfenced | `$(shield) AI ALLOWED` (VS Code) / `… · AI ALLOWED` (IntelliJ). Plain background. |
| Render — unknown | Hidden (don't misrepresent fence state) |
| VS Code wiring | New `envforge.fence.toggle` VSCode command in `commands.ts`: confirms via modal, sends `workspace/executeCommand` with `envforge.fence.enable`, refreshes status bar. Registered in `package.json` `contributes.commands`. |
| IntelliJ wiring | `EnvForgeStatusWidget` extended to compose `<N> vars · AI BLOCKED/ALLOWED`. Refresh moved off the UI thread (`executeOnPooledThread`). Existing `ToggleFenceAction` in Tools menu serves as the toggle action surface. |
| Toggle behavior | Click probes `envforge.fence.status`; calls `envforge.fence.toggle` which flips direction. User content in `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json` is preserved when fence is disabled; envforge-owned blocks are stripped surgically. `.envforgeignore` is fully envforge-owned and deleted on disable. Only *enabled* targets are written on enable. |
| CLI parity | `envforge fence` enables; `envforge fence --disable` disables; `envforge fence --status [--json]` reads state; `envforge fence config [--list\|--enable\|--disable TARGET\|--json]` manages per-target config. |
| VS Code tests | Manual smoke in both IDEs. LSP-side coverage via the C3 test IDs above. |

### L6 — Code actions (expanded quick-fix set)

| Field | Value |
|---|---|
| LSP method | `textDocument/codeAction` |
| Per-diagnostic actions | **Unknown key '...'** → `Add <KEY> to schema` (writes TOML block to schema file). **Missing required variable: ...** → `Add <KEY>` (appends to env doc). **Sensitive value for 'KEY'** → `Use secret reference for KEY` + `Mark KEY as secret in schema`. **Type validation failed / Invalid** → `Use default value for KEY` (when schema declares a default). |
| Aggregate actions | When 2+ missing-required diagnostics are present, an extra `Add all missing keys (N)` action combines all inserts into one edit. When the env doc has zero `EnvVar` lines and the schema declares keys, an extra `Generate .env from schema (N keys)` action scaffolds the whole file (sorted, defaults/examples populated). |
| Mark-as-secret edit shape | Inserts `sensitive = true\n` on the line immediately after the schema's `[KEY]` header. TOML semantics keep it inside the same table. Suppressed when `schema.sensitive == true` already. |
| Generate edit shape | Single TextEdit at the document head emitting `KEY=value\n` per schema-declared key, sorted lexicographically. Value preference: `default` > `example` > empty. |
| Bulk-add edit shape | Single TextEdit appending each missing key on its own line at the end of the document. |
| Threading | `code_actions(uri, entries, diagnostics, schema, schema_uri, schema_line_count, schema_lines)` — `schema_lines` (mapping `KEY → line number`) is required for the mark-as-secret action. |
| VS Code render | Lightbulb in the gutter; menu lists all applicable actions in the order returned. |
| IntelliJ render | Alt-Enter quick-fix list via lsp4ij. |
| Test IDs | `test_code_action_missing_required`, `test_code_action_sensitive_value`, `test_code_action_no_diagnostics`, `test_code_action_add_to_schema`, `test_code_action_add_to_schema_skipped_when_no_schema_uri`, `test_code_action_mark_secret_in_schema`, `test_code_action_mark_secret_suppressed_when_already_sensitive`, `test_code_action_add_all_missing_keys_bulk`, `test_code_action_bulk_skipped_for_single_missing`, `test_code_action_generate_from_schema_when_doc_empty`, `test_code_action_generate_suppressed_when_doc_has_env_lines` |

### L1 — Schema-aware completion (parity lock-in)

| Field | Value |
|---|---|
| LSP method | `textDocument/completion` |
| Capability | Trigger characters: `=`, `$` |
| Position kinds | **Key position** (cursor not after `=` and not after `$`): list schema-declared keys + envforge-managed keys, filter by typed prefix, exclude already-defined keys. **Value position** (cursor after `=` on same line): emit values for the declared `var_type` (Bool → `true`/`false`; Enum → declared values; otherwise default + example); plus any current managed value, plus `${OTHER}` references. **Ref position** (cursor immediately after `$` or inside `${`): emit every key from current document + managed vars, formatted as `${KEY}`. |
| Sort order | `sort_text` prefix `0_` for schema, `1_` for managed, `z_` for refs — schema completions always rank first. |
| CompletionItem shape (schema key) | `label = KEY`; `kind = VARIABLE`; `detail = "<type> [(required)]"`; `documentation = description` (Markdown); `text_edit.new_text = "KEY=<default-or-example-or-empty>"`. |
| Sensitivity rule | When inserting a managed-var current value, the **label is redacted** (`<head>***(N chars)`) so it never appears in completion history; the **raw value flows only through `text_edit.new_text`** so accepting the suggestion still works. This is enforced by `redact::redact_for_label` and is the regression guard for the two prior IDE-specific completion bugs. |
| Parity contract | The plugins do NOT re-implement any of the rules above. Plugins are dumb LSP clients; any divergence is a bug. The `test_completion_command_dispatch_marker` test fences the canonical output shape (label / kind / detail / new_text / documentation). |
| Test IDs | `test_completion_key_position_lists_schema_keys`, `test_completion_key_position_excludes_already_defined_keys`, `test_completion_value_position_enum_lists_allowed_values`, `test_completion_value_position_bool_lists_true_false`, `test_completion_value_redacts_sensitive_managed_in_label_keeps_raw_in_edit`, `test_completion_ref_position_lists_other_entries`, `test_completion_value_position_emits_dollar_refs_for_other_entries`, `test_completion_includes_managed_vars_when_no_schema`, `test_completion_key_position_filters_by_prefix`, `test_completion_key_insert_text_includes_default_when_present`, `test_completion_command_dispatch_marker` |

### P5 — AI-exposure heatmap (LSP custom request)

| Field | Value |
|---|---|
| LSP method | `envforge/exposureMap` (custom JSON-RPC request, registered via `LspService::build().custom_method(...)`) |
| Request params | `{ "uri": "file:///..." }` — target env-file URI |
| Response shape | `{ "entries": [{ "line": <u32>, "key": "<KEY>", "level": "red"\|"amber"\|"green", "reason": "<string>" }], "fence_active": <bool> }` |
| Wire format | `level` is lowercase string (serde rename); pinned by `test_exposure_map_serializes_levels_as_lowercase` so plugin decoders stay valid across refactors. |
| Classification precedence | 1. `fence_active == true` → all entries `green` (workspace fence instructs AI agents to refuse reads). 2. else if `schema.sensitive` or `is_sensitive_key(key)` → `amber` (AI-guard will redact in tool inputs but file content is plaintext on disk). 3. else → `red` (no protection). |
| Skipped lines | Comments, blanks, `Other` line types — no env-var content to classify. |
| Trigger | Plugin clients send on `did_open` and `did_change` for `.env*` files; refresh on save. |
| Fence probe | `check_fence_status(workspace_root)` runs per request. Lightweight disk-stat; cache TTL not yet introduced (cost is negligible vs the LSP roundtrip itself). |
| VS Code rendering | **Shipped.** `editors/vscode/src/exposure.ts` defines `ExposureRenderer` with three pre-allocated `TextEditorDecorationType`s. Inline SVG data URIs (no bundled assets) render red/amber/green dots in the gutter; matching overview-ruler ticks on the left lane; `MarkdownString` hover messages quote the level + reason. 150 ms debounce on document edits coalesces requests. Re-renders on active-editor change, text edit, save. Clears decorations on non-env files. |
| IntelliJ rendering | **Shipped.** `EnvForgeExposureEditorListener` registered as `editorFactoryListener` in plugin.xml. On editor open of a `.env*` file: shells out to `envforge exposure --file PATH` (subprocess), parses JSON, applies `RangeHighlighter` per env-var line via `MarkupModel.addLineHighlighter` + custom `GutterIconRenderer` that paints a colored circle using `JBColor` (theme-aware). Error-stripe marker color + tooltip mirror the gutter. Document-change listener debounces refresh at 250 ms. Why subprocess instead of lsp4ij custom request: keeps the Kotlin plugin free of lsp4ij private-API plumbing; the CLI reuses the same Rust classification function the LSP serves, so output is byte-identical. |
| CLI parity | `envforge exposure --file PATH` emits `ExposureMapResponse` JSON (same shape as the LSP response). Used by the IntelliJ plugin; also available for any other client that prefers subprocess calls over LSP custom requests. |
| Test IDs | `test_exposure_map_plaintext_classified_red`, `test_exposure_map_sensitive_classified_amber`, `test_exposure_map_fence_active_classifies_all_green`, `test_exposure_map_schema_sensitive_overrides_red`, `test_exposure_map_skips_comments_and_blanks`, `test_exposure_map_serializes_levels_as_lowercase`, `test_exposure_map_reports_line_numbers` |

### P7 — Canary tripwire indicator

| Field | Value |
|---|---|
| LSP method | `envforge/exposureMap` (extended) |
| Wire format | `ExposureEntry.canary: bool` (defaults to `false` for backward compatibility). When `true`, the entry's `reason` string is suffixed with `" Canary tripwire registered — an alert fires if this fake value appears in scanned tool output, logs, or files."` |
| Data source | `ops::canary::list_canaries()` queried once per exposure-map request and indexed into a HashSet for O(1) per-entry lookup. |
| Plugin rendering | **Shipped both IDEs.** Canary lines render a shield-shaped glyph in the gutter instead of the plain dot, keeping the red/amber/green threat color from the exposure tier. VS Code: inline shield SVG via separate `TextEditorDecorationType`. IntelliJ: custom `ShieldIcon` painted with `Graphics2D.Path2D` (no asset files). Hover banner switches to `"<LEVEL> · CANARY ACTIVE"` to make the tripwire status explicit. |
| Test ID | covered by exposure tests + canary-store integration on dev machines. |

### P8 — Plant-canary quick-fix

| Field | Value |
|---|---|
| LSP method | `textDocument/codeAction` |
| Trigger diagnostic | "Sensitive value for 'KEY'" (the existing secret-leak warning) |
| Action title | `Plant canary tripwire for <KEY>` |
| Action shape | Carries a `Command` (NOT a `WorkspaceEdit`) — the fake value is generated server-side via the `envforge.canary.plant` custom command so the canary payload never flows through plugin code paths where it could be logged. |
| Pattern hint | Inferred from key name: `*AWS*` → `aws_key`; `*TOKEN*` / `*API_KEY*` / `*APIKEY*` → `api_token`; else → `generic`. |
| Suppression | Action is omitted when the key already has a registered canary (`list_canaries` lookup). |
| Server command | `envforge.canary.plant` accepts `[{ "key": "<KEY>", "pattern": "<generic\|aws_key\|api_token>", "file": "<absolute path to .env*>" }]`. The `file` argument is optional; when provided, after `create_canary` mints the fake value, `place_canary_in_file(key, path, "bottom")` writes a `# envforge canary: KEY=VALUE` marker line into that file (preserves user content). Response: `{ key, fake_value, pattern, created_at, placed_in_file: <bool\|null>, file: "<path>"\|null }`. `placed_in_file: false` means the marker was already present (idempotent re-plant). |
| File URI threading | Both the P6 code-lens "Plant canary" action and the L6 plant-canary code-action now include the current document's file path in the command arguments. Plugins do nothing extra — the URI is derived server-side from the request context. |
| Related commands | `envforge.canary.list` — returns all registered canaries with `triggered` / `trigger_count` / `created_at`. Plugins can poll this to refresh visual state. |
| Test IDs | `test_canary_pattern_hint_via_plant_action`, `test_command_dispatch_canary_plant_rejects_missing_key`, `test_command_dispatch_canary_plant_rejects_empty_key`, `test_command_dispatch_canary_list_returns_array` |

### P3 — Volatile lease countdown (status bar)

| Field | Value |
|---|---|
| Data source | `ops::lease::list_leases()` (persistent — leases live in `~/.envforge/leases/*.toml`). Picks the soonest-expiring `status == "active"` lease. |
| LSP method | `envforge.volatile.status` custom command. Returns `null` when no active leases, otherwise `{ name, remaining_seconds, expires_at, key_count }`. |
| CLI parity | `envforge lease list --json` provides the same data. Used by both plugins via subprocess for status-bar refresh. |
| VS Code item | New `StatusBarItem` (priority 48, left lane). Text: `$(clock) volatile: <Hh Mm \| Mm Ss \| Ss>`. Tooltip names the lease, key count, remaining time. Hidden when no active leases. Background tints `warningBackground` at ≤5 min, `errorBackground` at ≤1 min. |
| VS Code refresh | Fast timer at 10 s (sub-minute precision). Slow timer (vars + fence) stays at 30 s — split to keep subprocess cost bounded. |
| IntelliJ widget | `EnvForgeStatusWidget` text now composes `<N> vars · AI BLOCKED \| AI ALLOWED · volatile: <duration>` segments. Lease parsing mirrors VS Code logic byte-for-byte; soonest-expiring active lease wins. Tooltip aggregates fence + lease state. |
| Click handlers | **VS Code:** click invokes `envforge.volatileExtend` → status probe → TTL QuickPick (5m/15m/30m/1h/2h/Custom) → `envforge.volatile.extend` LSP command → status bar refresh. **IntelliJ:** Tools > EnvForge > Extend Volatile Lease… → subprocess `envforge lease list --json` to pick soonest-expiring active → TTL chooser → `envforge lease renew NAME --ttl TTL --json`. Widget-click handler deferred. |
| Extend command | `envforge.volatile.extend` takes `{ name, ttl }`. TTL parsed via `session::parse_ttl` (accepts `30m`, `2h`, `1d` etc.). Calls `ops::lease::renew_lease(name, ttl_seconds)`. Returns `{ name, new_expires_at, ttl_seconds }` or structured error (`not found` / `invalid ttl` / `expired or revoked`). |
| Out of scope (first cut) | Visual countdown animation between polls. Deferred. |
| Test ID | `test_command_dispatch_volatile_status_returns_ok` (structural assertion — full lease lifecycle test requires sandboxed config dir, deferred). |

### C1 + C2 — Sync push/pull/status commands

| Field | Value |
|---|---|
| LSP methods | `envforge.sync.push`, `envforge.sync.pull`, `envforge.sync.status` (all via `workspace/executeCommand`) |
| Push args | `[{ "message": "<commit message>" }]` (optional). Empty / missing → server-side auto message. |
| Pull args | none |
| Status args | none |
| Implementation | Server re-executes itself via `std::env::current_exe()` and runs the matching CLI subcommand with `--json`. Stdout JSON forwarded verbatim on success. Subprocess isolation means a sync-op crash never takes down the LSP. |
| cwd | Workspace root from `initialize`. Sync subcommands fail with `"workspace root not available"` when absent. |
| Success shape | `{ "ok": true, "result": <JSON the CLI emitted> }`. The CLI's existing `--json` schemas pass through unchanged so any client that already understands the CLI output understands the LSP response. |
| Failure shape | `{ "ok": false, "error": "sync <action> failed", "detail": { "exit_code": <int>, "stdout": <JSON or string>, "stderr": "<string>" } }`. Lets clients render the same error UX whether the user invoked sync via terminal or via plugin. |
| Plugin parity | VS Code (`envforge.syncPush`, `envforge.syncPull`, `envforge.syncStatus`) and IntelliJ (`SyncPushAction`, `SyncPullAction`) already shell out to the CLI directly. The new LSP routes are additive — third-party LSP clients (Neovim, Emacs, and other LSP clients) can now drive sync without spawning subprocesses themselves. |
| Test IDs | `test_command_dispatch_sync_push_requires_workspace_root`, `test_command_dispatch_sync_pull_requires_workspace_root`, `test_command_dispatch_sync_status_requires_workspace_root`, `test_command_dispatch_sync_push_in_non_sync_dir_reports_error` |

### P6 — Actionable CodeLens on secret values

| Field | Value |
|---|---|
| LSP method | `textDocument/codeLens` |
| Capability | `code_lens_provider: { resolve_provider: false }` (already declared) |
| Per-line lenses on sensitive env-var lines | **Plant canary** (`$(bug) Plant canary`) — `envforge.canary.plant` command with `{ key, pattern }` (pattern hint mirrors L6 quick-fix). Suppressed when key already has a registered canary; replaced with non-clickable `$(bug) canary active` badge. **Activate fence** (`$(shield) Activate fence`) — `envforge.fence.enable` command, always offered (op is idempotent, cheaper than probing fence state per request). |
| Per-line lenses for schema-known keys | Decorative `type: <var_type>` and `required` (kept for parity with the prior experience; empty command strings means clients render the text but no click action). |
| Sensitivity rule | `schema.sensitive == true` OR `is_sensitive_key(key)` — same heuristic as exposure / hover / diagnostics. |
| Server wiring | `code_lenses(entries, schema, canary_keys)` — `canary_keys` is an optional `&HashSet<String>` so callers can supply the canary set without paying the disk hit when they don't care. The server populates it once per request via `list_canaries`. |
| VS Code render | Clickable inline links above the line. Click sends `workspace/executeCommand` for the named command + arguments. |
| IntelliJ render | Same via lsp4ij code-vision bridge. |
| Test IDs | `test_code_lens_sensitive_keys_emit_plant_and_fence`, `test_code_lens_plant_suppressed_when_canary_registered`, `test_code_lens_non_sensitive_emits_no_actions`, `test_code_lens_plant_pattern_hint_for_aws`, `test_code_lens_with_schema`, `test_code_lens_empty_entries`, `test_code_lens_only_comments` |

### P9 — File explorer decorations

| Field | Value |
|---|---|
| VS Code provider | `EnvFileDecorationProvider` registered via `window.registerFileDecorationProvider`. |
| Data source | `envforge exposure --file PATH` CLI subprocess (same data as P5 gutter heatmap → badge and gutter never disagree). |
| Filter | Only `.env*` files. Schema files (`.env.schema`, `.env.schema.toml`) excluded — they aren't secret stores. |
| Badge precedence | 1. `fence_active` → `🛡` green. 2. Any `red` entry → `!` red. 3. Any `amber` entry → `?` yellow. 4. All-green non-empty → `✓` green. 5. Empty / unreadable → no badge. |
| Tooltips | Each badge carries a one-line description shown on hover in the explorer / open-tabs UI. |
| Refresh triggers | `onDidSaveTextDocument` (per-file invalidate), `envforge.decorations.refreshAll` command (workspace-wide). The fence toggle command fires `refreshAll` so explorer badges flip on enable/disable. |
| Cache | Per-URI; populated lazily on first explorer query. `provideFileDecoration` returns immediately from cache and fans out a background subprocess on miss; emits `onDidChange` when the result lands. |
| IntelliJ provider | **Shipped.** `EnvForgeProjectViewDecorator` implements `ProjectViewNodeDecorator`. For `.env*` nodes it appends a colored badge (🛡 / ! / ? / ✓) via `PresentationData.addText` using `JBColor` so the glyph picks up the active JetBrains theme. Backed by the same `envforge exposure --file PATH --json` subprocess as VS Code → exact data parity. 30 s TTL cache keyed by path; stale entries serve while a background pooled-thread refresh runs; `ProjectView.updateFromRoot(true)` repaints when data lands. |

### C5 — Wrap command in volatile run

| Field | Value |
|---|---|
| LSP method | `workspace/executeCommand` → `envforge.run.volatile` |
| Args | `[{ "command": "<user shell command>", "ttl": "<duration, default '30m'>" }]` |
| Behavior | Server does NOT spawn the terminal. It returns the wrapper string the plugin should send to its own terminal API. LSP has no terminal concept; building the wrapper here just guarantees subprocess vs LSP callers wrap identically. |
| Response | `{ wrapper: "envforge run --volatile <ttl> -- <command>", ttl, original_command }` |
| VS Code wiring | **Shipped.** `envforge.runVolatile` command (command palette + `package.json` `contributes.commands`). Prompts for command (prefills with editor selection if present), TTL via QuickPick (5m/15m/30m/1h/2h/Custom), confirms, sends LSP request, opens a named `vscode.window.createTerminal` with the returned wrapper, `sendText(wrapper)`. |
| IntelliJ wiring | **Shipped.** `RunVolatileAction` in Tools > EnvForge menu. Two `Messages.showInputDialog` / `showEditableChooseDialog` prompts (command + TTL with presets 5m/15m/30m/1h/2h). Spawns via `EnvForgeRunner.run(["run", "--volatile", ttl, "--", command], …)`. Notification on completion. |
| Validation | Rejects missing or whitespace-only `command`. |
| Test IDs | `test_command_dispatch_run_volatile_builds_wrapper`, `test_command_dispatch_run_volatile_defaults_ttl_to_30m`, `test_command_dispatch_run_volatile_rejects_missing_command`, `test_command_dispatch_run_volatile_rejects_empty_command` |

### C6 — Reveal value with audit

| Field | Value |
|---|---|
| LSP method | `workspace/executeCommand` → `envforge.reveal.value` |
| Args | `[{ "key": "<KEY>", "reason": "<optional free-form string>" }]` |
| Behavior | Subprocesses `envforge get KEY --json`, captures `value` + `source_file`. Emits a `RuntimeEvent` (source: `Manual`, message: `"LSP reveal: KEY (reason)"` — message intentionally does NOT contain the value; the monitor bus also redacts high-entropy tokens as a defense in depth). Returns the value over the LSP wire. |
| Response | `{ key, value, source_file, revealed_at, reason }` |
| Audit log | Visible via `envforge monitor` event stream. Sec-ops can grep `LSP reveal:` to find all reveals. |
| Security note | The raw value crosses the LSP wire on purpose — the plugin needs it to display. Plugin clients SHOULD NOT log this response. The reveal action itself is audit-logged so any access is reviewable post-hoc. |
| VS Code wiring | **Shipped.** `envforge.revealValue` command. Accepts a tree-view item (uses `arg.envVar.key`) or prompts via input box. Asks for a free-form `reason` (audited). Modal confirm. Sends LSP request. Displays value via `showInformationMessage` (modal, not logged to output channel). `Copy to clipboard` button writes the value + auto-clears clipboard after 30 s if the clipboard contents still match. |
| IntelliJ wiring | **Shipped.** `RevealValueAction` in Tools > EnvForge menu. Prompts for key + reason, modal warning confirm, subprocesses `envforge get KEY --json`, surfaces value via `Messages.showYesNoDialog` with Copy/Close buttons. Copy → `CopyPasteManager.setContents(value)` + 30 s scheduled auto-clear that only fires if the clipboard still holds the revealed value. |
| Validation | Rejects missing or empty `key`. |
| Test IDs | `test_command_dispatch_reveal_value_rejects_missing_key`, `test_command_dispatch_reveal_value_rejects_empty_key` (full reveal flow not unit-tested — depends on user's shell config + real env state, exercised manually). |

### C4 — Canary scan + check commands

| Field | Value |
|---|---|
| LSP methods | `workspace/executeCommand` → `envforge.canary.scan` and `envforge.canary.check` |
| Scan args | `[{ "text": "<string>" }]` OR `[{ "file": "<absolute path>" }]`. One required. |
| Scan implementation | `text` → `scanner::scan_text`. `file` → opens via `std::fs::File::open` + `scanner::scan_reader` (line-by-line, suitable for very large logs). Matches v2 token regex `cnry_[A-Z2-7]{39}_[A-Z2-7]{13}`. |
| Scan response | `{ match_count: <usize>, matches: [{ token, byte_offset, line_number }] }` |
| Use case | Incident-response: paste a stack trace / log / leaked diff into the editor command palette, run `envforge.canary.scan { text }`, instantly learn whether any registered tripwire token is present. |
| Check args | none |
| Check response | `{ triggered_count: <usize>, triggered: [{ key, pattern, triggered, trigger_count, created_at }] }` — derived from `check_canaries()`. |
| VS Code wiring | **Shipped.** `envforge.canaryScan`: detects active-editor selection as input; otherwise QuickPick "Paste text…" / "Pick a file…". Pasted text → input box; file → `showOpenDialog`. Calls LSP. Empty result → info toast; matches → output channel with `line N: <token>` per hit + warning banner. `envforge.canaryCheck`: calls LSP. None triggered → info ("all quiet"); some triggered → output channel listing key/pattern/hit-count/created_at + error banner ("Review immediately"). |
| IntelliJ wiring | **Shipped both actions.** `CanaryCheckAction` subprocesses `envforge canary check --json` → `Messages.showWarningDialog` + IDE notification at appropriate severity. `CanaryScanAction`: editor-selection wins; otherwise `showChooseDialog` picks paste-text vs file-chooser. Pasted text writes to a tempfile (`createTempFile`, deleted in `finally`), file path goes directly. Subprocess `envforge canary scan --input PATH --json` → `Messages.showWarningDialog` with the match list + IDE notification. |
| Test IDs | `test_command_dispatch_canary_scan_text_finds_token`, `test_command_dispatch_canary_scan_text_no_match`, `test_command_dispatch_canary_scan_rejects_missing_args`, `test_command_dispatch_canary_scan_file_open_failure_propagates`, `test_command_dispatch_canary_check_returns_triggered_array` |

### G1 — Lifecycle dashboard

| Field | Value |
|---|---|
| LSP methods | `workspace/executeCommand` → `envforge.lifecycle.check` and `envforge.lifecycle.rule.list` |
| VS Code wiring | **Shipped.** Security view container includes "Lifecycle" category. Sub-items: "Run Lifecycle Check" (calls check), "Manage Rules" (calls rule list). Results displayed in a dedicated output channel. |
| IntelliJ wiring | **Shipped.** Security tool window tab includes "Lifecycle" node. Sub-nodes: "Run Lifecycle Check", "Manage Rules", "Audit Trail". Spawns CLI subprocesses via `EnvForgeRunner.run` and displays output in the IDE console. |
| Audit Trail | `envforge audit -n 100` command accessible from the Lifecycle node. |

### G2 — Analytics dashboard

| Field | Value |
|---|---|
| LSP methods | `workspace/executeCommand` → `envforge.analytics.unused` and `envforge.analytics.summary` |
| VS Code wiring | **Shipped.** Security view container includes "Analytics" category. Sub-items: "Show Unused Secrets", "Usage Summary", "Monitor Stream". |
| IntelliJ wiring | **Shipped.** Security tool window tab includes "Analytics" node. Sub-nodes: "Show Unused Secrets", "Usage Summary", "Monitor Stream". |
| Monitor Stream | Launches a persistent IDE terminal running `envforge monitor stream` for real-time visibility. |

### G3 — Profiles view parity

| Field | Value |
|---|---|
| VS Code wiring | **Shipped.** Dedicated "Profiles" view in the EnvForge activity bar container. Shows active/inactive profiles with their source files. Double-click to switch. |
| IntelliJ wiring | **Shipped.** Dedicated "Profiles" tab in the EnvForge tool window. Replaces the old Gear-menu profile switcher with a first-class tree view. Double-click a node to switch active profile. |

### E1 — Environment-aware `.env` intelligence (project manifest)

Driven by `.envforge.project.toml` (its `[[environments]]` list). All logic is
server-side and deterministic, so every client renders identically. See
`docs/envforge-project-toml.md` for the schema, recognition, and precedence.

| Field | Value |
|---|---|
| Recognition | `.envforge.project.toml` resolves to a concrete env-file set; the LSP recognizes those + the conventional `.env*` set. Absolute / `..`-escaping paths dropped. Reloaded live on manifest save; malformed manifest → `envforge-project` diagnostic + last-good fallback. |
| Key completion | Offers the project's known keys (union across declared environments) not already in the current file. `detail: "set in: <envs>"`. |
| Value completion | Offers a key's values from other environments via `text_edit.new_text`. **Sensitive keys never offer a raw cross-env value** — only a `(sensitive — set per environment)` marker. |
| Hover | Adds a "Set in environments" section listing which environments set the key (+ `(sensitive)` marker). **Never shows raw values** — the LSP is a read-only display boundary. |
| Missing-key diagnostic | `textDocument/publishDiagnostics`, source `envforge-env`, severity `Warning`, anchored at end-of-file. Fires for a key set in ≥1 other recognized environment but absent here. No false positives on non-manifest projects. |
| Go-to-definition | Env key → schema location **plus** the key's assignment in every other recognized env file (excludes the cursor's own occurrence). Returned as a `Location` array. |
| Sensitivity | A key is sensitive if the key-name heuristic flags it OR `.env.schema` marks it sensitive (union); applied uniformly across the key-set. |
| Client coverage | Profile variants attach via existing `.env.*` selectors on all four first-party clients. Non-`.env*` `extra_files` names: recognized server-side; client attach is a documented limitation (Growth). |
| Test IDs | `tests/project_resolve_tests.rs`, `tests/env_keyset_tests.rs`, `tests/completion_cross_env_tests.rs`, `tests/hover_cross_env_tests.rs`, `tests/cross_ide_conformance_tests.rs` |

### (future rows added here as each feature ships)
