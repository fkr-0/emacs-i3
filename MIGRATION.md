# Migration to the canonical Rust emacs-i3

This Rust implementation supersedes older shell-wrapper installations of
`emacs-i3` while preserving the familiar i3-style command contract.

## What stays compatible

- Existing i3 bindings such as `emacs-i3 focus left` keep the same command
  shape.
- When an Emacs frame is focused, Emacs gets the first opportunity to handle a
  supported operation.
- If Emacs returns `nil`, cannot be reached, times out, or does not own the
  focused i3 node, the original command is sent to i3.
- Unknown i3 commands continue to pass through unchanged.

The Rust implementation additionally supports move, resize, split, kill, and
layout operations inside Emacs, direct i3-tree inspection, bounded Emacs IPC,
configuration, diagnostics, and shell completions.

## Replace an older installation

From the Rust checkout:

```bash
cargo install --path . --locked --root "$HOME/.local" --force
command -v emacs-i3
emacs-i3 --version
emacs-i3 --diagnose
```

The first command intentionally replaces an old `~/.local/bin/emacs-i3`
artifact or symlink. It does not modify i3 configuration.

Make sure Emacs loads the Rust project's Lisp dispatcher:

```elisp
(add-to-list 'load-path "/path/to/emacs-i3/elisp")
(require 'emacs-i3)
```

## New optional runtime controls

No configuration file is required. When needed, the new controls are:

- `--socket` / `EMACS_I3_SOCKET` for an Emacs server socket;
- `--timeout-ms` / `EMACS_I3_TIMEOUT_MS` for bounded Emacs IPC;
- `--config` / `EMACS_I3_CONFIG` for an alternate TOML configuration file;
- `--diagnose [--json]` for read-only runtime inspection;
- `-v` or `-vv` for routing diagnostics;
- `--print-effective-config` for the resolved configuration;
- `--generate-completion <shell>` for completion source.

The default server remains `$XDG_RUNTIME_DIR/emacs/server`, matching the
historical Rust behavior.

## Qualification before changing bindings

Run both local lanes:

```bash
bash tests/run.sh
bash tests/e2e-desktop.sh
```

The desktop test is isolated from the current i3 session and creates temporary
Xvfb/i3/Emacs instances under `target/`.
