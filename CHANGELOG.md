# Changelog

All notable changes to `tidy-claude` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] — 2026-04-23

### Changed
- **Full rewrite in Rust.** The binary is now a statically-linked Rust
  executable installable via `cargo install --git`.
- Configuration format (`config.json`), backup tree layout, and CLI surface
  are **byte-compatible** with 1.x: existing backup repos continue to work
  without migration.
- `serde_json` is compiled with the `preserve_order` feature so JSON keys
  are kept in insertion order — no spurious diffs when backups travel
  between machines with different implementations.

### Added
- `cargo install --git https://github.com/sabbasth/tidy-claude` for
  distribution.
- GitHub Actions CI: rustfmt, clippy (`-D warnings`), tarpaulin coverage,
  and release builds on macOS (Apple Silicon) + Linux (x86_64).
- `cargo audit` step in CI.
- Integration tests using `assert_cmd` and `insta` snapshots for the
  backup tree structure.
- Parity shell script (`tests/parity.sh`) validating CLI behaviour.
- `status` subcommand showing the backup repo's git status.

### Removed
- **Python implementation** (`src/tidy_claude/`, `pyproject.toml`, `uv.lock`,
  `tests/*.py`). Users on 1.x should uninstall via `pipx uninstall
  tidy-claude` before installing 2.0 with `cargo install`.

### Security
- External `sh -c <cmd>` invocation for skill installation is preserved at
  parity with 1.x. The commands come from `skills.json` inside the
  user-controlled backup repo; do not sync from untrusted remotes. See
  [SECURITY.md](SECURITY.md).

## [1.0.0] — 2026-04 (Python)

Initial public release. Python 3.12 implementation using Click,
simple-term-menu, and platformdirs.
