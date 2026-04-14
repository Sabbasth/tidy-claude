"""Click CLI — thin wrappers around :mod:`tidy_claude.ops`."""

import json
import sys
import subprocess
from pathlib import Path

import click
from simple_term_menu import TerminalMenu

from .config import CLAUDE_DIR, CONFIG_FILE, ensure_config, get_data_dir, load_config, save_config
from .helpers import format_size
from .ops import (
    collect_projects,
    do_backup,
    do_cleanup,
    do_commit,
    do_pull,
    do_push,
    do_restore,
    do_skills,
)
from .state import RunState


def _print_summary(state: RunState, cmd: str):
    restore_parts = []
    backup_parts = []

    categories = ("memories", "agents", "configs", "settings")
    for cat in categories:
        n = state.stats.get(f"restore:{cat}", 0)
        if n:
            restore_parts.append(f"{n} {cat}")
    for cat in categories:
        n = state.stats.get(f"backup:{cat}", 0)
        if n:
            backup_parts.append(f"{n} {cat}")

    skills_n = state.stats.get("skills installed", 0)

    parts = []
    if restore_parts:
        parts.append("restored " + ", ".join(restore_parts))
    if skills_n:
        parts.append(f"installed {skills_n} skills")
    if backup_parts:
        parts.append("backed up " + ", ".join(backup_parts))

    summary = " | ".join(parts) if parts else "up to date"
    click.echo(f"{cmd}: {summary}")


@click.group(invoke_without_command=True)
@click.version_option(package_name="tidy-claude")
@click.option("--debug", is_flag=True, help="Enable verbose output.")
@click.pass_context
def cli(ctx, debug):
    """Backup, restore, and maintain Claude Code configuration."""
    ctx.ensure_object(dict)
    ctx.obj["state"] = RunState(debug=debug)
    ensure_config()
    if ctx.invoked_subcommand is None:
        ctx.invoke(sync_cmd)


@cli.command()
def status():
    """Show git status of the backup repo."""
    subprocess.run(["git", "status", "--short"], cwd=get_data_dir(), check=False)


@cli.command("sync")
@click.pass_context
def sync_cmd(ctx):
    """Full sync: pull + restore + skills + backup + push."""
    cfg = load_config()
    if not cfg.get("remote_backup"):
        click.echo("No remote configured. Run: tidy-claude config --remote-backup <git-url>")
        sys.exit(1)
    state = ctx.obj["state"]
    if not do_pull(state):
        sys.exit(1)
    do_restore(state)
    do_skills(state)
    do_backup(state)
    do_commit(state)
    do_push(state)
    _print_summary(state, "sync")


@cli.command()
@click.option("--data-dir", type=click.Path(), default=None,
              help="Set the data directory for backups.")
@click.option("--remote-backup", type=str, default=None,
              help="Set the git remote URL for the backup repo.")
def config(data_dir, remote_backup):
    """Show or update tidy-claude configuration."""
    if data_dir is None and remote_backup is None:
        click.echo(f"config: {CONFIG_FILE}")
        click.echo(json.dumps(load_config(), indent=2))
        return

    cfg = load_config()
    if data_dir is not None:
        cfg["data_dir"] = str(Path(data_dir).expanduser().resolve())
        click.echo(f"data_dir set to {cfg['data_dir']}")
    if remote_backup is not None:
        cfg["remote_backup"] = remote_backup
        data_path = Path(cfg["data_dir"])
        backup_dir = data_path / "backup"
        if not (backup_dir / ".git").exists():
            data_path.mkdir(parents=True, exist_ok=True)
            subprocess.run(
                ["git", "clone", remote_backup, "backup"],
                cwd=data_path, check=True, capture_output=True,
            )
        else:
            subprocess.run(
                ["git", "remote", "set-url", "origin", remote_backup],
                cwd=backup_dir, check=True, capture_output=True,
            )
        click.echo(f"remote_backup set to {remote_backup}")
    save_config(cfg)


@cli.command()
@click.option("--older-than", default=7, type=int, show_default=True,
              help="Delete sessions older than N days (0 = all).")
@click.option("-a", "--all", "all_projects", is_flag=True,
              help="Clean all projects without interactive selection.")
@click.option("--dry-run", is_flag=True,
              help="Show what would be deleted without deleting.")
@click.pass_context
def cleanup(ctx, older_than, all_projects, dry_run):
    """Remove old Claude session and conversation files.

    By default, shows an interactive menu to select projects.
    Use -a/--all to skip the menu and clean every project.
    """
    state = ctx.obj["state"]
    projects = collect_projects(CLAUDE_DIR / "projects")

    if not projects:
        click.echo("cleanup: no projects found")
        return

    if all_projects:
        selected = projects
    else:
        if not sys.stdin.isatty():
            raise click.UsageError("interactive mode requires a TTY; use --all")

        max_name = max(len(p.display_name) for p in projects)
        entries = [
            f"{p.display_name:<{max_name}}  {format_size(p.total_size):>10}   {p.session_count} sessions"
            for p in projects
        ]
        menu = TerminalMenu(
            entries,
            title="Select projects to clean (space = toggle, enter = confirm):",
            multi_select=True,
            show_multi_select_hint=True,
        )
        chosen = menu.show()

        if chosen is None:
            click.echo("cleanup: cancelled")
            return

        indices = [chosen] if isinstance(chosen, int) else list(chosen)
        selected = [projects[i] for i in indices]

    project_paths = [p.path for p in selected]
    res = do_cleanup(state, project_paths, older_than, dry_run)

    prefix = "would free" if dry_run else "freed"
    verb = "would delete" if dry_run else "deleted"
    parts = []
    if res.deleted_files:
        parts.append(f"{res.deleted_files} files")
    if res.deleted_dirs:
        parts.append(f"{res.deleted_dirs} subagent dirs")

    if parts:
        click.echo(f"cleanup: {verb} {', '.join(parts)} | {prefix} {format_size(res.freed_bytes)}")
    else:
        age = f" older than {older_than} days" if older_than > 0 else ""
        click.echo(f"cleanup: nothing to delete{age}")
