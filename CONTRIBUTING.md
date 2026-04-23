# Contributing

Thanks for your interest in tidy-claude!

## Setup

```bash
git clone https://github.com/sabbasth/tidy-claude.git
cd tidy-claude
cargo build
```

Requires Rust **1.70+** (edition 2021). Install via [rustup](https://rustup.rs) if needed.

## Running tests

```bash
cargo test                                  # all tests (unit + integration)
cargo test --lib                            # unit tests only
cargo test --test cli_integration           # CLI subprocess tests
cargo test --test backup_parity             # backup/restore parity
./tests/parity.sh                           # smoke test of built binary
```

## Linting & formatting

```bash
cargo fmt --all -- --check                  # rustfmt (CI enforces)
cargo clippy --all-targets -- -D warnings   # clippy lints (CI enforces)
cargo audit                                 # security advisories
```

## Project structure

```shell
src/
  config.rs    # Paths, config file loading, constants
  helpers.rs   # Pure functions (no side effects)
  ops.rs       # Filesystem I/O, git, subprocess, cleanup
  cli.rs       # clap CLI commands
  state.rs     # Mutable run-time state
  lib.rs       # crate root
  main.rs      # binary entry point
tests/
  backup_parity.rs    # backup/restore round-trip + insta snapshot
  cli_integration.rs  # subprocess tests with assert_cmd
  parity.sh           # smoke test (config + sync guard)
  snapshots/          # insta golden files
```

## Guidelines

- Keep `helpers.rs` free of side effects — pure functions only.
- Side-effectful code (file I/O, git, subprocess) goes in `ops.rs`.
- Add unit tests in the `mod tests` block of the file you're modifying.
- Cross-cutting behaviour (backup format, CLI contract) goes in `tests/`.
- `serde_json` is compiled with `preserve_order` — keep it that way to avoid
  noisy diffs in the backup repo shared across machines.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` before pushing.

## Pull requests

1. Fork the repo and create a branch from `main`.
2. Add tests if you're adding or changing behaviour.
3. Make sure `cargo test`, `cargo clippy`, and `cargo fmt --check` all pass.
4. Open a PR — CI will run the full matrix (macOS + Linux) automatically.

## Security

If you discover a vulnerability, please see [SECURITY.md](SECURITY.md) for
responsible disclosure.
