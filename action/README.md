# EnvForge GitHub Action

Manage environment variables and secrets in your CI/CD pipelines with [EnvForge](https://github.com/emreerinc/envforge).

## Features

- **Validate** `.env` files against `.env.schema` before deploy
- **Pull secrets** from 7 providers (AWS SSM, Vault, 1Password, Doppler, Infisical, GCP, Azure)
- **Export** EnvForge-managed variables into your workflow
- **Run** commands with process-scoped secret injection
- **Drift detection** across environment files

## Quick Start

```yaml
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env
```

## Modes

### `validate` — Schema Validation

Validate environment variables against `.env.schema`. Fails the step if validation errors are found.

```yaml
- uses: emreerinc/envforge/action@v1
  id: validate
  with:
    mode: validate
    schema: .env.schema
    env-file: .env.production
    environment: production
```

**Inputs:** `schema`, `env-file`, `environment`
**Outputs:** `validation-result` (`pass` or `fail`)

### `secrets-pull` — Pull from Secret Providers

Pull secrets from any supported provider and optionally export them to `GITHUB_ENV`.

```yaml
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production
    filter: 'DB_*'
```

**Inputs:** `provider` (required), `provider-path`, `filter`, `export-env`, `mask-values`
**Outputs:** `variables`, `count`

### `export` — Export Variables

Export EnvForge-managed environment variables into the workflow.

```yaml
- uses: emreerinc/envforge/action@v1
  with:
    mode: export
    filter: 'API_*'
```

**Inputs:** `filter`, `export-env`, `mask-values`
**Outputs:** `count`

### `run` — Run Command with Secrets

Run a command with EnvForge-managed variables injected into the process. Secrets never leak to other steps.

```yaml
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: npm test
    profile: staging
    resolve-secrets: 'true'
    env-files: |
      .env.test
      .env.local
```

**Inputs:** `command` (required), `profile`, `resolve-secrets`, `env-files`, `overrides`

### `drift` — Drift Detection

Compare `.env` files and detect configuration drift.

```yaml
- uses: emreerinc/envforge/action@v1
  id: drift
  with:
    mode: drift
    schema: .env.schema
    drift-envs: |
      .env.development
      .env.staging
      .env.production
```

**Inputs:** `drift-envs` (required), `schema`, `environment`
**Outputs:** `drift-result` (`clean` or `drift`)

## All Inputs

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `mode` | Yes | — | `validate`, `secrets-pull`, `export`, `run`, `drift` |
| `version` | No | `latest` | EnvForge version to install |
| `profile` | No | — | EnvForge profile name |
| `schema` | No | — | Path to `.env.schema` |
| `provider` | No | — | Secret provider name |
| `provider-path` | No | `''` | Path in secret provider |
| `env-file` | No | — | `.env` file path |
| `environment` | No | — | Environment name for schema overrides |
| `command` | No | — | Command to run (run mode) |
| `filter` | No | — | Key filter pattern |
| `resolve-secrets` | No | `false` | Resolve secret references at runtime |
| `export-env` | No | `true` | Export to `GITHUB_ENV` |
| `mask-values` | No | `true` | Mask values in Actions logs |
| `env-files` | No | — | Additional `.env` files (comma/newline separated) |
| `overrides` | No | — | `KEY=VALUE` overrides (comma/newline separated) |
| `drift-envs` | No | — | `.env` files for drift comparison |

## All Outputs

| Output | Modes | Description |
|--------|-------|-------------|
| `variables` | `secrets-pull` | JSON of exported variables |
| `count` | `secrets-pull`, `export` | Number of variables processed |
| `validation-result` | `validate` | `pass` or `fail` |
| `drift-result` | `drift` | `clean` or `drift` |

## Examples

### Validate on PR

```yaml
name: Validate ENV
on: [pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: emreerinc/envforge/action@v1
        with:
          mode: validate
          schema: .env.schema
          env-file: .env
```

### Deploy with Secrets

```yaml
name: Deploy
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: emreerinc/envforge/action@v1
        with:
          mode: secrets-pull
          provider: aws-ssm
          provider-path: /myapp/production

      - run: ./deploy.sh
        # All secrets are now in the environment
```

### Test with EnvForge Run

```yaml
name: Test
on: [push]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: emreerinc/envforge/action@v1
        with:
          mode: run
          command: cargo test
          resolve-secrets: 'true'
          overrides: |
            DATABASE_URL=postgres://localhost/test
            ENVIRONMENT=ci
```

### Drift Check on PR

```yaml
name: Drift Check
on: [pull_request]

jobs:
  drift:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: emreerinc/envforge/action@v1
        id: drift
        with:
          mode: drift
          schema: .env.schema
          drift-envs: |
            .env.development
            .env.staging
            .env.production

      - if: steps.drift.outputs.drift-result == 'drift'
        run: echo "⚠️ Environment drift detected!"
```

## Security

- Values are masked in GitHub Actions logs by default (`mask-values: true`)
- `run` mode injects secrets only into the subprocess — they don't persist in `GITHUB_ENV`
- Use `export-env: false` with `secrets-pull` to prevent secrets from leaking to subsequent steps

## License

MIT
