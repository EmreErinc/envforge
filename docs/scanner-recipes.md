# External Scanner Recipes

Ready-to-copy configuration snippets for common external security scanners integrated with EnvForge's scanner pipeline.

## How It Works

Each scanner is configured in `.envforge.project.toml` under the `[scanners]` section. EnvForge runs all enabled scanners concurrently, piping tool input/output via stdin. Exit code 0 = clean, non-zero = findings.

## Configuration Format

```toml
[scanners.<name>]
command = "<binary-or-script>"
args = ["<arg1>", "<arg2>"]
timeout_ms = 5000   # optional, default 5000
enabled = true      # optional, default true
```

## Manage Scanners

```bash
envforge scanner list                 # Show all scanners
envforge scanner test <name>          # Test with sample content
envforge scanner run <name> <content> # Run against arbitrary content
envforge scanner enable <name>        # Enable scanner
envforge scanner disable <name>       # Disable scanner
```

---

## Recipe 1: Lakera Guard

**Purpose**: Detect prompt injection and jailbreak attempts via Lakera Guard API.

**Installation**: Get an API key from [lakera.ai](https://www.lakera.ai/). Create a wrapper script:

```bash
cat > /usr/local/bin/lakera-guard << 'SCRIPT'
#!/bin/bash
# Lakera Guard API scanner
# Reads content from stdin, sends to Lakera API
# Exit 0 = safe, Exit 1 = flagged

LAKERA_API_KEY="${LAKERA_API_KEY:-}"
if [ -z "$LAKERA_API_KEY" ]; then
    echo "LAKERA_API_KEY not set" >&2
    exit 0  # Don't block if not configured
fi

CONTENT=$(cat)
RESPONSE=$(curl -s -X POST https://api.lakera.ai/v1/prompt_injection \
    -H "Authorization: Bearer $LAKERA_API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"input\": $(echo "$CONTENT" | jq -Rs .)}")

FLAGGED=$(echo "$RESPONSE" | jq -r '.results[0].flagged // false')
if [ "$FLAGGED" = "true" ]; then
    echo "LAKERA: Prompt injection detected"
    echo "$RESPONSE" | jq '.'
    exit 1
fi
exit 0
SCRIPT
chmod +x /usr/local/bin/lakera-guard
```

**Config**:

```toml
[scanners.lakera]
command = "lakera-guard"
timeout_ms = 3000
```

**Expected output (when flagged)**:
```
LAKERA: Prompt injection detected
{
  "results": [{"flagged": true, "category": "prompt_injection", ...}]
}
```

---

## Recipe 2: Gitleaks

**Purpose**: Detect hardcoded secrets, API keys, and tokens in tool input/output.

**Installation**: `brew install gitleaks` or see [gitleaks.io](https://gitleaks.io/).

**Config**:

```toml
[scanners.gitleaks]
command = "gitleaks"
args = ["detect", "--no-git", "--source=-", "--verbose"]
timeout_ms = 5000
```

**Note**: Gitleaks by default scans git repos. `--no-git --source=-` makes it read from stdin for EnvForge's pipeline. A small wrapper script may be needed for stable stdin behavior:

```bash
cat > /usr/local/bin/gitleaks-stdin << 'SCRIPT'
#!/bin/bash
# Gitleaks stdin wrapper
tmp=$(mktemp)
cat > "$tmp"
gitleaks detect --no-git --source="$tmp" --verbose 2>&1
EXIT=$?
rm -f "$tmp"
exit $EXIT
SCRIPT
chmod +x /usr/local/bin/gitleaks-stdin
```

Then use:

```toml
[scanners.gitleaks]
command = "gitleaks-stdin"
timeout_ms = 5000
```

**Expected output (when finding)**:
```
WRN leaks found: 1
[{
  "Description": "AWS Access Key",
  "Match": "AKIAIOSFODNN7EXAMPLE",
  ...
}]
```

---

## Recipe 3: ggshield (GitGuardian)

**Purpose**: Detect 350+ types of secrets via GitGuardian's CLI.

**Installation**: `pip install ggshield` or see [docs.gitguardian.com](https://docs.gitguardian.com/). Authenticate: `ggshield auth login`.

**Config**:

```toml
[scanners.ggshield]
command = "ggshield"
args = ["secret", "scan", "stdin", "--exit-zero=false"]
timeout_ms = 5000
```

**Note**: `--exit-zero=false` ensures ggshield exits non-zero when secrets are found (required for EnvForge's scanner pipeline). Without this flag, ggshield always exits 0.

**Expected output (when finding)**:
```
>>> GitGuardian has found 1 secret(s) in the scanned content.

> Incident N. 1
Secret detected: Generic High Entropy Secret
Validity: valid
| @@ -0,0 +1 @@
+ export API_KEY="sk-abc123..."
```

---

## Creating Custom Scanners

Any binary or script that reads stdin and exits non-zero on findings works as an EnvForge scanner.

**Minimal example**:

```bash
cat > /usr/local/bin/my-scanner << 'SCRIPT'
#!/bin/bash
CONTENT=$(cat)
if echo "$CONTENT" | grep -qi "secret"; then
    echo "MY-SCANNER: Suspicious content detected"
    exit 1
fi
exit 0
SCRIPT
chmod +x /usr/local/bin/my-scanner
```

```toml
[scanners.my-scanner]
command = "my-scanner"
timeout_ms = 2000
```

**Rules**:
- Read content from **stdin** (not file args)
- Exit **0** = clean (no findings)
- Exit **non-zero** = findings on stdout/stderr
- Keep execution fast (< 5s recommended)
- Handle empty input gracefully
