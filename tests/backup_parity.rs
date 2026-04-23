//! Parity tests — backup/restore round-trip and JSON key filtering.
//!
//! These tests call `do_backup` / `do_restore` directly with a controlled
//! fixture so the real `~/.claude` is never touched.  They verify:
//!
//! 1. **Round-trip**: every file in the fixture is faithfully preserved
//!    through backup → restore.
//! 2. **Key filtering**: `settings.json` and `claude.json` only contain
//!    the keys listed in `SETTINGS_JSON_KEYS` / `CLAUDE_JSON_KEYS` plus
//!    `SETTINGS_JSON_DEFAULTS`.
//! 3. **Tree structure** is snapshot-tested with `insta` so unintended
//!    additions or renames are caught immediately.

use std::fs;
use tempfile::TempDir;
use tidy_claude::ops::{do_backup, do_restore};
use tidy_claude::state::RunState;

// ── fixture builder ───────────────────────────────────────────────────────────

/// Build a minimal fake `~/.claude` tree under `home` and return
/// `(claude_dir, home_dir)`.
fn build_fixture(tmp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let home = tmp.path().join("home");
    let claude_dir = home.join(".claude");

    // Core markdown files
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("CLAUDE.md"),
        "# Instructions\n@tips.md\n",
    )
    .unwrap();
    fs::write(claude_dir.join("tips.md"), "some tips\n").unwrap();

    // memory/
    let memory = claude_dir.join("memory");
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("MEMORY.md"), "- remember this\n").unwrap();

    // agents/
    let agents = claude_dir.join("agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(agents.join("my-agent.md"), "agent config\n").unwrap();

    // settings.json — contains keys that should be filtered out
    fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "permissions": {"allow": ["Bash"]},
            "enabledPlugins": ["plugin-a"],
            "shouldNotBeBackedUp": "secret",
            "anotherPrivateKey": 42,
        }))
        .unwrap(),
    )
    .unwrap();

    // ~/.claude.json — MCP servers + keys that should be filtered
    fs::write(
        home.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {"server1": {"command": "npx"}},
            "privateStuff": "should-not-appear",
        }))
        .unwrap(),
    )
    .unwrap();

    (claude_dir, home)
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// The backup tree must contain exactly the expected files and nothing else.
#[test]
fn backup_tree_structure_matches_snapshot() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _) = build_fixture(&tmp);
    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    // Collect sorted relative paths
    let mut files: Vec<String> = walkdir::WalkDir::new(&backup_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(&backup_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/") // normalise for Windows CI
        })
        .collect();
    files.sort();

    insta::assert_debug_snapshot!(files);
}

/// Only SETTINGS_JSON_KEYS + SETTINGS_JSON_DEFAULTS survive in the backup.
#[test]
fn backup_filters_settings_json_keys() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _) = build_fixture(&tmp);
    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    let saved: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(backup_dir.join("claude/settings.json")).unwrap(),
    )
    .unwrap();

    // Expected keys present
    assert!(
        saved.get("permissions").is_some(),
        "permissions should be backed up"
    );
    assert!(
        saved.get("enabledPlugins").is_some(),
        "enabledPlugins should be backed up"
    );
    // SETTINGS_JSON_DEFAULTS key injected
    assert!(
        saved.get("autoMemoryDirectory").is_some(),
        "autoMemoryDirectory default should be injected"
    );
    // Private keys must be absent
    assert!(
        saved.get("shouldNotBeBackedUp").is_none(),
        "private key must not appear in backup"
    );
    assert!(
        saved.get("anotherPrivateKey").is_none(),
        "private key must not appear in backup"
    );
}

/// Only CLAUDE_JSON_KEYS survive in the claude.json backup.
#[test]
fn backup_filters_claude_json_keys() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _) = build_fixture(&tmp);
    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    let saved: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(backup_dir.join("claude/claude.json")).unwrap(),
    )
    .unwrap();

    assert!(
        saved.get("mcpServers").is_some(),
        "mcpServers should be backed up"
    );
    assert!(
        saved.get("privateStuff").is_none(),
        "private key must not appear in backup"
    );
}

/// Full round-trip: backup → restore → content identical to original.
#[test]
fn backup_restore_round_trip() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _home) = build_fixture(&tmp);
    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);

    // --- backup ---
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    // --- restore to a fresh target ---
    let restore_home = tmp.path().join("restore");
    let restore_claude = restore_home.join(".claude");
    fs::create_dir_all(&restore_home).unwrap();

    do_restore(&state, &backup_dir, &restore_claude).unwrap();

    // markdown files
    assert_eq!(
        fs::read_to_string(restore_claude.join("CLAUDE.md")).unwrap(),
        "# Instructions\n@tips.md\n"
    );
    assert_eq!(
        fs::read_to_string(restore_claude.join("tips.md")).unwrap(),
        "some tips\n"
    );

    // memory
    assert_eq!(
        fs::read_to_string(restore_claude.join("memory/MEMORY.md")).unwrap(),
        "- remember this\n"
    );

    // agents
    assert_eq!(
        fs::read_to_string(restore_claude.join("agents/my-agent.md")).unwrap(),
        "agent config\n"
    );

    // settings.json restored with only backed-up keys merged in
    let restored_settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(restore_claude.join("settings.json")).unwrap(),
    )
    .unwrap();
    assert!(restored_settings.get("permissions").is_some());
    assert!(restored_settings.get("enabledPlugins").is_some());

    // .claude.json restored
    let restored_claude_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(restore_home.join(".claude.json")).unwrap(),
    )
    .unwrap();
    assert!(restored_claude_json.get("mcpServers").is_some());
}

/// Restore onto an existing settings.json deep-merges rather than overwrites.
#[test]
fn restore_deep_merges_into_existing_settings() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _) = build_fixture(&tmp);
    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    // Pre-populate a restore target with an existing settings.json
    let restore_home = tmp.path().join("restore");
    let restore_claude = restore_home.join(".claude");
    fs::create_dir_all(&restore_claude).unwrap();
    let existing = serde_json::json!({
        "permissions": {"allow": ["Read"], "deny": ["Write"]},
        "localOnlyKey": "preserved",
    });
    fs::write(
        restore_claude.join("settings.json"),
        serde_json::to_string_pretty(&existing).unwrap(),
    )
    .unwrap();
    fs::write(restore_home.join(".claude.json"), "{}").unwrap();

    do_restore(&state, &backup_dir, &restore_claude).unwrap();

    let merged: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(restore_claude.join("settings.json")).unwrap(),
    )
    .unwrap();

    // Backed-up permissions should be merged in
    assert!(
        merged.get("permissions").is_some(),
        "backed-up permissions should survive merge"
    );
    // Local key that was not in the backup must be preserved
    assert_eq!(
        merged.get("localOnlyKey").and_then(|v| v.as_str()),
        Some("preserved"),
        "local-only key must be preserved after deep merge"
    );
}

/// `@ref.md` files that exist on disk are included; missing ones are not.
#[test]
fn backup_includes_referenced_mds_only_if_they_exist() {
    let tmp = TempDir::new().unwrap();
    let (claude_dir, _home) = build_fixture(&tmp);
    fs::write(
        claude_dir.join("CLAUDE.md"),
        "# Instructions\n@tips.md\n@missing.md\n",
    )
    .unwrap();

    let backup_dir = tmp.path().join("backup");
    fs::create_dir_all(&backup_dir).unwrap();

    let state = RunState::new(false);
    do_backup(&state, &backup_dir, &claude_dir).unwrap();

    assert!(
        backup_dir.join("claude/tips.md").exists(),
        "existing ref must be backed up"
    );
    assert!(
        !backup_dir.join("claude/missing.md").exists(),
        "non-existent ref must not appear in backup"
    );
}
