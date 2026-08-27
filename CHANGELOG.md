# Changelog

All notable changes to this project are documented here.

## Unreleased

## 0.2.2 - 2026-08-27

### Fixed

- Refreshed the pinned `nixpkgs-unstable` flake input from its 2023 revision to
  a current 2026 revision so Rust crate fetching uses the modern
  `static.crates.io` path instead of the stale fetcher that received HTTP 403
  responses from crates.io.

## 0.2.1 - 2026-08-27

### Added

- Added Rust CLI integration tests, direct Unix-socket Emacs protocol tests, and
  batch ERT coverage for the Emacs Lisp dispatcher.
- Added a phased feature roadmap covering protocol hardening, diagnostics,
  typed command handling, isolated desktop E2E qualification, and packaging.
- Added bounded Emacs IPC with configurable socket and timeout overrides.
- Added strict TOML configuration for Emacs matching, exact command aliases,
  tabbed fallback policy, socket selection, and timeout defaults.
- Added human/JSON read-only diagnostics, verbose route tracing, resolved
  configuration output, and generated shell completions.
- Added an isolated Xvfb+i3+Emacs desktop qualification covering split, tabbed,
  stacked, floating, multi-frame, and fallback/timeout behavior.
- Added core, Rust-1.85-MSRV, desktop-E2E, and Nix CI lanes plus migration
  documentation.

### Fixed

- Treat malformed or truncated Emacs server responses as typed protocol errors
  instead of panicking.
- Detect focused floating i3 windows and tolerate nameless nodes without
  panicking during Emacs-window detection.
- Escape backslashes and control characters when embedding i3 commands in an
  Emacs Lisp string.
- Preserve modern i3 window properties such as `machine`; older `i3ipc 0.10.1`
  discarded the entire property map when that field was present, preventing
  real Emacs frames from being recognized.

### Changed

- Parse the supported command family into typed Rust operations while keeping
  unknown commands unchanged for i3 fallback.
- Modernized the project to Rust edition 2024, Clap 4, and the crates.io
  `i3ipc-jl` continuation with a Cargo.lock-based reproducible dependency set.
- Preserved the 0.2.x command-line compatibility contract: existing i3-style
  commands keep their shape and unknown/unhandled commands continue to fall
  through to i3 unchanged.
- Updated public repository/package metadata for the maintained fork while
  retaining the original upstream project in repository history and README
  attribution.

### Verification

- Qualified 21 Rust unit tests, 8 CLI integration tests, and 11 Emacs ERT tests,
  plus the isolated Xvfb+i3+Emacs desktop lane and locked release build.
- The desktop qualification covers handled Emacs focus, bidirectional i3 edge
  fallback, tabbed/stacked layouts, floating and multiple Emacs frames, and
  bounded missing/stale/hung Emacs-server behavior.

## 0.2.0 - 2026-08-22

### Added

- Added broader Emacs-side support for i3-style commands, including focus,
  move, resize, split, kill, and layout toggle handling.
- Added regression coverage for command normalization, Emacs server nil
  responses, and tabbed i3 fallback behavior.
- Documented supported commands and local installation steps.

### Fixed

- Fixed Super+h/Super+l fallback when Emacs is focused by correctly treating
  raw Emacs server `nil` responses with trailing newlines as unhandled.
- Fixed horizontal focus fallback from Emacs inside tabbed or stacked i3
  containers by using i3 tab order commands.
- Improved Emacs window handling around minibuffers, missing neighbors, and
  command errors so unsupported operations fall back to i3.
- Improved runtime error reporting instead of panicking on missing i3, Emacs, or
  command state.

### Changed

- Switched from the old `c0deaddict/i3ipc-rs` window-properties fork to
  upstream `tmerr/i3ipc-rs` master.
- The CLI version is now derived from Cargo package metadata.
