"""Tests for tidy_claude.helpers — pure functions."""

from pathlib import Path

from tidy_claude.helpers import (
    deep_merge,
    diff_files,
    extract_keys,
    format_size,
    merge_keys_data,
    pretty_project_name,
    resolve_claude_md,
)


# ── deep_merge ───────────────────────────────────────────────────────

class TestDeepMerge:
    def test_scalar_overwrite(self):
        assert deep_merge({"a": 1}, {"a": 2}) == {"a": 2}

    def test_new_key(self):
        assert deep_merge({"a": 1}, {"b": 2}) == {"a": 1, "b": 2}

    def test_nested_dict(self):
        base = {"a": {"x": 1, "y": 2}}
        overlay = {"a": {"y": 3, "z": 4}}
        assert deep_merge(base, overlay) == {"a": {"x": 1, "y": 3, "z": 4}}

    def test_list_union(self):
        base = {"a": [1, 2]}
        overlay = {"a": [2, 3]}
        assert deep_merge(base, overlay) == {"a": [1, 2, 3]}

    def test_empty_overlay(self):
        base = {"a": 1}
        assert deep_merge(base, {}) == {"a": 1}

    def test_empty_base(self):
        assert deep_merge({}, {"a": 1}) == {"a": 1}


# ── diff_files ───────────────────────────────────────────────────────

class TestDiffFiles:
    def test_identical_files(self, tmp_path: Path):
        a = tmp_path / "a.txt"
        b = tmp_path / "b.txt"
        a.write_text("hello")
        b.write_text("hello")
        assert diff_files(a, b) == 0

    def test_modified_file(self, tmp_path: Path):
        a = tmp_path / "a.txt"
        b = tmp_path / "b.txt"
        a.write_text("hello")
        b.write_text("world")
        assert diff_files(a, b) == 1

    def test_missing_dst(self, tmp_path: Path):
        a = tmp_path / "a.txt"
        a.write_text("hello")
        assert diff_files(a, tmp_path / "nope.txt") == 1

    def test_directory_comparison(self, tmp_path: Path):
        src = tmp_path / "src"
        dst = tmp_path / "dst"
        src.mkdir()
        dst.mkdir()

        (src / "same.txt").write_text("same")
        (dst / "same.txt").write_text("same")
        (src / "changed.txt").write_text("new")
        (dst / "changed.txt").write_text("old")
        (src / "added.txt").write_text("new file")

        assert diff_files(src, dst) == 2  # changed + added


# ── format_size ──────────────────────────────────────────────────────

class TestFormatSize:
    def test_bytes(self):
        assert format_size(0) == "0 B"
        assert format_size(512) == "512 B"

    def test_kilobytes(self):
        assert format_size(1024) == "1.0 KB"
        assert format_size(1536) == "1.5 KB"

    def test_megabytes(self):
        assert format_size(1024 * 1024) == "1.0 MB"

    def test_gigabytes(self):
        assert format_size(1024 ** 3) == "1.0 GB"


# ── resolve_claude_md ────────────────────────────────────────────────

class TestResolveClaudeMd:
    def test_no_claude_md(self, tmp_path: Path):
        assert resolve_claude_md(tmp_path) == []

    def test_with_valid_refs(self, claude_tree: Path):
        result = resolve_claude_md(claude_tree)
        names = [p.name for p in result]
        assert "CLAUDE.md" in names
        assert "tips.md" in names
        # missing.md doesn't exist on disk → not returned
        assert "missing.md" not in names

    def test_no_refs(self, tmp_path: Path):
        (tmp_path / "CLAUDE.md").write_text("no references here\n")
        result = resolve_claude_md(tmp_path)
        assert len(result) == 1
        assert result[0].name == "CLAUDE.md"


# ── pretty_project_name ──────────────────────────────────────────────

class TestPrettyProjectName:
    HOME = Path("/Users/alice")

    def test_home_dir(self):
        assert pretty_project_name("-Users-alice", self.HOME) == "~"

    def test_github_org_repo(self):
        name = "-Users-alice-src-github-com-acme-widgets"
        assert pretty_project_name(name, self.HOME) == "acme/widgets"

    def test_github_repo_with_dashes(self):
        name = "-Users-alice-src-github-com-acme-my-cool-repo"
        assert pretty_project_name(name, self.HOME) == "acme/my-cool-repo"

    def test_dotfile_dir(self):
        name = "-Users-alice--config"
        assert pretty_project_name(name, self.HOME) == "~/.config"

    def test_plain_subdir(self):
        name = "-Users-alice-projects"
        assert pretty_project_name(name, self.HOME) == "~/projects"

    def test_unknown_prefix(self):
        assert pretty_project_name("-other-path", self.HOME) == "-other-path"


# ── extract_keys ─────────────────────────────────────────────────────

class TestExtractKeys:
    def test_subset(self):
        data = {"a": 1, "b": 2, "c": 3}
        assert extract_keys(data, ["a", "c"]) == {"a": 1, "c": 3}

    def test_missing_key(self):
        data = {"a": 1}
        assert extract_keys(data, ["a", "z"]) == {"a": 1}

    def test_with_defaults(self):
        data = {"a": 1}
        result = extract_keys(data, ["a"], defaults={"d": 42})
        assert result == {"a": 1, "d": 42}


# ── merge_keys_data ──────────────────────────────────────────────────

class TestMergeKeysData:
    def test_into_empty(self):
        assert merge_keys_data({"a": 1}, {}) == {"a": 1}

    def test_with_overlap(self):
        backed_up = {"a": {"x": 1}}
        current = {"a": {"y": 2}, "b": 3}
        result = merge_keys_data(backed_up, current)
        assert result == {"a": {"x": 1, "y": 2}, "b": 3}
