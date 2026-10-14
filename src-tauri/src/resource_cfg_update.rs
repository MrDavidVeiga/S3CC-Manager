//! Opt-in, conservative Resource.cfg depth extension for actual Sims 3 Mods trees.
//! Existing lines and priority groups are never rewritten.
use crate::resource_cfg::{find_resource_cfg, package_priority, parse_resource_cfg, ResourceCfgInfo, ResourceCfgRule};
use crate::manifest::ResourceCfgRestoreSnapshot;
use walkdir::WalkDir;
use chrono::Local;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeSet, HashMap, HashSet}, fs, io::Write, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCfgUpdate {
    pub path: String,
    pub added_rules: Vec<String>,
    pub backup_required: bool,
    pub original_hash: String,
}

pub struct AppliedResourceCfg {
    path: PathBuf,
    backup_path: Option<PathBuf>,
    installed_text: String,
}

impl AppliedResourceCfg {
    pub fn restore_snapshot(&self, change: &ResourceCfgUpdate) -> ResourceCfgRestoreSnapshot {
        ResourceCfgRestoreSnapshot {
            path: self.path.clone(),
            backup_path: self.backup_path.clone(),
            original_sha256: change.original_hash.clone(),
            updated_sha256: hash_text(&self.installed_text),
        }
    }
}

fn hash_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn mods_ancestor(root: &Path) -> Option<&Path> {
    root.ancestors().find(|p| crate::resource_cfg::is_mods_layout_root(p))
}

fn inspect_cfg(path: &Path) -> Result<(String, ResourceCfgInfo), String> {
    let text = if path.exists() {
        if !path.is_file() { return Err(format!("Resource.cfg is not a regular file: {}", path.display())); }
        fs::read_to_string(path).map_err(|e| format!("Resource.cfg must be readable UTF-8 before automatic editing: {e}"))?
    } else { String::new() };

    // Unknown directives, advanced conditional scans and malformed priorities
    // may change interpretation when appended; require manual review.
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.split(['#', ';']).next().unwrap_or("").trim();
        let Some(command) = trimmed.split_whitespace().next() else { continue };
        if !["priority", "packedfile", "directoryfiles"].contains(&command.to_ascii_lowercase().as_str()) {
            return Err(format!("Resource.cfg line {} uses an unsupported directive '{}'; left unchanged.", i + 1, command));
        }
        if command.eq_ignore_ascii_case("Priority")
            && trimmed.split_whitespace().nth(1).and_then(|x| x.parse::<i32>().ok()).is_none() {
            return Err(format!("Resource.cfg line {} has an invalid Priority; left unchanged.", i + 1));
        }
    }

    let info = if path.exists() {
        let parsed = parse_resource_cfg(path)?;
        if !parsed.precedence_reliable { return Err("Resource.cfg contains advanced rules; automatic modification is disabled.".to_owned()); }
        parsed
    } else {
        ResourceCfgInfo {
            path: path.to_string_lossy().to_string(),
            rules: Vec::new(), warnings: Vec::new(), precedence_reliable: true,
        }
    };
    Ok((text, info))
}

pub fn preview_resource_cfg_update(root: &Path, destinations: &[PathBuf]) -> Result<Option<ResourceCfgUpdate>, String> {
    let Some(mods) = mods_ancestor(root) else { return Ok(None) };
    let path = mods.join("Resource.cfg");
    if let Some(found) = find_resource_cfg(root) {
        if found != path {
            return Err(format!("Nested Resource.cfg '{}' requires manual inspection before editing the Mods-level rules.", found.display()));
        }
    }
    let (original, mut info) = inspect_cfg(&path)?;
    let mut added = BTreeSet::<(i32, String)>::new();
    // An established loading branch must have a single priority tier for
    // safe automatic extension. Never invent a winner between custom tiers.
    let mut tiers = HashMap::<String, HashSet<i32>>::new();
    for rule in &info.rules {
        if let Some(branch) = rule.pattern.replace('\\', "/").split('/').next() {
            let key = branch.to_ascii_lowercase();
            if key == "packages" || key == "overrides" {
                tiers.entry(key).or_default().insert(rule.priority);
            }
        }
    }

    for destination in destinations {
        // Disabled DBPF files are classified and moved with their .disabled
        // suffix, but they MUST NOT gain game loading rules or fail a preview.
        if destination.file_name().is_some_and(|name| {
            let name = name.to_string_lossy().to_ascii_lowercase();
            name.ends_with(".package.disabled") || name.ends_with(".package.duplicated")
        }) {
            continue;
        }
        let relative = destination.strip_prefix(mods)
            .map_err(|_| format!("Planned destination escaped Mods: {}", destination.display()))?;
        let parts = relative.components().map(|p| p.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>();
        if parts.len() < 2 || !parts.last().is_some_and(|f| f.to_ascii_lowercase().ends_with(".package")) {
            return Err(format!("Invalid package destination for Resource.cfg: {}", destination.display()));
        }
        let branch = match parts[0].to_ascii_lowercase().as_str() {
            "packages" => "Packages",
            "overrides" => "Overrides",
            _ => return Err(format!("Resource.cfg cannot load this outside Packages/Overrides: {}", destination.display())),
        };
        if package_priority(&info, mods, destination).is_some() { continue; }

        let existing_tiers = tiers.get(&branch.to_ascii_lowercase());
        if existing_tiers.is_some_and(|p| p.len() > 1) {
            return Err(format!("{} uses multiple Resource.cfg priorities; add the missing rules manually to preserve load order.", branch));
        }
        let priority = existing_tiers.and_then(|p| p.iter().next().copied())
            .unwrap_or(if branch == "Overrides" { 1000 } else { 500 });
        let depth = parts.len() - 2;
        if depth > 32 {
            return Err("Resource.cfg nesting exceeds 32 levels; manual review is required.".to_owned());
        }
        let pattern = format!("{}{}/*.package", branch, "/*".repeat(depth));
        if added.insert((priority, pattern.clone())) {
            info.rules.push(ResourceCfgRule { priority, pattern, source_line: 0 });
        }
    }

    let added_rules = added.into_iter()
        .map(|(priority, pattern)| format!("Priority {priority}\nPackedFile {pattern}"))
        .collect();
    Ok(Some(ResourceCfgUpdate {
        path: path.to_string_lossy().to_string(),
        added_rules,
        backup_required: path.exists(),
        original_hash: hash_text(&original),
    }))
}

fn render_updated_cfg(original: &str, added: &[String]) -> String {
    let newline = if original.contains("\r\n") { "\r\n" } else { "\n" };
    let mut text = original.to_string();
    if !text.is_empty() && !text.ends_with('\n') { text.push_str(newline); }
    if !text.is_empty() { text.push_str(newline); }
    text.push_str("# S3CC Manager: additional subfolder coverage (opt-in)");
    text.push_str(newline);
    for rule in added {
        text.push_str(&rule.replace('\n', newline));
        text.push_str(newline);
    }
    text
}

fn unique_sibling(path: &Path, suffix: &str) -> Result<PathBuf, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let parent = path.parent().ok_or("Resource.cfg has no parent")?;
    for index in 0..1000 {
        let filename = if index == 0 { format!("Resource.cfg.{suffix}-{stamp}") }
            else { format!("Resource.cfg.{suffix}-{stamp}-{index}") };
        let candidate = parent.join(filename);
        if !candidate.exists() { return Ok(candidate); }
    }
    Err("Could not reserve a unique Resource.cfg backup/temp name.".into())
}

pub fn apply_resource_cfg_update(change: &ResourceCfgUpdate) -> Result<Option<AppliedResourceCfg>, String> {
    if change.added_rules.is_empty() { return Ok(None); }
    let path = PathBuf::from(&change.path);
    let (original, _) = inspect_cfg(&path)?;
    if hash_text(&original) != change.original_hash {
        return Err("Resource.cfg changed after preview; rebuild the plan before organizing.".into());
    }
    let updated = render_updated_cfg(&original, &change.added_rules);
    let temp = unique_sibling(&path, "s3cc-temp")?;
    let mut handle = fs::OpenOptions::new().write(true).create_new(true).open(&temp)
        .map_err(|e| format!("Could not create temporary Resource.cfg: {e}"))?;
    if let Err(e) = handle.write_all(updated.as_bytes()).and_then(|_| handle.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(format!("Could not write temporary Resource.cfg: {e}"));
    }
    drop(handle);
    let backup = if path.exists() {
        let target = unique_sibling(&path, "s3cc-backup")?;
        if let Err(e) = fs::rename(&path, &target) {
            let _ = fs::remove_file(&temp);
            return Err(format!("Could not back up Resource.cfg: {e}"));
        }
        Some(target)
    } else { None };
    if let Err(e) = fs::rename(&temp, &path) {
        if let Some(backup) = &backup { let _ = fs::rename(backup, &path); }
        let _ = fs::remove_file(&temp);
        return Err(format!("Could not install Resource.cfg update; restoration attempted: {e}"));
    }
    Ok(Some(AppliedResourceCfg { path, backup_path: backup, installed_text: updated }))
}

// Only restore the cfg if every package move was rolled back. Keep a
// permanent backup even after successful organization.
pub fn rollback_resource_cfg_update(applied: AppliedResourceCfg) -> Result<(), String> {
    let current = fs::read_to_string(&applied.path)
        .map_err(|e| format!("Cannot verify Resource.cfg for rollback: {e}"))?;
    if current != applied.installed_text {
        return Err("Resource.cfg changed after update; refusing to overwrite external edits.".into());
    }
    if let Some(backup) = applied.backup_path {
        let original = fs::read(&backup).map_err(|e| format!("Could not read Resource.cfg backup: {e}"))?;
        fs::write(&applied.path, original).map_err(|e| format!("Could not restore original Resource.cfg: {e}"))?;
    } else {
        fs::remove_file(&applied.path).map_err(|e| format!("Could not remove newly created Resource.cfg: {e}"))?;
    }
    Ok(())
}

/// After a successful CC restore, restore the old cfg only when doing so
/// cannot hide an existing package. On any uncertainty keep the updated cfg
/// and its backup, and return an explicit warning.
pub fn restore_cfg_if_safe(
    root: &Path,
    snapshot: &ResourceCfgRestoreSnapshot,
) -> Result<Option<String>, String> {
    let mods = mods_ancestor(root).ok_or("Restore root has no Mods ancestor.")?;
    let expected = mods.join("Resource.cfg");
    if snapshot.path != expected {
        return Err("Resource.cfg restore record points outside the selected Mods root.".into());
    }
    let current = fs::read(&expected).map_err(|e| format!("Could not inspect Resource.cfg before restore: {e}"))?;
    let current_hash = format!("{:x}", Sha256::digest(&current));
    if current_hash.eq_ignore_ascii_case(&snapshot.original_sha256) {
        return Ok(None);
    }
    if !current_hash.eq_ignore_ascii_case(&snapshot.updated_sha256) {
        return Ok(Some("Resource.cfg was modified after organization. Kept current configuration and original backup.".into()));
    }

    let Some(backup) = &snapshot.backup_path else {
        // Removing the only loading file could hide CC: do not delete it.
        return Ok(Some("No original Resource.cfg existed; kept the generated configuration for safety.".into()));
    };
    let safe_backup = backup.parent() == Some(mods)
        && backup.file_name().and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("Resource.cfg.s3cc-backup-"));
    if !safe_backup {
        return Err("Resource.cfg manifest points to a backup outside the Mods directory.".into());
    }
    let old_content = fs::read(backup).map_err(|e| format!("Resource.cfg backup is missing: {e}"))?;
    if format!("{:x}", Sha256::digest(&old_content)) != snapshot.original_sha256.to_ascii_lowercase() {
        return Err("Resource.cfg backup checksum differs from the original; current configuration preserved.".into());
    }
    let old_rules = parse_resource_cfg(backup)?;
    if !old_rules.precedence_reliable {
        return Ok(Some("Original Resource.cfg has advanced directives; kept current configuration pending manual review.".into()));
    }
    for branch in ["Packages", "Overrides"] {
        let dir = mods.join(branch);
        if !dir.is_dir() { continue; }
        for item in WalkDir::new(&dir).follow_links(false).into_iter() {
            let item = item.map_err(|e| format!("Cannot validate packages before restoring Resource.cfg: {e}"))?;
            if !item.file_type().is_file() || !item.path().extension()
                .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("package")) {
                continue;
            }
            if package_priority(&old_rules, mods, item.path()).is_none() {
                return Ok(Some(format!(
                    "Original Resource.cfg does not load '{}'; kept expanded configuration to protect existing CC.",
                    item.path().display()
                )));
            }
        }
    }
    // Backups survive restore for manual recovery; never overwrite them.
    fs::write(&expected, old_content)
        .map_err(|e| format!("Cannot restore backed-up Resource.cfg (current file retained if possible): {e}"))?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_cfg_recovers_original_when_all_packages_are_covered() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-cfg-restore-{}-{nonce}", std::process::id()));
        let mods = root.join("Mods");
        fs::create_dir_all(mods.join("Packages")).unwrap();
        let cfg = mods.join("Resource.cfg");
        let original = "Priority 500\nPackedFile Packages/*.package\n";
        fs::write(&cfg, original).unwrap();
        let future = mods.join("Packages/Category/test.package");
        let change = preview_resource_cfg_update(&mods, &[future]).unwrap().unwrap();
        let applied = apply_resource_cfg_update(&change).unwrap().unwrap();
        let record = applied.restore_snapshot(&change);
        assert!(restore_cfg_if_safe(&mods, &record).unwrap().is_none());
        assert_eq!(fs::read_to_string(&cfg).unwrap(), original);
        assert!(record.backup_path.unwrap().exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn appends_only_missing_depth_and_preserves_priorities() {
        let root = std::env::temp_dir().join(format!("s3cc-cfg-depth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Mods/Packages")).unwrap();
        let mods = root.join("Mods");
        let cfg = mods.join("Resource.cfg");
        fs::write(&cfg, "Priority 400\r\nPackedFile Packages/*.package\r\nPriority 1200\r\nPackedFile Overrides/*.package\r\n").unwrap();
        let paths = vec![mods.join("Packages/CAS/Clothing/Male/top.package"), mods.join("Overrides/Scripts/core.package")];
        let preview = preview_resource_cfg_update(&mods, &paths).unwrap().unwrap();
        assert!(preview.added_rules.iter().any(|v| v.contains("Priority 400\nPackedFile Packages/*/*/*/*.package")));
        assert!(preview.added_rules.iter().any(|v| v.contains("Priority 1200\nPackedFile Overrides/*/*.package")));
        let applied = apply_resource_cfg_update(&preview).unwrap().unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("PackedFile Overrides/*/*.package"));
        rollback_resource_cfg_update(applied).unwrap();
        assert!(fs::read_to_string(&cfg).unwrap().contains("Priority 1200\r\nPackedFile Overrides/*.package"));
        let _ = fs::remove_dir_all(root);
    }
}
