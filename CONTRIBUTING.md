# Contributing to EnvForge

Thank you for your interest in contributing to EnvForge! This document outlines how to get started.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). Please be respectful and constructive.

## Getting Started

### Prerequisites

- Rust 1.75 or later (`rustup update stable`)
- Git
- Linux or macOS

### Setup

```bash
git clone https://github.com/emreerinc/envforge.git
cd envforge
cargo build
cargo test
```

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run checks: `cargo fmt && cargo clippy -- -D warnings && cargo test`
5. Commit with a descriptive message
6. Push and open a Pull Request

## Code Style

### Formatting
- **rustfmt** with default settings — run `cargo fmt` before committing
- CI will reject unformatted code

### Linting
- **Clippy strict** — `cargo clippy -- -D warnings`
- All warnings are treated as errors
- CI will reject any clippy warning

### Naming
- Follow standard Rust conventions: `snake_case` for functions/variables, `PascalCase` for types
- File names: `snake_case.rs`

### Testing
- All tests go in `tests/` directory (no in-module tests)
- Use descriptive test names: `test_parse_export_with_double_quotes`
- Use `insta` for snapshot tests where applicable
- Use `tempfile` for tests that need filesystem

## Pull Request Guidelines

### Before Submitting

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes (all tests)
- [ ] New features have tests
- [ ] No unnecessary files added

### PR Title Format

Use conventional commit style:

- `feat: add fish shell support`
- `fix: handle empty .zshrc correctly`
- `refactor: simplify parser regex`
- `docs: update README installation`
- `test: add round-trip tests for heredoc`
- `chore: update dependencies`

### PR Description

- Explain **what** changed and **why**
- Reference any related issues
- Include manual testing steps for TUI changes

## Architecture Overview

```
src/
├── model/     # Data types — LineNode, ShellFile, errors
├── parser/    # Shell file parsing & serialization
├── config/    # App configuration, backup, atomic writes
├── ops/       # Business operations (CRUD, profiles, encryption, etc.)
├── ui/        # TUI rendering and interaction
└── cli/       # CLI subcommand definitions and handlers
```

### Key Principles

1. **parser/** is the foundation — all other modules depend on it
2. **ops/** contains pure business logic — no I/O decisions, no UI
3. **ui/** and **cli/** are thin layers that call **ops/**
4. Never break round-trip fidelity — parse → serialize must be byte-identical

### Adding a New Feature

1. Add the operation logic in `src/ops/`
2. Add CLI subcommand in `src/cli/mod.rs` + handler in `src/cli/commands.rs`
3. Add TUI integration in `src/ui/app.rs` (key handler) + `src/ui/dialogs.rs` (if popup needed)
4. Add tests in `tests/`
5. Update help screen in `src/ui/dialogs.rs`

## Reporting Issues

- Use GitHub Issues
- Include: OS, shell, EnvForge version (`envforge --version`)
- For parser bugs: include (sanitized) sample of your shell config
- For TUI bugs: include terminal emulator name and size

## Feature Requests

- Open a GitHub Issue with `[Feature]` prefix
- Describe the use case, not just the solution
- We prioritize features that align with the core philosophy: **safe, non-destructive ENV management**

## Releases

See [VERSIONING.md](VERSIONING.md) for version policy.
