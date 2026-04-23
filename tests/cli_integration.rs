//! Integration tests for the CLI — port of tests/test_config_cli.py.
//!
//! Each test runs the real binary via `assert_cmd` in a subprocess, with
//! `TIDY_CLAUDE_CONFIG_DIR` pointing to a fresh temp directory so the real
//! user configuration is never touched.

use std::fs;
use std::path::Path;
use std::process::Command;

use predicates::prelude::*;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Write a `config.json` in `dir` and return the `TempDir` handle.
fn setup_config(dir: &Path, config: &serde_json::Value) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("config.json"),
        serde_json::to_string_pretty(config).unwrap() + "\n",
    )
    .unwrap();
}

/// Create a bare git repository and return its path as a string.
fn make_bare_remote(parent: &Path) -> String {
    let remote = parent.join("remote.git");
    fs::create_dir_all(&remote).unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&remote)
        .output()
        .expect("git init --bare failed");
    remote.to_string_lossy().into_owned()
}

/// Return a `Command` for the `tidy-claude` binary with the config dir preset.
fn tidy(config_dir: &Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("tidy-claude").unwrap();
    cmd.env("TIDY_CLAUDE_CONFIG_DIR", config_dir);
    cmd
}

// ── config show ───────────────────────────────────────────────────────────────

#[test]
fn config_shows_current_config() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    setup_config(
        &config_dir,
        &serde_json::json!({"data_dir": "/tmp/backups"}),
    );

    tidy(&config_dir)
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            config_dir.join("config.json").to_string_lossy().as_ref(),
        ))
        .stdout(predicate::str::contains("/tmp/backups"));
}

#[test]
fn config_shows_defaults_when_no_file() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    // No config.json written — let ensure_config create defaults.

    tidy(&config_dir)
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("data_dir"));
}

// ── config --remote-backup ────────────────────────────────────────────────────

#[test]
fn config_set_remote_backup() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let data_dir = tmp.path().join("data");
    let remote_url = make_bare_remote(tmp.path());

    setup_config(
        &config_dir,
        &serde_json::json!({"data_dir": data_dir.to_string_lossy().as_ref()}),
    );

    tidy(&config_dir)
        .args(["config", "--remote-backup", &remote_url])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "remote_backup set to {remote_url}"
        )));

    // Config file on disk must contain the new remote
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["remote_backup"].as_str().unwrap(), remote_url);
    assert_eq!(
        saved["data_dir"].as_str().unwrap(),
        data_dir.to_string_lossy().as_ref()
    );
}

#[test]
fn config_creates_backup_dir_and_git_repo() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let data_dir = tmp.path().join("data");
    let remote_url = make_bare_remote(tmp.path());

    setup_config(
        &config_dir,
        &serde_json::json!({"data_dir": data_dir.to_string_lossy().as_ref()}),
    );

    tidy(&config_dir)
        .args(["config", "--remote-backup", &remote_url])
        .assert()
        .success();

    let backup_dir = data_dir.join("backup");
    assert!(backup_dir.is_dir(), "backup dir should be created");
    assert!(backup_dir.join(".git").is_dir(), ".git should exist");
}

#[test]
fn config_set_data_dir_and_remote_together() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let remote_url = make_bare_remote(tmp.path());
    let new_data_dir = tmp.path().join("new-data");

    setup_config(&config_dir, &serde_json::json!({"data_dir": "/old"}));

    tidy(&config_dir)
        .args([
            "config",
            "--data-dir",
            new_data_dir.to_string_lossy().as_ref(),
            "--remote-backup",
            &remote_url,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("data_dir set to"))
        .stdout(predicate::str::contains("remote_backup set to"));

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["remote_backup"].as_str().unwrap(), remote_url);
    assert!(new_data_dir.join("backup").join(".git").is_dir());
}

#[test]
fn config_overwrite_existing_remote() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let old_remote = make_bare_remote(tmp.path());
    let data_dir = tmp.path().join("data");
    let backup_dir = data_dir.join("backup");

    setup_config(
        &config_dir,
        &serde_json::json!({
            "data_dir": data_dir.to_string_lossy().as_ref(),
            "remote_backup": old_remote,
        }),
    );

    // Simulate a previously initialised backup repo
    fs::create_dir_all(&backup_dir).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(&backup_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["remote", "add", "origin", &old_remote])
        .current_dir(&backup_dir)
        .output()
        .unwrap();

    // Create the new remote
    let new_remote_dir = tmp.path().join("other-remote.git");
    fs::create_dir_all(&new_remote_dir).unwrap();
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&new_remote_dir)
        .output()
        .unwrap();
    let new_remote = new_remote_dir.to_string_lossy().into_owned();

    tidy(&config_dir)
        .args(["config", "--remote-backup", &new_remote])
        .assert()
        .success();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["remote_backup"].as_str().unwrap(), new_remote);
}

// ── sync remote guard ─────────────────────────────────────────────────────────

#[test]
fn sync_fails_without_remote() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    setup_config(&config_dir, &serde_json::json!({"data_dir": "/tmp/d"}));

    tidy(&config_dir)
        .arg("sync")
        .assert()
        .failure()
        .stdout(predicate::str::contains("No remote configured"))
        .stdout(predicate::str::contains("--remote-backup"));
}

#[test]
fn sync_fails_with_empty_remote() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    setup_config(
        &config_dir,
        &serde_json::json!({"data_dir": "/tmp/d", "remote_backup": ""}),
    );

    tidy(&config_dir)
        .arg("sync")
        .assert()
        .failure()
        .stdout(predicate::str::contains("No remote configured"));
}

#[test]
fn default_invocation_fails_without_remote() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    setup_config(&config_dir, &serde_json::json!({"data_dir": "/tmp/d"}));

    // No subcommand → defaults to sync
    tidy(&config_dir)
        .assert()
        .failure()
        .stdout(predicate::str::contains("No remote configured"));
}
