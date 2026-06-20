# CI gating with EnvForge (FR24)

EnvForge's status commands return deterministic exit codes so a CI job can fail
the build when AI-secret hygiene regresses:

| Command | Exit 0 | Exit 2 |
|---|---|---|
| `envforge fence --status` | all detected AI tools covered | a detected tool is unfenced |
| `envforge mcp status` | no hardcoded credentials in AI/MCP config files | a hardcoded credential found |

(Exit `1` is reserved for command errors — distinct from a gate failure.)
Both accept `--json` for machine-readable output alongside the exit code.

## GitHub Actions

```yaml
name: envforge-guard
on: [push, pull_request]

jobs:
  ai-secret-guard:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install EnvForge
        run: cargo install --git https://github.com/envforge/envforge   # or pin a release

      - name: Fence coverage gate
        run: envforge fence --status        # exit 2 → fails the job if any detected tool is unfenced

      - name: MCP/agent config credential scan
        run: envforge mcp status            # exit 2 → fails the job on a hardcoded credential
```

## Notes

- Run `envforge fence` in onboarding so the repo ships fenced; the gate then
  catches regressions (a newly added AI tool left unfenced).
- `envforge mcp harden` is the one-shot fix for flagged credentials.
- Pin the EnvForge version in CI for reproducibility.
- For JSON consumption (dashboards, custom gates), add `--json` and parse
  `all_fenced` / the findings array.
