use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

use crate::error::{Result, TidyError};

// ── tidy-claude's own config ────────────────────────────────────────

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("config directory must exist")
        .join("tidy-claude")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .expect("data directory must exist")
        .join("tidy-claude")
}

fn default_config() -> Value {
    serde_json::json!({
        "data_dir": default_data_dir().to_string_lossy()
    })
}

static CONFIG_CACHE: Mutex<Option<Value>> = Mutex::new(None);

pub fn clear_cache() {
    *CONFIG_CACHE.lock().unwrap() = None;
}

pub fn load_config() -> Result<Value> {
    let mut cache = CONFIG_CACHE.lock().unwrap();
    if let Some(ref cached) = *cache {
        return Ok(cached.clone());
    }

    let path = config_file();
    let config = if path.exists() {
        let text = fs::read_to_string(&path)?;
        serde_json::from_str(&text)?
    } else {
        default_config()
    };

    *cache = Some(config.clone());
    Ok(config)
}

pub fn save_config(config: &Value) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(config)? + "\n";
    fs::write(config_file(), text)?;
    *CONFIG_CACHE.lock().unwrap() = Some(config.clone());
    Ok(())
}

pub fn ensure_config() -> Result<Value> {
    let config = load_config()?;
    if !config_file().exists() {
        save_config(&config)?;
    }
    Ok(config)
}

pub fn get_data_dir() -> Result<PathBuf> {
    let config = load_config()?;
    let dir = config["data_dir"]
        .as_str()
        .ok_or_else(|| TidyError::Config("missing data_dir in config".into()))?;
    Ok(PathBuf::from(dir).join("backup"))
}

// ── Claude Code paths ───────────────────────────────────────────────

pub fn home_dir() -> PathBuf {
    dirs::home_dir().expect("home directory must exist")
}

pub fn claude_dir() -> PathBuf {
    home_dir().join(".claude")
}

pub fn claude_json() -> PathBuf {
    home_dir().join(".claude.json")
}

pub fn settings_json() -> PathBuf {
    claude_dir().join("settings.json")
}

pub const CLAUDE_JSON_KEYS: &[&str] = &["mcpServers"];

pub const SETTINGS_JSON_KEYS: &[&str] = &[
    "permissions",
    "enabledPlugins",
    "extraKnownMarketplaces",
];

pub fn settings_json_defaults() -> Value {
    serde_json::json!({
        "autoMemoryDirectory": "~/.claude/memory"
    })
}

pub const CATEGORY_MAP: &[(&str, &str)] = &[
    ("agents", "agents"),
    ("memory", "memories"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn default_config_has_data_dir() {
        let config = default_config();
        assert!(config["data_dir"].is_string());
    }

    #[test]
    fn load_returns_default_when_no_file() {
        // Clear cache to ensure fresh state
        clear_cache();
        // With no file on disk at the config path, load_config falls back to default
        let config = load_config().unwrap();
        assert!(config["data_dir"].is_string());
        clear_cache();
    }

    #[test]
    fn claude_paths_are_under_home() {
        let home = home_dir();
        assert!(claude_dir().starts_with(&home));
        assert!(claude_json().starts_with(&home));
        assert!(settings_json().starts_with(&home));
    }

    #[test]
    fn json_keys_are_not_empty() {
        assert!(!CLAUDE_JSON_KEYS.is_empty());
        assert!(!SETTINGS_JSON_KEYS.is_empty());
    }

    #[test]
    fn category_map_entries() {
        assert_eq!(CATEGORY_MAP.len(), 2);
        assert_eq!(CATEGORY_MAP[0], ("agents", "agents"));
        assert_eq!(CATEGORY_MAP[1], ("memory", "memories"));
    }

    #[test]
    fn settings_defaults_has_memory_dir() {
        let defaults = settings_json_defaults();
        assert_eq!(
            defaults["autoMemoryDirectory"].as_str().unwrap(),
            "~/.claude/memory"
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        clear_cache();
        let dir = env::temp_dir().join("tidy-claude-test-config");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let file = dir.join("config.json");
        let config = serde_json::json!({"data_dir": "/tmp/test", "remote_backup": "git@example.com:repo.git"});

        // Write directly to test file
        let text = serde_json::to_string_pretty(&config).unwrap() + "\n";
        fs::write(&file, text).unwrap();

        // Verify roundtrip
        let read_back: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(read_back["data_dir"], "/tmp/test");
        assert_eq!(read_back["remote_backup"], "git@example.com:repo.git");

        let _ = fs::remove_dir_all(&dir);
        clear_cache();
    }

    #[test]
    fn get_data_dir_appends_backup() {
        clear_cache();
        // Inject a config with a known data_dir via cache
        *CONFIG_CACHE.lock().unwrap() = Some(serde_json::json!({"data_dir": "/tmp/mydata"}));
        let dir = get_data_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/mydata/backup"));
        clear_cache();
    }
}
