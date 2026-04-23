"""Side-effectful operations: filesystem I/O, git, subprocess."""

from __future__ import annotations

import json
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from .config import (
    CATEGORY_MAP,
    CLAUDE_DIR,
    CLAUDE_JSON,
    CLAUDE_JSON_KEYS,
    HOME,
    SETTINGS_JSON,
    SETTINGS_JSON_DEFAULTS,
    SETTINGS_JSON_KEYS,
    get_data_dir,
)
from .helpers import (
    deep_merge,
    diff_files,
    extract_keys,
    format_size,
    pretty_project_name,
    resolve_claude_md,
)
from .state import RunState


# ── file helpers ─────────────────────────────────────────────────────

def _copy_to_backup(state: RunState, src: Path, dst_rel: str, category: str):
    """Copy a file or directory into the backup tree."""
    dst = get_data_dir() / dst_rel
    changed = diff_files(src, dst)

    dst.parent.mkdir(parents=True, exist_ok=True)
    if src.is_dir():
        if dst.exists():
            shutil.rmtree(dst)
        shutil.copytree(src, dst)
        count = sum(1 for f in dst.rglob("*") if f.is_file())
        state.log(f"  copy  {src} -> {dst_rel}/ ({count} files)")
    else:
        shutil.copy2(src, dst)
        state.log(f"  copy  {src} -> {dst_rel}")

    if changed:
        state.count(f"backup:{category}", changed)


def _restore_copy(state: RunState, backup_rel: str, target: Path, category: str):
    """Restore a file or directory from backup to its live location."""
    src = get_data_dir() / backup_rel
    if not src.exists():
        state.log(f"  skip  {backup_rel} (not in backup)")
        return

    changed = diff_files(src, target)

    target.parent.mkdir(parents=True, exist_ok=True)
    if src.is_dir():
        shutil.copytree(src, target, dirs_exist_ok=True)
        count = sum(1 for f in target.rglob("*") if f.is_file())
        state.log(f"  restore  {backup_rel}/ -> {target} ({count} files)")
    else:
        shutil.copy2(src, target)
        state.log(f"  restore  {backup_rel} -> {target}")

    if changed:
        state.count(f"restore:{category}", changed)


def _extract_keys_to_file(state: RunState, src: Path, keys: list[str],
                          dst_rel: str, category: str,
                          defaults: dict | None = None):
    """Extract specific keys from a JSON file and write to backup."""
    if not src.exists():
        state.log(f"  skip  {src} (not found)")
        return

    data = json.loads(src.read_text())
    extracted = extract_keys(data, keys, defaults)

    dst = get_data_dir() / dst_rel
    new_content = json.dumps(extracted, indent=2) + "\n"
    changed = not dst.exists() or dst.read_text() != new_content

    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text(new_content)
    state.log(f"  extract  {src} -> {dst_rel} (keys: {', '.join(extracted)})")
    if changed:
        state.count(f"backup:{category}")


def _merge_keys_from_file(state: RunState, backup_rel: str, target: Path,
                          category: str):
    """Merge backed-up keys into an existing JSON file."""
    src = get_data_dir() / backup_rel
    if not src.exists():
        state.log(f"  skip  {backup_rel} (not in backup)")
        return

    backed_up = json.loads(src.read_text())

    if target.exists():
        current = json.loads(target.read_text())
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        current = {}

    old_content = json.dumps(current, indent=2)
    deep_merge(current, backed_up)
    new_content = json.dumps(current, indent=2)

    target.write_text(new_content + "\n")
    state.log(f"  merge  {backup_rel} -> {target} (keys: {', '.join(backed_up)})")
    if old_content != new_content:
        state.count(f"restore:{category}")


# ── public operations ────────────────────────────────────────────────
# pylint: disable=missing-docstring
def do_backup(state: RunState):
    for name in ("agents", "memory"):
        d = CLAUDE_DIR / name
        if d.exists():
            _copy_to_backup(state, d, f"claude/{name}", CATEGORY_MAP[name])

    for md in resolve_claude_md(CLAUDE_DIR):
        _copy_to_backup(state, md, f"claude/{md.name}", "configs")

    _extract_keys_to_file(state, CLAUDE_JSON, CLAUDE_JSON_KEYS,
                          "claude/claude.json", "settings")
    _extract_keys_to_file(state, SETTINGS_JSON, SETTINGS_JSON_KEYS,
                          "claude/settings.json", "settings",
                          SETTINGS_JSON_DEFAULTS)


def do_restore(state: RunState):
    for name in ("agents", "memory"):
        _restore_copy(state, f"claude/{name}", CLAUDE_DIR / name,
                      CATEGORY_MAP[name])

    backup_claude = get_data_dir() / "claude"
    if backup_claude.exists():
        for md in sorted(backup_claude.glob("*.md")):
            _restore_copy(state, f"claude/{md.name}", CLAUDE_DIR / md.name,
                          "configs")

    _merge_keys_from_file(state, "claude/claude.json", CLAUDE_JSON, "settings")
    _merge_keys_from_file(state, "claude/settings.json", SETTINGS_JSON,
                          "settings")


def do_skills(state: RunState):
    manifest = get_data_dir() / "skills.json"
    if not manifest.exists():
        raise SystemExit("error: skills.json not found in backup repo")

    data = json.loads(manifest.read_text())
    skills_dir = CLAUDE_DIR / "skills"

    for skill in data.get("skills", []):
        name = skill["name"]
        if (skills_dir / name).exists():
            state.log(f"  skip  {name} (already installed)")
            continue
        cmd = skill["install"]
        state.log(f"  install  {name} ({skill.get('source', '?')})")
        subprocess.run(cmd, shell=True, check=False,
                       capture_output=not state.debug)
        state.count("skills installed")


def do_pull(state: RunState) -> bool:
    data_dir = get_data_dir()
    # Skip pull if the repo has no commits yet (fresh clone of empty remote)
    head = subprocess.run(
        ["git", "rev-parse", "--verify", "HEAD"],
        cwd=data_dir, capture_output=True, check=False,
    )
    if head.returncode != 0:
        state.log("No commits yet, skipping pull.")
        return True

    result = subprocess.run(
        ["git", "pull", "--ff-only"],
        cwd=data_dir,
        capture_output=not state.debug,
        check=False,
    )
    if result.returncode != 0:
        import click  # pylint: disable=import-outside-toplevel
        click.echo("error: git pull --ff-only failed (history diverged?)")
        return False
    return True


def do_commit(state: RunState, message: str | None = None):
    subprocess.run(["git", "add", "-A"], cwd=get_data_dir(),
                   capture_output=not state.debug, check=True)

    result = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=get_data_dir(), check=False,
    )
    if result.returncode == 0:
        state.log("Nothing to commit.")
        return

    msg = message or "backup claude config"
    subprocess.run(["git", "commit", "-m", msg], cwd=get_data_dir(),
                   capture_output=not state.debug, check=True)


def do_push(state: RunState):
    subprocess.run(["git", "push", "-u", "origin", "HEAD"], cwd=get_data_dir(),
                   capture_output=not state.debug, check=True)


@dataclass
class ProjectInfo:
    """Metadata about a single project directory under ``~/.claude/projects``."""
    dirname: str
    path: Path
    display_name: str
    session_count: int
    total_size: int


def collect_projects(projects_dir: Path) -> list[ProjectInfo]:
    """Scan *projects_dir* and return a :class:`ProjectInfo` per sub-directory."""
    if not projects_dir.exists():
        return []

    result: list[ProjectInfo] = []
    for child in sorted(projects_dir.iterdir()):
        if not child.is_dir():
            continue
        jsonl_files = list(child.glob("*.jsonl"))
        total = sum(
            f.stat().st_size
            for f in child.rglob("*") if f.is_file()
        )
        result.append(ProjectInfo(
            dirname=child.name,
            path=child,
            display_name=pretty_project_name(child.name, HOME),
            session_count=len(jsonl_files),
            total_size=total,
        ))
    return result


@dataclass
class CleanupResult:
    deleted_files: int = 0
    deleted_dirs: int = 0
    freed_bytes: int = 0


def _named_sessions(claude_dir: Path, project_paths: list[Path]) -> dict[str, str]:
    """Return a mapping of session ID → name for sessions with a user-given name.

    Reads from ``sessions/*.json`` metadata first, then falls back to
    scanning ``custom-title`` entries inside ``.jsonl`` files.
    """
    result: dict[str, str] = {}

    # Pass 1: session metadata (fast, small files)
    sessions_dir = claude_dir / "sessions"
    if sessions_dir.exists():
        for sf in sessions_dir.glob("*.json"):
            try:
                data = json.loads(sf.read_text())
            except (json.JSONDecodeError, OSError):
                continue
            if data.get("name"):
                result[data["sessionId"]] = data["name"]

    # Pass 2: scan .jsonl files not already known
    for project_dir in project_paths:
        if not project_dir.exists():
            continue
        for jsonl in project_dir.glob("*.jsonl"):
            if jsonl.stem in result:
                continue
            try:
                for line in jsonl.open():
                    entry = json.loads(line)
                    if isinstance(entry, dict) and entry.get("type") == "custom-title":
                        result[jsonl.stem] = entry["customTitle"]
                        break
            except (json.JSONDecodeError, OSError):
                continue

    return result


def do_cleanup(
    state: RunState,
    project_paths: list[Path],
    older_than: int,
    dry_run: bool,
    claude_dir: Path = CLAUDE_DIR,
    with_named_sessions: bool = False,
) -> CleanupResult:
    """Delete sessions in *project_paths* older than *older_than* days.

    Also cleans stale session metadata in ``claude_dir/sessions/``.
    Named sessions are skipped unless *with_named_sessions* is True.
    """
    cutoff = time.time() - (older_than * 86400) if older_than > 0 else float("inf")
    named = {} if with_named_sessions else _named_sessions(claude_dir, project_paths)
    res = CleanupResult()

    for project_dir in project_paths:
        if not project_dir.exists():
            continue
        for jsonl in sorted(project_dir.glob("*.jsonl")):
            if jsonl.stem in named:
                state.log(f"  skip  {jsonl.stem} ({named[jsonl.stem]})")
                continue
            if older_than > 0 and jsonl.stat().st_mtime >= cutoff:
                continue

            size = jsonl.stat().st_size
            subagent_dir = jsonl.parent / jsonl.stem
            if subagent_dir.is_dir():
                dir_size = sum(
                    f.stat().st_size
                    for f in subagent_dir.rglob("*") if f.is_file()
                )
                if dry_run:
                    state.log(f"  would delete  {subagent_dir.relative_to(claude_dir)}/ ({format_size(dir_size)})") # pylint: disable=line-too-long
                else:
                    shutil.rmtree(subagent_dir)
                res.freed_bytes += dir_size
                res.deleted_dirs += 1

            if dry_run:
                state.log(f"  would delete  {jsonl.relative_to(claude_dir)} ({format_size(size)})")
            else:
                jsonl.unlink()
            res.freed_bytes += size
            res.deleted_files += 1

        # Remove project dir if empty after cleanup
        if project_dir.exists() and not any(project_dir.iterdir()):
            if dry_run:
                state.log(f"  would delete  {project_dir.relative_to(claude_dir)}/")
            else:
                project_dir.rmdir()
            res.deleted_dirs += 1

    # Session metadata (not project-specific)
    sessions_dir = claude_dir / "sessions"
    if sessions_dir.exists():
        for sf in sorted(sessions_dir.glob("*.json")):
            if not with_named_sessions:
                try:
                    data = json.loads(sf.read_text())
                except (json.JSONDecodeError, OSError):
                    data = {}
                if data.get("name"):
                    state.log(f"  skip  {sf.stem} ({data['name']})")
                    continue
            if older_than > 0 and sf.stat().st_mtime >= cutoff:
                continue
            size = sf.stat().st_size
            if dry_run:
                state.log(f"  would delete  {sf.relative_to(claude_dir)} ({format_size(size)})")
            else:
                sf.unlink()
            res.freed_bytes += size
            res.deleted_files += 1

    return res
