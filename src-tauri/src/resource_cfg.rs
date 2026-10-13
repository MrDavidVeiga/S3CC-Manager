use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCfgRule {
    pub priority: i32,
    pub pattern: String,
    pub source_line: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCfgInfo {
    pub path: String,
    pub rules: Vec<ResourceCfgRule>,
    pub warnings: Vec<String>,
    pub precedence_reliable: bool,
}

#[derive(Debug, Clone)]
pub struct PackagePriority {
    pub priority: i32,
    pub rule: String,
}

fn strip_comment(line: &str) -> &str {
    let hash = line.find('#');
    let semicolon = line.find(';');
    match (hash, semicolon) {
        (Some(a), Some(b)) => &line[..a.min(b)],
        (Some(a), None) => &line[..a],
        (None, Some(b)) => &line[..b],
        (None, None) => line,
    }
}

fn normalized_path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn wildcard_match_component(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut dp = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for i in 1..=pattern.len() {
        if pattern[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=pattern.len() {
        for j in 1..=value.len() {
            dp[i][j] = if pattern[i - 1] == b'*' {
                dp[i - 1][j] || dp[i][j - 1]
            } else {
                pattern[i - 1].eq_ignore_ascii_case(&value[j - 1]) && dp[i - 1][j - 1]
            };
        }
    }

    dp[pattern.len()][value.len()]
}

fn wildcard_path_match(pattern: &str, value: &str) -> bool {
    let pattern_parts = pattern
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    let value_parts = value
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<_>>();

    if pattern_parts.len() != value_parts.len() {
        return false;
    }

    pattern_parts
        .iter()
        .zip(value_parts.iter())
        .all(|(pattern, value)| wildcard_match_component(pattern, value))
}

/// Recognize an actual Mods tree even if the user scans a copied Mods folder.
/// A root with the literal name Mods is always eligible (including synthetic
/// test paths). Backup names require evidence of the Packages loading tree.
pub fn is_mods_layout_root(root: &Path) -> bool {
    let Some(name) = root.file_name() else { return false };
    let name = name.to_string_lossy().to_ascii_lowercase();
    if name == "mods" { return true; }
    let copy_name = name.starts_with("mods ") || name.starts_with("mods-")
        || name.starts_with("mods_") || name.starts_with("mods (");
    copy_name && root.join("Packages").is_dir()
        && (root.join("Overrides").is_dir() || root.join("Resource.cfg").is_file())
}

pub fn find_resource_cfg(selected_root: &Path) -> Option<PathBuf> {
    // A deeply selected Packages/Overrides subfolder must still honor the
    // Resource.cfg of its ancestor Mods directory. Do not stop at one parent.
    // Stop as soon as Mods is reached; do not accidentally use a config from
    // an unrelated grandparent above the game's Mods directory.
    for directory in selected_root.ancestors() {
        let candidate = directory.join("Resource.cfg");
        if candidate.is_file() {
            return Some(candidate);
        }
        if is_mods_layout_root(directory) {
            break;
        }
    }
    None
}

pub fn parse_resource_cfg(path: &Path) -> Result<ResourceCfgInfo, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;

    let mut priority = 0i32;
    let mut rules = Vec::new();
    let mut warnings = Vec::new();
    let mut precedence_reliable = true;

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(command) = parts.next() else {
            continue;
        };

        if matches!(
            command.to_ascii_lowercase().as_str(),
            "scan" | "select" | "end" | "stopscan"
        ) {
            precedence_reliable = false;
            warnings.push(format!(
                "Line {line_number}: '{command}' affects advanced Resource.cfg traversal/conditions and is not evaluated for precedence."
            ));
            continue;
        }

        if command.eq_ignore_ascii_case("DirectoryFiles") {
            warnings.push(format!(
                "Line {line_number}: DirectoryFiles is recognized as non-package loading metadata and is ignored by the package precedence analyzer."
            ));
            continue;
        }

        if command.eq_ignore_ascii_case("Priority") {
            let Some(value) = parts.next() else {
                precedence_reliable = false;
                warnings.push(format!("Line {line_number}: Priority has no value; precedence is not considered reliable."));
                continue;
            };
            match value.parse::<i32>() {
                Ok(value) => priority = value,
                Err(_) => {
                    precedence_reliable = false;
                    warnings.push(format!(
                        "Line {line_number}: invalid Priority value '{value}'; precedence is not considered reliable."
                    ));
                }
            }
            continue;
        }

        if command.eq_ignore_ascii_case("PackedFile") {
            let Some(pattern) = parts.next() else {
                precedence_reliable = false;
                warnings.push(format!("Line {line_number}: PackedFile has no path."));
                continue;
            };
            rules.push(ResourceCfgRule {
                priority,
                pattern: pattern.replace('\\', "/"),
                source_line: line_number,
            });
            continue;
        }

        // Do not silently ignore directives that can alter traversal or
        // loading. The analyzer only models the simple Priority/PackedFile
        // subset; an unknown command makes coverage/preference conclusions
        // unsafe and must remain visible to the user.
        precedence_reliable = false;
        warnings.push(format!(
            "Line {line_number}: unsupported Resource.cfg directive '{command}'; precedence is not inferred from this file."
        ));
    }

    Ok(ResourceCfgInfo {
        path: path.to_string_lossy().to_string(),
        rules,
        warnings,
        precedence_reliable,
    })
}

// Scanner cache paths can be ordinary Windows paths (C:\\...), while
// canonicalized roots can use the verbatim form (\\\\?\\C:\\...).
// Compare filesystem identities, not textual prefixes, before deriving
// a Resource.cfg-relative path. In particular, never turn a path outside
// the selected root into a valid match.
pub fn relative_package_path(root: &Path, package_path: &Path) -> Option<PathBuf> {
    if let Ok(relative) = package_path.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }

    let canonical_package = package_path.canonicalize().ok()?;
    if let Ok(relative) = canonical_package.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }

    let canonical_root = root.canonicalize().ok()?;
    canonical_package
        .strip_prefix(&canonical_root)
        .ok()
        .map(Path::to_path_buf)
}

pub fn package_priority(
    cfg: &ResourceCfgInfo,
    cfg_directory: &Path,
    package_path: &Path,
) -> Option<PackagePriority> {
    let relative = relative_package_path(cfg_directory, package_path)?;
    let relative = normalized_path_text(&relative);

    cfg.rules
        .iter()
        .filter(|rule| wildcard_path_match(&rule.pattern, &relative))
        .max_by_key(|rule| rule.priority)
        .map(|rule| PackagePriority {
            priority: rule.priority,
            rule: rule.pattern.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_found_above_nested_packages_and_overrides() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mods = std::env::temp_dir().join(format!("s3cc-cfg-{}-{}", std::process::id(), nonce)).join("Mods");
        let deep = mods.join("Packages").join("Clothing").join("Male");
        let over = mods.join("Overrides").join("Gameplay");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(&over).unwrap();
        std::fs::write(mods.join("Resource.cfg"), b"Priority 500\nPackedFile Packages/*/*/*/*.package\nPackedFile Overrides/*/*.package\n").unwrap();
        assert_eq!(find_resource_cfg(&deep), Some(mods.join("Resource.cfg")));
        assert_eq!(find_resource_cfg(&over), Some(mods.join("Resource.cfg")));
        std::fs::remove_dir_all(mods.parent().unwrap()).unwrap();
    }

    #[test]
    fn cached_package_paths_still_match_resource_cfg_after_root_canonicalization() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "s3cc-resource-cfg-test-{}-{unique}",
            std::process::id()
        ));
        let package_dir = root.join("Packages").join("Creators");
        std::fs::create_dir_all(&package_dir).unwrap();
        let package = package_dir.join("example.package");
        std::fs::write(&package, b"test").unwrap();

        let canonical_root = root.canonicalize().unwrap();
        let relative = relative_package_path(&canonical_root, &package).unwrap();
        assert_eq!(relative, PathBuf::from("Packages").join("Creators").join("example.package"));

        let cfg = ResourceCfgInfo {
            path: root.join("Resource.cfg").to_string_lossy().to_string(),
            rules: vec![ResourceCfgRule {
                priority: 500,
                pattern: "Packages/*/*.package".to_string(),
                source_line: 1,
            }],
            warnings: vec![],
            precedence_reliable: true,
        };
        let priority = package_priority(&cfg, &canonical_root, &package).unwrap();
        assert_eq!(priority.priority, 500);
        assert_eq!(priority.rule, "Packages/*/*.package");

        let outside = root.parent().unwrap().join("some-other-folder").join("example.package");
        assert!(relative_package_path(&canonical_root, &outside).is_none());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn wildcard_matches_one_directory_level_per_star() {
        assert!(wildcard_path_match(
            "Packages/*/*.package",
            "Packages/Hair/a.package"
        ));
        assert!(!wildcard_path_match(
            "Packages/*/*.package",
            "Packages/Hair/Female/a.package"
        ));
        assert!(wildcard_path_match(
            "Packages/*/*/*.package",
            "Packages/Hair/Female/a.package"
        ));
    }


    #[test]
    fn advanced_control_flow_marks_precedence_as_unreliable() {
        let dir = std::env::temp_dir();
        let path = dir.join("s3cc-organizer-resource-test.cfg");
        std::fs::write(
            &path,
            "Priority 500\nPackedFile Packages/*.package\nScan Overrides\n",
        )
        .unwrap();

        let parsed = parse_resource_cfg(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(!parsed.precedence_reliable);
        assert!(parsed.warnings.iter().any(|warning| warning.contains("Scan")));
    }

    #[test]
    fn unsupported_directives_are_reported_without_inventing_precedence() {
        let dir = std::env::temp_dir();
        let path = dir.join("s3cc-organizer-resource-unsupported-test.cfg");
        std::fs::write(
            &path,
            "Priority 500\nPackedFile Packages/*.package\nUnknownTraversalRule Packages/*\n",
        )
        .unwrap();

        let parsed = parse_resource_cfg(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(!parsed.precedence_reliable);
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("unsupported Resource.cfg directive 'UnknownTraversalRule'")));
        assert_eq!(parsed.rules.len(), 1);
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(wildcard_path_match(
            "Packages/*.package",
            "packages/Test.PACKAGE"
        ));
    }
}
