"""Shared fixtures for tidy-claude tests."""

import json
from pathlib import Path

import pytest


@pytest.fixture()
def claude_tree(tmp_path: Path):
    """Build a minimal ~/.claude-like tree and return its root."""
    claude_dir = tmp_path / ".claude"
    claude_dir.mkdir()

    # CLAUDE.md referencing two files
    (claude_dir / "CLAUDE.md").write_text("See @tips.md and @missing.md\n")
    (claude_dir / "tips.md").write_text("some tips\n")
    # missing.md intentionally absent

    # memory/
    mem = claude_dir / "memory"
    mem.mkdir()
    (mem / "MEMORY.md").write_text("- [A](a.md)\n")
    (mem / "a.md").write_text("memory a\n")

    return claude_dir


@pytest.fixture()
def backup_tree(tmp_path: Path):
    """Build a minimal backup repo tree."""
    backup = tmp_path / "backup"
    backup.mkdir()
    claude = backup / "claude"
    claude.mkdir()

    (claude / "claude.json").write_text(
        json.dumps({"mcpServers": {}}, indent=2) + "\n"
    )
    (claude / "settings.json").write_text(
        json.dumps({"permissions": {"allow": []}}, indent=2) + "\n"
    )

    return backup
