"""Pure / near-pure functions — no side effects beyond reading the filesystem."""

import re
from pathlib import Path


def diff_files(src: Path, dst: Path) -> int:
    """Count files in *src* that are new or modified compared to *dst*."""
    if src.is_dir():
        changed = 0
        for f in src.rglob("*"):
            if not f.is_file():
                continue
            dst_f = dst / f.relative_to(src)
            if not dst_f.exists() or f.read_bytes() != dst_f.read_bytes():
                changed += 1
        return changed
    if not dst.exists() or src.read_bytes() != dst.read_bytes():
        return 1
    return 0


def deep_merge(base: dict, overlay: dict) -> dict:
    """Merge *overlay* into *base* in place.

    Lists are unioned, dicts are recursed, scalars are overwritten.
    """
    for key, val in overlay.items():
        if key in base and isinstance(base[key], dict) and isinstance(val, dict):
            deep_merge(base[key], val)
        elif key in base and isinstance(base[key], list) and isinstance(val, list):
            for item in val:
                if item not in base[key]:
                    base[key].append(item)
        else:
            base[key] = val
    return base


def format_size(n_bytes: int | float) -> str:
    """Return a human-readable file size."""
    for unit in ("B", "KB", "MB", "GB"):
        if n_bytes < 1024:
            return f"{n_bytes:.1f} {unit}" if unit != "B" else f"{n_bytes} {unit}"
        n_bytes /= 1024
    return f"{n_bytes:.1f} TB"


def resolve_claude_md(claude_dir: Path) -> list[Path]:
    """Return CLAUDE.md and all files it references via ``@<name>.md``."""
    claude_md = claude_dir / "CLAUDE.md"
    if not claude_md.exists():
        return []

    files = [claude_md]
    content = claude_md.read_text()
    for ref in re.findall(r"@(\S+\.md)", content):
        ref_path = claude_dir / ref
        if ref_path.exists():
            files.append(ref_path)
    return files


def pretty_project_name(dirname: str, home: Path) -> str:
    """Convert an encoded project dir name to a readable label.

    Claude Code encodes the working directory path by replacing ``/`` and ``.``
    with ``-``.  We reverse that heuristically for display.
    """
    home_prefix = str(home).replace("/", "-").replace(".", "-")
    if dirname == home_prefix:
        return "~"
    if not dirname.startswith(home_prefix + "-"):
        return dirname
    suffix = dirname[len(home_prefix) + 1:]
    # github paths: src-github-com-ORG-REPO...
    gh = "src-github-com-"
    if suffix.startswith(gh):
        org_repo = suffix[len(gh):]
        if "-" in org_repo:
            org, repo = org_repo.split("-", 1)
            return f"{org}/{repo}"
        return org_repo
    # dotfile dirs like .claude (encoded as -claude)
    if suffix.startswith("-"):
        return "~/." + suffix[1:]
    return "~/" + suffix


def extract_keys(data: dict, keys: list[str],
                 defaults: dict | None = None) -> dict:
    """Pick *keys* from *data* and merge *defaults*."""
    extracted = {k: data[k] for k in keys if k in data}
    if defaults:
        extracted.update(defaults)
    return extracted


def merge_keys_data(backed_up: dict, current: dict) -> dict:
    """Deep-merge *backed_up* into *current* and return the result."""
    return deep_merge(current, backed_up)
