//! tidy-claude: Backup, sync, and clean up Claude Code configuration.
//!
//! Provides functionality to:
//! - Backup Claude settings, memories, agents across machines via git
//! - Restore settings and configuration
//! - Clean up old conversation logs and session files
//! - Sync everything in one command

pub mod config;
pub mod helpers;
pub mod state;
pub mod ops;
pub mod cli;

pub use config::RunConfig;
pub use state::RunState;
