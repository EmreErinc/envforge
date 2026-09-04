# EnvForge for VS Code

EnvForge is a Rust **CLI + TUI** for environment variables: **profiles** (dev / staging / prod), `.env.schema` validation, secret scanning, and encrypted sync. This extension is the VS Code client for that binary.

> Activates only in a **trusted workspace** — the language server and `envforge` binary don't start in an untrusted workspace.

## Core app (the CLI)

Install once, use in the terminal and in this extension:

```bash
brew install emreerinc/tap/envforge
# or
cargo install env-forge-tui
```

| Core capability | What it is |
|-----------------|------------|
| Profiles | Switch and diff named env sets (dev / staging / prod) |
| Schema | `.env.schema` types, required keys, examples |
| TUI | `envforge` full-screen manager in the terminal |
| Scan / doctor | Secret-leak scan and health check |
| Fence | Writes ignore/rules for **configured** AI tools. Not a sandbox. |
| Guard | Prompt-injection / canary scan on save (via LSP) |
| MCP scan | Warns on hardcoded credentials in MCP config files |
| `mcp serve` | Optional (`--features mcp-server`). Default brew and GitHub binaries do **not** include it. |

Linux and macOS. Source-available (Elastic License 2.0).

## What this extension adds

### Without the CLI (standalone)

Syntax highlighting for `.env` / `.env.schema`, file icons, sidebar chrome, and schema file templates. No live diagnostics until the CLI is present.

### With the CLI

- **LSP** — missing required vars, type errors, secret-leak warnings; hover (schema metadata, values redacted); completions; go-to-definition (`.env` key → `.env.schema`)
- **Sidebars** — Variables (prefix grouping), Profiles (click to switch), Security dashboard
- **Status bar** — variable count; fence indicator
- **Gutter** — exposure heatmap; canary tripwire glyphs (untouched vs triggered)
- **Commands** — validate, scan, export, sync, profile switch/diff, doctor, fence/guard, canary, volatile session, audit-logged reveal, lifecycle
- **Redaction** — sensitive values show as `***` in hover, completions, and diagnostics

## Requirements & execution modes

- **Standalone:** highlighting, decorators, and sidebar UI without a binary.
- **Full LSP / fence / profiles:** auto-download from the welcome page, or install the CLI with the tap above.

## Installation

### From Marketplace (recommended)

Search "EnvForge" in VS Code Extensions, or:

```bash
ext install emreerinc.envforge-env-manager
```

Or visit: [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager)

### From VSIX (local build)

```bash
cd editors/vscode
npm install
npm run bundle
npx vsce package --allow-missing-repository
code --install-extension envforge-env-manager-0.2.2.vsix
```

### From source (development)

```bash
cd editors/vscode
npm install
npm run compile
# Press F5 in VS Code to launch Extension Development Host
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `envforge.path` | `""` | Path to envforge binary. Empty = use system PATH |
| `envforge.lsp.enable` | `true` | Enable Language Server for diagnostics, hover, completions |
| `envforge.secretScanning.enable` | `true` | Enable secret leak warnings |

## Commands

Open Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and type "EnvForge":

| Command | Description |
|---------|-------------|
| EnvForge: List Variables | Show all environment variables |
| EnvForge: Validate Against Schema | Run schema validation |
| EnvForge: Scan for Secret Leaks | Scan for leaked secrets |
| EnvForge: Switch Profile | Pick a profile to switch to |
| EnvForge: Diff Profiles | Compare two profiles side-by-side |
| EnvForge: Generate Schema from .env | Auto-generate `.env.schema` |
| EnvForge: Export Variables | Export to dotenv/json/yaml/toml/docker/k8s/tfvars |
| EnvForge: Sync Status | Show local vs remote diff |
| EnvForge: Sync Push | Push changes to remote |
| EnvForge: Sync Pull | Pull changes from remote |
| EnvForge: Run Health Check | Run envforge doctor |
| EnvForge: Run All Checks | Run doctor + validate + scan + age + drift |
| EnvForge: Restart Language Server | Reload window to restart LSP |

## How It Works

The extension starts the EnvForge Language Server (`envforge lsp`) over stdio. The LSP server provides:

1. **Real-time diagnostics** — As you edit `.env` files, the server validates against `.env.schema`
2. **Hover information** — Hover over any key to see its schema metadata
3. **Completions** — Start typing at the beginning of a line to get key suggestions
4. **Go-to-definition** — `Cmd+Click` / `Ctrl+Click` on a key to jump to `.env.schema`

Commands use the CLI directly (`envforge <command> --json`) for operations like sync, export, and scanning.

## Schema Example

Create `.env.schema` in your project root:

```toml
[DATABASE_URL]
type = "url"
required = true
description = "PostgreSQL connection string"
example = "postgres://user:pass@localhost:5432/mydb"
sensitive = true

[PORT]
type = "port"
required = true
default = "3000"
description = "Server port"

[NODE_ENV]
type = "enum"
values = ["development", "staging", "production"]
default = "development"
```

The LSP will validate your `.env` file against this schema in real-time.
