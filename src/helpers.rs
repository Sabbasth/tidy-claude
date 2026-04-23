//! Pure helper functions: JSON merging, file diffing, string formatting.

use serde_json::Value;
use std::path::Path;

/// Deep merge two JSON values (right overwrites left).
pub fn deep_merge(left: &mut Value, right: &Value) {
    if let Value::Object(left_map) = left {
        if let Value::Object(right_map) = right {
            for (key, right_val) in right_map {
                let left_val = left_map
                    .entry(key.clone())
                    .or_insert_with(|| Value::Null);
                deep_merge(left_val, right_val);
            }
            return;
        }
    }
    *left = right.clone();
}

/// Check if two files differ (returns true if different).
pub fn diff_files(_src: &Path, _dst: &Path) -> bool {
    todo!("diff_files: compare file contents")
}

/// Extract specific keys from a JSON object.
pub fn extract_keys(_obj: &Value, _keys: &[&str]) -> Value {
    todo!("extract_keys: extract subset of JSON object")
}

/// Format a byte size as human-readable string.
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[(&str, u64)] = &[
        ("B", 1),
        ("KB", 1024),
        ("MB", 1024 * 1024),
        ("GB", 1024 * 1024 * 1024),
    ];

    for &(unit, divisor) in UNITS.iter().rev() {
        if bytes >= divisor {
            return format!("{:.1} {}", bytes as f64 / divisor as f64, unit);
        }
    }
    format!("0 B")
}

/// Pretty-print a project name from path.
pub fn pretty_project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("(unknown)")
        .to_string()
}

/// Resolve @-references in CLAUDE.md.
pub fn resolve_claude_md(_content: &str) -> String {
    todo!("resolve_claude_md: expand @-references")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge() {
        let mut left = json!({ "a": 1, "b": { "c": 2 } });
        let right = json!({ "b": { "d": 3 } });
        deep_merge(&mut left, &right);
        assert_eq!(left["b"]["c"], 2);
        assert_eq!(left["b"]["d"], 3);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }
}
