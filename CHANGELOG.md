# Changelog

All notable changes to this project are documented here.

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
