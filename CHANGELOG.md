# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.5] - 2026-05-08

### Security

Comprehensive security hardening pass — 50 fixes across the entire codebase, organized below by severity. No external CVEs filed. Verification: `cargo build`, `cargo clippy --all-targets -- -D warnings`, full `cargo test` (1132 lib tests + integration suites) all green.

#### Critical

- **Git remote URL allowlist + RCE protection** (`src/ops/sync/git.rs`): `clone_repo` validates remote URLs via new `validate_remote_url` and rejects `ext::` (RCE via git-remote-ext), `file://` (local file disclosure), `rsync://`, `ftp(s)://`, `gopher://`, leading-dash, control-character URLs. Allowed: `https://`, `http://`, `ssh://`, `git://`, `git+{ssh,https}://`, scp-like `user@host:path`. Clone runs with `-c protocol.ext.allow=never` and an explicit `--` separator.
- **Hook prev-state file relocated out of project dir** (`src/ops/hook.rs`, `src/cli/{mod,commands}.rs`): `.envforge-prev` previously written into the project directory let any repo replace it with attacker-controlled shell that the bash/zsh/fish hook would `eval`. Now stored in user envforge config dir under `hook-state/<sha256(canonical_path)>.prev` (mode 0600); hooks call new `envforge env-unload --dir <dir>` subcommand instead of `eval "$(cat ...)"`. Output re-validated line-by-line before printing. Legacy in-project prev files auto-removed on first `envforge env`.
- **DNS-rebinding defense in proxy** (`src/ops/proxy.rs`): two layered defects — (a) `is_origin_allowed` accepted requests without an `Origin` header, (b) `format_response` always sent `Access-Control-Allow-Origin: *` — composed into a real exploit (DNS-rebound `evil.com` → 127.0.0.1 + simple GET → secret exfil). New `extract_host` + `is_host_loopback` helpers; the request handler now requires `Host` to match `127.0.0.1` / `localhost` / `[::1]` (with optional port) before serving any route. CORS header changed from `*` to `null` plus `Vary: Origin`. `extract_host` rejects requests with multiple Host headers (RFC 7230 §5.4).

#### High

- **Provider arg-flag injection** (`src/ops/secrets/providers/{onepassword,aws_ssm,gcp,azure,doppler,infisical,akeyless,sops,conjur,pass,vault,keeper,bitwarden}.rs`): every provider's `pull` / `push` / `get` / `list` calls `validate_provider_arg` for paths and `validate_secret_name` for keys. Where the underlying CLI supports it, positional args are placed after `--`. Provider-specific helpers (`checked_profile`, `checked_project`, `checked_vault`, `checked_project_config`, `checked_env_project`, `checked_project_id`) consolidate credential-field validation. `validate_secret_name` extended to reject leading `-`, `=`, control chars; new `validate_provider_arg` and `validate_provider_response_value` / `validate_provider_response_label` helpers (64 KiB / 512 B caps + NUL/control-char rejection) applied across all 14 providers' parsers.
- **`ENVFORGE_EXTERNAL_SCANNER` shell-meta rejection** (`src/ops/external_scanner.rs`): the deprecated env-var path used `split_whitespace`, which re-interpreted `/bin/sh -c "x"` as a shell-style argv. Now rejects values containing shell metacharacters and requires an absolute path. Plus `MAX_SCANNER_INPUT_BYTES = 4 MiB` cap on stdin payload sent to scanner subprocesses.
- **fsync before atomic rename** (`src/config/writer.rs`, `src/ops/sync/init.rs`): both atomic-write paths call `temp.as_file().sync_all()` before `persist`. Defends against torn writes on power loss leaving zero-length / corrupted files (encrypted secrets, sync snapshots).
- **Refuse to write secret-bearing files on non-unix** (`src/ops/encrypt.rs`, `src/ops/secrets/{cache,credentials}.rs`): the `#[cfg(not(unix))]` branches previously wrote age private keys / decrypted cache / credential store with default ACLs (world-readable on Windows). Now return runtime error.
- **Zeroize-on-drop for in-memory secrets** (`src/ops/secrets/{cache,credentials}.rs`): `CacheEntry` impls `Drop` that `zeroize::Zeroize`s its `value`. New `Credentials` wrapper struct (Drop+Deref) and `read_all_credentials_zeroizing()` for opt-in zero-on-scope-exit; existing `with_credentials` closure pattern preserved.
- **Audit log permissions + fsync on append** (`src/ops/audit/emitter.rs`): both append (default umask → 0644) and first-write (`atomic_write` no chmod) paths now set 0600 (defensive chmod on append-open if existing file has looser perms); append path calls `sync_all` after `flush` so power-loss between flush and writeback can no longer drop entries.
- **Profile name path traversal** (`src/ops/profile.rs`): `create_profile(name)` interpolated `name` into `~/.env_managed.{name}` and wrote there; same in `delete_profile` / `switch_profile`. New `validate_profile_name` enforces `[A-Za-z0-9_-]{1,64}` (no leading dash). New `ProfileError::InvalidName` variant.
- **Sync `decrypt_snapshot` size caps** (`src/ops/sync/encryption.rs`): `MAX_SNAPSHOT_CIPHERTEXT_BYTES = 8 MiB` checked before `Decryptor::new`; `Read::take(reader, MAX_SNAPSHOT_PLAINTEXT_BYTES)` on the decrypted reader. Defends against decompression-bomb age files in malicious sync remotes.

#### Medium

- **URI path traversal rejection** (`src/ops/uri_resolve.rs`): new `validate_uri_path` blocks `..` / `.` segments, leading `/`, `//`, control characters before dispatch to provider backends.
- **Provider response value validation** (`src/ops/secrets/{provider,providers/{onepassword,keeper,bitwarden,aws_ssm,gcp,azure,doppler,infisical,akeyless,sops,vault,conjur,pass}}.rs`): every JSON parser caps secret values at 64 KiB and rejects NUL / control chars; labels capped at 512 B.
- **Canary alerts redact own value** (`src/ops/canary.rs`): `trigger_canary` redacts the canary's `fake_value` from `source` / `details` before writing to `canary-alerts.jsonl`, monitor stream, and stderr — preventing the canary log from leaking the canary value it was meant to detect.
- **Sync `verify_signatures` config flag** (`src/ops/sync/{model,git,pull}.rs`): new `[sync] verify_signatures` (default `false`); when set, `envforge sync pull` runs `git verify-commit HEAD` after fetch and fails closed.
- **Sanitize length filter dropped** (`src/ops/sanitize.rs`): the `>= 4` cutoff allowed 3-character tokens (PINs / OTPs) to pass through redaction unchanged. Filter now only skips empty values.
- **Cache provider name sanitization** (`src/ops/secrets/cache.rs`): `cache_file_path` now sanitizes both `provider` and `key` (alphabet `[A-Za-z0-9_-]`, len + NUL checks); same in `invalidate_provider_cache`. Closes a path traversal where `provider="../etc"` could land cache writes outside the cache dir. Plus process-wide `Mutex` (`OnceLock`) over read-miss → fetch → write critical section in `resolve_reference`.
- **Lease lock + renew** (`src/ops/lease.rs`): process-wide mutex serializes every lease op (create / revoke / revoke-all / check) closing the in-process TOCTOU. New `with_lease_check_locked(key, f)` keeps the lock held from access check through secret release atomically. New `renew_lease(name, ttl_seconds)` for atomic extend.
- **Quote-style serialize escapes embedded quotes** (`src/model/shell_file.rs`): `Double` escapes `\` and `"`; `Single` uses POSIX close-escape-reopen `'\''` pattern. Restores byte-for-byte round-trip invariant on adversarial values.
- **Scanner per-file size cap** (`src/ops/scanner.rs`): 10 MiB cap before `read_to_string`. Plus centralized `MAX_SHELL_FILE_BYTES = 10 MiB` in `parse_shell_file` itself with new `ParseError::FileTooLarge` variant — fixes `check.rs`, `doctor.rs`, `hook.rs`, `profile.rs`, and every other caller in one place.
- **Audit query bounded read + filter input cap** (`src/ops/audit/{query_engine,query_types}.rs`): `read_all_events` streams via `BufReader::lines` (no whole-file load); `MAX_EVENTS_LOADED = 250_000` cap with stderr notice on truncation. `MetadataKey(String)` filter capped at 256 B.
- **Age decryption ciphertext + plaintext caps** (`src/ops/encrypt.rs`): `MAX_CIPHERTEXT_BYTES = 1 MiB` checked before `Decryptor::new`; `Read::take(reader, MAX_PLAINTEXT_BYTES = 1 MiB)` on output.
- **Share metadata `created_by` documented unauthenticated; share TTL hard-blocks expired** (`src/ops/share.rs`): rustdoc on `ShareMeta.created_by` makes the unauthentic-hostname contract explicit. `receive_share` now returns `ShareError::Expired` (was warn-only); new `receive_share_with(data, allow_expired)` for opt-in override. 8 MiB caps on encrypted blob and decrypted plaintext.
- **AI-guard secret detection: anchored prefixes + entropy fallback** (`src/ops/audit/ai_guard_integration.rs`): prefix matches must be anchored at a non-token boundary AND followed by ≥16 token-alphabet bytes; pattern table extended (12 → 18 prefixes). Entropy fallback flags any 32+ char run of `[A-Za-z0-9_\-./]` containing upper + lower + digits as credential-like.
- **Lifecycle snapshot 0600 + fsync** (`src/ops/lifecycle/rollback.rs`): new `write_atomic_snapshot` sets 0600 on the tempfile before write, `sync_all`s before persist, refuses non-unix. Plus `MAX_SNAPSHOT_FILE_BYTES = 4 MiB` cap on snapshot reads in `restore_snapshot`.
- **Cron min-interval guard** (`src/ops/lifecycle/trigger_engine.rs`): `MIN_CRON_INTERVAL_SECS = 60`; rules whose two consecutive next events are < 60 s apart are rejected.
- **Lifecycle state log 0600 + fsync** (`src/ops/lifecycle/orchestrator.rs::apply_state_transition`): state-transition `.jsonl` files open with `OpenOptions::mode(0o600)` on Unix; `sync_all` after `writeln`; defensive post-write chmod for files inherited from older versions.
- **Rule TOML size cap** (`src/ops/lifecycle/rule_manager.rs`): `MAX_RULE_FILE_BYTES = 256 KiB` checked before `read_to_string` + `toml::from_str` in every reader.
- **`mcp_scan` size + recursion-depth caps** (`src/ops/mcp_scan.rs`): `MAX_MCP_CONFIG_BYTES = 1 MiB` size cap before parse; `walk_json` carries `depth` parameter and bails at `MAX_JSON_DEPTH = 64`.
- **Audit chain-state deletion detection** (`src/ops/audit/tamper.rs`): `load_chain_state` returns `TamperError::InvalidState` when the state file is missing but log files exist (was silently re-initializing).
- **Analytics events file mode at create time** (`src/ops/analytics/storage.rs`): `OpenOptions::mode(0o600)` at create instead of post-chmod with `.ok()`. Errors no longer dropped.
- **Snapshot file 0600** (`src/ops/snapshot.rs`): new `write_snapshot_secure` helper uses `OpenOptions::mode(0o600)` on Unix.
- **Changelog file 0600** (`src/ops/changelog.rs::log_change`): `OpenOptions::mode(0o600)` at create + post-write defensive chmod for stale 0644 files.
- **LSP root URI canonicalization + size caps** (`src/lsp/server.rs`): `load_schema_from_workspace` canonicalizes the client-supplied `rootUri`, requires resolved schema path inside the canonicalized root, refuses schemas larger than 1 MiB. `did_open` / `did_change` enforce `MAX_DOCUMENT_BYTES = 1 MiB` on client-supplied content.
- **LSP completion no longer suggests live secret values as labels** (`src/lsp/completion.rs`): `redact_value_for_label` returns a redacted preview for the `label`; real value flows via `insert_text`.
- **Glob matcher iterative DP** (`src/ops/sync/marking.rs`, `src/ops/secrets/modes.rs`): replaces a recursive backtracker that was exponential on adversarial inputs (`a*a*a*a*a*b` × `aaaa…`). Now `O(P × T)`.
- **Profile diff masks plaintext values** (`src/ops/profile_diff.rs`): `DiffEntry.value_a` / `value_b` go through `mask_diff_value` at construction; new `values_differ: bool` field; rustdoc makes the masking contract explicit.
- **Markdown report escaping + RFC 4180 CSV** (`src/ops/audit/report_generator.rs`): new `markdown_escape` (escapes `<`, `>`, `&`, backticks, pipes, `[`, `]`, folds newlines) applied to violation rendering; new `csv_field` helper wraps every CSV field in `"…"`, doubles internal quotes, folds `\r` / `\n`. All six CSV columns now go through it (was missing `secret_key` and improperly quoted).
- **`ensure_age_key` chmod via file handle (TOCTOU)** (`src/ops/encrypt.rs`): opens the file once with `O_NOFOLLOW`, runs `File::set_permissions` on the handle, reads the same fd. Closes a path-level race where a same-uid attacker could swap the path between metadata and chmod.

#### Low

- **Cache TOCTOU narrowed** (`src/ops/secrets/cache.rs::resolve_reference`): process-wide `Mutex` (`OnceLock`) over read-miss → fetch → write. Eliminates in-process double-fetch races; cross-process safety still rests on tempfile + atomic rename + 0o600.
- **Clipboard auto-clear TTL** (`src/ops/clipboard.rs`): new `copy_to_clipboard_with_ttl(text, secs)` spawns a background thread that, after the TTL expires, clears the clipboard if-and-only-if it still holds the value we wrote. `copy_value` defaults to 30 s TTL when the key matches `is_sensitive_key`. Best-effort: macOS Pasteboard history and X11 PRIMARY/SECONDARY may still retain the value.
- **Profile / project_id validation** (`src/ops/secrets/providers/{keeper,bitwarden}.rs`): `checked_profile` / `checked_project_id` helpers run `validate_provider_arg` over Keeper's `profile` and Bitwarden's `project_id` before they are passed positionally to `ksm` / `bws`.
- **Env-var pair validation at run-CLI boundary** (`src/ops/secrets/provider.rs`, providers/`{vault,conjur,pass}.rs`): new `validate_env_pair(name, value)` enforces POSIX env name regex (`[A-Za-z_][A-Za-z0-9_]*`), rejects NUL bytes in name / value, rejects `\n` / `\r` in value. Called from `run_cli`, `run_cli_with_stdin` (covers `run_cli_with_tempfile` transitively), plus the three direct-spawn paths in pass / vault / conjur.
- **Monitor event message redaction** (`src/ops/monitor/mod.rs`): `emit_event` passes every `RuntimeEvent` through `redact_runtime_event`, which replaces high-entropy tokens (24+ chars of `[A-Za-z0-9_\-]`) in `message` with `[REDACTED]`. Last-line safety net for callers that put a secret value into a runtime event message.
- **TUI input length cap** (`src/ui/input.rs`): `MAX_INPUT_LEN = 128 KiB`; `insert` refuses past the cap, `new` truncates oversized initial text at a UTF-8 char boundary. Defends against bracketed-paste OOM.
- **Multi-Host header rejection** (`src/ops/proxy.rs::extract_host`): RFC 7230 §5.4 — returns `None` when more than one `host:` header present; handler treats `None` as fail → 403.
- **`ResolvedEntry` masked Debug** (`src/ops/uri_resolve.rs`): manual `Debug` impl renders `value` as `***(<n> chars)`. Closes leak via `format!("{:?}", entry)` / panic / `dbg!()`.
- **Backup file 0600** (`src/config/backup.rs`): post-`fs::copy` chmod 0600 on Unix; errors propagated.
- **`secret-sources.toml` 0600** (`src/ops/secrets/age.rs::save_sources`): `OpenOptions::mode(0o600)` at create + defensive chmod for stale files.
- **`secrets config --show` value preview no longer reveals credential prefix** (`src/cli/secrets_cmd.rs`): both JSON and human modes now show `***(<n> chars)` instead of leaking the first 4 chars (which exposed `AKIA…`/`sk-…`/`ghp_…`/`xoxb-…` credential-type fingerprints).

### False positives ruled out (after seven independent rescans)

- **`ai_hooks.rs` shell injection** — `"$TOOL_INPUT"` is double-quoted in the hook command string; shell `$`-expansion inside double quotes does not re-interpret metacharacters in the expanded value.
- **`ai_guard.rs::is_sensitive_path` Unicode bypass** — heuristic prepass; actual fence / permission decisions go through exact-byte path matches.
- **CORS / Origin / IDN bypasses on the proxy** — `to_ascii_lowercase` covers ASCII case folding; non-ASCII homoglyphs don't match `https://` and fall through to scp-like check requiring `@`. URL `?` / `#` smuggling — git does not interpret query strings as CLI flags.
- **`run.rs` argv leak** — secrets flow via `cmd.envs(env)` after `env_clear()`, NOT argv. `/proc/PID/cmdline` does not see them.
- **`vault.rs` role_id/secret_id payload "leak"** — written to `vault`'s **stdin** (deliberately avoiding `/proc/PID/cmdline`), not argv. The secure path.
- **`copy_key_value` clipboard exposure** / **`export_format::export_docker_secrets`** — both by design (user explicitly invokes); mitigated by clipboard TTL auto-clear.
- **Custody chain forgery** — addressed by `AuditEvent.entry_hash`/`prev_hash` chain plus chain-state deletion detection.
- **Parser `&s[1..]` panic in `parse_quoted_value`** — only entered when `trimmed.starts_with('"' | '\'')`; never panics.
- **C2 BOM-prefixed prev-state bypass** — envforge writes the file; we never emit BOM. BOM-prefixed lines from external tampering fail `is_safe_unload_line`.
- **`validate_env_pair` not applied to credentials** — every credential map flows `build_provider_env` → `env_refs_from_env` → `run_cli` / `run_cli_with_stdin`, all of which call `validate_env_pair`.
- **Volatile mode heap fragmentation** — `modes.rs` uses `zeroize` correctly; secrets cleared before scope exit.
- **`copypasta` unmaintained** — verified currently maintained (0.10.2); no open advisories.
- **`secrets/age.rs`** — usage-tracking module (records access events), not a decryption module. No bespoke crypto surface.
- **`rotate_secret` missing snapshot** — `rotate.rs::apply_rotation` performs its own atomic-write of the new value; not a security gap.
- **Machine override no auth** — `sync/machine.rs::set_override` operates on the local user's sync repo. Sync is consent-based.
- **`AuditEvent.metadata` non-deterministic hash** — `serde_json::Value` with default serde_json (no `preserve_order` feature) is alphabetically ordered → already deterministic.
- **`CustodyLink` missing hash field** — `event_id` references the `AuditEvent` whose `entry_hash` / `prev_hash` form the integrity chain.
- **CLI `--token` / `--password` / `--api-key` flags exist** — verified across multiple sweeps: no such flags. Sensitive values flow via stdin / prompt / file.
- **TUI edit dialog renders plaintext** — by design (the user asked to edit).
- **Proxy approval prompt no stdin timeout** — by design (interactive synchronous approval).
- **`model/error.rs::IoError` includes absolute path** — standard Rust error reporting; user files live under `$HOME` and the user is the local viewer.

### Tests

- 1132 lib tests pass, all integration suites green.
- New tests for `validate_remote_url`, `extract_host`, `is_host_loopback`, `csv_field`, glob-DP edge cases, sanitize length filter.

### Changed

- **Cargo.toml**: bumped to 0.7.5.
- **`SyncSettings`**: new `verify_signatures: bool` field (default `false`).
- **`ProfileError`**: new `InvalidName` variant.
- **`ParseError`**: new `FileTooLarge` variant.
- New public helpers in `src/ops/secrets/provider.rs`: `validate_provider_response_value`, `validate_provider_response_label`, `validate_provider_arg`, `validate_env_pair`, `MAX_PROVIDER_VALUE_LEN`, `MAX_PROVIDER_LABEL_LEN`.
- New public helpers in `src/ops/audit/report_generator.rs`: `markdown_escape`, `csv_field`.
- New constants: `parse::MAX_SHELL_FILE_BYTES`, `LspBackend::MAX_DOCUMENT_BYTES`, `MAX_RULE_FILE_BYTES`, `MAX_EVENTS_LOADED`, `MAX_CIPHERTEXT_BYTES`, `MAX_PLAINTEXT_BYTES`, `MAX_SNAPSHOT_CIPHERTEXT_BYTES`, `MAX_SNAPSHOT_PLAINTEXT_BYTES`, `MIN_CRON_INTERVAL_SECS`.
- New lease APIs: `renew_lease`, `with_lease_check_locked`.
- New share API: `receive_share_with(data, allow_expired)`. `ShareError::Expired` now returned (was warn-only).
- New proxy helpers: `extract_host`, `is_host_loopback`. CORS header `*` → `null` + `Vary: Origin`.
- `ResolvedEntry` uses manual masked `Debug` (was `derive(Debug)`).
- `walk_json` in `mcp_scan.rs` gained required `depth: usize` parameter (private function; no public-API impact).
- `read_all_credentials` doc-commented to prefer `with_credentials` or `read_all_credentials_zeroizing`.

## [0.7.4] - 2026-05-07

### Added — Secret Lifecycle Automation

#### Lifecycle Engine (`envforge lifecycle`)
- **New module**: `src/ops/lifecycle/` — automated secret lifecycle with 6 submodules:
  - **Orchestrator** (`orchestrator.rs`): Create/rotate/decommission workflows with result types (CreateResult, RotateResult, DecommissionResult)
  - **Rule Manager** (`rule_manager.rs`): CRUD for `LifecycleRule` entries, enable/disable toggles, serialization to TOML
  - **Trigger Engine** (`trigger_engine.rs`): Evaluate `LifecycleTrigger` variants — Cron (minute/hour precision), AgeExceeded, FileChange, PolicyViolation, and Composite (All/Any/Not operators) with `EvaluationContext`
  - **State Machine** (`state_machine.rs`): 7-state lifecycle (Creating → Active → Rotating → PendingDeprecation → Deprecated → Decommissioned → Failed) with `StateTransition` history
  - **Rollback** (`rollback.rs`): Snapshot-based rollback with `SnapshotMeta`/`Snapshot` tracking, UUID-keyed storage
  - **Schema Lifecycle** (`schema_lifecycle.rs`): Auto-generate lifecycle rules from `.env.schema` TTL/auto_rotate fields
- **Model types**: `LifecycleRule`, `LifecycleTrigger`, `LifecycleAction`, `RotationStrategy`, `LifecycleState`, `SecretLifecycle`, `TriggerEvent`, `EvaluationContext`, `SecretTemplate`, `RotationPolicy`, `LifecycleOperation`, `Snapshot`, `StateEvent`, `DecommissionPlan`, `RollbackResult`, `RecoveryResult`
- **CLI**: `envforge lifecycle check` — evaluate all rules; `envforge lifecycle rule list|rotate-secret` — manage rules; `envforge lifecycle state <KEY>` — show lifecycle state; `envforge lifecycle snapshot list|delete` — manage snapshots
- **Config**: `[lifecycle]` section in `.envforge.project.toml` — `default_stale_threshold_days`, `default_grace_period_days`, `default_rotation_strategy`, `snapshot_retention_days`

### Added — Secret Usage Analytics

#### Analytics Engine (`envforge analytics`)
- **New module**: `src/ops/analytics.rs` — track and analyze secret usage patterns:
  - **Unused Detection** (`unused`): Detect dormant secrets with no access in N days, confidence scoring
  - **Low Usage** (`unused`): Flag secrets with access count below threshold in time window
  - **Deprecation** (`unused`): Generate phased deprecation timelines (review → deprecate → remove) with dependent count
  - **Aggregation** (`aggregation`): Daily bucketing by key+date, accessor counts, type breakdowns
  - **Event Storage** (`storage`): JSONL log in `~/.local/share/envforge/analytics/`, enrichment pipeline
  - **Collection** (`collector`): Hook into proxy/CLI/sync access, auto-enrich with provider/environment/risk level
- **Model types**: `AnalyticsConfig`, `RawAccessEvent`, `EnrichedAccessEvent`, `AccessorInfo`, `AccessType`, `AccessSource`, `RiskLevel`, `AnalyticsError`
- **CLI**: `envforge analytics unused [--threshold N]` — dormant secrets; `envforge analytics low-usage [--max-accesses N] [--days N]` — low activity; `envforge analytics deprecation` — deprecation timelines; `envforge analytics summary [--days N]` — event/secret counts; `envforge analytics recompute` — recalculate aggregates; `envforge analytics retention show|set --days N` — retention policy; `envforge analytics prune [--before DATE]` — remove old events
- **Config**: `[analytics]` section — `enabled`, `retention_days`, `max_events`, `auto_aggregate`, `store_path`

### Added — Real-Time Monitoring

#### Health Monitor (`envforge monitor`)
- **New module**: `src/ops/monitor/` — real-time infrastructure health probes:
  - **Health checks** (`health.rs`): Provider availability (registry count, binary reachability), canary integrity, encryption key accessibility, fence status — all non-blocking with latency tracking
  - **Event fingerprinting** (`fingerprint.rs`): Unique event identity for deduplication and audit
- **CLI**: `envforge monitor status` — run all health probes (text/JSON output); `envforge monitor stream` — live event stream (JSON Lines to stdout)

### Added — Documentation

- **API Reference** (`docs/api-reference.md`): Full library reference covering `envforge::parser` (5 pub fns), `envforge::model` (LineNode, ShellFile, Shell, session/lifecycle/analytics types), `envforge::config` (AppConfig, backup, atomic_write), `envforge::ops` (OpError + re-exports), `envforge::lsp` (start_lsp_server). Includes 5 How-to code recipes.
- **CLI Reference** (`docs/cli-reference.md`): Updated to v0.7.4. Added Quick Recipes table (14 workflows), lifecycle (6 subcommands), analytics (8 subcommands), monitor (2 subcommands) sections.
- **README**: Architecture tree updated to 50+ ops modules, feature counts updated (25 AI safety tools, 90+ commands), new Lifecycle/Analytics/Monitoring feature rows.

### Changed

- **Cargo.toml**: bumped version to 0.7.4
- **CLI**: Added `--k8s-name`, `--k8s-namespace` flags to `export` for Kubernetes Secret name/namespace customization

### Fixed

- **Test quality**: Removed dead code (`make_context_with_last`), fixed redundant clones, resolved `field_reassign_with_default` clippy lints, fixed `needless_collect` patterns across 5 test files
- **Cron trigger test**: Fixed spurious `>= 0` assertion (always true) in composite trigger test — replaced with no-panic verification

## [0.7.2] - 2026-05-06

### Added — AI Safety Hardening Suite

#### Adversarial Input Hardening (`envforge hardening`)
- **New module**: `src/ops/hardening.rs` — 4-layer adversarial input detection pipeline
  - **Control character strip**: Removes null bytes, BIDI override marks, zero-width characters, normalizes Cyrillic homoglyphs (`а`→`a`, `е`→`e`, `о`→`o`)
  - **Base64 decode**: Finds and decodes Base64/URL-safe Base64 substrings >20 chars, re-scans decoded content for secrets
  - **Split string detection**: Detects `"sec" + "ret"` concatenation, `${VAR}` template expansion, `['a','b'].join('')` array joins
  - **Encoding chain decode**: Recursive decode up to depth 3 (Base64 → hex → Base64), re-scans at each level
- **Config**: `[ai_guard.hardening]` section in `.envforge.project.toml` with per-layer toggles
  - `control_chars`, `base64_decode` (min_length), `split_strings`, `encoding_chain` (max_depth)
- **CLI**: `envforge hardening show` — display current config; `envforge hardening enable/disable <layer>` — toggle layers
- **AI Guard integration**: `run_guard()` PreTool/PostTool stages now scan hardened (derived) strings alongside original input
  - Warnings include source layer: `"Secret value detected in decoded input (key: API_KEY)"`

#### External Scanner Interface (`envforge scanner`)
- **New module**: `src/ops/external_scanner.rs` — first-class multi-scanner pipeline
  - Replaces legacy `ENVFORGE_EXTERNAL_SCANNER` env var (deprecated but still supported with warning)
  - `[scanners.NAME]` section in `.envforge.project.toml` with `command`, `args`, `timeout_ms`, `enabled`
  - Concurrent execution via Tokio — all enabled scanners run in parallel
  - Per-scanner timeout (default 5s) — never blocks tool execution
  - Exit code 0 = clean, non-zero = stdout/stderr lines become findings
- **CLI**: `envforge scanner list` — show configured scanners; `envforge scanner test <name>` — test with sample content; `envforge scanner run <name> <content>` — ad-hoc scan; `envforge scanner enable/disable <name>` — toggle
- **AI Guard integration**: Scanner pipeline runs automatically in PreTool and PostTool stages when scanners configured

#### Canary Coverage Expansion (`envforge canary`)
- **6 new pattern types** (12 total):
  - `database_url` — `postgres://canary_user:...@canary-host:5432/canary_db`
  - `jwt_token` — Valid 3-part JWT structure with fake claims
  - `openai_key` — `sk-canary-...` format
  - `private_key_pem` — Plausible PEM block with BEGIN/END markers
  - `smtp_credential` — `smtp://canary_user:...@smtp.canary.local:587`
  - `ftp_credential` — `ftp://canary_user:...@ftp.canary.local:21`
- **Auto-rotation**: `rotate_after_days` field (default 14) per canary
  - `envforge canary rotate --all` — rotate all eligible canaries
  - `envforge canary rotate --key <KEY>` — rotate specific canary
  - `--dry-run` shows what would be rotated without changes
  - Rotation resets `triggered` and `trigger_count` state
- **Placement**: `envforge canary place <KEY> <FILE> [--position top|middle|bottom|random]`
  - Injects `# envforge canary: KEY=VALUE` line at specified position
  - Detects and skips duplicates

#### AI Tool Session Management (`envforge session`)
- **New module**: `src/ops/session.rs` — lightweight per-AI-tool session scoping
  - `SessionManager` — thread-safe in-memory store (`Mutex<HashMap>`) for active sessions
  - `create_session(tool, ttl)` — start a session with auto-generated UUID and expiry
  - `stop_session(id)` — expire a session (uses `ENVFORGE_SESSION_ID` env var as fallback)
  - `list_sessions()` — show all sessions with remaining TTL
  - `cleanup_expired()` — remove stale sessions
  - `detect_ai_tool()` — auto-detects tool from env vars (`CLAUDE_CODE`, `CURSOR`, `GITHUB_COPILOT`)
- **New model**: `src/model/session.rs` — session data types
  - `SessionId`, `SessionState` (Active/Expired), `AiTool` (ClaudeCode, Copilot, Cursor, Unknown)
  - `SessionConfig` with configurable default TTL (default: 1h)
- **CLI**: `envforge session start` — start session; `envforge session stop [id]` — stop session; `envforge session list` — list sessions; `envforge session show <id>` — session details; `envforge session cleanup` — remove expired
- **5 unit tests**: session lifecycle, TTL parsing, cleanup, tool detection, duration formatting

### Deprecated

- **Intent 032 (Prompt Injection Detection)**: Deprecated in specsmd memory bank. Advisory-only prompt injection detection adds noise without security value. Replaced by the three features above which stay on envforge's core competency (secret/env-var protection).
- **Intent 034 (AI Context Isolation)**: Deprecated in specsmd memory bank. Full namespace isolation + inheritance provided marginal incremental value over existing Fence + Guard + Volatile + Canary stack. Replaced by lightweight session management which achieves ~70% of the security benefit with ~20% of the code. Removed `src/model/context_isolation.rs` (557 lines).

### Changed

- **AI Guard `run_guard()` signature**: Added `hardening` and `scanner_findings` parameters for integration with new safety layers
- **Project config schema**: Added `[ai_guard]` section with `hardening` and `scanners` sub-sections
- **Tokio features**: Added `process`, `time`, `io-util` for async scanner execution

### Fixed

- **Audit trail CLI routing**: Wired `AuditTrail` into `Commands` enum (`envforge audit-trail`). The 7 audit-trail subcommands (query, report, custody, integrity, stats, tail, retention) were fully implemented in `src/cli/audit_cmd.rs` but unreachable from the CLI. Added the missing enum variant and match arm.

### Dependencies

- Added `hex = "0.4"` for encoding chain decode layer

### Quality

- **805 total tests** (up from 610), all passing
  - 16 new hardening tests (control chars, Base64, split strings, encoding chains, composition)
  - 7 new external scanner tests (registry, concurrent execution, timeout, findings)
  - 6 new canary pattern tests (database_url, jwt, openai_key, pem, smtp, ftp)
  - 176 new audit trail tests (core-data, emitter, tamper, query-engine, custody, report-generator, ai-guard-integration)
  - 5 new session tests (lifecycle, TTL parsing, cleanup, tool detection, duration formatting)
- **cargo clippy**: 0 warnings
- **cargo fmt**: Clean
- No breaking changes — all existing AI Guard behavior preserved when new features disabled

#### AI Audit Trail (`envforge audit-trail`)
- **New module**: `src/ops/audit/` — comprehensive audit system with 8 submodules:
  - **Core Data** (`types.rs`, `query_types.rs`, `report_types.rs`): AuditEvent, Query, Filter, TimeRange, Pagination, ReportConfig, ComplianceScore, Aggregation types
  - **Event Emitter** (`emitter.rs`): JSONL log emitter with per-source separation, enrichment (hostname/PID/timestamp), atomic writes
  - **Tamper-Evident Writer** (`tamper.rs`): SHA-256 hash chain for all log entries, persistent chain state, integrity verification with ChainBreak detection
  - **Query Engine** (`query_engine.rs`): Execute queries with time/field filters, sort, pagination, aggregation (by EventType, Source, etc.)
  - **Chain of Custody** (`custody.rs`): Secret lineage tracking, session paths, ownership verification, custody gap detection
  - **Report Generator** (`report_generator.rs`): SOC2 compliance reports, violation detection (UnauthorizedAccess, CustodyGap, SecretExposure, AnomalousFrequency), compliance scoring, JSON/CSV/Markdown export
  - **AI Guard Integration** (`ai_guard_integration.rs`): Pre/post-tool audit events, secret binding/exposure logging, session lifecycle tracking, input secret detection
  - **CLI Commands** (`audit_cmd.rs`): `envforge audit-trail {query,report,custody,integrity,stats,tail,retention}` — 8 subcommands for full audit lifecycle
- **176 total audit tests** — all passing, covering all 8 submodules
- Full integration with existing `EventSource::AiGuard`, `EventType`, and emitter infrastructure
- No breaking changes — all existing audit (git sync) functionality preserved

## [0.7.0] - 2026-05-02

### Added — Editor CLI API Gaps (for Plugin Support v0.1.2)

- **New Subcommand: `search`**: Fuzzy search across all environment variables.
  - `envforge search <query>`: Interactive text output with scores.
  - `envforge search <query> --json`: Structured output with matched indices for IDE highlighting.
  - **Security**: Automatic value masking for sensitive keys (SECRET, TOKEN, PASSWORD, etc.).
- **Enhanced `move` (Rename)**: Support for in-place renaming of variables.
  - `envforge move OLD_KEY NEW_KEY`: Renames a key while preserving its value and position.
- **Enhanced `fence`**: Added status monitoring.
  - `envforge fence --status`: Check which AI ignore files exist and are correctly configured.
- **New Subcommand: `ai-hook status`**: Programmatic check for active security hooks.
  - `envforge ai-hook status [--json]`: Reports installation status for Claude Code and Cursor hooks.
- **New Subcommand: `mcp status`**: Security auditing for MCP configurations.
  - `envforge mcp status [--json]`: Scans Claude, Cursor, and Copilot configs for hardcoded plaintext secrets.
- **Comprehensive JSON Output**: Added `--json` support to 11 commands to support stable IDE integrations.
  - Now includes `ai-hook status`, `mcp status`, `canary list`, `profile list`, `snapshot list`, `sync status`, `audit`, `lease list`, `secrets providers`, `drift`, and `check`.
- **Standardized API**: All JSON outputs now include a `"version": 1` field for future-proof API stability.

### Changed

- **CLI Security**: `envforge list` and `envforge search` now automatically mask sensitive variable values (e.g., `ABC***XYZ`) to prevent plain-text exposure in terminal logs.
- **Internal**: derived `Serialize` on core models to support structured output across the codebase.
- **Testing**: Added 48 new integration tests covering JSON output schemas and new subcommand edge cases.

## [0.6.2] - 2026-04-27

### Added — Provider Security Hardening

- **Credential Exposure Prevention**: All 13 secret provider integrations now pass credentials via environment variables or stdin pipes instead of CLI flags, preventing credential leakage via `/proc/PID/cmdline` or `ps aux`.
  - `AkeylessProvider`: Migrated `access_id`/`access_key` from `--access-id`/`--access-key` CLI flags to `AKEYLESS_ACCESS_ID`/`AKEYLESS_ACCESS_KEY` environment variables.
  - `ConjurProvider`: Migrated `api_key` from `-p` CLI flag to stdin pipe, added `CONJUR_APPLIANCE_URL`, `CONJUR_ACCOUNT`, `CONJUR_AUTHN_LOGIN`, `CONJUR_AUTHN_API_KEY` environment variables.
  - `VaultProvider` (AppRole auth): Migrated `role_id`/`secret_id` from `vault write` positional args to stdin pipe.
- **Error Message Sanitization**: New `sanitize_error_output()` function redacts 11 credential patterns (tokens, API keys, passwords, etc.) from CLI error output before logging or display.
- **Input Validation**: New `validate_secret_name()` and `validate_secret_value()` functions reject null bytes, newlines, and oversized inputs in user-provided secret data.
- **CLI Version Verification**: New `minimum_version()` and `verify_version()` trait methods on all 13 providers, with minimum versions logged as warnings when CLI binaries are outdated.
- **CLI Binary Audit Workflow**: New `.github/workflows/cli-audit.yml` for weekly automated version checks of provider CLI binaries.

### Fixed — Encryption & File Permission Hardening

- **Age Plugin Feature Disabled**: `age` crate now compiled with `default-features = false` and only `armor` feature enabled, eliminating the RUSTSEC-2024-0433 arbitrary code execution attack vector.
- **Credential File Permissions**: `credentials.toml` now written with `0600` permissions using atomic writes (tempfile + rename). Permission check on load auto-fixes and warns if file is group/world-readable.
- **Age Key File Permissions**: `age.key` now written with `0600` permissions using atomic writes. Permission check on existing files auto-fixes and warns if overly permissive.
- **Secret Cache Permissions**: Cache files (containing decrypted secrets) now written with `0600` permissions instead of world-readable defaults.
- **SOPS Temp File**: Temp file for SOPS push operations now uses `NamedTempFile` with `0600` permissions instead of world-readable `/tmp` path.

### Changed — Security Documentation

- **SECURITY.md**: Complete rewrite with minimum CLI version table, credential passing method documentation, updated file permissions table, and CLI binary security model.
- **Provider Integrations Security Audit**: Comprehensive plan at `plans/provider-integrations-security-audit.md` with all 26 remediation items completed and verified.

## [0.6.1] - 2026-04-24

### Added — Automated Security Compliance
- **Security CI Workflows**: New GitHub Actions for automated security auditing and policy enforcement.
  - `cargo audit` scheduled daily checks for dependency vulnerabilities.
  - `cargo deny` enforcement of licenses, banned crates, and unmaintained packages.
  - `CodeQL` static application security testing (SAST) for automated code scanning.
- **Dependency Automation**: Configured Dependabot for weekly automated updates of Cargo dependencies and GitHub Actions.
- **Security Documentation**: Expanded `SECURITY.md` with details on automated scanning and security posture.

### Fixed — Dependency Vulnerabilities & Maintenance
- **Security Patches**:
  - Upgraded `rand` to `0.9.3` to fix unsoundness/Stacked Borrows violation (RUSTSEC-2026-0097).
  - Upgraded `age` to `0.11.3` to include the latest security and stability patches.
- **Maintenance & Soundness**:
  - Upgraded `ratatui` to `0.30.0`, moving to the latest modular architecture.
  - Fixed unsoundness in `lru` (RUSTSEC-2026-0002) and removed unmaintained `paste` (RUSTSEC-2024-0436) via the `ratatui` upgrade.
  - Migrated from deprecated and unsound `serde_yaml`/`serde_yml` to the maintained `serde_norway` fork.
  - Refreshed 23+ transitive dependencies to their latest compatible versions.

## [0.6.0] - 2026-04-23

### Added — Project-Scoped Configuration

18 new `envforge project` subcommands for managing project-level environment variables with multi-environment support.

#### Project Init & Wizard
- `envforge project init` — Create project config file (TOML/YAML/JSON format, user choice)
- `envforge project wizard` — 3-step guided setup: init → schema → key-value entry (resumable)
- Detects existing `.env` and `.env.schema` — prompts import or override
- Auto-adds `.env.*` patterns to `.gitignore`
- Config file: `.envforge.project.{toml,yaml,json}` (distinct from `.envforge.toml` shell hook)

#### Multi-Environment Support
- `envforge project env create <name>` — Create dev/staging/prod environments
- `envforge project env list` — List environments with active indicator
- `envforge project env switch <name>` — Switch active environment
- `envforge project env delete <name>` — Remove environment
- `envforge project env diff <a> <b>` — Compare two environments side-by-side
- Each environment maps to a separate `.env.<name>` file

#### Project Tools
- `envforge project validate` — Validate against project schema
- `envforge project scan` — Scan project for leaked secrets (`--staged`, `--mcp`)
- `envforge project schema generate` — Generate `.env.schema` from project `.env`
- `envforge project schema emit-ai` — AI-safe context (no values)
- `envforge project fence` — Create AI ignore rules for project
- `envforge project sanitize <file>` — Strip secrets using project env values
- `envforge project export` — Export project env (`--safe`, `--format json/yaml`)

#### Project Provider Integration
- `envforge project pull --from <provider>` — Pull secrets into project `.env`
- `envforge project push --to <provider>` — Push project `.env` to provider
- All 13 providers supported
- `envforge project status` — Project health overview
- `envforge project config` — View/edit project settings

#### Auto-Detection in `envforge run`
- `envforge run` auto-detects project config in cwd/parents
- Loads active environment's `.env` into env merge chain (after shell config, before `--env-file`)
- `--no-project` flag disables auto-detection
- Zero breaking changes to existing `envforge run` behavior

### Added — Provider Integration Documentation

Per-provider setup guides for all 13 secret manager providers, inline in CLI reference.

#### Provider Guides (in `docs/cli-reference.md`)
- Each provider: prerequisites, credential fields, path format, end-to-end workflow, auth details, troubleshooting
- Quick reference table with required fields and path behavior
- Providers: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault, Bitwarden, Akeyless, CyberArk Conjur, Mozilla SOPS, pass/gopass, Keeper

#### Documentation Site (`docs/docs.html`)
- Dynamic sidebar auto-generated from markdown headings (H2 sections, H3 sub-items)
- Collapsible sections with scroll-tracking active highlight
- H4 heading support for provider subsections
- Code block rendering fix (no more extra spacing from paragraph wrapping)
- Markdown link rendering (external links open in new tab)

### Changed

#### Test Suite
- 1325 total tests (up from 1191 in v0.5.7)
  - 59 new project-specific tests (46 core + 13 wizard)
  - 75 additional edge-case and coverage tests across modules
- All tests passing: 100% (0 failures)

#### Dependencies
- Added `serde_yaml = "0.9"` for YAML project config support

#### Documentation Updates
- `docs/cli-reference.md` — 4621 lines (up from ~2340), includes 18 project commands + 13 provider guides
- `README.md` — Added Projects to features table and Quick Start
- `PROVIDER_FRAMEWORK_GUIDE.md` — Updated test count (1325) and provider count (13)
- `action/action.yml` — Updated example version to 0.6.0
- Version bumped across all docs (v0.5.7 → v0.6.0)

#### Release Pipeline
- Multi-platform binary builds (x86_64 + aarch64, Linux + macOS)
- VSCode extension bundled with release
- IntelliJ plugin bundled with release
- Auto-extracted release notes from CHANGELOG

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **cargo clippy** — 0 warnings
- **cargo fmt** — Clean
- New dependency: `serde_yaml` only
- 26 files changed, +6800 lines across project, docs, and tests

## [0.5.7] - 2026-04-23

### Added — 6 New Secret Manager Providers

Total secret manager integrations expanded from 7 to **13 providers**.

#### Bitwarden Secrets Manager (`bitwarden`)
- CLI binary: `bws` (Bitwarden Secrets Manager CLI)
- Auth: Machine account access token (`BWS_ACCESS_TOKEN`)
- Features: Pull, push (create/update by key), list. Project-based organization.
- Install: `cargo install bws` or GitHub releases

#### Akeyless Vault (`akeyless`)
- CLI binary: `akeyless`
- Auth: Access ID + Access Key (profile or per-command)
- Features: Pull, push (create/update), get, list. Path-based hierarchy.
- Filters to STATIC_SECRET type only (ignores dynamic/rotated secrets)
- Install: Homebrew tap or binary download

#### CyberArk Conjur (`conjur`)
- CLI binary: `conjur` (Go-based CLI v8+)
- Auth: Account + URL + Login + API key (init + login flow)
- Features: Pull, push, get, list. Policy-based variable organization.
- Parses resource IDs (`account:variable:path/name`) automatically
- Install: `brew install cyberark/tools/conjur-cli`

#### Mozilla SOPS (`sops`)
- CLI binary: `sops`
- Auth: age key file (`SOPS_AGE_KEY_FILE`)
- Features: Pull (decrypt), push (decrypt-merge-encrypt cycle), get (extract), list
- **File-based paradigm**: `path` = encrypted file on disk, not remote URL
- Supports age, PGP, AWS KMS, GCP KMS, Azure KV encryption backends
- Install: `brew install sops`

#### pass/gopass (`pass`)
- CLI binary: `pass` or `gopass` (auto-detected, gopass preferred)
- Auth: GPG keyring (no explicit credentials needed)
- Features: Pull, push, get, list. Directory-based organization.
- Scans `~/.password-store/**/*.gpg` for secret enumeration
- Custom store path via `PASSWORD_STORE_DIR`
- Install: `brew install pass` / `brew install gopass`

#### Keeper Secrets Manager (`keeper`)
- CLI binary: `ksm`
- Auth: Device config (one-time token initialization via `ksm profile init`)
- Features: Pull, get, list. Push = update only (create not supported).
- Parses complex nested record JSON with typed field arrays
- Install: `pip3 install keeper-secrets-manager-cli`

### Changed

#### Test Suite Expansion
- 1191 total tests (up from 697 in v0.5.6)
  - 494 new tests including 110+ provider-specific tests for 6 new providers
  - Filesystem scanning tests for pass/gopass provider
  - Complex JSON parsing tests for Keeper nested records
  - Edge case coverage: missing fields, malformed entries, type fallback chains
- All tests passing: 100% (0 failures)

#### Provider Registry
- `create_default_registry()` now registers 13 providers
- Registry tests updated to verify all 13 providers

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **No API changes** — All existing 7 providers unchanged
- **No new dependencies** — All providers use existing CLI-wrapper pattern
- **cargo clippy** — 0 new warnings
- **cargo fmt** — Clean

## [0.5.6] - 2026-04-22

### Fixed — Critical Bug Fixes & Stability

#### UTF-8 Character Boundary Safety
- **Critical**: `scanner.rs::truncate_line()` now respects multi-byte UTF-8 character boundaries
  - Previously panicked when truncating error messages containing multi-byte characters (em-dash "—", accented letters, emoji, etc.)
  - Now iterates through `.chars()` to determine safe truncation points
  - Affects secret scanning output formatting across all UI contexts
  - Test `test_check_run_skips_missing_prerequisites` now passes (was panicking on em-dash)

#### Documentation Examples Correction
- Fixed crate-level documentation examples in `src/lib.rs`
  - Corrected API names: `parse_file` → `parse_shell_file`
  - Removed reference to non-existent `add_or_update()` function
  - Doc tests now compile and pass (2/2 examples verified)

### Changed

#### Test Suite Expansion
- 697 total tests (up from 664 in v0.5.3)
  - Library unit tests: 389
  - Integration tests: 306 (11 test suites)
  - Doc tests: 2 (new)
- All tests passing: 100% (0 failures)
- No new test dependencies

#### Code Quality
- 0 clippy warnings (maintained from v0.5.5)
- UTF-8 safety verified across all string handling paths
- Documentation accuracy: 100% verified against actual codebase

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **No API changes** — Drop-in replacement for v0.5.5
- **No new dependencies** — All changes internal to existing modules

## [0.5.4] - 2026-04-20

### Added — Advanced AI Safety

#### Canary Secrets (Honeypot Credentials)
- `envforge canary create KEY [--pattern aws_key|github_token|stripe_key|slack_token|gitlab_token|generic]` — Plant fake credentials
- `envforge canary list` — Show all canary secrets with trigger status
- `envforge canary check` — Check for triggered canaries (exfiltration detected)
- `envforge canary delete KEY` — Remove a canary
- Generates plausible-looking fake values per pattern type
- Integrated into AI guard: canary values in tool output trigger alerts
- JSONL alert log at `~/.config/envforge/canary-alerts.jsonl`

#### Zero-Access Approval Flow
- `envforge proxy --require-approval` — Human must approve each secret access request
- Agent request triggers terminal prompt: `🔒 Secret access request: KEY from 127.0.0.1`
- Human types `y` to approve, anything else denies
- Denied requests return 403 and are logged to audit trail
- Combinable with `--require-lease` for layered security

#### Secret Dependency Mapping
- `envforge deps KEY [--source]` — Find all references to an env var across project
- Scans: EnvForge managed files, .env files, config files (docker-compose, terraform, k8s, GitHub Actions)
- `--source` enables source code scanning (9 languages: JS, Python, Rust, Go, Java, Ruby, PHP, C, Shell)
- Skips `.git`, `node_modules`, `target`, `vendor`, `__pycache__`
- Grouped output by reference type with file:line context
- Answers: "If I rotate DB_PASSWORD, what breaks?"

#### External Scanner Hook
- Set `ENVFORGE_EXTERNAL_SCANNER="ggshield secret scan"` to delegate detection to external tools
- AI guard automatically calls external scanner on tool inputs/outputs
- Integrates with ggshield (500+ secret detectors) without competing
- Fallback: if external scanner not set, uses built-in pattern matching

#### Built-in Man Pages
- `envforge man` — Full command index grouped by category
- `envforge man COMMAND` — Detailed man page for any command (NAME, SYNOPSIS, DESCRIPTION, OPTIONS, EXAMPLES)
- 87 command entries parsed from embedded CLI reference
- Short name lookup: `man list` and `man "envforge list"` both work
- "Did you mean?" suggestions for typos

#### IDE Extensions & Language Server
- `envforge lsp` — Built-in LSP server (stdio transport) for IDE integration
- **VS Code extension** — [Marketplace](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager)
  - Diagnostics, hover, completions, go-to-definition via LSP
  - Variables panel with prefix grouping, sensitive value masking
  - Profiles panel with one-click switching
  - 13 commands: validate, scan, export, sync, profile switch/diff, schema generate
  - Copy Key Name / Copy Value via click and context menu
  - Status bar with variable count
  - Syntax highlighting for `.env` and `.env.schema` files
  - Search in API documentation (`/` or `Cmd+K`)
- **IntelliJ IDEA plugin** — [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/31385-envforge)
  - Same LSP features via LSP4IJ (diagnostics, hover, completions, go-to-definition)
  - Tool window with Variables + Profiles panels
  - Prefix grouping toggle, sensitive value masking
  - Right-click: Copy Key Name, Copy Value, Copy KEY=VALUE
  - 11 actions under Tools > EnvForge menu
  - Supports IntelliJ 2024.2 — 2025.2+
- Completions source from all envforge-managed vars (not hardcoded list)
- Value completions: bool → true/false, enum → allowed values, defaults, `${VAR}` references
- `K` key in TUI — copy key name to clipboard (new)
- `envforge copy KEY --key-only` — copy key name via CLI (new)

#### Documentation & Shell Setup
- `docs/cli-reference.md` — 2,266-line comprehensive CLI reference
- `docs/docs.html` — Styled documentation viewer with sidebar navigation
- Shell completions for zsh (Oh-My-Zsh compatible), bash, fish, Kiro CLI, Fig
- `envforge completions <shell> --install` — auto-install completions to correct system path
- Kiro CLI integration: auto-configures `devCompletionsFolder` + `developerMode`, strips TypeScript annotations for JS compatibility
- 3-page TUI help system (`?` key): Shortcuts, CLI Reference, About — navigate with Tab/1/2/3
- Landing page (`docs/index.html`) updated with install + completion setup instructions

### Quality
- 693 total tests (was 664), all passing
- 25 new tests (canary, deps, external scanner, man pages, TUI help)
- New modules: `canary.rs`, `deps.rs`, `man.rs`
- No new crate dependencies

## [0.5.3] - 2026-04-20

### Added — AI Safety Hardening

#### 3-Stage AI Guard Hooks
- `envforge ai-guard pre-tool TOOL INPUT` — Pre-tool scanning invoked by Claude Code/Cursor hooks
- **Sensitive file alerts**: Warns when AI agent accesses `.env`, `.pem`, `.ssh/`, `.aws/`, `credentials` files
- **Secret-in-command detection**: Catches known secret values in Bash command inputs
- **Post-tool output scanning**: Detects secrets leaked in tool output
- Safe file exclusions: `.env.schema`, `.env.example`, `.env.ai.md` don't trigger alerts
- Enhanced `envforge ai-hook install claude-code` now installs PreToolUse + PostToolUse hooks

#### Session Leases with Killswitch
- `envforge lease create --ttl 1h [--keys KEY1,KEY2] [--name SESSION]` — Time-bounded secret access
- `envforge lease list` — Show active leases with remaining time and key scope
- `envforge lease cleanup` — Remove expired/revoked leases
- `envforge revoke --all` — **KILLSWITCH**: Instantly revoke all active leases
- `envforge revoke --name SESSION` — Revoke specific lease
- Duration formats: `30m`, `1h`, `8h`, `24h`, `7d`
- Proxy integration: `envforge proxy --require-lease` enforces lease check per request

#### Proxy Domain Allowlist
- `envforge proxy --allow-origins api.stripe.com,api.openai.com` — Restrict which origins can call the proxy
- Default: localhost only (127.0.0.1, ::1)
- Denied origins logged to audit trail with 403 response

#### Secret Access Audit Log (JSONL)
- Every proxy request logged to `~/.config/envforge/access-audit.jsonl`
- Fields: timestamp, action, key accessed, client address, granted/denied
- **Values NEVER logged** — only key names and metadata
- `envforge audit --access` — View proxy access audit trail
- Append-only format for compliance

#### Sensitive File Access Alerts
- AI guard detects access to 14+ sensitive file patterns:
  `.env`, `.pem`, `.key`, `.p12`, `.ssh/`, `.aws/`, `.gnupg/`, `credentials`, `secret`, `token`, `id_rsa`, `id_ed25519`
- Integrated into PreToolUse hooks — alerts before AI agent reads sensitive files

### Quality
- 664 total tests (was 606), all passing
- 58 new tests across 5 features
- New modules: `ai_guard.rs`, `lease.rs`
- Enhanced: `proxy.rs` (audit + allowlist), `ai_hooks.rs` (3-stage)
- No new crate dependencies

## [0.5.2] - 2026-04-20

### Added — AI Safety Suite

#### MCP Config Hardening
- `envforge mcp harden` — Auto-rewrite MCP config files replacing plaintext secrets with `${VAR}` references
- Backs up originals as `.json.bak` before modifying
- `--dry-run` to preview changes without modifying files
- Covers Claude Desktop, Cursor, GitHub Copilot, project `.mcp.json`

#### Secret Fence
- `envforge fence` — Create AI tool ignore rules for all supported tools in one command
- Generates: `.envforgeignore`, `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json`
- Idempotent: running twice doesn't duplicate rules
- `--dry-run` to preview files that would be created

#### AI Context Auto-Update
- `.env.ai.md` automatically regenerated when `envforge set`, `envforge import`, or `envforge secrets pull` modifies variables
- Only triggers when `.env.ai.md` already exists in project directory
- Keeps AI context file in sync with actual env configuration

#### Prompt Sanitizer
- `envforge sanitize FILE [--output FILE]` — Replace all known secret values in any file with `${KEY}` placeholders
- Longest-match-first replacement to avoid partial substitutions
- Skips values shorter than 4 characters to avoid false positives
- Works on any file type: code, configs, logs, docs

#### AI Leak Report
- `envforge audit --ai-leaks` — Scan git history for secrets leaked in AI-assisted commits
- Detects commits co-authored by Claude, Copilot, Cursor, and other AI tools
- Scans diffs for API key patterns, connection strings, and high-entropy tokens
- Reports: commit hash, date, AI tool, file path, leaked patterns

#### AI Coding Tool Hooks
- `envforge ai-hook install claude-code` — Install EnvForge security hooks in Claude Code
- `envforge ai-hook install cursor` — Add security rules to Cursor
- `envforge ai-hook remove claude-code|cursor` — Remove hooks
- Claude Code: PostToolUse hook scanning for secrets after Write/Edit operations
- Cursor: Rules file with secret safety instructions

#### Agent Credential Proxy
- `envforge proxy [--port 8100] [--keys KEY1,KEY2] [--profile NAME]` — Local HTTP proxy for AI agent credential access
- Endpoints: `GET /env` (all vars), `GET /env/KEY` (single), `GET /health`
- `--keys` restricts which secrets are served (scoped access)
- JSON responses with CORS headers for browser-based agents
- Secrets served via HTTP API — never written to disk files

### Quality
- 606 total tests (was 547), all passing
- 59 new tests across 7 AI safety features
- New modules: `fence.rs`, `sanitize.rs`, `ai_hooks.rs`, `proxy.rs`
- No new crate dependencies

## [0.5.1] - 2026-04-20

### Added

#### AI-Safe Schema Emission
- `envforge schema emit-ai [--output FILE] [--infer]` — Generate AI-agent-safe context file
- Contains variable names, types, descriptions, sensitivity flags — **NO actual values**
- AI coding tools get full context without seeing secrets
- `--infer` auto-detects types from current env values when no `.env.schema` exists
- Addresses GitGuardian 2026 finding: AI-assisted commits leak secrets at 2x baseline rate

#### MCP Configuration Scanning
- `envforge scan --mcp` — Scan AI tool config files for hardcoded credentials
- Scans: Claude Desktop, Cursor, GitHub Copilot, project `.mcp.json`
- Detects 23+ API key patterns (OpenAI, Stripe, AWS, GitHub, Slack, etc.)
- Detects connection strings with embedded passwords
- Shows masked values with fix suggestions: `→ Replace with: ${API_KEY}`
- `--json` output for CI integration

#### Docker Compose Secrets Export
- `envforge export --format docker-secrets` — Generate Docker `/run/secrets/` file structure
- Outputs shell script creating individual secret files + docker-compose.yml snippet
- Ready for Docker Compose `secrets:` mount configuration

#### Runtime Log Redaction
- `envforge run --redact` — Pipe subprocess stdout/stderr through secret redaction filter
- Automatically masks known sensitive values as `[REDACTED:KEY_NAME]`
- Runs in parallel threads for stdout and stderr (no performance penalty)
- Skips short values (<4 chars) to avoid false positives
- Combinable with `--volatile`, `--resolve`, `--profiles`

#### URI-Based Secret References
- `envforge resolve-uri FILE` — Resolve provider URIs in config files to actual values
- Supported URI schemes: `vault://`, `aws-ssm://`, `1password://`, `doppler://`, `infisical://`, `gcp://`, `azure://`
- Regular URLs (`https://`, `postgres://`) are NOT treated as secret URIs
- `--env` flag outputs `.env` format; default outputs shell `export` statements
- `--output FILE` writes resolved output to file
- Error per-URI without blocking other resolutions

### Quality
- 547 total tests (was 485), all passing
- 62 new tests across 5 features (21 MCP scan, 29 URI resolve, 3 AI schema, 4 docker-secrets, 5 redaction)
- New modules: `mcp_scan.rs`, `uri_resolve.rs`
- No new crate dependencies

## [0.5.0] - 2026-04-20

### Added

#### Pre-Commit Hook Integration
- `envforge scan --install-hook` — Auto-install git pre-commit hook running `envforge scan --staged`
- Appends to existing pre-commit hooks (doesn't overwrite)
- `envforge scan --remove-hook` — Clean removal of EnvForge hook lines
- Hook blocks commits when secrets detected (non-zero exit)

#### Shell Auto-Load Hook
- `eval "$(envforge hook zsh)"` — direnv-style auto-load on directory change
- Supports zsh (chpwd), bash (PROMPT_COMMAND), and fish (--on-variable PWD)
- Auto-detects `.envforge.toml` or `.env.schema` in directory
- Auto-unloads when leaving directory (restores previous env)
- `envforge env [--dir PATH]` — Output shell export statements for eval
- `.envforge.toml` project config with `profile` field

#### Volatile Mode (AI Agent Safety)
- `envforge run --volatile` — Secrets resolved in memory only, no disk I/O for secret values
- Forces `--resolve` mode automatically
- Ignores `--env-file` flags (no .env disk reads in volatile mode)
- `--dry-run` masks sensitive values as `****`
- Protects against AI agent file scanning (Claude Code, Cursor, Copilot)

#### Secure Secret Sharing
- `envforge share create --recipient <AGE_PUBKEY> [--keys|--all|--filter]` — Create encrypted share file
- `envforge share receive FILE [--import]` — Decrypt and import shared secrets
- Encrypted with recipient's age public key (not sender's)
- `--expire HOURS` — Soft expiry metadata (warns on receive after expiry)
- Self-contained `.age` file format with sender metadata

#### JSON Schema for `.env.schema`
- `envforge schema json-schema` — Output JSON Schema (Draft 2020-12) for `.env.schema` format
- Covers all fields: type, required, default, description, example, sensitive, pattern, values, min, max
- Environment override sub-objects supported
- Enables VSCode/JetBrains autocomplete and validation

#### Rotate with Propagation
- `envforge rotate KEY --propagate` — Auto-push to provider AND sync after local rotation
- No interactive prompts in propagate mode
- Graceful failure: provider/sync errors don't roll back local change
- Summary: `Rotated: local ✓, vault ✓, sync ✓`
- Works with `--stale --propagate` for bulk rotation

#### Git Author Audit Trail
- `envforge audit` — Per-variable change history with author attribution from sync git history
- `--key NAME` — Filter by variable name
- `--since DATE` — Filter changes after date
- `--machine ID` — Filter by machine
- `--json` — Machine-readable output
- Shows action type: added, modified, removed

#### Token TTL (Credential Expiry)
- `envforge secrets config PROVIDER --set key=value --ttl 8h` — Set credential expiry
- Duration formats: `8h`, `24h`, `7d`, `30d`
- Expired credentials rejected with clear message and renewal hint
- `envforge secrets status` shows TTL remaining per provider
- `envforge doctor` warns about credentials expiring within 24h
- TTL metadata stored in `_meta` sections of `credentials.toml`

#### Offline Fallback & Cache Management
- `envforge run --resolve` now falls back to stale cached values when provider unreachable
- Warning on stderr: "Using cached value for KEY (provider unreachable)"
- `envforge secrets cache list` — Show all cached secrets with provider, age, fresh/expired status
- `envforge secrets cache clear [--provider NAME]` — Clear cache (all or per-provider)

#### Multi-Profile Merge
- `envforge run --profiles dev,staging,custom` — Load and merge multiple profiles
- Left-to-right precedence (last profile wins on conflicts)
- Each profile overlaid on shared vars
- Compatible with `--env-file` and `--override` (highest precedence)
- Error if both `--profile` and `--profiles` used

### Quality
- 485 total tests (was 444), all passing
- 41 new tests across 10 features
- New modules: `hook.rs`, `share.rs`, `schema_json.rs`, `audit.rs`
- No new crate dependencies
- Clippy: 0 errors, 11 style warnings

## [0.4.3] - 2026-04-20

### Added

#### Unified Check (`envforge check`)
- `envforge check` — Single command running all health/safety checks: doctor, validate, scan, age, drift
- Grouped output by category with colored pass/fail/warning indicators
- Interactive fix hints per failure (`→ Run: envforge <fix command>`)
- `--only doctor,scan,age` — Run subset of categories
- `--json` — Machine-readable output for CI/CD
- Graceful skip when prerequisites missing (no schema → skip validate/drift)
- Non-zero exit on errors, zero on warnings-only

#### Encrypted Sync
- Sync snapshots now encrypted with age (X25519) before git commit
- Transparent auto-decrypt on `sync pull` — no extra flags needed
- Backward compatible: `read_snapshot` auto-detects encrypted vs plaintext
- `encrypted` config field in sync settings (default: `true` for new repos)
- Old unencrypted repos work without migration — next push encrypts automatically
- Clear error on key mismatch: "Cannot decrypt sync data. Key mismatch or corrupted."

#### Environment Snapshots (`envforge snapshot`)
- `envforge snapshot create [NAME]` — Capture active profile env state
- `envforge snapshot list` — Show all snapshots with name, date, variable count
- `envforge snapshot restore [NAME|--last]` — Restore with auto-backup before write
- `envforge snapshot diff [NAME|--last]` — Color-coded diff (added/removed/changed)
- `envforge snapshot delete NAME` — Remove snapshot with confirmation
- Auto-prune: keeps last 20 snapshots, oldest pruned on create
- Snapshots stored in `~/.config/envforge/snapshots/` as TOML

#### Key Explain (`envforge explain`)
- `envforge explain KEY` — Unified X-ray showing all known info about a key
- **Source**: file path, line number, export style, active/commented status
- **Profile**: which profile defines it (shared/profile-specific)
- **Schema**: type, required, description, sensitive flag (if `.env.schema` exists)
- **Encryption**: plaintext or encrypted status
- **Reference**: secret reference provider/path (if `ref:` value)
- **Sync**: synced/local/untracked status
- **Age**: days since last pull, stale flag
- `--json` — Full structured JSON output
- Similar key suggestions when key not found (substring + Levenshtein matching)

#### Secret Rotation (`envforge rotate`)
- `envforge rotate KEY` — Interactive 3-step flow: show masked current → prompt new → confirm
- Masked value display (`sk-ab****56` format)
- Atomic local update with backup before write
- Auto-resets secret age to 0 after rotation
- Logs rotation event to changelog
- Optional provider push: `Push to vault? [y/N]`
- Optional sync push: `Push to sync? [y/N]`
- `--dry-run` — Preview rotation plan without changes
- `--stale` — Bulk rotate all secrets older than 90 days (rotate/skip/quit per key)
- Handles encrypted values transparently (re-encrypts new value)

### Quality
- 444 total tests (was 398), all passing
- 46 new tests across 5 features
- No new crate dependencies
- Clippy clean

## [0.4.2] - 2026-04-20

### Added

#### Multi-Format Export
- `envforge export --format <fmt>` — Export env vars in 7 formats: `dotenv`, `json`, `yaml`, `toml`, `docker`, `k8s`, `tfvars`
- **JSON** — `{"KEY": "VALUE"}` object, valid JSON
- **YAML** — Properly quoted booleans (`true`/`false`), numbers, and YAML-special values (`null`, `~`, `yes`, `no`)
- **TOML** — All values quoted, backslash and quote escaping
- **Docker** — Bare `KEY=VALUE` for `--env-file`, no quotes or comments
- **Kubernetes Secret** — Full manifest with base64-encoded `data:` section
  - `--k8s-name NAME` — Set Secret name (default: `envforge-secrets`)
  - `--k8s-namespace NS` — Set namespace (default: `default`)
- **Terraform tfvars** — Lowercase keys, quoted values
- Works with existing `--filter` flag

#### Secret Age Tracking
- `envforge secrets age [--threshold DAYS] [--stale-only]` — Show age of all tracked secrets
- Automatically tracks when secrets are pulled from providers
- Flags stale secrets exceeding threshold (default: 90 days)
- Color-coded output: green ✓ for fresh, red ⚠ for stale
- `--json` output with age_days, stale flag, provider info
- Persistent tracking in `~/.config/envforge/secret-sources.toml`

#### Provider Diff
- `envforge secrets diff --from <provider> --path <path> [--filter PATTERN]` — Compare local ENV vs provider state
- Shows 4 categories: same, changed, only-local, only-remote
- Color-coded diff output with truncated values
- `--json` output for CI/CD integration
- Works with all 7 secret providers

#### GitHub Action (`action/`)
- **Composite GitHub Action** for CI/CD integration — manage ENV vars and secrets in pipelines
- 5 operation modes: `validate`, `secrets-pull`, `export`, `run`, `drift`
- **validate** — Check `.env` files against `.env.schema`, fail step on validation errors
- **secrets-pull** — Pull secrets from any of 7 providers (Vault, AWS SSM, 1Password, Doppler, Infisical, GCP, Azure) into `GITHUB_ENV`
- **export** — Export EnvForge-managed variables into workflow environment
- **run** — Execute commands with process-scoped secret injection (secrets never persist in `GITHUB_ENV`)
- **drift** — Detect configuration drift across `.env` files in PRs
- Automatic binary installation from GitHub Releases (Linux x86_64/aarch64, macOS x86_64/aarch64)
- Version pinning (`version` input) or auto-resolve latest release
- Value masking in GitHub Actions logs by default (`mask-values: true`)
- Multiline-safe `GITHUB_ENV` export using heredoc delimiters
- All 4 outputs wired: `variables`, `count`, `validation-result`, `drift-result`
- Example workflow (`.github/workflows/envforge-example.yml`)
- Comprehensive test suite (`action/tests/test_action.sh`) — 21 tests covering syntax, input validation, integration, structure, and permissions

#### Usage
```yaml
# Validate .env on PRs
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env

# Pull secrets from AWS SSM
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production

# Run tests with injected secrets
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: cargo test
    resolve-secrets: 'true'
```

### Quality
- 398 total tests (was 357), all passing
- 27 new Tier 1 feature tests (export formats, secret age, edge cases)
- 21 action tests, all passing
- Shell scripts pass `bash -n` syntax validation
- Action outputs correctly wired to composite step

## [0.4.0] - 2026-04-17

### Added

#### Subprocess Runner (`envforge run`)
- `envforge run [flags] -- <command> [args]` — Run any command with injected ENV vars
- `--profile NAME` — Use a specific profile without switching the active profile
- `--resolve` — Decrypt `ENC[age:...]` values and resolve `ref:` secret references at runtime
- `--env-file PATH` — Load additional .env file(s), repeatable, override order preserved
- `--override KEY=VALUE` — Override specific variables, repeatable
- `--dry-run` — Preview injected ENV vars without executing the command
- Exit code and signal passthrough from child process
- ENV merge order: shell env → shared file → profile file → env-file(s) → overrides

#### ENV Schema (`.env.schema`)
- `.env.schema` TOML format with per-variable type, required, default, description, example, sensitive, pattern, values, min, max
- Type system: `string`, `number`, `bool`, `url`, `email`, `enum`, `regex`, `port`
- Environment-specific overrides: `[VARIABLE.production]` sections
- `envforge validate --schema PATH [--env PATH] [--environment NAME]` — Schema-based validation with CI-friendly exit codes
- Schema and `config.toml` `[validation]` rules merged (schema takes priority)
- `envforge schema generate [--output PATH]` — Auto-generate schema from existing ENV with type heuristics
- `envforge docs --schema PATH [--output PATH]` — Generate Markdown documentation table
- `envforge drift --envs FILE1 FILE2 ...` — Multi-environment drift detection with color-coded matrix
- `envforge init --schema PATH [--output PATH]` — Interactive onboarding wizard for new developers

#### AI Agent Safety
- `envforge export --safe` — Export with sensitive values redacted as `[REDACTED]`
- `envforge export --env-example` — Generate `.env.example` from schema with placeholder values
- `.envforgeignore` convention — List files AI tools should not read (`.gitignore` syntax)
- `envforge doctor` AI safety check — Warns when `.env` exists without `.envforgeignore`

#### Git Merge Driver
- `envforge git install-merge-driver` — Register semantic `.env` merge driver in git config
- Three-way merge with key=value understanding: auto-merges non-conflicting changes
- Per-key conflict markers for true conflicts (same key changed on both sides)
- `envforge git remove-merge-driver` — Clean uninstall

#### Health Check (`envforge doctor`)
- 10 health checks: config, encryption key, shell files, duplicates, validation, references, AI safety, sync, providers, credentials
- Every warning includes an actionable fix suggestion (cyan `→` hint)
- `--verbose` for detailed output, `--json` for machine-readable output

#### Other
- `envforge secrets resolve [--key KEY]` — Output `export KEY='value'` for shell init (`eval "$(envforge secrets resolve)"`)
- `envforge profile diff A B` — Side-by-side profile comparison with color-coded output

### Fixed

#### Secret Manager Providers (all 7 validated against official CLI docs)
- **AWS SSM**: Added pagination loop for `get-parameters-by-path` (>10 results), added `--recursive` flag
- **Doppler**: Filter system keys (`DOPPLER_PROJECT`, `DOPPLER_CONFIG`, `DOPPLER_ENVIRONMENT`) from pull results, batch push into single CLI call
- **Infisical**: Changed pull from invalid `secrets get --plain --format=json` to correct `export --format=json`, fixed set syntax from `set KEY VALUE` to `set KEY=VALUE`
- **Azure Key Vault**: Added underscore-to-hyphen name mapping (`DB_HOST` → `DB-HOST` on push, reverse on pull)
- **Vault**: Fixed `kv list` parsing to read `data.keys` instead of bare array
- **1Password**: Fixed `--fields` flag to `--fields=label=KEY` per official docs
- **GCP**: Handle empty `secrets list` output (empty string instead of `[]`)

### Quality
- 357 total tests (was 256), all passing
- 59 new provider validation tests covering all 7 providers
- 42 new feature tests (schema, run, safe export, doctor, drift, merge)
- Clippy clean, rustfmt clean
- No new crate dependencies

## [0.3.0] - 2026-04-15

### Added

#### Secret Manager Integration
- **Provider pattern** with `SecretProvider` trait — extensible for any secret manager
- 7 built-in providers: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault
- `envforge secrets pull --from <provider>` — Pull secrets from any provider
- `envforge secrets push --to <provider> [--keys|--all|--filter]` — Push secrets to provider
- `envforge secrets ref <KEY> --from <provider> --path <path>` — Create lazy references
- `envforge secrets unref <KEY>` — Remove a reference
- `envforge secrets resolve` — Resolve all references (with TTL cache + offline fallback)
- `envforge secrets config <provider> --set key=value` — Store credentials (age encrypted)
- `envforge secrets providers` — List all providers with binary and config status
- `envforge secrets status` — Show configured providers
- **Credential store** — Provider credentials encrypted with age in `credentials.toml`
- **Reference cache** — TTL-based caching for resolved secrets, offline fallback
- **authenticate()** and **validate_config()** on SecretProvider trait
- **Source tracking** — Pulled keys track which provider they came from
- **Progress indicator** — Shown for large pull/push operations (>50 keys)
- Vault AppRole authentication support
- AWS profile and IAM role authentication support
- "Did you mean?" suggestion for misspelled provider names

### Quality
- 256 total tests, all passing
- Clippy clean, rustfmt clean

## [0.2.0] - 2026-04-15

### Added

#### Remote Sync
- **Git-based sync** to share ENV variables across machines via any Git remote
- `envforge sync init [--remote URL] [--machine-id ID] [--force]` — Initialize sync repository
- `envforge sync push [-m MSG] [--dry-run]` — Export marked keys, commit, and push
- `envforge sync pull [--dry-run]` — Pull remote snapshot and apply changes
- `envforge sync status` — Show diff between local state and sync snapshot
- `envforge sync mark <KEY|PATTERN> --sync|--local [--all]` — Select which keys to sync
- `envforge sync list-keys` — View all keys with sync/local status
- `envforge sync override <KEY> <VALUE> [--remove] [--list]` — Machine-specific overrides
- `envforge sync history [-n N]` — View snapshot commit history
- `envforge sync rollback <COMMIT|--last>` — Restore previous snapshot with auto-backup
- `envforge sync log [-n N]` — View sync operation log
- `envforge sync machine` — Show machine identity and override count
- Selective sync with glob pattern support (`AWS_*`, `DB_?`)
- Auto-generated machine identity (`{hostname}-{hex}`) per machine
- Machine overrides — per-machine values that take precedence over shared snapshot
- Three-way conflict detection when both local and remote modify the same key
- Conflict resolution strategies: `keep-local`, `keep-remote`, `manual-edit`, with configurable default
- Offline-first design — all operations work locally, remote sync is optional
- Sync history and rollback via Git commit history
- Sync operation log (local, append-only, auto-rotated at 100 entries)
- JSON output (`--json`) and dry-run (`--dry-run`) for all sync commands

### Changed

- `envforge sync mark` now requires explicit `--sync` or `--local` flag (no silent defaults)
- `--all` flag no longer requires a key argument

### Dependencies

- Added `hostname` (0.4) for machine ID generation
- Added `rand` (0.9) for random hex suffix

### Quality

- 115 sync-specific tests (82 unit + 33 integration)
- 240 total project tests, all passing
- Clippy clean (`-D warnings`), rustfmt clean

## [0.1.0] - 2026-04-12

### Added

#### Core
- Shell file parser with line-level AST and byte-for-byte round-trip fidelity
- Support for `.zshrc`, `.bashrc`, `.bash_profile`, `.profile`, `.zprofile`
- Auto-detection of shell type from `$SHELL`
- Soft-delete with envforge tags (never physically removes content)
- Atomic file writes (tempfile + rename)
- SHA-256 hash verification before writes
- Automatic backup before every write (last 10 retained)
- Protected zone management (header/footer offsets, conda/Amazon Q block detection)
- Reference file strategy (`~/.env_managed` with source injection)
- Conflict detection (hash-based external change detection)

#### TUI Interface
- Table view with KEY / VALUE / LOCATION columns
- Keyboard navigation (vim-style j/k, search with /)
- Mouse support (click to select, scroll to navigate)
- Value masking for sensitive keys (SECRET, TOKEN, PASSWORD, CREDENTIAL, KEY)
- Edit popup, add dialog, confirm dialogs, diff preview
- Fuzzy search with match character highlighting
- ENV grouping — auto-detected prefixes + user-defined groups in config
- Collapsible group headers (arrow keys to expand/collapse)
- Active/passive toggle with Space key (visual status box)
- Full in-session undo history (`u` key)
- Profile switching with `P` key selector popup
- Profile badge in header
- Import (`I`) and export (`E`) from TUI
- Status bar with unsaved indicator, undo count, notifications
- Help screen (`?`)
- Save with `S` or `Ctrl+S`

#### CLI Interface
- `envforge list` — list all variables (with `--json`)
- `envforge get KEY` — get a value
- `envforge set KEY=VALUE` — create or update
- `envforge delete KEY` — soft-delete
- `envforge copy KEY` — copy to clipboard
- `envforge move KEY` — move to reference file
- `envforge import <file>` — import from .env file
- `envforge export [path]` — export to .env format
- `envforge duplicates` — detect duplicate keys
- `envforge scan [path]` — scan for leaked secrets
- `envforge validate` — validate ENV values against config rules
- `envforge encrypt KEY` — encrypt value with age
- `envforge decrypt KEY` — decrypt value
- `envforge profile list|switch|create|delete` — manage profiles
- `envforge log [KEY]` — view change history
- `envforge completions zsh|bash|fish` — generate shell completions
- `envforge diff` — show pending changes
- `envforge config` — show configuration
- `envforge --dry-run` — preview without writing
- First-run setup wizard

#### Profiles
- Multiple environment profiles (dev/staging/prod)
- Shared ENV file (`~/.env_managed.shared`) always sourced
- Profile-specific files (`~/.env_managed.{name}`)
- Precedence: profile > shared > shell config
- Last used profile remembered across sessions
- Migration from single reference file to profile-based

#### Security
- ENV encryption at rest using age (X25519)
- Auto-generated age keypair with 0600 permissions
- Secret scanning — detect leaked values in source code
- Git staged file scanning (`--staged` flag for pre-commit hooks)
- Sensitive value masking in TUI and changelog

#### Quality
- ENV validation rules in config (url, number, bool, email, nonempty, regex)
- Automatic changelog on save
- Log viewer with key filtering
- 125 automated tests
- CI pipeline (GitHub Actions: Linux + macOS matrix)
- Release pipeline (multi-platform binaries)
