#!/usr/bin/env bash
set -euo pipefail

MODE="${INPUT_MODE}"
PROFILE="${INPUT_PROFILE:-}"
SCHEMA="${INPUT_SCHEMA:-}"
PROVIDER="${INPUT_PROVIDER:-}"
PROVIDER_PATH="${INPUT_PROVIDER_PATH:-}"
ENV_FILE="${INPUT_ENV_FILE:-}"
ENVIRONMENT="${INPUT_ENVIRONMENT:-}"
COMMAND="${INPUT_COMMAND:-}"
FILTER="${INPUT_FILTER:-}"
RESOLVE_SECRETS="${INPUT_RESOLVE_SECRETS:-false}"
EXPORT_ENV="${INPUT_EXPORT_ENV:-true}"
MASK_VALUES="${INPUT_MASK_VALUES:-true}"
ENV_FILES="${INPUT_ENV_FILES:-}"
OVERRIDES="${INPUT_OVERRIDES:-}"
DRIFT_ENVS="${INPUT_DRIFT_ENVS:-}"
QUARANTINE="${INPUT_QUARANTINE:-auto}"
ALLOW_KEYS="${INPUT_ALLOW_KEYS:-}"

# --------------------------------------------------------------------------
# CI Trust Classification + Quarantine
# --------------------------------------------------------------------------
# Runs once at the top so the rest of run.sh sees a possibly-scrubbed env.

apply_ci_trust() {
  if [ "${QUARANTINE}" = "off" ]; then
    echo "EnvForge: quarantine=off; skipping trust classification" >&2
    envforge ci-trust summary >> "${GITHUB_STEP_SUMMARY:-/dev/null}" 2>/dev/null || true
    return 0
  fi

  local verdict_json verdict_level
  verdict_json="$(envforge ci-trust classify --json 2>/dev/null || echo '{}')"
  verdict_level="$(echo "${verdict_json}" | grep -oE '"Untrusted"|"Suspicious"|"Trusted"' | head -1 | tr -d '"')"
  verdict_level="${verdict_level:-Untrusted}"

  if [ "${verdict_level}" = "Untrusted" ] || [ "${QUARANTINE}" = "force" ]; then
    echo "::warning::EnvForge: untrusted CI trigger detected (or --quarantine=force); scrubbing secrets"

    # Build allow-key flags from comma/newline-separated input
    local allow_flags=""
    if [ -n "${ALLOW_KEYS}" ]; then
      local _ifs="${IFS}"
      IFS=$',\n'
      for k in ${ALLOW_KEYS}; do
        k="${k// /}"  # strip whitespace
        [ -n "${k}" ] && allow_flags="${allow_flags} --allow-key ${k}"
      done
      IFS="${_ifs}"
    fi

    # Run quarantine; emitted lines install the scrubbed environment in this shell
    local force_flag=""
    [ "${QUARANTINE}" = "force" ] && force_flag="--force"
    eval "$(envforge ci-trust quarantine ${force_flag} ${allow_flags})"
  fi

  envforge ci-trust summary >> "${GITHUB_STEP_SUMMARY:-/dev/null}" 2>/dev/null || true
}

apply_ci_trust

# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------

# Export a KEY=VALUE to GITHUB_ENV using heredoc delimiter for multiline safety
export_var() {
  local key="$1"
  local value="$2"

  if [ "${MASK_VALUES}" = "true" ] && [ -n "${value}" ]; then
    echo "::add-mask::${value}"
  fi

  # Use heredoc delimiter for values that may contain newlines or special chars
  {
    echo "${key}<<ENVFORGE_EOF"
    echo "${value}"
    echo "ENVFORGE_EOF"
  } >> "${GITHUB_ENV}"
}

# --------------------------------------------------------------------------
# Mode: validate
# --------------------------------------------------------------------------
mode_validate() {
  echo "::group::EnvForge: Validating environment"

  local -a args=("envforge" "validate" "--json")

  if [ -n "${SCHEMA}" ]; then
    args+=("--schema" "${SCHEMA}")
  fi
  if [ -n "${ENV_FILE}" ]; then
    args+=("--env" "${ENV_FILE}")
  fi
  if [ -n "${ENVIRONMENT}" ]; then
    args+=("--environment" "${ENVIRONMENT}")
  fi

  echo "Running: ${args[*]}"

  local output
  local rc=0
  output=$("${args[@]}" 2>&1) || rc=$?

  if [ "${rc}" -eq 0 ]; then
    echo "validation-result=pass" >> "${GITHUB_OUTPUT}"
    echo "Validation passed"
  else
    echo "validation-result=fail" >> "${GITHUB_OUTPUT}"
    echo "::error::Validation failed"
    echo "${output}"
    echo "::endgroup::"
    exit 1
  fi

  echo "::endgroup::"
}

# --------------------------------------------------------------------------
# Mode: secrets-pull
# --------------------------------------------------------------------------
mode_secrets_pull() {
  echo "::group::EnvForge: Pulling secrets from ${PROVIDER}"

  if [ -z "${PROVIDER}" ]; then
    echo "::error::Input 'provider' is required for secrets-pull mode"
    exit 1
  fi

  local -a args=("envforge" "secrets" "pull" "--from" "${PROVIDER}" "--json")

  if [ -n "${PROVIDER_PATH}" ]; then
    args+=("--path" "${PROVIDER_PATH}")
  fi
  if [ -n "${FILTER}" ]; then
    args+=("--filter" "${FILTER}")
  fi

  echo "Running: ${args[*]}"
  local output
  output=$("${args[@]}" 2>&1)

  # Extract total count from JSON
  local total
  total=$(echo "${output}" | jq -r '.total // 0')
  echo "count=${total}" >> "${GITHUB_OUTPUT}"
  echo "variables=${output}" >> "${GITHUB_OUTPUT}"

  # Export to GITHUB_ENV if requested
  if [ "${EXPORT_ENV}" = "true" ]; then
    # Re-run without --json to get human-readable output with key names
    # Then use envforge export to get KEY=VALUE pairs
    local -a export_args=("envforge" "secrets" "pull" "--from" "${PROVIDER}")
    if [ -n "${PROVIDER_PATH}" ]; then
      export_args+=("--path" "${PROVIDER_PATH}")
    fi
    if [ -n "${FILTER}" ]; then
      export_args+=("--filter" "${FILTER}")
    fi

    local export_output
    export_output=$("${export_args[@]}" 2>&1) || true

    # Parse "  + KEY" lines from pull output and resolve via envforge get
    while IFS= read -r line; do
      if [[ "${line}" =~ ^[[:space:]]*\+[[:space:]]+([A-Za-z_][A-Za-z0-9_]*) ]]; then
        local key="${BASH_REMATCH[1]}"
        local value
        value=$(envforge get "${key}" 2>/dev/null) || continue
        export_var "${key}" "${value}"
      fi
    done <<< "${export_output}"
  fi

  echo "Pulled ${total} secrets from ${PROVIDER}"
  echo "::endgroup::"
}

# --------------------------------------------------------------------------
# Mode: export
# --------------------------------------------------------------------------
mode_export() {
  echo "::group::EnvForge: Exporting environment variables"

  local -a args=("envforge" "export")

  if [ -n "${FILTER}" ]; then
    args+=("--filter" "${FILTER}")
  fi

  echo "Running: ${args[*]}"
  local output
  output=$("${args[@]}" 2>&1)

  if [ "${EXPORT_ENV}" = "true" ]; then
    local count=0
    while IFS= read -r line; do
      # Skip empty lines and comments
      [[ -z "${line}" || "${line}" =~ ^# ]] && continue

      # Parse KEY=VALUE (handle quoted values)
      if [[ "${line}" =~ ^([A-Za-z_][A-Za-z0-9_]*)=(.*) ]]; then
        local key="${BASH_REMATCH[1]}"
        local value="${BASH_REMATCH[2]}"

        # Strip surrounding quotes
        if [[ "${value}" =~ ^\"(.*)\"$ ]] || [[ "${value}" =~ ^\'(.*)\'$ ]]; then
          value="${BASH_REMATCH[1]}"
        fi

        export_var "${key}" "${value}"
        count=$((count + 1))
      fi
    done <<< "${output}"

    echo "count=${count}" >> "${GITHUB_OUTPUT}"
    echo "Exported ${count} variables to GITHUB_ENV"
  else
    echo "${output}"
  fi

  echo "::endgroup::"
}

# --------------------------------------------------------------------------
# Mode: run
# --------------------------------------------------------------------------
mode_run() {
  echo "::group::EnvForge: Running command"

  if [ -z "${COMMAND}" ]; then
    echo "::error::Input 'command' is required for run mode"
    exit 1
  fi

  local -a args=("envforge" "run")

  if [ -n "${PROFILE}" ]; then
    args+=("--profile" "${PROFILE}")
  fi
  if [ "${RESOLVE_SECRETS}" = "true" ]; then
    args+=("--resolve")
  fi

  # Add env files
  if [ -n "${ENV_FILES}" ]; then
    while IFS= read -r ef; do
      ef="$(echo "${ef}" | xargs)"
      [ -z "${ef}" ] && continue
      args+=("--env-file" "${ef}")
    done <<< "$(echo "${ENV_FILES}" | tr ',' '\n')"
  fi

  # Add overrides
  if [ -n "${OVERRIDES}" ]; then
    while IFS= read -r ov; do
      ov="$(echo "${ov}" | xargs)"
      [ -z "${ov}" ] && continue
      args+=("--override" "${ov}")
    done <<< "$(echo "${OVERRIDES}" | tr ',' '\n')"
  fi

  args+=("--")
  # Split command string into words for proper passing
  read -ra cmd_parts <<< "${COMMAND}"
  args+=("${cmd_parts[@]}")

  echo "Running: ${args[*]}"
  echo "::endgroup::"

  # Run without set -e so we can capture exit code
  local rc=0
  "${args[@]}" || rc=$?
  exit ${rc}
}

# --------------------------------------------------------------------------
# Mode: drift
# --------------------------------------------------------------------------
mode_drift() {
  echo "::group::EnvForge: Checking environment drift"

  if [ -z "${DRIFT_ENVS}" ]; then
    echo "::error::Input 'drift-envs' is required for drift mode"
    exit 1
  fi

  local -a args=("envforge" "drift" "--json")

  if [ -n "${SCHEMA}" ]; then
    args+=("--schema" "${SCHEMA}")
  fi
  if [ -n "${ENVIRONMENT}" ]; then
    args+=("--environment" "${ENVIRONMENT}")
  fi

  # Add env files
  while IFS= read -r ef; do
    ef="$(echo "${ef}" | xargs)"
    [ -z "${ef}" ] && continue
    args+=("--envs" "${ef}")
  done <<< "$(echo "${DRIFT_ENVS}" | tr ',' '\n')"

  echo "Running: ${args[*]}"
  local output
  local rc=0
  output=$("${args[@]}" 2>&1) || rc=$?

  if [ "${rc}" -eq 0 ]; then
    echo "drift-result=clean" >> "${GITHUB_OUTPUT}"
    echo "No drift detected"
  else
    echo "drift-result=drift" >> "${GITHUB_OUTPUT}"
    echo "::warning::Environment drift detected"
    echo "${output}"
  fi

  echo "::endgroup::"
}

# --------------------------------------------------------------------------
# Main dispatch
# --------------------------------------------------------------------------
case "${MODE}" in
  validate)
    mode_validate
    ;;
  secrets-pull)
    mode_secrets_pull
    ;;
  export)
    mode_export
    ;;
  run)
    mode_run
    ;;
  drift)
    mode_drift
    ;;
  *)
    echo "::error::Unknown mode: ${MODE}. Supported: validate, secrets-pull, export, run, drift"
    exit 1
    ;;
esac
