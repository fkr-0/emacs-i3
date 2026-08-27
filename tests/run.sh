#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
bash -n tests/e2e-desktop.sh

if command -v emacs >/dev/null 2>&1; then
  emacs --batch -Q \
    -L "$repo_root/elisp" \
    -l "$repo_root/tests/emacs-i3-tests.el" \
    -f ert-run-tests-batch-and-exit
else
  printf '%s\n' 'error: Emacs is required for the ERT portion of the test suite' >&2
  exit 1
fi
