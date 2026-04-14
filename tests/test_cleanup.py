"""Tests for the cleanup operation."""

import os
import time
from pathlib import Path

from tidy_claude.ops import CleanupResult, collect_projects, do_cleanup
from tidy_claude.state import RunState


def _make_old(path: Path, days: int = 30):
    """Set mtime to *days* ago."""
    old_time = time.time() - (days * 86400)
    os.utime(path, (old_time, old_time))


def _build_project(tmp_path: Path, name: str = "some-project"):
    """Create a fake project dir and return (claude_dir, project_dir)."""
    claude_dir = tmp_path / ".claude"
    project = claude_dir / "projects" / name
    project.mkdir(parents=True, exist_ok=True)
    (claude_dir / "sessions").mkdir(parents=True, exist_ok=True)
    return claude_dir, project


class TestCollectProjects:
    def test_empty(self, tmp_path: Path):
        assert collect_projects(tmp_path / "nope") == []

    def test_counts_sessions(self, tmp_path: Path):
        projects_dir = tmp_path / "projects"
        proj = projects_dir / "my-proj"
        proj.mkdir(parents=True)
        (proj / "a.jsonl").write_text("{}")
        (proj / "b.jsonl").write_text("{}")

        infos = collect_projects(projects_dir)
        assert len(infos) == 1
        assert infos[0].session_count == 2
        assert infos[0].total_size > 0


class TestDoCleanup:
    def test_old_files_deleted(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        old_jsonl = project / "abc-123.jsonl"
        old_jsonl.write_text("{}")
        _make_old(old_jsonl, days=30)

        sessions = claude_dir / "sessions"
        old_session = sessions / "99999.json"
        old_session.write_text('{"pid": 1}')
        _make_old(old_session, days=30)

        state = RunState(debug=True)
        res = do_cleanup(state, [project], older_than=7, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 2
        assert not old_jsonl.exists()
        assert not old_session.exists()
        assert not project.exists()

    def test_recent_files_kept(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        recent = project / "new.jsonl"
        recent.write_text("{}")

        state = RunState()
        res = do_cleanup(state, [project], older_than=7, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 0
        assert recent.exists()

    def test_subagent_dir_removed(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        jsonl = project / "conv-uuid.jsonl"
        jsonl.write_text("{}")
        sub = project / "conv-uuid" / "subagents"
        sub.mkdir(parents=True)
        (sub / "agent-abc.jsonl").write_text("{}")
        _make_old(jsonl, days=30)

        state = RunState()
        res = do_cleanup(state, [project], older_than=7, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 1
        assert res.deleted_dirs == 2  # subagent dir + empty project dir
        assert not jsonl.exists()
        assert not (project / "conv-uuid").exists()
        assert not project.exists()

    def test_dry_run_keeps_files(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        jsonl = project / "old.jsonl"
        jsonl.write_text("{}")
        _make_old(jsonl, days=30)

        state = RunState(debug=True)
        res = do_cleanup(state, [project], older_than=7, dry_run=True,
                         claude_dir=claude_dir)

        assert res.deleted_files == 1
        assert res.freed_bytes > 0
        assert jsonl.exists()

    def test_nothing_to_clean(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        state = RunState()
        res = do_cleanup(state, [project], older_than=7, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 0
        assert res.deleted_dirs == 1  # empty project dir removed
        assert res.freed_bytes == 0

    def test_older_than_zero_deletes_everything(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        # A file created just now
        recent = project / "brand-new.jsonl"
        recent.write_text("{}")

        state = RunState()
        res = do_cleanup(state, [project], older_than=0, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 1
        assert not recent.exists()

    def test_only_selected_projects_cleaned(self, tmp_path: Path):
        claude_dir = tmp_path / ".claude"
        proj_a = claude_dir / "projects" / "proj-a"
        proj_b = claude_dir / "projects" / "proj-b"
        proj_a.mkdir(parents=True)
        proj_b.mkdir(parents=True)
        (claude_dir / "sessions").mkdir(parents=True)

        a_file = proj_a / "old.jsonl"
        a_file.write_text("{}")
        _make_old(a_file, days=30)

        b_file = proj_b / "old.jsonl"
        b_file.write_text("{}")
        _make_old(b_file, days=30)

        state = RunState()
        do_cleanup(state, [proj_a], older_than=7, dry_run=False,
                   claude_dir=claude_dir)

        assert not a_file.exists()
        assert not proj_a.exists()  # emptied → removed
        assert b_file.exists()  # untouched

    def test_empty_project_dir_removed(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        old = project / "only.jsonl"
        old.write_text("{}")
        _make_old(old, days=30)

        state = RunState()
        res = do_cleanup(state, [project], older_than=7, dry_run=False,
                         claude_dir=claude_dir)

        assert res.deleted_files == 1
        assert res.deleted_dirs == 1
        assert not project.exists()

    def test_non_empty_project_dir_kept(self, tmp_path: Path):
        claude_dir, project = _build_project(tmp_path)

        old = project / "old.jsonl"
        old.write_text("{}")
        _make_old(old, days=30)

        recent = project / "recent.jsonl"
        recent.write_text("{}")

        state = RunState()
        do_cleanup(state, [project], older_than=7, dry_run=False,
                   claude_dir=claude_dir)

        assert not old.exists()
        assert project.exists()
        assert recent.exists()
