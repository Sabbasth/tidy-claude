//! CLI: commands and argument parsing via clap.

use anyhow::Result;
use clap::{Parser, Subcommand};
use crate::state::RunState;

#[derive(Parser)]
#[command(name = "tidy-claude")]
#[command(about = "Backup, sync, and clean up Claude Code configuration", long_about = None)]
#[command(version)]
struct Cli {
    /// Enable debug logging
    #[arg(global = true, long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync everything: pull + restore + skills + backup + push
    Sync,

    /// Show current configuration
    Config,

    /// Clean up old conversation logs and session files
    Cleanup {
        /// Only delete sessions older than N days
        #[arg(long, default_value = "7")]
        older_than: u32,

        /// Process all projects (non-interactive)
        #[arg(short)]
        all: bool,

        /// Preview deletions without actually deleting
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run the CLI.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let state = RunState::new(cli.debug);

    match cli.command.unwrap_or(Commands::Sync) {
        Commands::Sync => {
            state.log("Running sync...");
            return Err(anyhow::anyhow!("sync: not yet implemented"));
        }
        Commands::Config => {
            state.log("Showing config...");
            return Err(anyhow::anyhow!("config: not yet implemented"));
        }
        Commands::Cleanup {
            older_than,
            all,
            dry_run,
        } => {
            state.log(&format!(
                "Cleanup: older_than={}, all={}, dry_run={}",
                older_than, all, dry_run
            ));
            return Err(anyhow::anyhow!("cleanup: not yet implemented"));
        }
    }
}
