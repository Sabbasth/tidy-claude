//! Pure / near-pure functions — no side effects beyond reading the filesystem.

use regex::Regex;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn diff_files(src: &Path, dst: &Path) -> usize {
    if src.is_dir() {
        let mut changed = 0usize;
        for entry in walkdir::WalkDir::new(src)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let rel = path.strip_prefix(src).expect("path should be under src");
            let dst_f = dst.join(rel);
            let differs = !dst_f.exists()
                || fs::read(path).expect("read src") != fs::read(&dst_f).unwrap_or_default();
            if differs {
                changed += 1;
            }
        }
        changed
    } else if !dst.exists() || fs::read(src).expect("read src") != fs::read(dst).unwrap_or_default() {
        1
    } else {
        0
    }
}

pub fn deep_merge(base: &mut Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, val) in overlay_map {
                match base_map.get_mut(key) {
                    Some(existing) => {
                        deep_merge(existing, val);
                    }
                    None => {
                        base_map.insert(key.clone(), val.clone());
                    }
                }
            }
            Value::Object(base_map.clone())
        }
        (Value::Array(base_arr), Value::Array(overlay_arr)) => {
            for item in overlay_arr {
                if !base_arr.contains(item) {
                    base_arr.push(item.clone());
                }
            }
            Value::Array(base_arr.clone())
        }
        (base_slot, other) => {
            *base_slot = other.clone();
            base_slot.clone()
        }
    }
}

pub fn extract_keys(data: &Value, keys: &[&str], defaults: Option<&Value>) -> Value {
    let mut out = Map::new();

    if let Some(map) = data.as_object() {
        for key in keys {
            if let Some(value) = map.get(*key) {
                out.insert((*key).to_string(), value.clone());
            }
        }
    }

    if let Some(defaults) = defaults.and_then(Value::as_object) {
        for (key, value) in defaults {
            out.insert(key.clone(), value.clone());
        }
    }

    Value::Object(out)
}

pub fn merge_keys_data(backed_up: &Value, current: &Value) -> Value {
    let mut current = current.clone();
    deep_merge(&mut current, backed_up)
}

pub fn format_size(n_bytes: u64) -> String {
    if n_bytes < 1024 {
        return format!("{} B", n_bytes);
    }

    let mut size = n_bytes as f64;
    for unit in ["KB", "MB", "GB"] {
        size /= 1024.0;
        if size < 1024.0 || unit == "GB" {
            return format!("{size:.1} {unit}");
        }
    }

    format!("{size:.1} TB")
}

pub fn pretty_project_name(dirname: &str, home: &Path) -> String {
    let home_prefix = home.to_string_lossy().replace('/', "-").replace('.', "-");
    if dirname == home_prefix {
        return "~".to_string();
    }
    if !dirname.starts_with(&(home_prefix.clone() + "-")) {
        return dirname.to_string();
    }

    let suffix = &dirname[home_prefix.len() + 1..];
    if let Some(org_repo) = suffix.strip_prefix("src-github-com-") {
        if let Some((org, repo)) = org_repo.split_once('-') {
            return format!("{org}/{repo}");
        }
        return org_repo.to_string();
    }
    if let Some(rest) = suffix.strip_prefix('-') {
        return format!("~/.{rest}");
    }
    format!("~/{suffix}")
}

pub fn resolve_claude_md(claude_dir: &Path) -> Vec<PathBuf> {
    let claude_md = claude_dir.join("CLAUDE.md");
    if !claude_md.exists() {
        return Vec::new();
    }

    let mut files = vec![claude_md.clone()];
    let content = fs::read_to_string(&claude_md).unwrap_or_default();
    let re = Regex::new(r"@(\S+\.md)").expect("valid regex");
    for cap in re.captures_iter(&content) {
        let ref_path = claude_dir.join(&cap[1]);
        if ref_path.exists() {
            files.push(ref_path);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn deep_merge_scalar_overwrite() {
        let mut base = json!({"a": 1});
        let overlay = json!({"a": 2});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": 2}));
    }

    #[test]
    fn deep_merge_new_key() {
        let mut base = json!({"a": 1});
        let overlay = json!({"b": 2});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn deep_merge_nested_dict() {
        let mut base = json!({"a": {"x": 1, "y": 2}});
        let overlay = json!({"a": {"y": 3, "z": 4}});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": {"x": 1, "y": 3, "z": 4}}));
    }

    #[test]
    fn deep_merge_list_union() {
        let mut base = json!({"a": [1, 2]});
        let overlay = json!({"a": [2, 3]});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": [1, 2, 3]}));
    }

    #[test]
    fn deep_merge_empty_overlay() {
        let mut base = json!({"a": 1});
        let overlay = json!({});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": 1}));
    }

    #[test]
    fn deep_merge_empty_base() {
        let mut base = json!({});
        let overlay = json!({"a": 1});
        assert_eq!(deep_merge(&mut base, &overlay), json!({"a": 1}));
    }

    #[test]
    fn diff_files_identical_files() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "hello").unwrap();
        fs::write(&b, "hello").unwrap();
        assert_eq!(diff_files(&a, &b), 0);
    }

    #[test]
    fn diff_files_modified_file() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "hello").unwrap();
        fs::write(&b, "world").unwrap();
        assert_eq!(diff_files(&a, &b), 1);
    }

    #[test]
    fn diff_files_missing_dst() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        fs::write(&a, "hello").unwrap();
        assert_eq!(diff_files(&a, &dir.path().join("nope.txt")), 1);
    }

    #[test]
    fn diff_files_directory_comparison() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dst).unwrap();
        fs::write(src.join("same.txt"), "same").unwrap();
        fs::write(dst.join("same.txt"), "same").unwrap();
        fs::write(src.join("changed.txt"), "new").unwrap();
        fs::write(dst.join("changed.txt"), "old").unwrap();
        fs::write(src.join("added.txt"), "new file").unwrap();
        assert_eq!(diff_files(&src, &dst), 2);
    }

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
        assert_eq!(format_size(1024_u64.pow(3)), "1.0 GB");
    }

    #[test]
    fn resolve_claude_md_no_file() {
        let dir = tempdir().unwrap();
        assert!(resolve_claude_md(dir.path()).is_empty());
    }

    #[test]
    fn resolve_claude_md_with_valid_refs() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "hello @tips.md @missing.md").unwrap();
        fs::write(dir.path().join("tips.md"), "tip").unwrap();
        let result = resolve_claude_md(dir.path());
        let names: Vec<_> = result.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert!(names.contains(&"CLAUDE.md".to_string()));
        assert!(names.contains(&"tips.md".to_string()));
        assert!(!names.contains(&"missing.md".to_string()));
    }

    #[test]
    fn resolve_claude_md_no_refs() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "no references here\n").unwrap();
        let result = resolve_claude_md(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name().unwrap().to_string_lossy(), "CLAUDE.md");
    }

    #[test]
    fn pretty_project_name_cases() {
        let home = Path::new("/Users/alice");
        assert_eq!(pretty_project_name("-Users-alice", home), "~");
        assert_eq!(pretty_project_name("-Users-alice-src-github-com-acme-widgets", home), "acme/widgets");
        assert_eq!(pretty_project_name("-Users-alice-src-github-com-acme-my-cool-repo", home), "acme/my-cool-repo");
        assert_eq!(pretty_project_name("-Users-alice--config", home), "~/.config");
        assert_eq!(pretty_project_name("-Users-alice-projects", home), "~/projects");
        assert_eq!(pretty_project_name("-other-path", home), "-other-path");
    }

    #[test]
    fn extract_keys_subset() {
        let data = json!({"a": 1, "b": 2, "c": 3});
        assert_eq!(extract_keys(&data, &["a", "c"], None), json!({"a": 1, "c": 3}));
    }

    #[test]
    fn extract_keys_missing_key() {
        let data = json!({"a": 1});
        assert_eq!(extract_keys(&data, &["a", "z"], None), json!({"a": 1}));
    }

    #[test]
    fn extract_keys_with_defaults() {
        let data = json!({"a": 1});
        let defaults = json!({"d": 42});
        assert_eq!(extract_keys(&data, &["a"], Some(&defaults)), json!({"a": 1, "d": 42}));
    }

    #[test]
    fn merge_keys_data_into_empty() {
        let backed_up = json!({"a": 1});
        let current = json!({});
        assert_eq!(merge_keys_data(&backed_up, &current), json!({"a": 1}));
    }

    #[test]
    fn merge_keys_data_with_overlap() {
        let backed_up = json!({"a": {"x": 1}});
        let current = json!({"a": {"y": 2}, "b": 3});
        assert_eq!(merge_keys_data(&backed_up, &current), json!({"a": {"x": 1, "y": 2}, "b": 3}));
    }
}
