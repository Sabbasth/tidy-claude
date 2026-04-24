use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;

/// Count files in `src` that are new or modified compared to `dst`.
pub fn diff_files(src: &Path, dst: &Path) -> std::io::Result<usize> {
    if src.is_dir() {
        let mut changed = 0;
        for entry in walkdir(src)? {
            let rel = entry.strip_prefix(src).unwrap();
            let dst_f = dst.join(rel);
            if !dst_f.exists() || fs::read(&entry)? != fs::read(&dst_f)? {
                changed += 1;
            }
        }
        return Ok(changed);
    }
    if !dst.exists() || fs::read(src)? != fs::read(dst)? {
        return Ok(1);
    }
    Ok(0)
}

/// Recursively list all files under `dir`.
fn walkdir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk_recursive(dir, &mut files)?;
    Ok(files)
}

fn walk_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

/// Merge `overlay` into `base` in place.
///
/// Objects are recursed, arrays are unioned, scalars are overwritten.
pub fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, oval) in overlay_map {
                let entry = base_map.entry(key.clone());
                match entry {
                    serde_json::map::Entry::Occupied(mut e) => {
                        let bval = e.get_mut();
                        if bval.is_object() && oval.is_object() {
                            deep_merge(bval, oval);
                        } else if bval.is_array() && oval.is_array() {
                            let base_arr = bval.as_array_mut().unwrap();
                            for item in oval.as_array().unwrap() {
                                if !base_arr.contains(item) {
                                    base_arr.push(item.clone());
                                }
                            }
                        } else {
                            *bval = oval.clone();
                        }
                    }
                    serde_json::map::Entry::Vacant(e) => {
                        e.insert(oval.clone());
                    }
                }
            }
        }
        (base, overlay) => {
            *base = overlay.clone();
        }
    }
}

/// Return a human-readable file size.
pub fn format_size(n_bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut size = n_bytes as f64;
    for unit in &units {
        if size < 1024.0 {
            if *unit == "B" {
                return format!("{} B", n_bytes);
            }
            return format!("{:.1} {unit}", size);
        }
        size /= 1024.0;
    }
    format!("{size:.1} TB")
}

/// Return CLAUDE.md and all files it references via `@<name>.md`.
pub fn resolve_claude_md(claude_dir: &Path) -> Vec<PathBuf> {
    let claude_md = claude_dir.join("CLAUDE.md");
    if !claude_md.exists() {
        return vec![];
    }

    let mut files = vec![claude_md.clone()];
    let content = match fs::read_to_string(&claude_md) {
        Ok(c) => c,
        Err(_) => return files,
    };

    let re = Regex::new(r"@(\S+\.md)").unwrap();
    for cap in re.captures_iter(&content) {
        let ref_path = claude_dir.join(&cap[1]);
        if ref_path.exists() {
            files.push(ref_path);
        }
    }
    files
}

/// Convert an encoded project dir name to a readable label.
pub fn pretty_project_name(dirname: &str, home: &Path) -> String {
    let home_prefix = home
        .to_string_lossy()
        .replace('/', "-")
        .replace('.', "-");

    if dirname == home_prefix {
        return "~".into();
    }
    let prefix_dash = format!("{home_prefix}-");
    if !dirname.starts_with(&prefix_dash) {
        return dirname.into();
    }
    let suffix = &dirname[prefix_dash.len()..];

    // github paths: src-github-com-ORG-REPO...
    let gh = "src-github-com-";
    if let Some(org_repo) = suffix.strip_prefix(gh) {
        return if let Some((org, repo)) = org_repo.split_once('-') {
            format!("{org}/{repo}")
        } else {
            org_repo.into()
        };
    }

    // dotfile dirs (encoded as -something)
    if let Some(rest) = suffix.strip_prefix('-') {
        return format!("~/.{rest}");
    }

    format!("~/{suffix}")
}

/// Pick `keys` from `data` and merge `defaults`.
pub fn extract_keys(
    data: &Value,
    keys: &[&str],
    defaults: Option<&Value>,
) -> Value {
    let mut extracted = serde_json::Map::new();
    if let Value::Object(map) = data {
        for &key in keys {
            if let Some(val) = map.get(key) {
                extracted.insert(key.to_string(), val.clone());
            }
        }
    }
    if let Some(Value::Object(defs)) = defaults {
        for (k, v) in defs {
            extracted.insert(k.clone(), v.clone());
        }
    }
    Value::Object(extracted)
}

/// Deep-merge `backed_up` into `current` and return the result.
pub fn merge_keys_data(backed_up: &Value, current: &Value) -> Value {
    let mut result = current.clone();
    deep_merge(&mut result, backed_up);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ── deep_merge ──────────────────────────────────────────────────

    #[test]
    fn deep_merge_scalar_overwrite() {
        let mut base = serde_json::json!({"a": 1});
        deep_merge(&mut base, &serde_json::json!({"a": 2}));
        assert_eq!(base, serde_json::json!({"a": 2}));
    }

    #[test]
    fn deep_merge_new_key() {
        let mut base = serde_json::json!({"a": 1});
        deep_merge(&mut base, &serde_json::json!({"b": 2}));
        assert_eq!(base, serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn deep_merge_nested_dict() {
        let mut base = serde_json::json!({"a": {"x": 1, "y": 2}});
        deep_merge(&mut base, &serde_json::json!({"a": {"y": 3, "z": 4}}));
        assert_eq!(base, serde_json::json!({"a": {"x": 1, "y": 3, "z": 4}}));
    }

    #[test]
    fn deep_merge_list_union() {
        let mut base = serde_json::json!({"a": [1, 2]});
        deep_merge(&mut base, &serde_json::json!({"a": [2, 3]}));
        assert_eq!(base, serde_json::json!({"a": [1, 2, 3]}));
    }

    #[test]
    fn deep_merge_empty_overlay() {
        let mut base = serde_json::json!({"a": 1});
        deep_merge(&mut base, &serde_json::json!({}));
        assert_eq!(base, serde_json::json!({"a": 1}));
    }

    #[test]
    fn deep_merge_empty_base() {
        let mut base = serde_json::json!({});
        deep_merge(&mut base, &serde_json::json!({"a": 1}));
        assert_eq!(base, serde_json::json!({"a": 1}));
    }

    // ── diff_files ──────────────────────────────────────────────────

    #[test]
    fn diff_files_identical() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "hello").unwrap();
        fs::write(&b, "hello").unwrap();
        assert_eq!(diff_files(&a, &b).unwrap(), 0);
    }

    #[test]
    fn diff_files_modified() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "hello").unwrap();
        fs::write(&b, "world").unwrap();
        assert_eq!(diff_files(&a, &b).unwrap(), 1);
    }

    #[test]
    fn diff_files_missing_dst() {
        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.txt");
        fs::write(&a, "hello").unwrap();
        assert_eq!(diff_files(&a, &dir.path().join("nope.txt")).unwrap(), 1);
    }

    #[test]
    fn diff_files_directory_comparison() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        fs::write(src.join("same.txt"), "same").unwrap();
        fs::write(dst.join("same.txt"), "same").unwrap();
        fs::write(src.join("changed.txt"), "new").unwrap();
        fs::write(dst.join("changed.txt"), "old").unwrap();
        fs::write(src.join("added.txt"), "new file").unwrap();

        assert_eq!(diff_files(&src, &dst).unwrap(), 2);
    }

    // ── format_size ─────────────────────────────────────────────────

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    // ── resolve_claude_md ───────────────────────────────────────────

    #[test]
    fn resolve_claude_md_no_file() {
        let dir = TempDir::new().unwrap();
        assert!(resolve_claude_md(dir.path()).is_empty());
    }

    #[test]
    fn resolve_claude_md_with_valid_refs() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("CLAUDE.md"), "See @tips.md and @missing.md\n").unwrap();
        fs::write(root.join("tips.md"), "some tips\n").unwrap();
        // missing.md intentionally absent

        let result = resolve_claude_md(root);
        let names: Vec<_> = result.iter().map(|p| p.file_name().unwrap().to_str().unwrap()).collect();
        assert!(names.contains(&"CLAUDE.md"));
        assert!(names.contains(&"tips.md"));
        assert!(!names.contains(&"missing.md"));
    }

    #[test]
    fn resolve_claude_md_no_refs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "no references here\n").unwrap();
        let result = resolve_claude_md(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name().unwrap().to_str().unwrap(), "CLAUDE.md");
    }

    // ── pretty_project_name ─────────────────────────────────────────

    #[test]
    fn pretty_project_name_home_dir() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(pretty_project_name("-Users-alice", &home), "~");
    }

    #[test]
    fn pretty_project_name_github_org_repo() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(
            pretty_project_name("-Users-alice-src-github-com-acme-widgets", &home),
            "acme/widgets"
        );
    }

    #[test]
    fn pretty_project_name_github_repo_with_dashes() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(
            pretty_project_name("-Users-alice-src-github-com-acme-my-cool-repo", &home),
            "acme/my-cool-repo"
        );
    }

    #[test]
    fn pretty_project_name_dotfile_dir() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(
            pretty_project_name("-Users-alice--config", &home),
            "~/.config"
        );
    }

    #[test]
    fn pretty_project_name_plain_subdir() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(
            pretty_project_name("-Users-alice-projects", &home),
            "~/projects"
        );
    }

    #[test]
    fn pretty_project_name_unknown_prefix() {
        let home = PathBuf::from("/Users/alice");
        assert_eq!(pretty_project_name("-other-path", &home), "-other-path");
    }

    // ── extract_keys ────────────────────────────────────────────────

    #[test]
    fn extract_keys_subset() {
        let data = serde_json::json!({"a": 1, "b": 2, "c": 3});
        let result = extract_keys(&data, &["a", "c"], None);
        assert_eq!(result, serde_json::json!({"a": 1, "c": 3}));
    }

    #[test]
    fn extract_keys_missing_key() {
        let data = serde_json::json!({"a": 1});
        let result = extract_keys(&data, &["a", "z"], None);
        assert_eq!(result, serde_json::json!({"a": 1}));
    }

    #[test]
    fn extract_keys_with_defaults() {
        let data = serde_json::json!({"a": 1});
        let defaults = serde_json::json!({"d": 42});
        let result = extract_keys(&data, &["a"], Some(&defaults));
        assert_eq!(result, serde_json::json!({"a": 1, "d": 42}));
    }

    // ── merge_keys_data ─────────────────────────────────────────────

    #[test]
    fn merge_keys_data_into_empty() {
        let result = merge_keys_data(&serde_json::json!({"a": 1}), &serde_json::json!({}));
        assert_eq!(result, serde_json::json!({"a": 1}));
    }

    #[test]
    fn merge_keys_data_with_overlap() {
        let backed_up = serde_json::json!({"a": {"x": 1}});
        let current = serde_json::json!({"a": {"y": 2}, "b": 3});
        let result = merge_keys_data(&backed_up, &current);
        assert_eq!(result, serde_json::json!({"a": {"x": 1, "y": 2}, "b": 3}));
    }
}
