"""Path constants and configuration keys."""

from pathlib import Path

HOME = Path.home()
BACKUP_DIR = Path(__file__).resolve().parent.parent.parent
CLAUDE_DIR = HOME / ".claude"
CLAUDE_JSON = HOME / ".claude.json"
SETTINGS_JSON = CLAUDE_DIR / "settings.json"

CLAUDE_JSON_KEYS = ["mcpServers"]
SETTINGS_JSON_KEYS = ["permissions", "enabledPlugins", "extraKnownMarketplaces"]

SETTINGS_JSON_DEFAULTS = {
    "autoMemoryDirectory": "~/.claude/memory",
}

CATEGORY_MAP = {"agents": "agents", "memory": "memories"}
