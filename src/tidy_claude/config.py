"""Path constants and configuration keys."""

import json
from pathlib import Path

import platformdirs

HOME = Path.home()

# ── tidy-claude's own config ────────────────────────────────────────

CONFIG_DIR = Path(platformdirs.user_config_dir("tidy-claude"))
CONFIG_FILE = CONFIG_DIR / "config.json"
DEFAULT_DATA_DIR = Path(platformdirs.user_data_dir("tidy-claude"))

_config_cache: dict | None = None


def _default_config() -> dict:
    return {"data_dir": str(DEFAULT_DATA_DIR)}


def load_config() -> dict:
    """Return the current config, reading from disk on first call."""
    global _config_cache
    if _config_cache is not None:
        return _config_cache
    if CONFIG_FILE.exists():
        _config_cache = json.loads(CONFIG_FILE.read_text())
    else:
        _config_cache = _default_config()
    return _config_cache


def save_config(config: dict) -> None:
    """Write *config* to disk and update the cache."""
    global _config_cache
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    CONFIG_FILE.write_text(json.dumps(config, indent=2) + "\n")
    _config_cache = config


def ensure_config() -> dict:
    """Load config, creating the file with defaults if it doesn't exist."""
    config = load_config()
    if not CONFIG_FILE.exists():
        save_config(config)
    return config


def get_data_dir() -> Path:
    """Return the configured data directory (where backups live)."""
    return Path(load_config()["data_dir"])


# ── Claude Code paths ───────────────────────────────────────────────

CLAUDE_DIR = HOME / ".claude"
CLAUDE_JSON = HOME / ".claude.json"
SETTINGS_JSON = CLAUDE_DIR / "settings.json"

CLAUDE_JSON_KEYS = ["mcpServers"]
SETTINGS_JSON_KEYS = ["permissions", "enabledPlugins", "extraKnownMarketplaces"]

SETTINGS_JSON_DEFAULTS = {
    "autoMemoryDirectory": "~/.claude/memory",
}

CATEGORY_MAP = {"agents": "agents", "memory": "memories"}
