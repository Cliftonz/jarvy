# Changelog

All notable changes to the Jarvy VS Code extension are documented here. The
format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Initial release (PRD-017 Phase 3).
- Live validation of `jarvy.toml` via `jarvy validate --strict --file <path> --format json`,
  publishing diagnostics into the Problems panel on open, save, and (debounced) change.
- Line-accurate diagnostics when jarvy reports a line number; file-level fallback otherwise.
- Status bar item showing valid / invalid / no-config state.
- Commands: `jarvy.validate`, `jarvy.setup` (integrated terminal), `jarvy.doctor` (output channel).
- Quick Fix offering *Run jarvy setup* on "Unknown tool" diagnostics.
- `jarvy-toml` language contribution with lightweight TOML syntax highlighting.
- Graceful handling when the `jarvy` binary is missing: a warning with a link to install docs.
- Settings: `jarvy.executablePath`, `jarvy.validate.onSave`, `jarvy.validate.onChange`,
  `jarvy.validate.strict`, `jarvy.validate.debounceMs`.
