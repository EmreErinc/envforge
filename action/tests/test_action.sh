#!/usr/bin/env bash
# Local test harness for EnvForge GitHub Action scripts.
# Simulates GitHub Actions environment variables and verifies behavior.
#
# Usage: bash action/tests/test_action.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ACTION_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${ACTION_DIR}/.." && pwd)"

PASS=0
FAIL=0
TESTS_RUN=0

# Create temp files to simulate GITHUB_ENV / GITHUB_OUTPUT / GITHUB_PATH
TMPDIR_TEST="$(mktemp -d)"
export GITHUB_ENV="${TMPDIR_TEST}/github_env"
export GITHUB_OUTPUT="${TMPDIR_TEST}/github_output"
export GITHUB_PATH="${TMPDIR_TEST}/github_path"
touch "${GITHUB_ENV}" "${GITHUB_OUTPUT}" "${GITHUB_PATH}"

cleanup() {
  rm -rf "${TMPDIR_TEST}"
}
trap cleanup EXIT

# --------------------------------------------------------------------------
# Test helpers
# --------------------------------------------------------------------------
reset_outputs() {
  : > "${GITHUB_ENV}"
  : > "${GITHUB_OUTPUT}"
  : > "${GITHUB_PATH}"
}

assert_contains() {
  local file="$1"
  local pattern="$2"
  local desc="$3"
  TESTS_RUN=$((TESTS_RUN + 1))
  if grep -q "${pattern}" "${file}" 2>/dev/null; then
    echo "  PASS: ${desc}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${desc}"
    echo "    Expected '${pattern}' in $(basename "${file}")"
    echo "    Content: $(cat "${file}")"
    FAIL=$((FAIL + 1))
  fi
}

assert_not_contains() {
  local file="$1"
  local pattern="$2"
  local desc="$3"
  TESTS_RUN=$((TESTS_RUN + 1))
  if ! grep -q "${pattern}" "${file}" 2>/dev/null; then
    echo "  PASS: ${desc}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${desc}"
    echo "    Did not expect '${pattern}' in $(basename "${file}")"
    FAIL=$((FAIL + 1))
  fi
}

assert_exit_code() {
  local expected="$1"
  local actual="$2"
  local desc="$3"
  TESTS_RUN=$((TESTS_RUN + 1))
  if [ "${actual}" -eq "${expected}" ]; then
    echo "  PASS: ${desc}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${desc} (expected exit ${expected}, got ${actual})"
    FAIL=$((FAIL + 1))
  fi
}

# --------------------------------------------------------------------------
# Test: install.sh platform detection (dry check, won't actually download)
# --------------------------------------------------------------------------
echo ""
echo "=== Test: install.sh platform detection ==="
reset_outputs

OS="$(uname -s)"
ARCH="$(uname -m)"
echo "  Platform: ${OS} / ${ARCH}"

# Verify script is syntactically valid
TESTS_RUN=$((TESTS_RUN + 1))
if bash -n "${ACTION_DIR}/scripts/install.sh" 2>/dev/null; then
  echo "  PASS: install.sh syntax valid"
  PASS=$((PASS + 1))
else
  echo "  FAIL: install.sh has syntax errors"
  FAIL=$((FAIL + 1))
fi

# --------------------------------------------------------------------------
# Test: run.sh syntax check
# --------------------------------------------------------------------------
echo ""
echo "=== Test: run.sh syntax check ==="
TESTS_RUN=$((TESTS_RUN + 1))
if bash -n "${ACTION_DIR}/scripts/run.sh" 2>/dev/null; then
  echo "  PASS: run.sh syntax valid"
  PASS=$((PASS + 1))
else
  echo "  FAIL: run.sh has syntax errors"
  FAIL=$((FAIL + 1))
fi

# --------------------------------------------------------------------------
# Test: unknown mode exits with error
# --------------------------------------------------------------------------
echo ""
echo "=== Test: unknown mode rejection ==="
reset_outputs

export INPUT_MODE="invalid-mode"
rc=0
bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?
assert_exit_code 1 "${rc}" "unknown mode exits non-zero"

# --------------------------------------------------------------------------
# Test: run mode requires command
# --------------------------------------------------------------------------
echo ""
echo "=== Test: run mode requires command ==="
reset_outputs

export INPUT_MODE="run"
export INPUT_COMMAND=""
rc=0
bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?
assert_exit_code 1 "${rc}" "run mode without command exits non-zero"

# --------------------------------------------------------------------------
# Test: secrets-pull mode requires provider
# --------------------------------------------------------------------------
echo ""
echo "=== Test: secrets-pull requires provider ==="
reset_outputs

export INPUT_MODE="secrets-pull"
export INPUT_PROVIDER=""
rc=0
bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?
assert_exit_code 1 "${rc}" "secrets-pull without provider exits non-zero"

# --------------------------------------------------------------------------
# Test: drift mode requires drift-envs
# --------------------------------------------------------------------------
echo ""
echo "=== Test: drift requires drift-envs ==="
reset_outputs

export INPUT_MODE="drift"
export INPUT_DRIFT_ENVS=""
rc=0
bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?
assert_exit_code 1 "${rc}" "drift without drift-envs exits non-zero"

# --------------------------------------------------------------------------
# Test: run mode with envforge binary (if available)
# --------------------------------------------------------------------------
echo ""
echo "=== Test: run mode with echo command ==="
reset_outputs

if command -v envforge &>/dev/null; then
  export INPUT_MODE="run"
  export INPUT_COMMAND="echo hello_envforge"
  export INPUT_PROFILE=""
  export INPUT_RESOLVE_SECRETS="false"
  export INPUT_ENV_FILES=""
  export INPUT_OVERRIDES=""
  rc=0
  output=$(bash "${ACTION_DIR}/scripts/run.sh" 2>&1) || rc=$?

  if echo "${output}" | grep -q "hello_envforge"; then
    TESTS_RUN=$((TESTS_RUN + 1))
    echo "  PASS: run mode executed command successfully"
    PASS=$((PASS + 1))
  else
    TESTS_RUN=$((TESTS_RUN + 1))
    echo "  FAIL: run mode output missing expected string"
    echo "    Output: ${output}"
    FAIL=$((FAIL + 1))
  fi
else
  echo "  SKIP: envforge binary not in PATH"
fi

# --------------------------------------------------------------------------
# Test: validate mode with envforge binary (if available)
# --------------------------------------------------------------------------
echo ""
echo "=== Test: validate mode ==="
reset_outputs

if command -v envforge &>/dev/null; then
  # Create a temp .env.schema and .env for validation
  SCHEMA_FILE="${TMPDIR_TEST}/test.env.schema"
  ENV_TEST_FILE="${TMPDIR_TEST}/test.env"

  cat > "${SCHEMA_FILE}" <<'SCHEMA'
[variables.APP_NAME]
type = "string"
required = true
description = "Application name"

[variables.PORT]
type = "port"
required = false
default = "3000"
SCHEMA

  cat > "${ENV_TEST_FILE}" <<'ENV'
APP_NAME=myapp
PORT=8080
ENV

  export INPUT_MODE="validate"
  export INPUT_SCHEMA="${SCHEMA_FILE}"
  export INPUT_ENV_FILE="${ENV_TEST_FILE}"
  export INPUT_ENVIRONMENT=""
  rc=0
  bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?

  assert_exit_code 0 "${rc}" "validate passes with valid env"
  assert_contains "${GITHUB_OUTPUT}" "validation-result=pass" "output contains validation-result=pass"
else
  echo "  SKIP: envforge binary not in PATH"
fi

# --------------------------------------------------------------------------
# Test: export mode with envforge binary (if available)
# --------------------------------------------------------------------------
echo ""
echo "=== Test: export mode ==="
reset_outputs

if command -v envforge &>/dev/null; then
  export INPUT_MODE="export"
  export INPUT_FILTER=""
  export INPUT_EXPORT_ENV="true"
  export INPUT_MASK_VALUES="false"
  rc=0
  bash "${ACTION_DIR}/scripts/run.sh" >/dev/null 2>&1 || rc=$?

  assert_contains "${GITHUB_OUTPUT}" "count=" "output contains count"
else
  echo "  SKIP: envforge binary not in PATH"
fi

# --------------------------------------------------------------------------
# Test: action.yml is valid YAML
# --------------------------------------------------------------------------
echo ""
echo "=== Test: action.yml validity ==="
TESTS_RUN=$((TESTS_RUN + 1))
if python3 -c "import yaml" 2>/dev/null; then
  if python3 -c "import yaml; yaml.safe_load(open('${ACTION_DIR}/action.yml'))" 2>/dev/null; then
    echo "  PASS: action.yml is valid YAML"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: action.yml is not valid YAML"
    FAIL=$((FAIL + 1))
  fi
else
  echo "  SKIP: python3 yaml module not available"
  TESTS_RUN=$((TESTS_RUN - 1))
fi

# --------------------------------------------------------------------------
# Test: action.yml has required fields
# --------------------------------------------------------------------------
echo ""
echo "=== Test: action.yml required fields ==="

assert_contains "${ACTION_DIR}/action.yml" "name:" "has name field"
assert_contains "${ACTION_DIR}/action.yml" "description:" "has description field"
assert_contains "${ACTION_DIR}/action.yml" "runs:" "has runs field"
assert_contains "${ACTION_DIR}/action.yml" "using: 'composite'" "uses composite action type"
assert_contains "${ACTION_DIR}/action.yml" "branding:" "has branding for marketplace"

# --------------------------------------------------------------------------
# Test: action.yml outputs are wired to step
# --------------------------------------------------------------------------
echo ""
echo "=== Test: action.yml outputs wired ==="

assert_contains "${ACTION_DIR}/action.yml" "steps.envforge.outputs.variables" "variables output wired"
assert_contains "${ACTION_DIR}/action.yml" "steps.envforge.outputs.count" "count output wired"
assert_contains "${ACTION_DIR}/action.yml" "steps.envforge.outputs.validation-result" "validation-result output wired"
assert_contains "${ACTION_DIR}/action.yml" "steps.envforge.outputs.drift-result" "drift-result output wired"

# --------------------------------------------------------------------------
# Test: scripts are executable
# --------------------------------------------------------------------------
echo ""
echo "=== Test: scripts executable ==="
TESTS_RUN=$((TESTS_RUN + 1))
if [ -x "${ACTION_DIR}/scripts/install.sh" ]; then
  echo "  PASS: install.sh is executable"
  PASS=$((PASS + 1))
else
  echo "  FAIL: install.sh not executable"
  FAIL=$((FAIL + 1))
fi

TESTS_RUN=$((TESTS_RUN + 1))
if [ -x "${ACTION_DIR}/scripts/run.sh" ]; then
  echo "  PASS: run.sh is executable"
  PASS=$((PASS + 1))
else
  echo "  FAIL: run.sh not executable"
  FAIL=$((FAIL + 1))
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo "=========================================="
echo "Results: ${PASS} passed, ${FAIL} failed, ${TESTS_RUN} total"
echo "=========================================="

if [ "${FAIL}" -gt 0 ]; then
  exit 1
fi
