#!/usr/bin/env bash
# EnvForge local install / update — builds the CLI + IDE plugins from this repo
# and installs them onto this machine. Idempotent: re-run any time to update.
#
#   ./scripts/envforge-install.sh            # install/update everything detected
#   ./scripts/envforge-install.sh app        # CLI only
#   ./scripts/envforge-install.sh brew       # Homebrew formula test only
#   ./scripts/envforge-install.sh vscode     # VS Code extension only
#   ./scripts/envforge-install.sh intellij   # IntelliJ plugin only
#   ./scripts/envforge-install.sh zed        # Zed (prints dev-extension steps)
#
# Each component is independent: one failing does not abort the others.

set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
skip() { printf '  \033[33m–\033[0m %s\n' "$*"; }
fail() { printf '  \033[31m✗\033[0m %s\n' "$*"; }

WANT="${1:-all}"
want() { [ "$WANT" = "all" ] || [ "$WANT" = "$1" ]; }

# ---------------------------------------------------------------- App (CLI/LSP)
install_app() {
  bold "==> envforge CLI + LSP"
  if ! command -v cargo >/dev/null; then fail "cargo not found — install Rust"; return 1; fi
  if cargo install --path "$REPO" --force; then
    ok "installed $(envforge --version 2>/dev/null) → $(command -v envforge)"
  else fail "cargo install failed"; return 1; fi
}

# ----------------------------------------------------------------- VS Code ext
install_vscode() {
  bold "==> VS Code extension"
  if ! command -v code >/dev/null; then skip "no 'code' CLI on PATH — skipping"; return 0; fi
  ( cd "$REPO/editors/vscode" \
    && npm install --no-audit --no-fund >/dev/null 2>&1 \
    && npm run bundle >/dev/null 2>&1 \
    && npx --yes @vscode/vsce package --no-dependencies -o envforge.vsix >/dev/null 2>&1 \
    && code --install-extension envforge.vsix --force ) \
    && ok "installed VS Code extension (reload window: Cmd+Shift+P → Reload Window)" \
    || { fail "VS Code extension build/install failed"; return 1; }
}

# ---------------------------------------------------------------- IntelliJ plugin
install_intellij() {
  bold "==> IntelliJ plugin"
  local base="$HOME/Library/Application Support/JetBrains"
  [ -d "$base" ] || { skip "no JetBrains config dir — skipping"; return 0; }
  bold "    building (gradle downloads the IDE SDK on first run — may take a while)"
  if ! ( cd "$REPO/editors/intellij" && ./gradlew buildPlugin -q ); then
    fail "gradle buildPlugin failed (network/JDK?) — skipping install"; return 1
  fi
  local zip; zip="$(ls -t "$REPO"/editors/intellij/build/distributions/*.zip 2>/dev/null | head -1)"
  [ -n "$zip" ] || { fail "no plugin zip produced"; return 1; }
  # latest IntelliJIdea config dir (per install convention)
  local idea; idea="$(ls -d "$base"/IntelliJIdea* 2>/dev/null | sort -V | tail -1)"
  [ -n "$idea" ] || { skip "no IntelliJIdea config dir found"; return 0; }
  mkdir -p "$idea/plugins"
  rm -rf "$idea/plugins/envforge-intellij"
  unzip -oq "$zip" -d "$idea/plugins" \
    && ok "installed → $idea/plugins/envforge-intellij (restart IntelliJ)" \
    || { fail "unzip into plugins dir failed"; return 1; }
}

# ----------------------------------------------------------------- Neovim plugin
install_nvim() {
  bold "==> Neovim plugin"
  if ! command -v nvim >/dev/null; then skip "nvim not installed — skipping"; return 0; fi
  local dst="$HOME/.local/share/nvim/site/pack/envforge/start/envforge"
  mkdir -p "$(dirname "$dst")"
  rm -rf "$dst"; cp -R "$REPO/editors/nvim" "$dst" \
    && ok "installed → $dst (add: require('envforge').setup() to init.lua)" \
    || { fail "copy failed"; return 1; }
}

# --------------------------------------------------------------------- Zed ext
install_zed() {
  bold "==> Zed extension"
  [ -d /Applications/Zed.app ] || { skip "Zed.app not found — skipping"; return 0; }
  rustup target add wasm32-wasip1 >/dev/null 2>&1 || true
  skip "Zed dev extensions install via the GUI (no stable CLI flag):"
  echo "      1) Open Zed → Cmd+Shift+X (Extensions)"
  echo "      2) 'Install Dev Extension' → select: $REPO/editors/zed"
  echo "      (requires rustup wasm32-wasip1 target — added above if rustup present)"
}

# ---------------------------------------------------------------- Homebrew formula
install_brew() {
  bold "==> Homebrew formula"
  if ! command -v brew >/dev/null; then skip "brew CLI not installed — skipping"; return 0; fi
  local formula_path="$REPO/Formula/envforge.rb"
  if [ -f "$formula_path" ]; then
    ok "Formula present → $formula_path ($(brew --version 2>/dev/null | head -1))"
  else
    fail "Formula not found at $formula_path"
  fi
}

run() { want "$1" && { "install_$1"; echo; }; }
run app
run brew
run vscode
run intellij
run nvim
run zed
bold "Done. Restart/reload each IDE to pick up the new plugin."
