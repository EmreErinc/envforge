#!/usr/bin/env bash
# Populate assets/sigstore-trust-root.json with the current Sigstore TUF root.
#
# RUN BEFORE EVERY RELEASE that touches the envbom airgap path. The bundled
# placeholder (`assets/sigstore-trust-root.json`) is NOT usable for verifying
# real Sigstore-signed bundles — it must be replaced with current Fulcio root
# certs + Rekor public key from the official Sigstore TUF metadata.
#
# Sigstore TUF root reference:
#   https://github.com/sigstore/root-signing
#   https://tuf-repo-cdn.sigstore.dev/
#
# This script is intentionally hand-curated rather than fully automated: TUF
# metadata changes infrequently and changes should be reviewed by a human
# (release manager) before being committed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ASSET="${ROOT}/assets/sigstore-trust-root.json"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "EnvForge trust-root updater"
echo "==========================="
echo
echo "This script downloads the current Sigstore Fulcio root cert chain and"
echo "Rekor public key, then writes them to:"
echo "  ${ASSET}"
echo
echo "Source: https://github.com/sigstore/root-signing"
echo

if ! command -v cosign >/dev/null 2>&1; then
    echo "ERROR: cosign not found on PATH. Install via:"
    echo "  brew install cosign  # macOS"
    echo "  go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
    exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: jq not found on PATH. Install via:"
    echo "  brew install jq  # macOS"
    exit 1
fi

# Step 1: Fetch the Sigstore TUF root via cosign.
# `cosign initialize` populates ~/.sigstore/root with the latest TUF root.
echo "[1/3] Fetching Sigstore TUF root via cosign..."
cosign initialize 2>&1 | tail -5

SIGSTORE_DIR="${HOME}/.sigstore/root"
if [ ! -d "${SIGSTORE_DIR}" ]; then
    echo "ERROR: cosign did not populate ${SIGSTORE_DIR}"
    exit 1
fi

# Step 2: Extract Fulcio root cert + Rekor public key.
# Sigstore's TUF metadata stores these as targets.
echo "[2/3] Extracting Fulcio root + Rekor public key..."

FULCIO_PEM_FILE="${SIGSTORE_DIR}/targets/fulcio_v1.crt.pem"
REKOR_PEM_FILE="${SIGSTORE_DIR}/targets/rekor.pub"

if [ ! -f "${FULCIO_PEM_FILE}" ]; then
    echo "ERROR: Fulcio root cert not found at ${FULCIO_PEM_FILE}"
    echo "Sigstore TUF target layout may have changed. Inspect ${SIGSTORE_DIR}/targets/"
    exit 1
fi
if [ ! -f "${REKOR_PEM_FILE}" ]; then
    echo "ERROR: Rekor public key not found at ${REKOR_PEM_FILE}"
    exit 1
fi

FULCIO_PEM="$(cat "${FULCIO_PEM_FILE}")"
REKOR_PEM="$(cat "${REKOR_PEM_FILE}")"
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Step 3: Build the trust-root JSON with jq (handles escaping safely).
echo "[3/3] Writing ${ASSET}..."
jq -n \
    --arg fulcio "${FULCIO_PEM}" \
    --arg rekor "${REKOR_PEM}" \
    --arg ts "${NOW}" \
    '{
        fulcio_root_pem: $fulcio,
        rekor_pubkey_pem: $rekor,
        bundled_at: $ts,
        source: "Bundled"
    }' > "${ASSET}.tmp"

mv "${ASSET}.tmp" "${ASSET}"

echo
echo "✓ Updated ${ASSET}"
echo "  bundled_at: ${NOW}"
echo
echo "Next steps:"
echo "  1. Review the diff: git diff -- ${ASSET}"
echo "  2. Run airgap test: cargo test --features sigstore --lib envbom::airgap"
echo "  3. Commit: git add ${ASSET} && git commit -m 'chore: refresh Sigstore trust root'"
