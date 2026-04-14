# Contributing

Thanks for your interest in tidy-claude!

## Setup

```bash
git clone https://github.com/sabbasth/tidy-claude.git
cd tidy-claude
uv sync --group dev
```

## Running tests

```bash
uv run pytest -v
```

## Project structure

```
src/tidy_claude/
  config.py    # Paths, config file loading
  helpers.py   # Pure functions (no side effects)
  ops.py       # Filesystem I/O, git, subprocess calls
  cli.py       # Click CLI commands
  state.py     # Mutable run-time state
```

## Guidelines

- Keep `helpers.py` free of side effects — pure functions only.
- Side-effectful code (file I/O, git, subprocesses) goes in `ops.py`.
- Add tests for new helpers in `tests/test_helpers.py`.
- Run `uv run pytest -v` before pushing.

## Pull requests

1. Fork the repo and create a branch from `main`.
2. Add tests if you're adding or changing behavior.
3. Make sure all tests pass.
4. Open a PR — CI will run tests and build automatically.
