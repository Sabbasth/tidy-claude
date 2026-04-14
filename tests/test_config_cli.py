"""Tests for the config subcommand and sync remote guard."""

import json
import subprocess
from pathlib import Path

import pytest
from click.testing import CliRunner

import tidy_claude.cli as cli_mod
import tidy_claude.config as cfg_mod
from tidy_claude.cli import cli


def _reset_config(tmp_path, monkeypatch, initial=None):
    """Point config module at a temp dir and reset the cache."""
    config_dir = tmp_path / "config"
    config_dir.mkdir(exist_ok=True)
    config_file = config_dir / "config.json"

    if initial is not None:
        config_file.write_text(json.dumps(initial, indent=2) + "\n")

    monkeypatch.setattr(cfg_mod, "CONFIG_DIR", config_dir)
    monkeypatch.setattr(cfg_mod, "CONFIG_FILE", config_file)
    monkeypatch.setattr(cfg_mod, "_config_cache", None)
    monkeypatch.setattr(cli_mod, "CONFIG_FILE", config_file)
    return config_file


def _make_bare_remote(tmp_path: Path) -> str:
    """Create a bare git repo and return its path as a string."""
    remote = tmp_path / "remote.git"
    remote.mkdir()
    subprocess.run(["git", "init", "--bare"], cwd=remote,
                   capture_output=True, check=True)
    return str(remote)


class TestConfigShow:
    def test_shows_current_config(self, tmp_path, monkeypatch):
        data = {"data_dir": "/tmp/backups"}
        config_file = _reset_config(tmp_path, monkeypatch, initial=data)

        result = CliRunner().invoke(cli, ["config"])

        assert result.exit_code == 0
        assert str(config_file) in result.output
        assert "/tmp/backups" in result.output

    def test_shows_defaults_when_no_file(self, tmp_path, monkeypatch):
        _reset_config(tmp_path, monkeypatch)

        result = CliRunner().invoke(cli, ["config"])

        assert result.exit_code == 0
        assert "data_dir" in result.output


class TestConfigRemoteBackup:
    def test_set_remote_backup(self, tmp_path, monkeypatch):
        remote_url = _make_bare_remote(tmp_path)
        data_dir = str(tmp_path / "data")
        _reset_config(tmp_path, monkeypatch, initial={"data_dir": data_dir})

        result = CliRunner().invoke(
            cli, ["config", "--remote-backup", remote_url],
        )

        assert result.exit_code == 0
        assert f"remote_backup set to {remote_url}" in result.output

        saved = json.loads((tmp_path / "config" / "config.json").read_text())
        assert saved["remote_backup"] == remote_url
        assert saved["data_dir"] == data_dir

    def test_creates_backup_dir_and_git_repo(self, tmp_path, monkeypatch):
        remote_url = _make_bare_remote(tmp_path)
        data_dir = tmp_path / "data"
        _reset_config(tmp_path, monkeypatch, initial={"data_dir": str(data_dir)})

        result = CliRunner().invoke(
            cli, ["config", "--remote-backup", remote_url],
        )

        assert result.exit_code == 0
        backup_dir = data_dir / "backup"
        assert backup_dir.is_dir()
        assert (backup_dir / ".git").is_dir()

    def test_set_data_dir_and_remote_together(self, tmp_path, monkeypatch):
        remote_url = _make_bare_remote(tmp_path)
        _reset_config(tmp_path, monkeypatch, initial={"data_dir": "/old"})
        new_dir = tmp_path / "new"

        result = CliRunner().invoke(
            cli,
            ["config", "--data-dir", str(new_dir), "--remote-backup", remote_url],
        )

        assert result.exit_code == 0
        assert "data_dir set to" in result.output
        assert "remote_backup set to" in result.output

        saved = json.loads((tmp_path / "config" / "config.json").read_text())
        assert saved["remote_backup"] == remote_url
        assert (new_dir / "backup" / ".git").is_dir()

    def test_overwrite_existing_remote(self, tmp_path, monkeypatch):
        old_remote = _make_bare_remote(tmp_path)
        data_dir = tmp_path / "data"
        initial = {"data_dir": str(data_dir), "remote_backup": old_remote}
        _reset_config(tmp_path, monkeypatch, initial=initial)

        # Simulate a previously initialised backup repo
        backup_dir = data_dir / "backup"
        backup_dir.mkdir(parents=True)
        subprocess.run(["git", "init"], cwd=backup_dir,
                       capture_output=True, check=True)
        subprocess.run(["git", "remote", "add", "origin", old_remote],
                       cwd=backup_dir, capture_output=True, check=True)

        new_remote = str(tmp_path / "other-remote.git")
        (tmp_path / "other-remote.git").mkdir()
        subprocess.run(["git", "init", "--bare"], cwd=new_remote,
                       capture_output=True, check=True)

        result = CliRunner().invoke(
            cli, ["config", "--remote-backup", new_remote],
        )

        assert result.exit_code == 0
        saved = json.loads((tmp_path / "config" / "config.json").read_text())
        assert saved["remote_backup"] == new_remote


class TestSyncRemoteGuard:
    def test_sync_fails_without_remote(self, tmp_path, monkeypatch):
        _reset_config(tmp_path, monkeypatch, initial={"data_dir": "/tmp/d"})

        result = CliRunner().invoke(cli, ["sync"])

        assert result.exit_code != 0
        assert "No remote configured" in result.output
        assert "--remote-backup" in result.output

    def test_sync_fails_with_empty_remote(self, tmp_path, monkeypatch):
        _reset_config(
            tmp_path, monkeypatch,
            initial={"data_dir": "/tmp/d", "remote_backup": ""},
        )

        result = CliRunner().invoke(cli, ["sync"])

        assert result.exit_code != 0
        assert "No remote configured" in result.output

    def test_default_invocation_fails_without_remote(self, tmp_path, monkeypatch):
        """Running `tidy-claude` with no subcommand triggers sync."""
        _reset_config(tmp_path, monkeypatch, initial={"data_dir": "/tmp/d"})

        result = CliRunner().invoke(cli, [])

        assert result.exit_code != 0
        assert "No remote configured" in result.output
