//! Side-effectful operations: filesystem I/O, git, subprocess.

use anyhow::Result;
use crate::state::RunState;
use std::path::Path;

/// Backup Claude configuration to the backup directory.
pub fn backup(state: &RunState, _backup_dir: &Path) -> Result<()> {
    state.log("Backing up Claude configuration...");
    todo!("backup: copy settings, memories, agents, MCP servers")
}

/// Restore Claude configuration from the backup directory.
pub fn restore(state: &RunState, _backup_dir: &Path) -> Result<()> {
    state.log("Restoring Claude configuration...");
    todo!("restore: restore from backup")
}

/// Git pull (rebase).
pub fn git_pull(state: &RunState, _repo_dir: &Path) -> Result<()> {
    state.log("Pulling from git...");
    todo!("git_pull: git pull --rebase")
}

/// Git add, commit, push.
pub fn git_push(state: &RunState, _repo_dir: &Path, _message: &str) -> Result<()> {
    state.log("Pushing to git...");
    todo!("git_push: git add, commit, push")
}

/// Install skills via `npx skills add`.
pub fn install_skills(state: &RunState) -> Result<()> {
    state.log("Installing skills...");
    todo!("install_skills: run npx skills add")
}

/// Cleanup old Claude session files.
pub fn cleanup_sessions(state: &RunState, _projects: &[String], _older_than_days: u32) -> Result<()> {
    state.log("Cleaning up old sessions...");
    todo!("cleanup_sessions: delete old session files")
}

/// Sync: pull + restore + skills + backup + push.
pub fn sync(state: &RunState, _backup_dir: &Path) -> Result<()> {
    state.log("Starting full sync...");
    git_pull(state, _backup_dir)?;
    restore(state, _backup_dir)?;
    install_skills(state)?;
    backup(state, _backup_dir)?;
    git_push(state, _backup_dir, "chore: auto-sync")?;
    Ok(())
}
