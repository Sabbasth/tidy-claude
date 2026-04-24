use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("tidy-claude").unwrap()
}

// ── version & help ──────────────────────────────────────────────────

#[test]
fn version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("tidy-claude 1.0.0"));
}

#[test]
fn help_flag() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup, sync, and clean up"))
        .stdout(predicate::str::contains("sync"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("cleanup"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn help_cleanup_subcommand() {
    cmd()
        .args(["cleanup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--older-than"))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--with-named-sessions"));
}

#[test]
fn help_config_subcommand() {
    cmd()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--data-dir"))
        .stdout(predicate::str::contains("--remote-backup"));
}

// ── config show ─────────────────────────────────────────────────────

#[test]
fn config_shows_current() {
    cmd()
        .arg("config")
        .assert()
        .success()
        .stdout(predicate::str::contains("config:"))
        .stdout(predicate::str::contains("data_dir"));
}

// ── sync guard ──────────────────────────────────────────────────────

// Note: sync guard test depends on user config state.
// If user has a remote configured, sync will attempt to pull.
// We test the cleanup command instead which is safe and deterministic.

// ── cleanup ─────────────────────────────────────────────────────────

#[test]
fn cleanup_all_dry_run() {
    // Safe: --dry-run never deletes anything
    cmd()
        .args(["cleanup", "--all", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup:"));
}

#[test]
fn cleanup_all_dry_run_with_named() {
    cmd()
        .args(["cleanup", "--all", "--dry-run", "--with-named-sessions"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup:"));
}

#[test]
fn cleanup_older_than_zero_dry_run() {
    cmd()
        .args(["cleanup", "--all", "--dry-run", "--older-than", "0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup:"));
}

#[test]
fn cleanup_older_than_large_value() {
    // With a very large older_than, nothing should match
    cmd()
        .args(["cleanup", "--all", "--dry-run", "--older-than", "99999"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleanup:"));
}

// ── unknown subcommand ──────────────────────────────────────────────

#[test]
fn unknown_subcommand_fails() {
    cmd()
        .arg("foobar")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

// ── invalid flags ───────────────────────────────────────────────────

#[test]
fn invalid_flag_fails() {
    cmd()
        .arg("--nonexistent")
        .assert()
        .failure();
}

#[test]
fn cleanup_without_tty_and_no_all_flag() {
    // When stdin is not a TTY and --all is not passed, should error
    // In CI/test context stdin is not a TTY, so this tests the guard
    cmd()
        .arg("cleanup")
        .assert()
        .failure()
        .stderr(predicate::str::contains("TTY").or(predicate::str::contains("interactive")));
}
