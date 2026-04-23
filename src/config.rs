//! Path constants and configuration keys.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static CONFIG_CACHE: OnceLock<Mutex<Option<Value>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<Value>> {
    CONFIG_CACHE.get_or_init(|| Mutex::new(None))
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn project_dirs() -> directories::ProjectDirs {
    directories::ProjectDirs::from("", "", "tidy-claude")
        .expect("unable to determine project directories")
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

pub static HOME: Lazy<PathBuf> = Lazy::new(home_dir);

pub fn config_dir() -> PathBuf {
    env_path("TIDY_CLAUDE_CONFIG_DIR").unwrap_or_else(|| project_dirs().config_dir().to_path_buf())
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

pub fn default_data_dir() -> PathBuf {
    env_path("TIDY_CLAUDE_DATA_DIR").unwrap_or_else(|| project_dirs().data_dir().to_path_buf())
}

pub fn claude_dir() -> PathBuf {
    HOME.join(".claude")
}

pub static CLAUDE_DIR: Lazy<PathBuf> = Lazy::new(claude_dir);
pub static CLAUDE_JSON: Lazy<PathBuf> = Lazy::new(|| HOME.join(".claude.json"));
pub static SETTINGS_JSON: Lazy<PathBuf> = Lazy::new(|| CLAUDE_DIR.join("settings.json"));

pub const CLAUDE_JSON_KEYS: &[&str] = &["mcpServers"];
pub const SETTINGS_JSON_KEYS: &[&str] = &["permissions", "enabledPlugins", "extraKnownMarketplaces"];

pub static SETTINGS_JSON_DEFAULTS: Lazy<Value> = Lazy::new(|| {
    json!({
        "autoMemoryDirectory": "~/.claude/memory",
    })
});

pub static CATEGORY_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([("agents", "agents"), ("memory", "memories")])
});

fn default_config() -> Value {
    json!({"data_dir": default_data_dir().to_string_lossy().to_string()})
}

pub fn load_config() -> Result<Value> {
    if let Some(cfg) = cache().lock().expect("config cache poisoned").as_ref() {
        return Ok(cfg.clone());
    }

    let path = config_file();
    let cfg = if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse config file {}", path.display()))?
    } else {
        default_config()
    };

    *cache().lock().expect("config cache poisoned") = Some(cfg.clone());
    Ok(cfg)
}

pub fn save_config(config: &Value) -> Result<()> {
    let dir = config_dir();
    let path = config_file();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config dir {}", dir.display()))?;
    fs::write(&path, serde_json::to_string_pretty(config)? + "\n")
        .with_context(|| format!("failed to write config file {}", path.display()))?;
    *cache().lock().expect("config cache poisoned") = Some(config.clone());
    Ok(())
}

pub fn ensure_config() -> Result<Value> {
    let cfg = load_config()?;
    let path = config_file();
    if !path.exists() {
        save_config(&cfg)?;
    }
    Ok(cfg)
}

pub fn get_data_dir() -> Result<PathBuf> {
    let cfg = load_config()?;
    let data_dir = cfg
        .get("data_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    Ok(data_dir.join("backup"))
}

#[cfg(test)]
pub fn reset_config_cache() {
    *cache().lock().expect("config cache poisoned") = None;
}

pub struct RunConfig {
    pub debug: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self { debug: false }
    }
}
