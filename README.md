# Emacs i3 unified window management

`emacs-i3` makes Emacs window navigation and i3 container navigation behave as
one hierarchy. An i3-style command is first offered to Emacs when the focused
i3 node is an Emacs frame. If Emacs cannot handle it, the original command is
sent to i3. Unknown commands are deliberately preserved as an escape hatch to
the window manager.

Inspired by https://sqrtminusone.xyz/posts/2021-10-04-emacs-i3/.

This repository is maintained as a fork of
https://github.com/c0deaddict/emacs-i3. The fork keeps upstream history and
focuses on hardened Emacs/i3 routing, diagnostics, qualification, and current
packaging while preserving unknown-command fallback to i3.

## Install

The supported local installation path is Cargo into `~/.local/bin`:

```bash
cargo install --path . --locked --root "$HOME/.local" --force
```

If an older shell-wrapper or other local `emacs-i3` installation is still
present, the `--force` install replaces that binary/symlink with this Rust
implementation.
See [`MIGRATION.md`](MIGRATION.md) for the compatibility notes.

Load the accompanying Emacs Lisp from this checkout (or install it on your
Emacs `load-path`) and require it before using the i3 bindings:

```elisp
(add-to-list 'load-path "/path/to/emacs-i3/elisp")
(require 'emacs-i3)
```

The Nix flake also exposes `packages.<system>.emacs-i3` and installs
`emacs-i3.el` under `share/emacs/site-lisp`.

## Usage

Typical i3 bindings are unchanged:

```text
bindsym Mod4+h exec emacs-i3 focus left
bindsym Mod4+j exec emacs-i3 focus down
bindsym Mod4+k exec emacs-i3 focus up
bindsym Mod4+l exec emacs-i3 focus right
```

Supported Emacs-side operations include:

```bash
emacs-i3 focus left
emacs-i3 move right
emacs-i3 resize grow width 10 px
emacs-i3 layout toggle split
emacs-i3 split h
emacs-i3 kill
```

Commands not recognized by the Emacs dispatcher, for example
`emacs-i3 workspace next`, fall through to i3 unchanged.

## Runtime configuration

Zero configuration preserves the 0.2.x behavior. Optional configuration is
read from `$XDG_CONFIG_HOME/emacs-i3/config.toml`, or
`~/.config/emacs-i3/config.toml` when `XDG_CONFIG_HOME` is unset:

```toml
timeout_ms = 250
emacs_classes = ["Emacs"]
emacs_name_prefixes = ["emacs: "]
tabbed_horizontal_focus = true

[aliases]
"go west" = "focus left"
```

The optional `socket` key can select a non-default Emacs server socket. Command
aliases are exact command aliases and may chain; cycles are rejected.

Runtime precedence is explicit and deterministic:

- config file: `--config` > `EMACS_I3_CONFIG` > default XDG path;
- socket: `--socket` > `EMACS_I3_SOCKET` > config `socket` >
  `$XDG_RUNTIME_DIR/emacs/server`;
- timeout: `--timeout-ms` > `EMACS_I3_TIMEOUT_MS` > config `timeout_ms`.

Inspect the fully resolved values without changing focus:

```bash
emacs-i3 --print-effective-config
emacs-i3 --diagnose
emacs-i3 --diagnose --json
```

`-v` explains routing decisions on stderr; `-vv` also prints resolved runtime
configuration. Emacs IPC connect/read/write operations are bounded by the
configured timeout so a stale or hung server cannot indefinitely block an i3
key binding.

Generate shell completion source with, for example:

```bash
emacs-i3 --generate-completion zsh > _emacs-i3
emacs-i3 --generate-completion bash > emacs-i3.bash
```

## Tests

The core suite runs Rust unit/integration tests, formatting, warning-free
Clippy, shell syntax checks, and the real Emacs Lisp dispatcher under ERT:

```bash
bash tests/run.sh
```

The desktop qualification creates its own Xvfb display, i3 IPC socket, Emacs
daemon/GUI frames, and neighboring X client. It does not use the active desktop:

```bash
bash tests/e2e-desktop.sh
```

That lane verifies Emacs-internal focus, i3 edge fallback in both directions,
tabbed and stacked fallback, floating and multi-frame detection, headless
diagnostics, and bounded missing/stale/hung Emacs-server behavior.

The same gates are exposed through `bridge.yml` and split into core, desktop,
and Nix jobs in GitHub Actions.
