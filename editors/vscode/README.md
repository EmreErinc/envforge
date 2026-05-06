# EnvForge for VS Code

Environment variable management with schema validation, secret scanning, and multi-profile support.

## Features

- **Diagnostics** — Missing required vars, type errors, secret leak warnings
- **Hover** — Schema info: type, description, default, example, sensitive flag
- **Completions** — Key names from `.env.schema` (only missing keys suggested)
- **Go-to-definition** — Click a key in `.env` to jump to its `.env.schema` section
- **Syntax highlighting** — `.env` and `.env.schema` files
- **13 commands** — Validate, scan, export, sync, profile switch, health check
- **Status bar** — Variable count, auto-refreshes

## Requirements

EnvForge CLI must be installed:

```bash
cargo install env-forge-tui
```

Verify:

```bash
envforge --version
```

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
code --install-extension envforge-env-manager-0.1.3.vsix
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
