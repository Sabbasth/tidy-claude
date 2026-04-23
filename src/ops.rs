//! Side-effectful operations: filesystem I/O, git, subprocess.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::config::{
    get_data_dir, CLAUDE_DIR, CLAUDE_JSON_KEYS, SETTINGS_JSON_DEFAULTS, SETTINGS_JSON_KEYS,
};
use crate::helpers::{deep_merge, extract_keys, format_size, resolve_claude_md};
use crate::state::RunState;

// ── private fs helpers ────────────────────────────────────────────────────────

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

fn copy_to_backup(state: &RunState, src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if src.is_dir() {
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        copy_dir_all(src, dst)?;
        let count = walkdir::WalkDir::new(dst)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count();
        state.log(&format!(
            "  copy  {} -> {}/ ({} files)",
            src.display(),
            dst.display(),
            count
        ));
    } else {
        fs::copy(src, dst)
            .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
        state.log(&format!("  copy  {} -> {}", src.display(), dst.display()));
    }
    Ok(())
}

fn restore_copy(state: &RunState, src: &Path, target: &Path) -> Result<()> {
    if !src.exists() {
        state.log(&format!("  skip  {} (not in backup)", src.display()));
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if src.is_dir() {
        copy_dir_all(src, target)?;
        let count = walkdir::WalkDir::new(target)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .count();
        state.log(&format!(
            "  restore  {}/ -> {} ({} files)",
            src.display(),
            target.display(),
            count
        ));
    } else {
        fs::copy(src, target)
            .with_context(|| format!("restore {} -> {}", src.display(), target.display()))?;
        state.log(&format!(
            "  restore  {} -> {}",
            src.display(),
            target.display()
        ));
    }
    Ok(())
}

fn extract_keys_to_file(
    state: &RunState,
    src: &Path,
    keys: &[&str],
    dst: &Path,
    defaults: Option<&Value>,
) -> Result<()> {
    if !src.exists() {
        state.log(&format!("  skip  {} (not found)", src.display()));
        return Ok(());
    }
    let text = fs::read_to_string(src).with_context(|| format!("read {}", src.display()))?;
    let data: Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", src.display()))?;
    let extracted = extract_keys(&data, keys, defaults);
    let new_content = serde_json::to_string_pretty(&extracted)? + "\n";
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dst, &new_content)?;
    let key_list: Vec<_> = extracted
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    state.log(&format!(
        "  extract  {} -> {} (keys: {})",
        src.display(),
        dst.display(),
        key_list.join(", ")
    ));
    Ok(())
}

fn merge_keys_from_file(state: &RunState, src: &Path, target: &Path) -> Result<()> {
    if !src.exists() {
        state.log(&format!("  skip  {} (not in backup)", src.display()));
        return Ok(());
    }
    let backed_up_text = fs::read_to_string(src)?;
    let backed_up: Value = serde_json::from_str(&backed_up_text)?;
    let mut current: Value = if target.exists() {
        serde_json::from_str(&fs::read_to_string(target)?)?
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        Value::Object(Map::new())
    };
    deep_merge(&mut current, &backed_up);
    let new_content = serde_json::to_string_pretty(&current)? + "\n";
    fs::write(target, &new_content)?;
    let key_list: Vec<_> = backed_up
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    state.log(&format!(
        "  merge  {} -> {} (keys: {})",
        src.display(),
        target.display(),
        key_list.join(", ")
    ));
    Ok(())
}

// ── backup / restore ──────────────────────────────────────────────────────────

pub fn do_backup(state: &RunState, backup_dir: &Path, claude_dir: &Path) -> Result<()> {
    let claude_json = claude_dir
        .parent()
        .expect("claude_dir has no parent")
        .join(".claude.json");
    let settings_json = claude_dir.join("settings.json");

    for name in ["agents", "memory"] {
        let src = claude_dir.join(name);
        if src.exists() {
            copy_to_backup(state, &src, &backup_dir.join("claude").join(name))?;
        }
    }
    for md in resolve_claude_md(claude_dir) {
        let dst = backup_dir
            .join("claude")
            .join(md.file_name().unwrap_or_default());
        copy_to_backup(state, &md, &dst)?;
    }
    extract_keys_to_file(
        state,
        &claude_json,
        CLAUDE_JSON_KEYS,
        &backup_dir.join("claude/claude.json"),
        None,
    )?;
    extract_keys_to_file(
        state,
        &settings_json,
        SETTINGS_JSON_KEYS,
        &backup_dir.join("claude/settings.json"),
        Some(&*SETTINGS_JSON_DEFAULTS),
    )?;
    Ok(())
}

pub fn do_restore(state: &RunState, backup_dir: &Path, claude_dir: &Path) -> Result<()> {
    let claude_json = claude_dir
        .parent()
        .expect("claude_dir has no parent")
        .join(".claude.json");
    let settings_json = claude_dir.join("settings.json");

    for name in ["agents", "memory"] {
        restore_copy(
            state,
            &backup_dir.join("claude").join(name),
            &claude_dir.join(name),
        )?;
    }
    let backup_claude = backup_dir.join("claude");
    if backup_claude.exists() {
        let mut mds: Vec<_> = fs::read_dir(&backup_claude)?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();
        mds.sort_by_key(|e| e.file_name());
        for entry in mds {
            let md = entry.path();
            restore_copy(
                state,
                &md,
                &claude_dir.join(md.file_name().unwrap_or_default()),
            )?;
        }
    }
    merge_keys_from_file(state, &backup_dir.join("claude/claude.json"), &claude_json)?;
    merge_keys_from_file(
        state,
        &backup_dir.join("claude/settings.json"),
        &settings_json,
    )?;
    Ok(())
}

// ── git operations ────────────────────────────────────────────────────────────

fn git_cmd(state: &RunState, args: &[&str], cwd: &Path) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    if !state.debug {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    cmd.output().expect("git not found in PATH")
}

pub fn do_pull(state: &RunState, repo_dir: &Path) -> Result<bool> {
    let head = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repo_dir)
        .output()?;
    if !head.status.success() {
        state.log("No commits yet, skipping pull.");
        return Ok(true);
    }
    let result = git_cmd(state, &["pull", "--ff-only"], repo_dir);
    if !result.status.success() {
        eprintln!("error: git pull --ff-only failed (history diverged?)");
        return Ok(false);
    }
    Ok(true)
}

pub fn do_commit(state: &RunState, repo_dir: &Path, message: Option<&str>) -> Result<()> {
    git_cmd(state, &["add", "-A"], repo_dir);
    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_dir)
        .output()?;
    if diff.status.success() {
        state.log("Nothing to commit.");
        return Ok(());
    }
    let msg = message.unwrap_or("backup claude config");
    let out = git_cmd(state, &["commit", "-m", msg], repo_dir);
    if !out.status.success() {
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

pub fn do_push(state: &RunState, repo_dir: &Path) -> Result<()> {
    let out = git_cmd(state, &["push", "-u", "origin", "HEAD"], repo_dir);
    if !out.status.success() {
        anyhow::bail!("git push failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

// ── skills ────────────────────────────────────────────────────────────────────

pub fn do_skills(state: &RunState, backup_dir: &Path) -> Result<()> {
    let manifest = backup_dir.join("skills.json");
    if !manifest.exists() {
        anyhow::bail!("error: skills.json not found in backup repo");
    }
    let data: Value = serde_json::from_str(&fs::read_to_string(&manifest)?)?;
    let skills_dir = CLAUDE_DIR.join("skills");
    let skills = match data.get("skills").and_then(Value::as_array) {
        Some(s) => s.clone(),
        None => return Ok(()),
    };
    for skill in &skills {
        let name = skill.get("name").and_then(Value::as_str).unwrap_or("");
        if skills_dir.join(name).exists() {
            state.log(&format!("  skip  {} (already installed)", name));
            continue;
        }
        let cmd = skill.get("install").and_then(Value::as_str).unwrap_or("");
        let source = skill.get("source").and_then(Value::as_str).unwrap_or("?");
        state.log(&format!("  install  {} ({})", name, source));
        Command::new("sh")
            .args(["-c", cmd])
            .output()
            .with_context(|| format!("failed to run install command for {}", name))?;
    }
    Ok(())
}

// ── cleanup ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub dirname: String,
    pub path: PathBuf,
    pub display_name: String,
    pub session_count: usize,
    pub total_size: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CleanupResult {
    pub deleted_files: usize,
    pub deleted_dirs: usize,
    pub freed_bytes: u64,
}

pub fn collect_projects(projects_dir: &Path) -> Vec<ProjectInfo> {
    if !projects_dir.exists() {
        return Vec::new();
    }
    let mut entries: Vec<_> = match fs::read_dir(projects_dir) {
        Ok(e) => e.filter_map(Result::ok).collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by_key(|e| e.file_name());

    entries
        .into_iter()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let path = e.path();
            let dirname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let session_count = fs::read_dir(&path)
                .map(|rd| {
                    rd.filter_map(Result::ok)
                        .filter(|f| f.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
                        .count()
                })
                .unwrap_or(0);
            let total_size = walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|f| f.path().is_file())
                .filter_map(|f| f.metadata().ok())
                .map(|m| m.len())
                .sum();
            let display_name = crate::helpers::pretty_project_name(&dirname, &crate::config::HOME);
            ProjectInfo {
                dirname,
                path,
                display_name,
                session_count,
                total_size,
            }
        })
        .collect()
}

fn mtime_secs(path: &Path) -> f64 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
        })
        .unwrap_or(0.0)
}

fn named_sessions(claude_dir: &Path, project_paths: &[&Path]) -> HashMap<String, String> {
    let mut result = HashMap::new();

    // Pass 1: sessions/*.json metadata files
    let sessions_dir = claude_dir.join("sessions");
    if sessions_dir.exists() {
        let entries = fs::read_dir(&sessions_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok);
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                    if let (Some(sid), Some(name)) = (
                        data.get("sessionId").and_then(Value::as_str),
                        data.get("name")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty()),
                    ) {
                        result.insert(sid.to_string(), name.to_string());
                    }
                }
            }
        }
    }

    // Pass 2: scan .jsonl content for custom-title entries
    for &project_dir in project_paths {
        if !project_dir.exists() {
            continue;
        }
        let entries = fs::read_dir(project_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok);
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if result.contains_key(&stem) {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                for line in text.lines() {
                    if let Ok(entry) = serde_json::from_str::<Value>(line) {
                        if entry.get("type").and_then(Value::as_str) == Some("custom-title") {
                            if let Some(title) = entry.get("customTitle").and_then(Value::as_str) {
                                result.insert(stem.clone(), title.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

pub fn do_cleanup(
    state: &RunState,
    project_paths: &[&Path],
    older_than: u32,
    dry_run: bool,
    claude_dir: &Path,
    with_named_sessions: bool,
) -> Result<CleanupResult> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let cutoff = if older_than > 0 {
        now - (older_than as f64 * 86400.0)
    } else {
        f64::INFINITY
    };

    let named = if with_named_sessions {
        HashMap::new()
    } else {
        named_sessions(claude_dir, project_paths)
    };

    let mut res = CleanupResult::default();

    for &project_dir in project_paths {
        if !project_dir.exists() {
            continue;
        }

        let mut jsonl_files: Vec<_> = fs::read_dir(project_dir)?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
            .collect();
        jsonl_files.sort_by_key(|e| e.file_name());

        for entry in jsonl_files {
            let jsonl = entry.path();
            let stem = jsonl
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if let Some(name) = named.get(&stem) {
                state.log(&format!("  skip  {} ({})", stem, name));
                continue;
            }

            if older_than > 0 && mtime_secs(&jsonl) >= cutoff {
                continue;
            }

            let size = jsonl.metadata().map(|m| m.len()).unwrap_or(0);

            // Handle companion subagent directory (same stem as the .jsonl)
            let subagent_dir = jsonl.parent().unwrap().join(&stem);
            if subagent_dir.is_dir() {
                let dir_size: u64 = walkdir::WalkDir::new(&subagent_dir)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum();
                if dry_run {
                    state.log(&format!(
                        "  would delete  {}/ ({})",
                        subagent_dir
                            .strip_prefix(claude_dir)
                            .unwrap_or(&subagent_dir)
                            .display(),
                        format_size(dir_size)
                    ));
                } else {
                    fs::remove_dir_all(&subagent_dir)?;
                }
                res.freed_bytes += dir_size;
                res.deleted_dirs += 1;
            }

            if dry_run {
                state.log(&format!(
                    "  would delete  {} ({})",
                    jsonl.strip_prefix(claude_dir).unwrap_or(&jsonl).display(),
                    format_size(size)
                ));
            } else {
                fs::remove_file(&jsonl)?;
            }
            res.freed_bytes += size;
            res.deleted_files += 1;
        }

        // Remove the project dir if it is now empty
        if project_dir.exists() && fs::read_dir(project_dir)?.next().is_none() {
            if dry_run {
                state.log(&format!(
                    "  would delete  {}/",
                    project_dir
                        .strip_prefix(claude_dir)
                        .unwrap_or(project_dir)
                        .display()
                ));
            } else {
                fs::remove_dir(project_dir)?;
            }
            res.deleted_dirs += 1;
        }
    }

    // Clean stale session metadata files
    let sessions_dir = claude_dir.join("sessions");
    if sessions_dir.exists() {
        let mut session_files: Vec<_> = fs::read_dir(&sessions_dir)?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .collect();
        session_files.sort_by_key(|e| e.file_name());

        for entry in session_files {
            let sf = entry.path();
            if !with_named_sessions {
                let data: Value = fs::read_to_string(&sf)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
                    .unwrap_or(Value::Null);
                if let Some(name) = data
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    let stem = sf.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    state.log(&format!("  skip  {} ({})", stem, name));
                    continue;
                }
            }
            if older_than > 0 && mtime_secs(&sf) >= cutoff {
                continue;
            }
            let size = sf.metadata().map(|m| m.len()).unwrap_or(0);
            if dry_run {
                state.log(&format!(
                    "  would delete  {} ({})",
                    sf.strip_prefix(claude_dir).unwrap_or(&sf).display(),
                    format_size(size)
                ));
            } else {
                fs::remove_file(&sf)?;
            }
            res.freed_bytes += size;
            res.deleted_files += 1;
        }
    }

    Ok(res)
}

// ── sync ──────────────────────────────────────────────────────────────────────

pub fn do_sync(state: &RunState) -> Result<()> {
    let backup_dir = get_data_dir()?;
    if !do_pull(state, &backup_dir)? {
        anyhow::bail!("sync aborted: git pull --ff-only failed");
    }
    do_restore(state, &backup_dir, &CLAUDE_DIR)?;
    do_skills(state, &backup_dir)?;
    do_backup(state, &backup_dir, &CLAUDE_DIR)?;
    do_commit(state, &backup_dir, None)?;
    do_push(state, &backup_dir)?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::{set_file_mtime, FileTime};
    use std::fs;
    use tempfile::tempdir;

    fn make_old(path: &Path, days: u64) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(days * 86400);
        set_file_mtime(path, FileTime::from_unix_time(secs as i64, 0)).unwrap();
    }

    /// Create a minimal Claude dir structure under `tmp` and return
    /// `(claude_dir, project_dir)`.
    fn build_project(tmp: &Path, name: &str) -> (PathBuf, PathBuf) {
        let claude_dir = tmp.join(".claude");
        let project = claude_dir.join("projects").join(name);
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(claude_dir.join("sessions")).unwrap();
        (claude_dir, project)
    }

    fn add_session_meta(claude_dir: &Path, filename: &str, session_id: &str, name: Option<&str>) {
        let sessions_dir = claude_dir.join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();
        let mut meta = serde_json::json!({"pid": 1, "sessionId": session_id});
        if let Some(n) = name {
            meta["name"] = serde_json::json!(n);
        }
        fs::write(
            sessions_dir.join(filename),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();
    }

    // ── collect_projects ───────────────────────────────────────────────────────

    #[test]
    fn collect_projects_empty_dir() {
        let dir = tempdir().unwrap();
        assert!(collect_projects(&dir.path().join("nope")).is_empty());
    }

    #[test]
    fn collect_projects_counts_sessions() {
        let dir = tempdir().unwrap();
        let proj = dir.path().join("my-proj");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("a.jsonl"), "{}").unwrap();
        fs::write(proj.join("b.jsonl"), "{}").unwrap();

        let infos = collect_projects(dir.path());
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].session_count, 2);
        assert!(infos[0].total_size > 0);
    }

    // ── do_cleanup ─────────────────────────────────────────────────────────────

    #[test]
    fn cleanup_old_files_deleted() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let old_jsonl = project.join("abc-123.jsonl");
        fs::write(&old_jsonl, "{}").unwrap();
        make_old(&old_jsonl, 30);

        let old_session = claude_dir.join("sessions/99999.json");
        fs::write(&old_session, r#"{"pid":1}"#).unwrap();
        make_old(&old_session, 30);

        let state = RunState::new(true);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 2);
        assert!(!old_jsonl.exists());
        assert!(!old_session.exists());
        assert!(!project.exists());
    }

    #[test]
    fn cleanup_recent_files_kept() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let recent = project.join("new.jsonl");
        fs::write(&recent, "{}").unwrap();

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 0);
        assert!(recent.exists());
    }

    #[test]
    fn cleanup_subagent_dir_removed() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let jsonl = project.join("conv-uuid.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        let sub = project.join("conv-uuid/subagents");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("agent-abc.jsonl"), "{}").unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert_eq!(res.deleted_dirs, 2); // subagent dir + empty project dir
        assert!(!jsonl.exists());
        assert!(!(project.join("conv-uuid")).exists());
        assert!(!project.exists());
    }

    #[test]
    fn cleanup_dry_run_keeps_files() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let jsonl = project.join("old.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(true);
        let res = do_cleanup(&state, &[project.as_path()], 7, true, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert!(res.freed_bytes > 0);
        assert!(jsonl.exists()); // dry run — file still there
    }

    #[test]
    fn cleanup_nothing_to_clean() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 0);
        assert_eq!(res.deleted_dirs, 1); // empty project dir removed
        assert_eq!(res.freed_bytes, 0);
    }

    #[test]
    fn cleanup_older_than_zero_deletes_everything() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let recent = project.join("brand-new.jsonl");
        fs::write(&recent, "{}").unwrap();

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 0, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert!(!recent.exists());
    }

    #[test]
    fn cleanup_only_selected_projects() {
        let dir = tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        let proj_a = claude_dir.join("projects/proj-a");
        let proj_b = claude_dir.join("projects/proj-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();
        fs::create_dir_all(claude_dir.join("sessions")).unwrap();

        let a_file = proj_a.join("old.jsonl");
        fs::write(&a_file, "{}").unwrap();
        make_old(&a_file, 30);

        let b_file = proj_b.join("old.jsonl");
        fs::write(&b_file, "{}").unwrap();
        make_old(&b_file, 30);

        let state = RunState::new(false);
        do_cleanup(&state, &[proj_a.as_path()], 7, false, &claude_dir, false).unwrap();

        assert!(!a_file.exists());
        assert!(!proj_a.exists());
        assert!(b_file.exists()); // untouched
    }

    #[test]
    fn cleanup_empty_project_dir_removed() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let old = project.join("only.jsonl");
        fs::write(&old, "{}").unwrap();
        make_old(&old, 30);

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert_eq!(res.deleted_dirs, 1);
        assert!(!project.exists());
    }

    #[test]
    fn cleanup_non_empty_project_dir_kept() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let old = project.join("old.jsonl");
        fs::write(&old, "{}").unwrap();
        make_old(&old, 30);
        let recent = project.join("recent.jsonl");
        fs::write(&recent, "{}").unwrap();

        let state = RunState::new(false);
        do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert!(!old.exists());
        assert!(project.exists());
        assert!(recent.exists());
    }

    // ── named sessions ─────────────────────────────────────────────────────────

    #[test]
    fn named_session_skipped_by_default() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "1.json", "abc-named", Some("my session"));
        let jsonl = project.join("abc-named.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(true);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 0);
        assert!(jsonl.exists());
    }

    #[test]
    fn named_session_deleted_with_flag() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "1.json", "abc-named", Some("my session"));
        let jsonl = project.join("abc-named.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, true).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert!(!jsonl.exists());
    }

    #[test]
    fn unnamed_session_still_deleted() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "1.json", "abc-named", Some("kept"));

        let named_jsonl = project.join("abc-named.jsonl");
        fs::write(&named_jsonl, "{}").unwrap();
        make_old(&named_jsonl, 30);

        let unnamed_jsonl = project.join("def-unnamed.jsonl");
        fs::write(&unnamed_jsonl, "{}").unwrap();
        make_old(&unnamed_jsonl, 30);

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert!(named_jsonl.exists());
        assert!(!unnamed_jsonl.exists());
    }

    #[test]
    fn named_session_metadata_preserved() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "1.json", "abc-named", Some("important"));
        let session_meta = claude_dir.join("sessions/1.json");
        make_old(&session_meta, 30);

        let state = RunState::new(false);
        do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert!(session_meta.exists());
    }

    #[test]
    fn named_session_metadata_deleted_with_flag() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "1.json", "abc-named", Some("important"));
        let session_meta = claude_dir.join("sessions/1.json");
        make_old(&session_meta, 30);

        let state = RunState::new(false);
        do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, true).unwrap();

        assert!(!session_meta.exists());
    }

    #[test]
    fn named_session_via_jsonl_fallback() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named-in-jsonl";
        let jsonl = project.join(format!("{sid}.jsonl"));
        let lines = [
            r#"{"type":"user","message":"hello"}"#,
            &format!(r#"{{"type":"custom-title","customTitle":"My Title","sessionId":"{sid}"}}"#),
        ]
        .join("\n")
            + "\n";
        fs::write(&jsonl, lines).unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(true);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, false).unwrap();

        assert_eq!(res.deleted_files, 0);
        assert!(jsonl.exists());
    }

    #[test]
    fn named_via_jsonl_deleted_with_flag() {
        let dir = tempdir().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named-in-jsonl";
        let jsonl = project.join(format!("{sid}.jsonl"));
        let line =
            format!(r#"{{"type":"custom-title","customTitle":"My Title","sessionId":"{sid}"}}"#)
                + "\n";
        fs::write(&jsonl, line).unwrap();
        make_old(&jsonl, 30);

        let state = RunState::new(false);
        let res = do_cleanup(&state, &[project.as_path()], 7, false, &claude_dir, true).unwrap();

        assert_eq!(res.deleted_files, 1);
        assert!(!jsonl.exists());
    }
}
