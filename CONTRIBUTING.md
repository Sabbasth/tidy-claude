# Contributing

Thanks for your interest in tidy-claude!

## Setup

```bash
git clone https://github.com/sabbasth/tidy-claude.git
cd tidy-claude
cargo build
```

## Running tests

```bash
cargo test
```

## Project structure

```
src/
  main.rs      # Entry point
  cli.rs       # Clap CLI commands
  config.rs    # Paths, config file loading
  helpers.rs   # Pure functions (no side effects)
  ops.rs       # Filesystem I/O, git, subprocess calls
  state.rs     # Mutable run-time state
  error.rs     # Unified error type (thiserror)
tests/
  cli_integration.rs  # E2E tests with assert_cmd
```

## Guidelines

- Keep `helpers.rs` free of side effects — pure functions only.
- Side-effectful code (file I/O, git, subprocesses) goes in `ops.rs`.
- Add tests for new helpers as `#[cfg(test)]` inline tests in `helpers.rs`.
- Run `cargo test` before pushing.

## Pull requests

1. Fork the repo and create a branch from `main`.
2. Add tests if you're adding or changing behavior.
3. Make sure all tests pass.
4. Open a PR — CI will run tests and build automatically.
