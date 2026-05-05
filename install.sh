#!/usr/bin/env sh
set -eu

repo="${CODEXCTL_REPO:-https://github.com/ChaosRealmsAI/codexctl}"

if ! command -v cargo >/dev/null 2>&1; then
  printf '%s\n' "error: cargo was not found."
  printf '%s\n' "Install Rust first: https://www.rust-lang.org/tools/install"
  exit 1
fi

set -- --git "$repo" --force

if [ -n "${CODEXCTL_TAG:-}" ]; then
  set -- "$@" --tag "$CODEXCTL_TAG"
elif [ -n "${CODEXCTL_REV:-}" ]; then
  set -- "$@" --rev "$CODEXCTL_REV"
elif [ -n "${CODEXCTL_BRANCH:-}" ]; then
  set -- "$@" --branch "$CODEXCTL_BRANCH"
fi

cargo install "$@"
codexctl --version
printf '%s\n' "Run: codexctl doctor"
