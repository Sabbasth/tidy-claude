//! CLI: commands and argument parsing via clap.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use dialoguer::MultiSelect;

use crate::config::{
    config_file, default_data_dir, ensure_config, get_data_dir, load_config, save_config,
    CLAUDE_DIR,
};
use crate::helpers::format_size;
use crate::ops::{
    collect_projects, do_backup, do_cleanup, do_commit, do_pull, do_push, do_restore, do_skills,
};
use crate::state::RunState;

// ── arg types ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "tidy-claude", version)]
#[command(about = "Backup, sync, and clean up Claude Code configuration")]
struct Cli {
    /// Enable verbose output
    #[arg(global = true, long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Full sync: pull + restore + skills + backup + push
    Sync,

    /// Show git status of the backup repo
    Status,

    /// Show or update tidy-claude configuration
    Config(ConfigArgs),

    /// Remove old Claude session and conversation files
    Cleanup {
        /// Delete sessions older than N days (0 = all)
        #[arg(long, default_value = "7", value_name = "DAYS")]
        older_than: u32,

        /// Clean all projects without interactive selection
        #[arg(short = 'a', long = "all")]
        all_projects: bool,

        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,

        /// Include named sessions (excluded by default)
        #[arg(long)]
        with_named_sessions: bool,
    },
}

#[derive(Args)]
struct ConfigArgs {
    /// Set the data directory for backups
    #[arg(long, value_name = "PATH")]
    data_dir: Option<PathBuf>,

    /// Set the git remote URL for the backup repo
    #[arg(long, value_name = "URL")]
    remote_backup: Option<String>,
}

// ── entry point ───────────────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let state = RunState::new(cli.debug);
    ensure_config()?;

    match cli.command.unwrap_or(Commands::Sync) {
        Commands::Sync => cmd_sync(&state),
        Commands::Status => cmd_status(),
        Commands::Config(args) => cmd_config(args),
        Commands::Cleanup {
            older_than,
            all_projects,
            dry_run,
            with_named_sessions,
        } => cmd_cleanup(
            &state,
            older_than,
            all_projects,
            dry_run,
            with_named_sessions,
        ),
    }
}

// ── sync ──────────────────────────────────────────────────────────────────────

fn cmd_sync(state: &RunState) -> Result<()> {
    let cfg = load_config()?;
    let remote = cfg
        .get("remote_backup")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if remote.is_empty() {
        println!("No remote configured. Run: tidy-claude config --remote-backup <git-url>");
        process::exit(1);
    }

    let backup_dir = get_data_dir()?;
    if !do_pull(state, &backup_dir)? {
        process::exit(1);
    }
    do_restore(state, &backup_dir, &CLAUDE_DIR)?;
    do_skills(state, &backup_dir)?;
    do_backup(state, &backup_dir, &CLAUDE_DIR)?;
    do_commit(state, &backup_dir, None)?;
    do_push(state, &backup_dir)?;
    println!("sync: up to date");
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

fn cmd_status() -> Result<()> {
    let backup_dir = get_data_dir()?;
    std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(&backup_dir)
        .status()?;
    Ok(())
}

// ── config ────────────────────────────────────────────────────────────────────

fn cmd_config(args: ConfigArgs) -> Result<()> {
    if args.data_dir.is_none() && args.remote_backup.is_none() {
        let path = config_file();
        println!("config: {}", path.display());
        println!("{}", serde_json::to_string_pretty(&load_config()?)?);
        return Ok(());
    }

    let mut cfg = load_config()?;

    if let Some(dir) = args.data_dir {
        let s = dir.to_string_lossy().to_string();
        cfg["data_dir"] = serde_json::json!(s);
        println!("data_dir set to {s}");
    }

    if let Some(remote) = args.remote_backup {
        let data_dir = cfg
            .get("data_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(default_data_dir);
        let backup_dir = data_dir.join("backup");

        if !backup_dir.join(".git").exists() {
            std::fs::create_dir_all(&data_dir)?;
            let out = std::process::Command::new("git")
                .args(["clone", &remote, "backup"])
                .current_dir(&data_dir)
                .output()
                .context("git clone failed")?;
            if !out.status.success() {
                anyhow::bail!("git clone failed: {}", String::from_utf8_lossy(&out.stderr));
            }
        } else {
            let out = std::process::Command::new("git")
                .args(["remote", "set-url", "origin", &remote])
                .current_dir(&backup_dir)
                .output()
                .context("git remote set-url failed")?;
            if !out.status.success() {
                anyhow::bail!(
                    "git remote set-url failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
        cfg["remote_backup"] = serde_json::json!(remote.clone());
        println!("remote_backup set to {remote}");
    }

    save_config(&cfg)?;
    Ok(())
}

// ── cleanup ───────────────────────────────────────────────────────────────────

fn cmd_cleanup(
    state: &RunState,
    older_than: u32,
    all_projects: bool,
    dry_run: bool,
    with_named_sessions: bool,
) -> Result<()> {
    let projects = collect_projects(&CLAUDE_DIR.join("projects"));

    if projects.is_empty() {
        println!("cleanup: no projects found");
        return Ok(());
    }

    let selected_paths: Vec<PathBuf> = if all_projects {
        projects.iter().map(|p| p.path.clone()).collect()
    } else {
        if !std::io::stdin().is_terminal() {
            anyhow::bail!("interactive mode requires a TTY; use --all");
        }
        let max_name = projects
            .iter()
            .map(|p| p.display_name.len())
            .max()
            .unwrap_or(0);
        let items: Vec<String> = projects
            .iter()
            .map(|p| {
                format!(
                    "{:<max_name$}  {:>10}   {} sessions",
                    p.display_name,
                    format_size(p.total_size),
                    p.session_count,
                )
            })
            .collect();

        let chosen = MultiSelect::new()
            .with_prompt("Select projects to clean (space = toggle, enter = confirm)")
            .items(&items)
            .interact_opt()?;

        match chosen {
            None => {
                println!("cleanup: cancelled");
                return Ok(());
            }
            Some(indices) => indices
                .into_iter()
                .map(|i| projects[i].path.clone())
                .collect(),
        }
    };

    let path_refs: Vec<&std::path::Path> = selected_paths.iter().map(|p| p.as_path()).collect();
    let res = do_cleanup(
        state,
        &path_refs,
        older_than,
        dry_run,
        &CLAUDE_DIR,
        with_named_sessions,
    )?;

    let prefix = if dry_run { "would free" } else { "freed" };
    let verb = if dry_run { "would delete" } else { "deleted" };
    let mut parts = Vec::new();
    if res.deleted_files > 0 {
        parts.push(format!("{} files", res.deleted_files));
    }
    if res.deleted_dirs > 0 {
        parts.push(format!("{} subagent dirs", res.deleted_dirs));
    }

    if parts.is_empty() {
        let age = if older_than > 0 {
            format!(" older than {older_than} days")
        } else {
            String::new()
        };
        println!("cleanup: nothing to delete{age}");
    } else {
        println!(
            "cleanup: {} {} | {} {}",
            verb,
            parts.join(", "),
            prefix,
            format_size(res.freed_bytes)
        );
    }
    Ok(())
}
