# emacs-i3 roadmap

The Rust implementation is the canonical `emacs-i3`. The roadmap is organized
around preserving one invariant: an Emacs operation is attempted only for an
Emacs-focused i3 node, and every unhandled/failed operation remains available
to i3 as a deterministic fallback.

## Implementation status — 2026-08-27

### Phase 1 — harden the 0.2 contract — implemented

- [x] Remove ordinary-input panic paths from Emacs protocol handling and i3
  node inspection.
- [x] Escape Elisp string metacharacters at the Rust/Emacs protocol boundary.
- [x] Cover floating nodes, tabbed/stacked fallback, nameless nodes, malformed
  server replies, and real Emacs Lisp dispatch through ERT.
- [x] Require formatting, Rust tests, warning-free Clippy, and ERT as the core
  local gate.

### Phase 2 — explicit runtime and diagnostics — implemented

- [x] Add `--socket`, `EMACS_I3_SOCKET`, and deterministic socket precedence.
- [x] Bound Unix-socket connect, read, and write operations with a configurable
  timeout (`--timeout-ms` / `EMACS_I3_TIMEOUT_MS`).
- [x] Add `-v`/`-vv` route diagnostics without changing quiet default output.
- [x] Add read-only human and JSON `--diagnose` modes.
- [x] Move from the broken historical `i3ipc` parser to the crates.io
  `i3ipc-jl` continuation, which retains modern window properties such as
  `machine` instead of discarding the complete property map.

### Phase 3 — typed commands and configuration — implemented

- [x] Parse focus/move/resize/layout/split/kill into typed operations while
  preserving the normalized unknown command unchanged as i3 fallback input.
- [x] Add strict TOML configuration for aliases, Emacs class/name matching,
  socket selection, timeout, and tabbed-horizontal fallback policy.
- [x] Preserve zero-config 0.2.x behavior.
- [x] Add transitive exact aliases with cycle rejection.
- [x] Add `--print-effective-config` and shell completion generation.

### Phase 4 — isolated desktop qualification — implemented locally

- [x] Run a private Xvfb + i3 + Emacs desktop with independent IPC sockets.
- [x] Verify internal Emacs focus and edge fallback in both directions.
- [x] Verify split, tabbed, stacked, floating, and multi-frame semantics in the
  isolated desktop lane, with minibuffer inclusion/exclusion covered by ERT.
- [x] Verify diagnostics without `DISPLAY`, missing `XDG_RUNTIME_DIR`, stale
  sockets, and a server that accepts but never responds.
- [x] Enforce a bounded fallback latency assertion; the local qualification
  observed the 50 ms timeout path completing in tens of milliseconds.

### Phase 5 — packaging and lifecycle — implementation complete

- [x] Move to Rust edition 2024 and Clap 4 with a declared Rust 1.85 MSRV.
- [x] Add a CI guard that compiles all targets with the declared MSRV.
- [x] Keep Cargo dependencies registry-pinned by `Cargo.lock` and make the Nix
  package consume the same lockfile without ad-hoc Git output hashes.
- [x] Install the Lisp dispatcher alongside the Nix binary.
- [x] Add separate GitHub Actions core, desktop-E2E, and Nix package jobs.
- [x] Document one local binary installation path and scripting-shim migration.

The remaining release work is operational rather than implementation work:
observe the first remote CI/Nix run, choose the next SemVer version, update the
release section, tag, and publish only when explicitly authorized.

## Future candidates (not part of this implementation set)

- A resident daemon only if profiling shows process startup/IPC setup is a
  meaningful key-binding latency cost.
- Sway support only as an explicit backend with its own qualification matrix;
  accidental compatibility is not a support contract.
- Richer command policies only if they retain unknown-command fallthrough and
  do not turn Emacs into an implicit consumer of arbitrary i3 syntax.
