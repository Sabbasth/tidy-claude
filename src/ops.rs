use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::config;
use crate::error::{Result, TidyError};
use crate::helpers::{
    deep_merge, diff_files, extract_keys, format_size, pretty_project_name, resolve_claude_md,
};
use crate::state::RunState;

// ── file helpers ────────────────────────────────────────────────────

fn copy_to_backup(state: &mut RunState, src: &Path, dst_rel: &str, category: &str, data_dir: &Path) -> Result<()> {
    let dst = data_dir.join(dst_rel);
    let changed = diff_files(src, &dst).unwrap_or(1);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.is_dir() {
        if dst.exists() {
            fs::remove_dir_all(&dst)?;
        }
        copy_dir_recursive(src, &dst)?;
        let count = count_files(&dst);
        state.log(&format!("  copy  {} -> {dst_rel}/ ({count} files)", src.display()));
    } else {
        fs::copy(src, &dst)?;
        state.log(&format!("  copy  {} -> {dst_rel}", src.display()));
    }

    if changed > 0 {
        state.count(&format!("backup:{category}"), changed);
    }
    Ok(())
}

fn restore_copy(state: &mut RunState, backup_rel: &str, target: &Path, category: &str, data_dir: &Path) -> Result<()> {
    let src = data_dir.join(backup_rel);
    if !src.exists() {
        state.log(&format!("  skip  {backup_rel} (not in backup)"));
        return Ok(());
    }

    let changed = diff_files(&src, target).unwrap_or(1);

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.is_dir() {
        copy_dir_recursive(&src, target)?;
        let count = count_files(target);
        state.log(&format!("  restore  {backup_rel}/ -> {} ({count} files)", target.display()));
    } else {
        fs::copy(&src, target)?;
        state.log(&format!("  restore  {backup_rel} -> {}", target.display()));
    }

    if changed > 0 {
        state.count(&format!("restore:{category}"), changed);
    }
    Ok(())
}

fn extract_keys_to_file(
    state: &mut RunState,
    src: &Path,
    keys: &[&str],
    dst_rel: &str,
    category: &str,
    defaults: Option<&Value>,
    data_dir: &Path,
) -> Result<()> {
    if !src.exists() {
        state.log(&format!("  skip  {} (not found)", src.display()));
        return Ok(());
    }

    let text = fs::read_to_string(src)?;
    let data: Value = serde_json::from_str(&text)?;
    let extracted = extract_keys(&data, keys, defaults);

    let dst = data_dir.join(dst_rel);
    let new_content = serde_json::to_string_pretty(&extracted)? + "\n";
    let changed = if dst.exists() {
        fs::read_to_string(&dst).map_or(true, |old| old != new_content)
    } else {
        true
    };

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&dst, &new_content)?;

    let key_names: Vec<_> = extracted
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    state.log(&format!(
        "  extract  {} -> {dst_rel} (keys: {})",
        src.display(),
        key_names.join(", ")
    ));
    if changed {
        state.count(&format!("backup:{category}"), 1);
    }
    Ok(())
}

fn merge_keys_from_file(
    state: &mut RunState,
    backup_rel: &str,
    target: &Path,
    category: &str,
    data_dir: &Path,
) -> Result<()> {
    let src = data_dir.join(backup_rel);
    if !src.exists() {
        state.log(&format!("  skip  {backup_rel} (not in backup)"));
        return Ok(());
    }

    let backed_up: Value = serde_json::from_str(&fs::read_to_string(&src)?)?;

    let mut current: Value = if target.exists() {
        serde_json::from_str(&fs::read_to_string(target)?)?
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        Value::Object(serde_json::Map::new())
    };

    let old_content = serde_json::to_string_pretty(&current)?;
    deep_merge(&mut current, &backed_up);
    let new_content = serde_json::to_string_pretty(&current)?;

    fs::write(target, format!("{new_content}\n"))?;

    let key_names: Vec<_> = backed_up
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    state.log(&format!(
        "  merge  {backup_rel} -> {} (keys: {})",
        target.display(),
        key_names.join(", ")
    ));
    if old_content != new_content {
        state.count(&format!("restore:{category}"), 1);
    }
    Ok(())
}

// ── utility ─────────────────────────────────────────────────────────

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn count_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_files(&path);
            } else if path.is_file() {
                count += 1;
            }
        }
    }
    count
}

fn dir_size(dir: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                size += dir_size(&path);
            } else if path.is_file() {
                size += path.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    size
}

// ── public operations ───────────────────────────────────────────────

pub fn do_backup(state: &mut RunState) -> Result<()> {
    let data_dir = config::get_data_dir()?;
    let claude_dir = config::claude_dir();

    for (name, category) in config::CATEGORY_MAP {
        let d = claude_dir.join(name);
        if d.exists() {
            copy_to_backup(state, &d, &format!("claude/{name}"), category, &data_dir)?;
        }
    }

    for md in resolve_claude_md(&claude_dir) {
        let name = md.file_name().unwrap().to_string_lossy();
        copy_to_backup(state, &md, &format!("claude/{name}"), "configs", &data_dir)?;
    }

    extract_keys_to_file(
        state,
        &config::claude_json(),
        config::CLAUDE_JSON_KEYS,
        "claude/claude.json",
        "settings",
        None,
        &data_dir,
    )?;

    let defaults = config::settings_json_defaults();
    extract_keys_to_file(
        state,
        &config::settings_json(),
        config::SETTINGS_JSON_KEYS,
        "claude/settings.json",
        "settings",
        Some(&defaults),
        &data_dir,
    )?;

    Ok(())
}

pub fn do_restore(state: &mut RunState) -> Result<()> {
    let data_dir = config::get_data_dir()?;
    let claude_dir = config::claude_dir();

    for (name, category) in config::CATEGORY_MAP {
        restore_copy(
            state,
            &format!("claude/{name}"),
            &claude_dir.join(name),
            category,
            &data_dir,
        )?;
    }

    let backup_claude = data_dir.join("claude");
    if backup_claude.exists() {
        let mut mds: Vec<_> = fs::read_dir(&backup_claude)?
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "md")
            })
            .collect();
        mds.sort_by_key(|e| e.file_name());
        for entry in mds {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            restore_copy(
                state,
                &format!("claude/{name_str}"),
                &claude_dir.join(&*name_str),
                "configs",
                &data_dir,
            )?;
        }
    }

    merge_keys_from_file(state, "claude/claude.json", &config::claude_json(), "settings", &data_dir)?;
    merge_keys_from_file(state, "claude/settings.json", &config::settings_json(), "settings", &data_dir)?;

    Ok(())
}

pub fn do_skills(state: &mut RunState) -> Result<()> {
    let data_dir = config::get_data_dir()?;
    let manifest = data_dir.join("skills.json");
    if !manifest.exists() {
        return Err(TidyError::Config("skills.json not found in backup repo".into()));
    }

    let data: Value = serde_json::from_str(&fs::read_to_string(&manifest)?)?;
    let skills_dir = config::claude_dir().join("skills");

    if let Some(skills) = data["skills"].as_array() {
        for skill in skills {
            let name = skill["name"].as_str().unwrap_or("unknown");
            if skills_dir.join(name).exists() {
                state.log(&format!("  skip  {name} (already installed)"));
                continue;
            }
            let cmd = skill["install"].as_str().unwrap_or("");
            let source = skill["source"].as_str().unwrap_or("?");
            state.log(&format!("  install  {name} ({source})"));
            let mut child = Command::new("sh");
            child.arg("-c").arg(cmd);
            if !state.debug {
                child.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            }
            let _ = child.status();
            state.count("skills installed", 1);
        }
    }

    Ok(())
}

pub fn do_pull(state: &mut RunState) -> Result<bool> {
    let data_dir = config::get_data_dir()?;

    // Skip pull if repo has no commits yet
    let head = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(&data_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if !head.success() {
        state.log("No commits yet, skipping pull.");
        return Ok(true);
    }

    let mut cmd = Command::new("git");
    cmd.args(["pull", "--ff-only"]).current_dir(&data_dir);
    if !state.debug {
        cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    }
    let status = cmd.status()?;

    if !status.success() {
        eprintln!("error: git pull --ff-only failed (history diverged?)");
        return Ok(false);
    }
    Ok(true)
}

pub fn do_commit(state: &mut RunState, message: Option<&str>) -> Result<()> {
    let data_dir = config::get_data_dir()?;

    let mut add_cmd = Command::new("git");
    add_cmd.args(["add", "-A"]).current_dir(&data_dir);
    if !state.debug {
        add_cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    }
    let status = add_cmd.status()?;
    if !status.success() {
        return Err(TidyError::Git("git add -A failed".into()));
    }

    let diff = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(&data_dir)
        .status()?;

    if diff.success() {
        state.log("Nothing to commit.");
        return Ok(());
    }

    let msg = message.unwrap_or("backup claude config");
    let mut commit_cmd = Command::new("git");
    commit_cmd.args(["commit", "-m", msg]).current_dir(&data_dir);
    if !state.debug {
        commit_cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    }
    let status = commit_cmd.status()?;
    if !status.success() {
        return Err(TidyError::Git("git commit failed".into()));
    }
    Ok(())
}

pub fn do_push(state: &mut RunState) -> Result<()> {
    let data_dir = config::get_data_dir()?;
    let mut cmd = Command::new("git");
    cmd.args(["push", "-u", "origin", "HEAD"]).current_dir(&data_dir);
    if !state.debug {
        cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(TidyError::Git("git push failed".into()));
    }
    let _ = state;
    Ok(())
}

// ── project collection & cleanup ────────────────────────────────────

pub struct ProjectInfo {
    pub dirname: String,
    pub path: PathBuf,
    pub display_name: String,
    pub session_count: usize,
    pub total_size: u64,
}

#[derive(Default, Debug, PartialEq)]
pub struct CleanupResult {
    pub deleted_files: usize,
    pub deleted_dirs: usize,
    pub freed_bytes: u64,
}

pub fn collect_projects(projects_dir: &Path, home: &Path) -> Vec<ProjectInfo> {
    if !projects_dir.exists() {
        return vec![];
    }

    let mut entries: Vec<_> = fs::read_dir(projects_dir)
        .unwrap_or_else(|_| panic!("cannot read {}", projects_dir.display()))
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    entries
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            let dirname = entry.file_name().to_string_lossy().to_string();
            let jsonl_count = fs::read_dir(&path)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "jsonl")
                })
                .count();
            let total = dir_size(&path);
            ProjectInfo {
                display_name: pretty_project_name(&dirname, home),
                dirname,
                path,
                session_count: jsonl_count,
                total_size: total,
            }
        })
        .collect()
}

fn named_sessions(claude_dir: &Path, project_paths: &[&Path]) -> HashMap<String, String> {
    let mut result = HashMap::new();

    // Pass 1: session metadata (fast, small files)
    let sessions_dir = claude_dir.join("sessions");
    if sessions_dir.exists() {
        if let Ok(entries) = fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(true, |e| e != "json") {
                    continue;
                }
                let data: Value = match fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
                {
                    Some(v) => v,
                    None => continue,
                };
                if let (Some(sid), Some(name)) = (
                    data["sessionId"].as_str(),
                    data["name"].as_str(),
                ) {
                    if !name.is_empty() {
                        result.insert(sid.to_string(), name.to_string());
                    }
                }
            }
        }
    }

    // Pass 2: scan .jsonl files not already known
    for project_dir in project_paths {
        if !project_dir.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(project_dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "jsonl") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            if result.contains_key(&stem) {
                continue;
            }
            let Ok(file) = fs::File::open(&path) else { continue };
            let reader = std::io::BufReader::new(file);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let Ok(entry): std::result::Result<Value, _> = serde_json::from_str(&line) else {
                    continue;
                };
                if entry.is_object()
                    && entry["type"].as_str() == Some("custom-title")
                {
                    if let Some(title) = entry["customTitle"].as_str() {
                        result.insert(stem.clone(), title.to_string());
                        break;
                    }
                }
            }
        }
    }

    result
}

pub fn do_cleanup(
    state: &mut RunState,
    project_paths: &[&Path],
    older_than: u32,
    dry_run: bool,
    claude_dir: &Path,
    with_named_sessions: bool,
) -> CleanupResult {
    let cutoff = if older_than > 0 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            - (older_than as f64 * 86400.0)
    } else {
        f64::INFINITY
    };

    let named = if with_named_sessions {
        HashMap::new()
    } else {
        named_sessions(claude_dir, project_paths)
    };

    let mut res = CleanupResult::default();

    for project_dir in project_paths {
        if !project_dir.exists() {
            continue;
        }

        let mut jsonls: Vec<_> = fs::read_dir(project_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "jsonl"))
            .collect();
        jsonls.sort_by_key(|e| e.file_name());

        for entry in &jsonls {
            let path = entry.path();
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();

            if let Some(name) = named.get(&stem) {
                state.log(&format!("  skip  {stem} ({name})"));
                continue;
            }

            let mtime = path
                .metadata()
                .and_then(|m| m.modified())
                .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|e| std::io::Error::other(e)))
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            if older_than > 0 && mtime >= cutoff {
                continue;
            }

            let size = path.metadata().map(|m| m.len()).unwrap_or(0);

            // Subagent directory
            let subagent_dir = path.parent().unwrap().join(&stem);
            if subagent_dir.is_dir() {
                let sub_size = dir_size(&subagent_dir);
                if dry_run {
                    if let Ok(rel) = subagent_dir.strip_prefix(claude_dir) {
                        state.log(&format!(
                            "  would delete  {}/ ({})",
                            rel.display(),
                            format_size(sub_size)
                        ));
                    }
                } else {
                    let _ = fs::remove_dir_all(&subagent_dir);
                }
                res.freed_bytes += sub_size;
                res.deleted_dirs += 1;
            }

            if dry_run {
                if let Ok(rel) = path.strip_prefix(claude_dir) {
                    state.log(&format!(
                        "  would delete  {} ({})",
                        rel.display(),
                        format_size(size)
                    ));
                }
            } else {
                let _ = fs::remove_file(&path);
            }
            res.freed_bytes += size;
            res.deleted_files += 1;
        }

        // Remove project dir if empty after cleanup
        if project_dir.exists() {
            let is_empty = fs::read_dir(project_dir)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if is_empty {
                if dry_run {
                    if let Ok(rel) = project_dir.strip_prefix(claude_dir) {
                        state.log(&format!("  would delete  {}/", rel.display()));
                    }
                } else {
                    let _ = fs::remove_dir(project_dir);
                }
                res.deleted_dirs += 1;
            }
        }
    }

    // Session metadata (not project-specific)
    let sessions_dir = claude_dir.join("sessions");
    if sessions_dir.exists() {
        let mut session_files: Vec<_> = fs::read_dir(&sessions_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            .collect();
        session_files.sort_by_key(|e| e.file_name());

        for entry in &session_files {
            let path = entry.path();

            if !with_named_sessions {
                let data: Value = fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
                    .unwrap_or(Value::Object(serde_json::Map::new()));
                if let Some(name) = data["name"].as_str() {
                    if !name.is_empty() {
                        state.log(&format!("  skip  {} ({})", path.file_stem().unwrap().to_string_lossy(), name));
                        continue;
                    }
                }
            }

            let mtime = path
                .metadata()
                .and_then(|m| m.modified())
                .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|e| std::io::Error::other(e)))
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            if older_than > 0 && mtime >= cutoff {
                continue;
            }

            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            if dry_run {
                if let Ok(rel) = path.strip_prefix(claude_dir) {
                    state.log(&format!(
                        "  would delete  {} ({})",
                        rel.display(),
                        format_size(size)
                    ));
                }
            } else {
                let _ = fs::remove_file(&path);
            }
            res.freed_bytes += size;
            res.deleted_files += 1;
        }
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;

    fn make_old(path: &Path, days: u64) {
        let old_time = SystemTime::now() - Duration::from_secs(days * 86400);
        let secs = old_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        set_file_mtime(path, FileTime::from_unix_time(secs as i64, 0)).unwrap();
    }

    fn build_project(tmp: &Path, name: &str) -> (PathBuf, PathBuf) {
        let claude_dir = tmp.join(".claude");
        let project = claude_dir.join("projects").join(name);
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(claude_dir.join("sessions")).unwrap();
        (claude_dir, project)
    }

    // ── collect_projects ────────────────────────────────────────────

    #[test]
    fn collect_projects_empty() {
        let dir = TempDir::new().unwrap();
        assert!(collect_projects(&dir.path().join("nope"), &PathBuf::from("/Users/alice")).is_empty());
    }

    #[test]
    fn collect_projects_counts_sessions() {
        let dir = TempDir::new().unwrap();
        let projects_dir = dir.path().join("projects");
        let proj = projects_dir.join("my-proj");
        fs::create_dir_all(&proj).unwrap();
        fs::write(proj.join("a.jsonl"), "{}").unwrap();
        fs::write(proj.join("b.jsonl"), "{}").unwrap();

        let infos = collect_projects(&projects_dir, &PathBuf::from("/Users/alice"));
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].session_count, 2);
        assert!(infos[0].total_size > 0);
    }

    // ── do_cleanup ──────────────────────────────────────────────────

    #[test]
    fn old_files_deleted() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let old_jsonl = project.join("abc-123.jsonl");
        fs::write(&old_jsonl, "{}").unwrap();
        make_old(&old_jsonl, 30);

        let sessions = claude_dir.join("sessions");
        let old_session = sessions.join("99999.json");
        fs::write(&old_session, r#"{"pid": 1}"#).unwrap();
        make_old(&old_session, 30);

        let mut state = RunState { debug: true, ..Default::default() };
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 2);
        assert!(!old_jsonl.exists());
        assert!(!old_session.exists());
        assert!(!project.exists());
    }

    #[test]
    fn recent_files_kept() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let recent = project.join("new.jsonl");
        fs::write(&recent, "{}").unwrap();

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 0);
        assert!(recent.exists());
    }

    #[test]
    fn subagent_dir_removed() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let jsonl = project.join("conv-uuid.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        let sub = project.join("conv-uuid").join("subagents");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("agent-abc.jsonl"), "{}").unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 1);
        assert_eq!(res.deleted_dirs, 2); // subagent dir + empty project dir
        assert!(!jsonl.exists());
        assert!(!project.join("conv-uuid").exists());
        assert!(!project.exists());
    }

    #[test]
    fn dry_run_keeps_files() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let jsonl = project.join("old.jsonl");
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState { debug: true, ..Default::default() };
        let res = do_cleanup(&mut state, &[project.as_path()], 7, true, &claude_dir, false);

        assert_eq!(res.deleted_files, 1);
        assert!(res.freed_bytes > 0);
        assert!(jsonl.exists());
    }

    #[test]
    fn nothing_to_clean() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 0);
        assert_eq!(res.deleted_dirs, 1); // empty project dir removed
        assert_eq!(res.freed_bytes, 0);
    }

    #[test]
    fn older_than_zero_deletes_everything() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let recent = project.join("brand-new.jsonl");
        fs::write(&recent, "{}").unwrap();

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 0, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 1);
        assert!(!recent.exists());
    }

    #[test]
    fn only_selected_projects_cleaned() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        let proj_a = claude_dir.join("projects").join("proj-a");
        let proj_b = claude_dir.join("projects").join("proj-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();
        fs::create_dir_all(claude_dir.join("sessions")).unwrap();

        let a_file = proj_a.join("old.jsonl");
        fs::write(&a_file, "{}").unwrap();
        make_old(&a_file, 30);

        let b_file = proj_b.join("old.jsonl");
        fs::write(&b_file, "{}").unwrap();
        make_old(&b_file, 30);

        let mut state = RunState::default();
        do_cleanup(&mut state, &[proj_a.as_path()], 7, false, &claude_dir, false);

        assert!(!a_file.exists());
        assert!(!proj_a.exists());
        assert!(b_file.exists());
    }

    #[test]
    fn empty_project_dir_removed() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let old = project.join("only.jsonl");
        fs::write(&old, "{}").unwrap();
        make_old(&old, 30);

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 1);
        assert_eq!(res.deleted_dirs, 1);
        assert!(!project.exists());
    }

    #[test]
    fn non_empty_project_dir_kept() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");

        let old = project.join("old.jsonl");
        fs::write(&old, "{}").unwrap();
        make_old(&old, 30);

        let recent = project.join("recent.jsonl");
        fs::write(&recent, "{}").unwrap();

        let mut state = RunState::default();
        do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert!(!old.exists());
        assert!(project.exists());
        assert!(recent.exists());
    }

    // ── named sessions ──────────────────────────────────────────────

    fn add_session_meta(claude_dir: &Path, session_id: &str, name: Option<&str>) {
        let sessions_dir = claude_dir.join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();
        let mut meta = serde_json::json!({"pid": 1, "sessionId": session_id});
        if let Some(n) = name {
            meta["name"] = Value::String(n.to_string());
        }
        fs::write(sessions_dir.join("1.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    }

    #[test]
    fn named_session_skipped_by_default() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named";
        add_session_meta(&claude_dir, sid, Some("my session"));

        let jsonl = project.join(format!("{sid}.jsonl"));
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState { debug: true, ..Default::default() };
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 0);
        assert!(jsonl.exists());
    }

    #[test]
    fn named_session_deleted_with_flag() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named";
        add_session_meta(&claude_dir, sid, Some("my session"));

        let jsonl = project.join(format!("{sid}.jsonl"));
        fs::write(&jsonl, "{}").unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, true);

        assert_eq!(res.deleted_files, 1);
        assert!(!jsonl.exists());
    }

    #[test]
    fn unnamed_session_still_deleted() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "abc-named", Some("kept"));

        let named_jsonl = project.join("abc-named.jsonl");
        fs::write(&named_jsonl, "{}").unwrap();
        make_old(&named_jsonl, 30);

        let unnamed_jsonl = project.join("def-unnamed.jsonl");
        fs::write(&unnamed_jsonl, "{}").unwrap();
        make_old(&unnamed_jsonl, 30);

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 1);
        assert!(named_jsonl.exists());
        assert!(!unnamed_jsonl.exists());
    }

    #[test]
    fn named_session_metadata_preserved() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "abc-named", Some("important"));

        let session_meta = claude_dir.join("sessions").join("1.json");
        make_old(&session_meta, 30);

        let mut state = RunState::default();
        do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert!(session_meta.exists());
    }

    #[test]
    fn named_session_metadata_deleted_with_flag() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        add_session_meta(&claude_dir, "abc-named", Some("important"));

        let session_meta = claude_dir.join("sessions").join("1.json");
        make_old(&session_meta, 30);

        let mut state = RunState::default();
        do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, true);

        assert!(!session_meta.exists());
    }

    #[test]
    fn named_via_jsonl_fallback() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named-in-jsonl";

        let jsonl = project.join(format!("{sid}.jsonl"));
        let lines = format!(
            "{}\n{}\n",
            serde_json::json!({"type": "user", "message": "hello"}),
            serde_json::json!({"type": "custom-title", "customTitle": "My Title", "sessionId": sid}),
        );
        fs::write(&jsonl, &lines).unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState { debug: true, ..Default::default() };
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, false);

        assert_eq!(res.deleted_files, 0);
        assert!(jsonl.exists());
    }

    #[test]
    fn named_via_jsonl_deleted_with_flag() {
        let dir = TempDir::new().unwrap();
        let (claude_dir, project) = build_project(dir.path(), "some-project");
        let sid = "abc-named-in-jsonl";

        let jsonl = project.join(format!("{sid}.jsonl"));
        let lines = format!(
            "{}\n",
            serde_json::json!({"type": "custom-title", "customTitle": "My Title", "sessionId": sid}),
        );
        fs::write(&jsonl, &lines).unwrap();
        make_old(&jsonl, 30);

        let mut state = RunState::default();
        let res = do_cleanup(&mut state, &[project.as_path()], 7, false, &claude_dir, true);

        assert_eq!(res.deleted_files, 1);
        assert!(!jsonl.exists());
    }
}
