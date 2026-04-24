use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

use clap::{Parser, Subcommand};
use dialoguer::MultiSelect;

use crate::config;
use crate::helpers::format_size;
use crate::ops;
use crate::state::RunState;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "tidy-claude", version = VERSION, about = "Backup, sync, and clean up Claude Code configuration")]
struct Cli {
    /// Enable debug output
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync configuration: pull, restore, backup, push
    Sync,
    /// Show or update configuration
    Config {
        /// Set the backup data directory
        #[arg(long)]
        data_dir: Option<String>,
        /// Set the git remote URL for backups
        #[arg(long)]
        remote_backup: Option<String>,
    },
    /// Clean up old conversation logs and session files
    Cleanup {
        /// Delete sessions older than N days (default: 7)
        #[arg(long, default_value_t = 7)]
        older_than: u32,
        /// Skip interactive menu, clean all projects
        #[arg(short, long)]
        all: bool,
        /// Preview deletions without removing
        #[arg(long)]
        dry_run: bool,
        /// Include user-named sessions in cleanup
        #[arg(long)]
        with_named_sessions: bool,
    },
    /// Show git status of the backup repository
    Status,
}

fn print_summary(state: &RunState, cmd: &str) {
    let categories = ["memories", "agents", "configs", "settings"];

    let restore_parts: Vec<String> = categories
        .iter()
        .filter_map(|cat| {
            let key = format!("restore:{cat}");
            state.stats.get(&key).filter(|&&n| n > 0).map(|n| format!("{n} {cat}"))
        })
        .collect();

    let backup_parts: Vec<String> = categories
        .iter()
        .filter_map(|cat| {
            let key = format!("backup:{cat}");
            state.stats.get(&key).filter(|&&n| n > 0).map(|n| format!("{n} {cat}"))
        })
        .collect();

    let skills_n = state.stats.get("skills installed").copied().unwrap_or(0);

    let mut parts = Vec::new();
    if !restore_parts.is_empty() {
        parts.push(format!("restored {}", restore_parts.join(", ")));
    }
    if skills_n > 0 {
        parts.push(format!("installed {skills_n} skills"));
    }
    if !backup_parts.is_empty() {
        parts.push(format!("backed up {}", backup_parts.join(", ")));
    }

    let summary = if parts.is_empty() {
        "up to date".to_string()
    } else {
        parts.join(" | ")
    };
    println!("{cmd}: {summary}");
}

fn cmd_sync(state: &mut RunState) {
    let cfg = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let remote = cfg["remote_backup"].as_str().unwrap_or("");
    if remote.is_empty() {
        eprintln!("No remote configured. Run: tidy-claude config --remote-backup <git-url>");
        std::process::exit(1);
    }

    match ops::do_pull(state) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    if let Err(e) = ops::do_restore(state) {
        eprintln!("error restoring: {e}");
        std::process::exit(1);
    }
    if let Err(e) = ops::do_skills(state) {
        eprintln!("error installing skills: {e}");
        std::process::exit(1);
    }
    if let Err(e) = ops::do_backup(state) {
        eprintln!("error backing up: {e}");
        std::process::exit(1);
    }
    if let Err(e) = ops::do_commit(state, None) {
        eprintln!("error committing: {e}");
        std::process::exit(1);
    }
    if let Err(e) = ops::do_push(state) {
        eprintln!("error pushing: {e}");
        std::process::exit(1);
    }

    print_summary(state, "sync");
}

fn cmd_config(data_dir: Option<String>, remote_backup: Option<String>) {
    if data_dir.is_none() && remote_backup.is_none() {
        println!("config: {}", config::config_file().display());
        match config::load_config() {
            Ok(cfg) => println!("{}", serde_json::to_string_pretty(&cfg).unwrap()),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let mut cfg = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if let Some(dir) = data_dir {
        let resolved = PathBuf::from(shellexpand_tilde(&dir))
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&dir));
        let s = resolved.to_string_lossy().to_string();
        cfg["data_dir"] = serde_json::Value::String(s.clone());
        println!("data_dir set to {s}");
    }

    if let Some(ref remote) = remote_backup {
        cfg["remote_backup"] = serde_json::Value::String(remote.clone());

        let data_path = PathBuf::from(cfg["data_dir"].as_str().unwrap_or(""));
        let backup_dir = data_path.join("backup");

        if !backup_dir.join(".git").exists() {
            std::fs::create_dir_all(&data_path).ok();
            let status = Command::new("git")
                .args(["clone", remote, "backup"])
                .current_dir(&data_path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Err(e) = status {
                eprintln!("error: git clone failed: {e}");
                std::process::exit(1);
            }
        } else {
            let status = Command::new("git")
                .args(["remote", "set-url", "origin", remote])
                .current_dir(&backup_dir)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if let Err(e) = status {
                eprintln!("error: git remote set-url failed: {e}");
                std::process::exit(1);
            }
        }
        println!("remote_backup set to {remote}");
    }

    if let Err(e) = config::save_config(&cfg) {
        eprintln!("error saving config: {e}");
        std::process::exit(1);
    }
}

fn cmd_cleanup(
    state: &mut RunState,
    older_than: u32,
    all_projects: bool,
    dry_run: bool,
    with_named_sessions: bool,
) {
    let claude_dir = config::claude_dir();
    let home = config::home_dir();
    let projects = ops::collect_projects(&claude_dir.join("projects"), &home);

    if projects.is_empty() {
        println!("cleanup: no projects found");
        return;
    }

    let selected_paths: Vec<PathBuf> = if all_projects {
        projects.iter().map(|p| p.path.clone()).collect()
    } else {
        if !std::io::stdin().is_terminal() {
            eprintln!("error: interactive mode requires a TTY; use --all");
            std::process::exit(1);
        }

        let max_name = projects.iter().map(|p| p.display_name.len()).max().unwrap_or(0);
        let entries: Vec<String> = projects
            .iter()
            .map(|p| {
                format!(
                    "{:<width$}  {:>10}   {} sessions",
                    p.display_name,
                    format_size(p.total_size),
                    p.session_count,
                    width = max_name,
                )
            })
            .collect();

        let chosen = MultiSelect::new()
            .with_prompt("Select projects to clean (space = toggle, enter = confirm)")
            .items(&entries)
            .report(false)
            .interact_opt();

        match chosen {
            Ok(Some(indices)) if !indices.is_empty() => {
                let names: Vec<_> = indices.iter().map(|&i| projects[i].display_name.as_str()).collect();
                println!("selected: {}", names.join(", "));
                indices.iter().map(|&i| projects[i].path.clone()).collect()
            }
            _ => {
                println!("cleanup: cancelled");
                return;
            }
        }
    };

    let path_refs: Vec<&std::path::Path> = selected_paths.iter().map(|p| p.as_path()).collect();
    let res = ops::do_cleanup(state, &path_refs, older_than, dry_run, &claude_dir, with_named_sessions);

    let prefix = if dry_run { "would free" } else { "freed" };
    let verb = if dry_run { "would delete" } else { "deleted" };

    let mut parts = Vec::new();
    if res.deleted_files > 0 {
        parts.push(format!("{} files", res.deleted_files));
    }
    if res.deleted_dirs > 0 {
        parts.push(format!("{} subagent dirs", res.deleted_dirs));
    }

    if !parts.is_empty() {
        println!(
            "cleanup: {verb} {} | {prefix} {}",
            parts.join(", "),
            format_size(res.freed_bytes)
        );
    } else {
        let age = if older_than > 0 {
            format!(" older than {older_than} days")
        } else {
            String::new()
        };
        println!("cleanup: nothing to delete{age}");
    }
}

fn cmd_status() {
    match config::get_data_dir() {
        Ok(data_dir) => {
            let _ = Command::new("git")
                .args(["status", "--short"])
                .current_dir(data_dir)
                .status();
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

/// Expand leading ~ to home directory.
fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

pub fn run() {
    let cli = Cli::parse();
    let mut state = RunState {
        debug: cli.debug,
        ..Default::default()
    };

    let _ = config::ensure_config();

    match cli.command {
        None | Some(Commands::Sync) => cmd_sync(&mut state),
        Some(Commands::Config { data_dir, remote_backup }) => cmd_config(data_dir, remote_backup),
        Some(Commands::Cleanup { older_than, all, dry_run, with_named_sessions }) => {
            cmd_cleanup(&mut state, older_than, all, dry_run, with_named_sessions);
        }
        Some(Commands::Status) => cmd_status(),
    }
}
