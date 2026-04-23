//! Configuration: paths, constants, and settings keys to preserve.

use std::env;
use std::path::PathBuf;

/// Home directory.
fn home_dir() -> PathBuf {
    env::var("HOME")
        .ok()
        .and_then(|h| if h.is_empty() { None } else { Some(PathBuf::from(h)) })
        .unwrap_or_else(|| PathBuf::from("/root"))
}

/// Claude configuration directory (~/.claude).
pub fn claude_dir() -> PathBuf {
    home_dir().join(".claude")
}

/// Claude settings.json path.
pub fn settings_json() -> PathBuf {
    claude_dir().join("settings.json")
}

/// Claude .json (MCP servers).
pub fn claude_json() -> PathBuf {
    home_dir().join(".claude.json")
}

pub struct RunConfig {
    pub debug: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self { debug: false }
    }
}
