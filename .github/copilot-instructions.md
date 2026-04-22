# Copilot Instructions for EnvForge

## Build, Test, and Lint Commands

**Build:**
```bash
cargo build                    # Debug build
cargo build --release         # Release build
```

**Test:**
```bash
cargo test                     # Run all tests
cargo test --lib             # Run library tests only
cargo test --test parser_tests  # Run specific test file
cargo test test_parse_export_with_double_quotes  # Run specific test by name
```

**Formatting and Linting:**
```bash
cargo fmt                      # Auto-format code
cargo fmt --check             # Check formatting without changes
cargo clippy -- -D warnings   # Lint (warnings treated as errors)
```

**Pre-commit Verification:**
```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

All pull requests must pass these checks. CI will reject unformatted code or clippy warnings.

## High-Level Architecture

EnvForge is a Rust CLI + TUI application for managing environment variables across multiple shell types. The codebase is organized into distinct layers:

### Module Structure
```
src/
├── model/         Data types and domain objects (LineNode, ShellFile, errors)
├── parser/        Deterministic shell file parsing and serialization
├── config/        Application config, backup management, atomic writes
├── ops/           Pure business operations (CRUD, profiles, encryption, sync, etc.)
├── ui/            TUI rendering and event handling (ratatui + crossterm)
├── cli/           CLI subcommand definitions and argument handlers
└── lsp/           Language Server Protocol support
```

### Design Principles

1. **Parser is the foundation** — all other modules depend on it. Changes to parser behavior affect the entire system.
2. **Round-trip fidelity** — parse → serialize must produce byte-identical output. This is critical for shell config management.
3. **ops/ contains pure business logic** — no I/O decisions, no UI concerns. This layer orchestrates all the "what to do" logic.
4. **ui/ and cli/ are thin presentation layers** — they call ops/ functions and format results. No business logic here.

### Data Flow Example

When adding a new environment variable:
1. **CLI layer** (`src/cli/commands.rs`) — parses user arguments
2. **ops layer** (`src/ops/crud.rs`) — performs add logic, manages state, validates conflicts
3. **Parser layer** (`src/parser/parse.rs`) — serializes the updated state back to shell syntax
4. **Config layer** (`src/config/`) — handles atomic writes and backups

## Key Conventions

### Error Handling
- Use `thiserror::Error` for all custom error types (defined in `src/model/error.rs`)
- Error types are descriptive and include context (e.g., file paths)
- Propagate errors with `?` operator; avoid `.unwrap()` in library code

### Testing
- **All integration and functional tests** live in `tests/` directory (no in-module tests)
- Test file naming: `{feature}_tests.rs` (e.g., `parser_tests.rs`, `ops_tests.rs`)
- Test function naming: `test_{what_is_being_tested}_{condition}` (e.g., `test_parse_export_with_double_quotes`)
- Use `insta` for snapshot testing where output stability matters
- Use `tempfile` for tests that need filesystem operations
- Each test should be self-contained with its own fixtures

### Code Style
- **Naming:** `snake_case` for functions/variables, `PascalCase` for types, `snake_case.rs` for files
- **Formatting:** Run `cargo fmt` — CI enforces this
- **Linting:** `cargo clippy -- -D warnings` must pass — all warnings are treated as errors
- **Documentation:** Doc comments on public items; use examples in doc tests where appropriate

### Parser Module Contract
- Functions in `parser/parse.rs` and `parser/detect.rs` must handle shell syntax variations (bash, zsh, fish, etc.)
- Any parser change that alters serialization output must update snapshot tests
- Parser functions should not perform I/O or sidesteps; keep pure

### Adding a New Feature

1. **Define the operation** in `src/ops/` as a pure function that doesn't know about CLI or UI
2. **Add CLI integration** in `src/cli/mod.rs` (subcommand definition) and `src/cli/commands.rs` (handler)
3. **Add TUI integration** in `src/ui/app.rs` (key handler) and `src/ui/dialogs.rs` if a popup is needed
4. **Add comprehensive tests** in `tests/{feature}_tests.rs`
5. **Update help text** in `src/ui/dialogs.rs` if user-facing

### Shell Config Handling
- EnvForge works with multiple shell types (bash, zsh, fish, sh)
- Parser must be shell-agnostic where possible; use detection for shell-specific features
- Never assume only one shell type is in use in a single file
- Test parsing against actual shell configs when possible

### Dependency Management
- Keep `Cargo.toml` lean; justify new dependencies
- Key dependencies:
  - `ratatui` — TUI rendering
  - `crossterm` — terminal control
  - `serde`/`toml` — configuration
  - `age` — encryption
  - `tokio` — async for LSP and proxy server
  - `insta` — snapshot testing

### AI Safety Context
- This tool is specifically designed to protect secrets from AI agents
- Many features (fence, ai-guard, MCP scan, canary, etc.) are AI-specific
- When modifying security features, consider attack vectors from AI systems
- Redaction and masking are first-class concerns, not afterthoughts

## Running a Single Test with Output

To run a single test and see detailed output:
```bash
cargo test test_name -- --nocapture --test-threads=1
```

To run with backtrace on panic:
```bash
RUST_BACKTRACE=1 cargo test test_name -- --nocapture
```
