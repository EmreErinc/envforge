#!/usr/bin/env bash
set -euo pipefail

VERSION="${INPUT_VERSION:-latest}"
REPO="emreerinc/envforge"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)  TARGET_OS="unknown-linux-gnu" ;;
  Darwin) TARGET_OS="apple-darwin" ;;
  *)
    echo "::error::Unsupported OS: ${OS}"
    exit 1
    ;;
esac

case "${ARCH}" in
  x86_64)  TARGET_ARCH="x86_64" ;;
  aarch64|arm64) TARGET_ARCH="aarch64" ;;
  *)
    echo "::error::Unsupported architecture: ${ARCH}"
    exit 1
    ;;
esac

TARGET="${TARGET_ARCH}-${TARGET_OS}"

# Resolve version
if [ "${VERSION}" = "latest" ]; then
  echo "::group::Resolving latest EnvForge version"
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
  echo "Resolved: v${VERSION}"
  echo "::endgroup::"
fi

# Strip leading 'v' if present
VERSION="${VERSION#v}"

ARCHIVE="envforge-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE}"

echo "::group::Installing EnvForge v${VERSION} (${TARGET})"
echo "Downloading from: ${URL}"

INSTALL_DIR="${HOME}/.envforge/bin"
mkdir -p "${INSTALL_DIR}"

# Download and extract
curl -fsSL "${URL}" -o "/tmp/${ARCHIVE}"
tar xzf "/tmp/${ARCHIVE}" -C "${INSTALL_DIR}"
chmod +x "${INSTALL_DIR}/envforge"
rm -f "/tmp/${ARCHIVE}"

# Add to PATH
echo "${INSTALL_DIR}" >> "${GITHUB_PATH}"
export PATH="${INSTALL_DIR}:${PATH}"

# Verify installation
envforge --version
echo "::endgroup::"
