# tidy-claude

Backup, sync, and clean up [Claude Code](https://claude.ai/code) configuration across machines.

## Features

- **Backup / Restore** settings, memories, agents, CLAUDE.md, MCP servers, and skills across machines via git
- **Cleanup** old conversation logs and session files interactively or in bulk
- **Sync** everything in one command: pull, restore, install skills, backup, push

## Install

```bash
uv tool install tidy-claude         # or: pipx install tidy-claude
```

Or from source:

```bash
git clone https://github.com/sabbasth/tidy-claude.git
cd tidy-claude
uv sync
```

## Usage

```bash
tidy-claude backup          # copy live config to repo (default)
tidy-claude restore         # restore config from repo to live locations
tidy-claude sync            # pull + restore + skills + backup + push
tidy-claude skills          # install missing skills from skills.json
tidy-claude status          # git status of the config repo
tidy-claude push            # backup + commit + push
tidy-claude cleanup         # interactive project picker + delete old sessions
tidy-claude cleanup -a      # clean all projects (non-interactive)
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

## Development

```bash
uv sync --group dev
uv run pytest -v
```

## License

MIT
