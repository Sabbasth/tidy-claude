# tidy-claude

[![Rust CI](https://github.com/sabbasth/tidy-claude/actions/workflows/rust.yml/badge.svg)](https://github.com/sabbasth/tidy-claude/actions/workflows/rust.yml)

Backup, sync, and clean up [Claude Code](https://claude.ai/code) configuration across machines.

## Features

- **Backup / Restore** settings, memories, agents, CLAUDE.md, MCP servers, and skills across machines via git
- **Cleanup** old conversation logs and session files interactively or in bulk
- **Sync** everything in one command: pull, restore, install skills, backup, push

## Install

```bash
cargo install --git https://github.com/sabbasth/tidy-claude
```

Requires Rust **1.70+** (install via [rustup](https://rustup.rs) if needed).

> Upgrading from 1.x (Python)? `pipx uninstall tidy-claude` first, then run the
> command above. Your backup repo and `config.json` are byte-compatible and
> need no migration.

## Usage

```bash
tidy-claude                 # sync (default)
tidy-claude sync            # pull + restore + skills + backup + push
tidy-claude config          # show current configuration
tidy-claude cleanup         # interactive project picker + delete old sessions
```

Add `--debug` before any subcommand for verbose output.

### Cleanup

By default, `cleanup` opens an interactive menu where you select projects with arrow keys and space. Sessions older than 7 days in selected projects are deleted.

```bash
tidy-claude cleanup                        # interactive, default 7 days
tidy-claude cleanup --older-than 30        # only sessions older than 30 days
tidy-claude cleanup --older-than 0         # delete everything in selected projects
tidy-claude cleanup -a --dry-run           # preview what would be deleted across all projects
```

### What's synced

| Item | Source | Backed-up keys |
|------|--------|---------------|
| Memories | `~/.claude/memory/` | all files |
| Agents | `~/.claude/agents/` | all files |
| CLAUDE.md | `~/.claude/CLAUDE.md` | full file + `@`-referenced files |
| Settings | `~/.claude/settings.json` | `permissions`, `enabledPlugins`, `extraKnownMarketplaces` |
| MCP servers | `~/.claude.json` | `mcpServers` |
| Skills | `skills.json` (in repo) | manifest for `npx skills add` |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

See [SECURITY.md](SECURITY.md) for the threat model and disclosure policy.

## License

MIT
