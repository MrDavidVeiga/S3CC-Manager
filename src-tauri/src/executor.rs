use crate::{
    workspace::ensure_writable,
    i18n::AppLanguage,
    manifest::{
        replace_manifest_atomic, sha256_file, write_manifest_atomic, RestoreEntry, RestoreManifest,
    },
    planner::{
        build_organization_plan_with_cfg, normalized_path_key, OrganizationAnalysisSnapshot,
        OrganizationPlan, PlanItem,
    },
    scanner::{invalidate_latest_scan_for, scan_packages_core},
    operation,
    resource_cfg::{parse_resource_cfg, package_priority},
    resource_cfg_update::{apply_resource_cfg_update, rollback_resource_cfg_update},
};
use chrono::{Local, SecondsFormat};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub status: String,
    /// Verified moves allow the frontend to reconcile its scanned library
    /// without decoding thousands of DBPFs again after organization.
    pub moved_files: Vec<OrganizationMove>,
    pub organized_directory: String,
    pub manifest_path: Option<String>,
    pub moved: usize,
    pub already_organized: usize,
    pub rolled_back: usize,
    pub old_folders_removed: usize,
    pub old_folders_retained: usize,
    pub remaining_legacy_files: usize,
    pub remaining_legacy_examples: Vec<String>,
    pub remaining_legacy_details: Vec<LegacyRemainder>,
    pub errors: Vec<String>,
    pub analysis_before: OrganizationAnalysisSnapshot,
    pub execution: OrganizationAnalysisSnapshot,
    pub analysis_current: Option<OrganizationAnalysisSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationMove {
    pub source_path: String,
    pub destination_path: String,
    pub destination_relative_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRemainder {
    pub relative_path: String,
    pub reason: String,
    pub plan_status: Option<String>,
    pub warning: Option<String>,
}

fn organized_parent(root: &Path, pairs: &[(PathBuf, PathBuf, String, u64)]) -> PathBuf {
    let Some((_, first, _, _)) = pairs.first() else {
        return root.to_path_buf();
    };
    let mut common = first.parent().unwrap_or(root).to_path_buf();
    for (_, destination, _, _) in pairs.iter().skip(1) {
        while !destination.starts_with(&common) {
            let Some(parent) = common.parent() else { return root.to_path_buf() };
            common = parent.to_path_buf();
        }
    }
    if !common.starts_with(root) { return root.to_path_buf(); }
    common
}

fn make_manifest_path(root: &Path) -> Result<PathBuf, String> {
    let base = std::env::var_os("S3CC_RESTORE_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root
            .parent()
            .unwrap_or(root)
            .join("S3CC Manager")
            .join("Restore Manifests"));

    fs::create_dir_all(&base)
        .map_err(|error| format!("Could not create restore manifest directory {}: {error}", base.display()))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    for suffix in 0..10_000usize {
        let file_name = if suffix == 0 {
            format!("S3CC-Manager-Restore-{stamp}.txt")
        } else {
            format!("S3CC-Manager-Restore-{stamp}-{suffix}.txt")
        };
        let candidate = base.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Could not allocate a unique restore manifest filename.".to_string())
}

fn ready_items(plan_items: &[PlanItem]) -> Vec<&PlanItem> {
    plan_items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready"))
        .collect()
}

/// Validate the complete ready set before creating directories, editing
/// Resource.cfg, or moving the first file. The planner normally catches these
/// conditions, but execution also receives frozen plans from the UI/history,
/// so this second gate is required for stale or externally altered plans.
fn validate_ready_execution_paths(root: &Path, ready: &[&PlanItem]) -> Result<(), String> {
    let mut destinations = HashMap::<String, &PlanItem>::new();
    let mut sources = HashMap::<String, Vec<&PlanItem>>::new();

    for item in ready {
        let source = Path::new(&item.source_path);
        let destination = item
            .destination_path
            .as_deref()
            .map(Path::new)
            .ok_or_else(|| format!("Planner item {} has no destination.", item.name))?;
        let source_key = normalized_path_key(root, source);
        let destination_key = normalized_path_key(root, destination);

        sources
            .entry(source_key)
            .or_default()
            .push(*item);

        if let Some(other) = destinations.get(&destination_key) {
            return Err(format!(
                "Plan contains multiple ready operations for the same destination: '{}' and '{}' -> {}. No files were moved.",
                other.name,
                item.name,
                destination.display()
            ));
        }
        destinations.insert(destination_key, *item);
    }

    for item in ready {
        let destination = Path::new(item.destination_path.as_deref().unwrap());
        let destination_key = normalized_path_key(root, destination);
        if let Some(peers) = sources.get(&destination_key) {
            if peers
                .iter()
                .any(|peer| normalized_path_key(root, Path::new(&peer.source_path)) != destination_key)
            {
                let names = peers
                    .iter()
                    .map(|peer| peer.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ");
                return Err(format!(
                    "Plan destination overlaps another ready source: {} -> {} (source peer(s): {}). No files were moved.",
                    item.name,
                    destination.display(),
                    names
                ));
            }
        }

        if destination.exists() {
            return Err(format!(
                "Destination already exists before execution; refusing overwrite: {}",
                destination.display()
            ));
        }
    }

    Ok(())
}

fn requires_post_execution_package_scan(plan: &OrganizationPlan) -> bool {
    // A plan built from an explicit residual selection must not turn into a
    // second full DBPF scan merely because one of its items is a package.
    // The planner uses the same threshold as `scan_selected_roots`: below
    // 512 selected packages it has already produced a focused scan and the
    // executor can derive its post-state from the frozen plan.  Full plans
    // retain the historical scan so the UI receives a complete current
    // inventory after a broad organization.
    plan.stats.selected >= 512
}

fn incremental_current_snapshot(
    plan: &OrganizationPlan,
    remaining_legacy_files: usize,
) -> OrganizationAnalysisSnapshot {
    let mut snapshot = plan.analysis_before.clone();
    snapshot.phase = "analysis_current".to_string();
    snapshot.with_plan(&plan.stats, remaining_legacy_files)
}

fn package_extension(path: &Path) -> bool {
    let lower = path.file_name().map(|v| v.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    lower.ends_with(".package") || lower.ends_with(".package.disabled")
}

fn snapshot_extension(path: &Path) -> bool {
    if package_extension(path) { return true; }
    path.extension().and_then(|value| value.to_str()).is_some_and(|value| {
        matches!(value.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff")
    })
}

fn snapshot_entries(
    root: &Path,
    ready: &[&PlanItem],
    targeted_only: bool,
) -> Result<Vec<RestoreEntry>, String> {
    let mut planned = HashMap::<PathBuf, (&PlanItem, PathBuf)>::new();

    for &item in ready {
        let source = PathBuf::from(&item.source_path)
            .canonicalize()
            .map_err(|error| format!("Could not resolve planned source {}: {error}", item.source_path))?;
        let destination = item
            .destination_relative_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| format!("Planner item {} is missing destination.", item.name))?;
        planned.insert(source, (item, destination));
    }

    // A focused preview/auxiliary transaction needs a rollback snapshot only
    // for the files that it will move. Hashing the complete Packages tree at
    // this point defeats the prebuilt-plan optimization and can keep a
    // three-image cleanup busy for hours. Full package plans retain the
    // historical complete snapshot for broad recovery.
    let mut package_paths = if targeted_only {
        ready
            .iter()
            .map(|item| PathBuf::from(&item.source_path))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if !targeted_only {
        for entry in WalkDir::new(root).follow_links(false).into_iter() {
            let entry = entry.map_err(|error| format!("Could not snapshot package tree: {error}"))?;
            if entry.file_type().is_file() && snapshot_extension(entry.path()) {
                package_paths.push(entry.path().to_path_buf());
            }
        }
    }
    // The denominator describes only the active phase. Snapshot files are
    // not added to the move count, so '529 / 1757' can never be shown again.
    operation::set_total("organize", package_paths.len());
    operation::update("organize", 0, None, "snapshotting");

    let mut entries = Vec::with_capacity(package_paths.len());
    for (index, file) in package_paths.iter().enumerate() {
        let absolute = file
            .canonicalize()
            .map_err(|error| format!("Could not resolve {}: {error}", file.display()))?;
        let original_relative = absolute
            .strip_prefix(root)
            .map_err(|_| format!("Snapshot file escaped root: {}", absolute.display()))?
            .to_path_buf();

        let (sha256, size) = sha256_file(&absolute)
            .map_err(|error| format!("Could not hash snapshot file {}: {error}", absolute.display()))?;

        let organized_relative = if let Some((item, destination)) = planned.get(&absolute) {
            let expected_hash = item
                .sha256
                .as_ref()
                .ok_or_else(|| format!("Planner item {} is missing SHA-256.", item.name))?;
            if item.size != size || !expected_hash.eq_ignore_ascii_case(&sha256) {
                return Err(format!(
                    "File changed between Planner and manifest snapshot: {}",
                    absolute.display()
                ));
            }
            destination.clone()
        } else {
            original_relative.clone()
        };

        entries.push(RestoreEntry {
            sha256,
            size,
            original_relative_path: original_relative,
            organized_relative_path: organized_relative,
        });
        operation::update("organize", index + 1, None, "snapshotting");
    }

    entries.sort_by_key(|entry| {
        entry
            .original_relative_path
            .to_string_lossy()
            .to_ascii_lowercase()
    });

    Ok(entries)
}

fn manifest_from_snapshot(
    root: &Path,
    language: AppLanguage,
    entries: Vec<RestoreEntry>,
    created_directories: &[String],
) -> RestoreManifest {
    RestoreManifest {
        version: 1,
        created_at: Local::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        organization_language: language,
        root: root.to_path_buf(),
        status: "PENDING".to_string(),
        created_directories: created_directories.iter().map(PathBuf::from).collect(),
        resource_cfg_restore: None,
        entries,
    }
}

fn verify_identity(path: &Path, expected_hash: &str, expected_size: u64) -> Result<(), String> {
    let (hash, size) = sha256_file(path)
        .map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    if size != expected_size || !hash.eq_ignore_ascii_case(expected_hash) {
        return Err(format!(
            "Identity changed for {} (expected size/hash {} / {}, got {} / {}).",
            path.display(),
            expected_size,
            expected_hash,
            size,
            hash
        ));
    }
    Ok(())
}

fn remove_empty_created_directories(root: &Path, directories: &[String]) {
    let mut ordered = directories
        .iter()
        .map(|relative| root.join(relative))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for path in ordered {
        if path.is_dir() {
            let _ = fs::remove_dir(&path);
        }
    }
}

// After a successful organization, remove *every* genuinely empty
// subdirectory within the selected tree, not just the parents of moved files.
// This also covers folders left empty by prior organizations or user-created
// empty category trees. Never delete the selected root, Mods/Packages or
// Mods/Overrides themselves; never follow symlinks/junctions or delete files.
//
// The retained counter only describes directories on this transaction's old
// source paths that still contain data (duplicates, companions, etc.). Other
// nonempty folders are simply left untouched.
pub(crate) fn cleanup_empty_directories_after_organization(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
) -> (usize, usize, Vec<String>) {
    cleanup_empty_directories_after_organization_with_protected(
        root,
        moved_pairs,
        None,
        &[],
    )
}

fn cleanup_empty_directories_after_organization_scoped(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    scope_roots: Option<&[PathBuf]>,
) -> (usize, usize, Vec<String>) {
    cleanup_empty_directories_after_organization_with_protected(
        root,
        moved_pairs,
        scope_roots,
        &[],
    )
}

fn cleanup_empty_directories_after_organization_with_protected(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    scope_roots: Option<&[PathBuf]>,
    protected_relative_directories: &[String],
) -> (usize, usize, Vec<String>) {
    let mods_root = crate::resource_cfg::is_mods_layout_root(root);
    let is_protected_empty_directory = |path: &Path| {
        protected_relative_directories.iter().any(|relative| {
            normalized_path_key(root, path)
                == normalized_path_key(root, Path::new(relative))
        })
    };
    let mut candidates = Vec::<PathBuf>::new();
    let mut warnings = Vec::<String>::new();

    let walk_roots = scope_roots
        .filter(|roots| !roots.is_empty())
        .map(|roots| roots.to_vec())
        .unwrap_or_else(|| vec![root.to_path_buf()]);
    for walk_root in walk_roots {
        if !walk_root.is_dir() {
            continue;
        }
        if scope_roots.is_none() {
            let minimum_depth = usize::from(walk_root == root);
            for entry in WalkDir::new(&walk_root)
                .follow_links(false)
                .min_depth(minimum_depth)
                .into_iter()
            {
                match entry {
                    Ok(entry) if entry.file_type().is_dir() => {
                        let path = entry.path();
                        if is_protected_empty_directory(path) {
                            continue;
                        }
                        // These are the two loading branch roots. They must survive
                        // even if they contain zero packages after organization.
                        if mods_root && path.parent() == Some(root) && path.file_name()
                            .is_some_and(|name| {
                                name.to_string_lossy().eq_ignore_ascii_case("Packages")
                                    || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
                            })
                        {
                            continue;
                        }
                        candidates.push(path.to_path_buf());
                    }
                    Ok(_) => {}
                    Err(error) => warnings.push(format!(
                        "Could not inspect a folder during empty-directory cleanup: {error}"
                    )),
                }
            }
        } else {
            // A focused plan only needs the old source directory and its
            // ancestors. Recursing from a source parent such as `Jonas`
            // would inspect unrelated siblings and defeat incremental mode.
            if !is_protected_empty_directory(&walk_root) {
                candidates.push(walk_root.to_path_buf());
            }
        }

        // The recursive walk is intentionally limited to the selected old
        // source branch.  Add only its ancestors as direct candidates so a
        // chain such as `Author\Legacy\Sliders` can be removed after its
        // children disappear, without walking every sibling below `Author`.
        if scope_roots.is_some() {
            let mut ancestor = walk_root.parent();
            while let Some(folder) = ancestor {
                if folder == root || !folder.starts_with(root) {
                    break;
                }
                if !is_protected_empty_directory(folder) {
                    candidates.push(folder.to_path_buf());
                }
                ancestor = folder.parent();
            }
        }
    }

    // Remove children before their parents. As each descendant disappears,
    // another formerly nonempty parent can become empty in the same pass.
    candidates.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    let mut removed = 0;
    for folder in candidates {
        if is_protected_empty_directory(&folder) {
            continue;
        }
        match fs::remove_dir(&folder) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // The child may have removed this directory earlier in the
                // same cleanup pass. Missing is the expected idempotent
                // outcome, not a cleanup failure.
            }
            Err(error) => {
                // A nonempty directory is expected and must be kept intact.
                // Surface only failures concerning an actually empty folder.
                match fs::read_dir(&folder) {
                    Ok(mut contents) => {
                        if contents.next().is_none() {
                            warnings.push(format!(
                                "Could not remove empty folder {}: {error}",
                                folder.display()
                            ));
                        }
                    },
                    Err(read_error) if read_error.kind() == std::io::ErrorKind::NotFound => {
                        // A concurrent/earlier removal is also expected while
                        // walking ancestors in the same cleanup pass.
                    }
                    Err(read_error) => warnings.push(format!(
                        "Could not inspect folder {} after failed cleanup: {read_error}",
                        folder.display()
                    )),
                }
            }
        }
    }

    let mut old_folders = BTreeSet::<PathBuf>::new();
    for (source, _, _, _) in moved_pairs {
        let mut current = source.parent();
        while let Some(folder) = current {
            if folder == root || !folder.starts_with(root) {
                break;
            }
            // The branch roots are intentionally kept, not "occupied".
            let protected = mods_root && folder.parent() == Some(root) &&
                folder.file_name().is_some_and(|name| {
                    name.to_string_lossy().eq_ignore_ascii_case("Packages")
                        || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
                });
            if !protected {
                old_folders.insert(folder.to_path_buf());
            }
            current = folder.parent();
        }
    }
    let retained = old_folders
        .into_iter()
        .filter(|path| path.is_dir() && !is_protected_empty_directory(path))
        .count();
    (removed, retained, warnings)
}

fn cleanup_scope_roots(root: &Path, plan: &OrganizationPlan) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    for item in &plan.items {
        let source = PathBuf::from(&item.source_path);
        let Ok(_) = source.strip_prefix(root) else {
            continue;
        };
        let Some(parent) = source.parent() else {
            continue;
        };
        // Scope cleanup to the actual old-source branches.  Using only the
        // first component (for example `Packages\Cabelos`) made a residual
        // plan walk an entire category tree even when it contained one file.
        if parent.is_dir() && parent.starts_with(root) {
            roots.insert(parent.to_path_buf());
        }
    }
    roots.into_iter().collect()
}

// An empty-folder cleanup cannot remove folders that still contain
// duplicates, disabled packages, images, downloads or other user data.
// Report every leftover below the complete old source subtree, without
// following links or silently deleting unrelated content. This is important
// for nested legacy trees: checking only the immediate directory misses a
// package one level deeper and makes the cleanup report falsely optimistic.
fn remaining_legacy_sources(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
) -> (usize, Vec<String>) {
    let details = remaining_legacy_details(root, moved_pairs, &[]);
    let count = details.len();
    let examples = details
        .into_iter()
        .take(12)
        .map(|detail| detail.relative_path)
        .collect();
    (count, examples)
}

fn same_relative_path(left: &str, right: &str) -> bool {
    left.replace('/', "\\")
        .trim_matches('\\')
        .eq_ignore_ascii_case(right.replace('/', "\\").trim_matches('\\'))
}

fn remaining_reason(item: &PlanItem) -> String {
    match item.plan_status.as_str() {
        "duplicate_skipped" => "duplicate_exact_or_destination_same_content".to_string(),
        "collision_different_content" => "collision_different_content".to_string(),
        "blocked" => "blocked_or_protected".to_string(),
        status if status.starts_with("already_") => "already_organized".to_string(),
        status if status.starts_with("keep_") => "kept_by_protection_rule".to_string(),
        _ => format!("plan_status:{}", item.plan_status),
    }
}

// Perform a second recursive reconciliation after planning/moving. Every
// remaining file below an old source subtree receives a reason. Plan items
// have an authoritative status; packages outside the explicit selection are
// reported as not selected, and non-package files are preserved as companions.
fn remaining_legacy_details(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    plan_items: &[PlanItem],
) -> Vec<LegacyRemainder> {
    let moved_destinations = moved_pairs.iter()
        .filter_map(|(_, destination, _, _)| destination.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('/', "\\").to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut folders = BTreeSet::new();
    for (source, _, _, _) in moved_pairs {
        if let Some(parent) = source.parent() {
            if parent.starts_with(root) && parent != root {
                folders.insert(parent.to_path_buf());
            }
        }
    }

    for item in plan_items {
        let source = PathBuf::from(&item.source_path);
        let Some(parent) = source.parent() else { continue; };
        if !source.is_file() || !parent.starts_with(root) || parent == root {
            continue;
        }
        let source_relative = item.source_relative_path.replace('/', "\\");
        let destination_same = item.destination_relative_path.as_ref()
            .is_some_and(|destination| same_relative_path(&source_relative, destination));
        if !destination_same {
            folders.insert(parent.to_path_buf());
        }
    }

    let mut remaining = BTreeSet::new();
    let mut details = Vec::new();
    for folder in folders {
        for entry in WalkDir::new(folder).follow_links(false).into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Ok(relative) = entry.path().strip_prefix(root) {
                    let relative = relative.to_string_lossy().replace('/', "\\");
                    // A destination can legitimately remain below the same
                    // ancestor as its old source (for example
                    // Packages\\Substituições -> Packages\\Substituições\\CAS).
                    // It is current content, not a legacy remainder.
                    if moved_destinations.contains(&relative.to_ascii_lowercase()) {
                        continue;
                    }
                    if !remaining.insert(relative.clone()) {
                        continue;
                    }
                    let plan = plan_items.iter().find(|item| {
                        same_relative_path(&item.source_relative_path, &relative)
                    });
                    let (reason, plan_status, warning) = if let Some(item) = plan {
                        (
                            remaining_reason(item),
                            Some(item.plan_status.clone()),
                            item.warnings.first().cloned(),
                        )
                    } else if entry.path().extension().is_some_and(|extension| {
                        extension.to_string_lossy().eq_ignore_ascii_case("package")
                            || extension.to_string_lossy().eq_ignore_ascii_case("disabled")
                    }) {
                        ("not_selected".to_string(), None, None)
                    } else {
                        ("companion_or_non_package_preserved".to_string(), None, None)
                    };
                    details.push(LegacyRemainder {
                        relative_path: relative,
                        reason,
                        plan_status,
                        warning,
                    });
                }
            }
        }
    }
    details.sort_by_key(|detail| detail.relative_path.to_ascii_lowercase());
    details
}

#[tauri::command]
pub async fn execute_organization(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
) -> Result<ExecutionResult, String> {
    execute_organization_with_cfg(folder, language, selected_paths, false, None, Vec::new()).await
}

#[tauri::command]
pub async fn execute_organization_with_cfg(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
    update_resource_cfg: bool,
    expected_cfg_hash: Option<String>,
    expected_cfg_rules: Vec<String>,
) -> Result<ExecutionResult, String> {
    // The extensive snapshot/hash work MUST run off the WebView/main
    // thread. Otherwise progress polling cannot update the UI.
    // Keep the file-move/rollback sequence unchanged and uninterruptible.
    operation::begin("organize", "planning");
    let result = tauri::async_runtime::spawn_blocking(move || {
        execute_organization_core(
            folder, language, selected_paths, update_resource_cfg,
            expected_cfg_hash, expected_cfg_rules,
        )
    })
    .await
    .map_err(|error| format!("Organization worker failed: {error}"))
    .and_then(|output| output);
    match &result {
        Ok(output) if output.status == "COMPLETE" || output.status == "NO_CHANGES" =>
            operation::finish("organize", "complete", None),
        Ok(output) => operation::finish("organize", "failed", Some(output.status.clone())),
        Err(error) => operation::finish("organize", "failed", Some(error.clone())),
    }
    result
}

fn execute_organization_core(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
    update_resource_cfg: bool,
    expected_cfg_hash: Option<String>,
    expected_cfg_rules: Vec<String>,
) -> Result<ExecutionResult, String> {
    let plan = build_organization_plan_with_cfg(folder, language, selected_paths, update_resource_cfg)?;
    execute_organization_plan_core(
        plan,
        language,
        update_resource_cfg,
        expected_cfg_hash,
        expected_cfg_rules,
    )
}

/// Execute a plan that was already built by the planner.
///
/// The executor still revalidates every source path and SHA-256 while creating
/// its rollback snapshot, so a plan cannot silently apply to changed files.
/// Keeping this entry point separate prevents a caller that already has a
/// preview (for example, a three-image cleanup) from paying for a second full
/// scan of the entire Packages tree before the move begins.
#[tauri::command]
pub async fn execute_organization_plan_with_cfg(
    plan: OrganizationPlan,
    language: AppLanguage,
    update_resource_cfg: bool,
    expected_cfg_hash: Option<String>,
    expected_cfg_rules: Vec<String>,
) -> Result<ExecutionResult, String> {
    operation::begin("organize", "executing_plan");
    let result = tauri::async_runtime::spawn_blocking(move || {
        execute_organization_plan_core(
            plan, language, update_resource_cfg, expected_cfg_hash, expected_cfg_rules,
        )
    })
    .await
    .map_err(|error| format!("Organization worker failed: {error}"))
    .and_then(|output| output);
    match &result {
        Ok(output) if output.status == "COMPLETE" || output.status == "NO_CHANGES" =>
            operation::finish("organize", "complete", None),
        Ok(output) => operation::finish("organize", "failed", Some(output.status.clone())),
        Err(error) => operation::finish("organize", "failed", Some(error.clone())),
    }
    result
}

fn execute_organization_plan_core(
    plan: OrganizationPlan,
    language: AppLanguage,
    update_resource_cfg: bool,
    expected_cfg_hash: Option<String>,
    expected_cfg_rules: Vec<String>,
) -> Result<ExecutionResult, String> {

    // Only explicitly ready items will be moved. Keep every blocked item,
    // duplicate and conflicting destination in place for later manual review.
    // The plan and file SHA-256 identities are rebuilt immediately before
    // execution, and the rollback manifest records only ready movements.
    let root = PathBuf::from(&plan.root);
    ensure_writable(&root)?;
    let ready = ready_items(&plan.items);
    validate_ready_execution_paths(&root, &ready)?;
    let move_total = ready.len();
    operation::update("organize", 0, None, "preparing");

    if ready.is_empty() {
        // A previous organization may already have emptied legacy category
        // directories. Clean only genuinely empty ones, even if no new move
        // was necessary. Never remove files or the Packages/Overrides roots.
        let focused_cleanup_roots = cleanup_scope_roots(&root, &plan);
        let (removed, retained, cleanup_warnings) = if requires_post_execution_package_scan(&plan) {
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &[],
                None,
                &plan.protected_empty_directories,
            )
        } else {
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &[],
                Some(&focused_cleanup_roots),
                &plan.protected_empty_directories,
            )
        };
        let remaining_legacy_details = remaining_legacy_details(&root, &[], &plan.items);
        let remaining_legacy_files = remaining_legacy_details.len();
        let remaining_legacy_examples = remaining_legacy_details
            .iter()
            .take(12)
            .map(|detail| detail.relative_path.clone())
            .collect();
        let analysis_current = if requires_post_execution_package_scan(&plan) {
            scan_packages_core(root.to_string_lossy().to_string(), language, None)
                .ok()
                .map(|scan| OrganizationAnalysisSnapshot::from_scan("analysis_current", &scan.stats))
        } else {
            Some(incremental_current_snapshot(&plan, remaining_legacy_files))
        };
        return Ok(ExecutionResult {
            status: "NO_CHANGES".to_string(),
            moved_files: Vec::new(),
            organized_directory: root.to_string_lossy().to_string(),
            manifest_path: None,
            moved: 0,
            already_organized: plan.stats.already_organized,
            rolled_back: 0,
            old_folders_removed: removed,
            old_folders_retained: retained,
            remaining_legacy_files,
            remaining_legacy_examples,
            remaining_legacy_details,
            errors: cleanup_warnings,
            analysis_before: plan.analysis_before.clone(),
            execution: plan.analysis_before.clone().with_plan(&plan.stats, remaining_legacy_files),
            analysis_current,
        });
    }

    let manifest_path = make_manifest_path(&root)?;
    let snapshot = snapshot_entries(
        &root,
        &ready,
        !requires_post_execution_package_scan(&plan),
    )?;
     operation::set_total("organize", move_total);
    operation::update("organize", 0, None, "preparing");
    let mut manifest = manifest_from_snapshot(
        &root,
        language,
        snapshot,
        &plan.directories_to_create,
    );
    // Persist recovery information before touching Resource.cfg or CC files.
    write_manifest_atomic(&manifest_path, &manifest)?;

    let mut applied_cfg = None;
    if update_resource_cfg {
        let change = plan.resource_cfg_update.as_ref()
            .ok_or("Resource.cfg opt-in requires a valid Mods/Packages or Mods/Overrides root.")?;
        if expected_cfg_hash.as_deref() != Some(change.original_hash.as_str())
            || expected_cfg_rules != change.added_rules {
            manifest.status = "ROLLED_BACK".to_string();
            let _ = replace_manifest_atomic(&manifest_path, &manifest);
            return Err("Resource.cfg preview is stale; review the new plan before organizing.".into());
        }
        applied_cfg = apply_resource_cfg_update(change)?;

        // Every updated target must be loadable before the first package move.
        // A parsing / IO failure here must also roll the newly installed cfg back.
        let cfg_validation = (|| -> Result<(), String> {
            let cfg_path = PathBuf::from(&change.path);
            let parsed = parse_resource_cfg(&cfg_path)?;
            let cfg_dir = cfg_path.parent().ok_or("Resource.cfg parent missing")?;
            let uncovered = ready.iter().filter(|item| {
                // .package.disabled remains disabled even after relocation;
                // Resource.cfg intentionally has no PackedFile rule for it.
                package_extension(Path::new(&item.source_path))
                    && item.source_path.to_ascii_lowercase().ends_with(".package")
                    && item.destination_path.as_ref()
                        .is_none_or(|path| package_priority(&parsed, cfg_dir, Path::new(path)).is_none())
            }).count();
            if uncovered != 0 {
                return Err(format!("Updated Resource.cfg misses {uncovered} planned packages."));
            }
            Ok(())
        })();
        if let Err(error) = cfg_validation {
            if let Some(applied) = applied_cfg.take() {
                rollback_resource_cfg_update(applied)?;
            }
            manifest.status = "ROLLED_BACK".to_string();
            let _ = replace_manifest_atomic(&manifest_path, &manifest);
            return Err(format!("{error} No packages were moved; Resource.cfg was restored."));
        }

        if let Some(applied) = &applied_cfg {
            manifest.resource_cfg_restore = Some(applied.restore_snapshot(change));
            if let Err(error) = replace_manifest_atomic(&manifest_path, &manifest) {
                if let Some(applied) = applied_cfg.take() {
                    rollback_resource_cfg_update(applied)?;
                }
                return Err(format!("Could not persist Resource.cfg recovery metadata: {error}"));
            }
        }
    }

    operation::set_total("organize", move_total);
    operation::update("organize", 0, None, "moving");
    let mut moved_pairs: Vec<(PathBuf, PathBuf, String, u64)> = Vec::new();
    let mut errors = Vec::new();

    for item in ready {
        let source = PathBuf::from(&item.source_path);
        let destination = item
            .destination_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| format!("Planner item {} has no destination.", item.name))?;
        let expected_hash = item
            .sha256
            .as_ref()
            .ok_or_else(|| format!("Planner item {} has no SHA-256.", item.name))?;

        let step_result = (|| -> Result<(), String> {
            if !source.is_file() {
                return Err(format!("Source disappeared before move: {}", source.display()));
            }
            if destination.exists() {
                return Err(format!(
                    "Destination appeared after preflight; refusing overwrite: {}",
                    destination.display()
                ));
            }

            verify_identity(&source, expected_hash, item.size)?;

            let parent = destination
                .parent()
                .ok_or_else(|| format!("Destination has no parent: {}", destination.display()))?;
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;

            fs::rename(&source, &destination).map_err(|error| {
                format!(
                    "Could not move {} -> {}: {error}",
                    source.display(),
                    destination.display()
                )
            })?;

            // rename() stays on the same filesystem and changes only the
            // directory entry; SHA-256 was checked immediately before it.
            // Re-hashing the unchanged bytes at the destination doubled I/O
            // for large CC files without adding a separate copy-verification
            // guarantee. The recovery manifest retains the verified hash.
            Ok(())
        })();

        match step_result {
            Ok(()) => {
                moved_pairs.push((
                    source,
                    destination,
                    expected_hash.clone(),
                    item.size,
                ));
                operation::update("organize", moved_pairs.len(), None, "moving");
            },
            Err(error) => {
                errors.push(error);
                break;
            }
        }
    }

    if !errors.is_empty() {
        operation::set_total("organize", moved_pairs.len());
        operation::update("organize", 0, None, "rolling_back");
        let mut rollback_errors = Vec::new();
        let mut rolled_back = 0usize;

        for (source, destination, expected_hash, expected_size) in moved_pairs.iter().rev() {
            let rollback = (|| -> Result<(), String> {
                if source.exists() {
                    return Err(format!(
                        "Rollback destination already exists: {}",
                        source.display()
                    ));
                }
                if !destination.is_file() {
                    return Err(format!(
                        "Moved file disappeared before rollback: {}",
                        destination.display()
                    ));
                }

                verify_identity(destination, expected_hash, *expected_size)?;

                if let Some(parent) = source.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        format!("Could not recreate original folder {}: {error}", parent.display())
                    })?;
                }

                fs::rename(destination, source).map_err(|error| {
                    format!(
                        "Could not rollback {} -> {}: {error}",
                        destination.display(),
                        source.display()
                    )
                })?;

                // As above, rename only changes the directory entry.
                // The quarantined destination was SHA-256 verified just
                // before rollback; avoid an identical extra full-file read.
                Ok(())
            })();

            match rollback {
                Ok(()) => rolled_back += 1,
                Err(error) => rollback_errors.push(error),
            }
        }

        remove_empty_created_directories(&root, &plan.directories_to_create);

        manifest.status = if rollback_errors.is_empty() {
            "ROLLED_BACK".to_string()
        } else {
            "ROLLBACK_INCOMPLETE".to_string()
        };

        if let Err(error) = replace_manifest_atomic(&manifest_path, &manifest) {
            rollback_errors.push(error);
        }

        if rollback_errors.is_empty() {
            if let Some(applied) = applied_cfg.take() {
                match rollback_resource_cfg_update(applied) {
                    Ok(()) => {
                        manifest.resource_cfg_restore = None;
                        if let Err(error) = replace_manifest_atomic(&manifest_path, &manifest) {
                            rollback_errors.push(error);
                            manifest.status = "ROLLBACK_INCOMPLETE".into();
                        }
                    }
                    Err(error) => {
                        rollback_errors.push(error);
                        manifest.status = "ROLLBACK_INCOMPLETE".to_string();
                        let _ = replace_manifest_atomic(&manifest_path, &manifest);
                    }
                }
            }
        }
        errors.extend(rollback_errors);

        return Ok(ExecutionResult {
            status: manifest.status.clone(),
            moved_files: Vec::new(),
            organized_directory: root.to_string_lossy().to_string(),
            manifest_path: Some(manifest_path.to_string_lossy().to_string()),
            moved: moved_pairs.len(),
            already_organized: plan.stats.already_organized,
            rolled_back,
            old_folders_removed: 0,
            old_folders_retained: 0,
            remaining_legacy_files: 0,
            remaining_legacy_examples: Vec::new(),
            remaining_legacy_details: Vec::new(),
            errors,
            analysis_before: plan.analysis_before.clone(),
            execution: plan.analysis_before.clone().with_plan(&plan.stats, 0),
            analysis_current: None,
        });
    }

    operation::set_total("organize", 1);
    operation::update("organize", 0, None, "cleaning");
    let focused_cleanup_roots = cleanup_scope_roots(&root, &plan);
    let (old_folders_removed, old_folders_retained, cleanup_warnings) =
        if requires_post_execution_package_scan(&plan) {
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &moved_pairs,
                None,
                &plan.protected_empty_directories,
            )
        } else {
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &moved_pairs,
                Some(&focused_cleanup_roots),
                &plan.protected_empty_directories,
            )
        };

    let remaining_legacy_details =
        remaining_legacy_details(&root, &moved_pairs, &plan.items);
    let remaining_legacy_files = remaining_legacy_details.len();
    let remaining_legacy_examples = remaining_legacy_details
        .iter()
        .take(12)
        .map(|detail| detail.relative_path.clone())
        .collect();

    manifest.status = "COMPLETE".to_string();
    replace_manifest_atomic(&manifest_path, &manifest)?;
    operation::update("organize", 1, None, "complete");

    // Invalidate the Rust scan cache before any later plan preview: the
    // cache contains pre-move absolute paths and must never be reused.
    invalidate_latest_scan_for(&root);
    let analysis_current = if requires_post_execution_package_scan(&plan) {
        scan_packages_core(root.to_string_lossy().to_string(), language, None)
            .ok()
            .map(|scan| OrganizationAnalysisSnapshot::from_scan("analysis_current", &scan.stats))
    } else {
        Some(incremental_current_snapshot(&plan, remaining_legacy_files))
    };
    let moved_files = moved_pairs.iter().map(|(source, destination, _, _)| {
        OrganizationMove {
            source_path: source.to_string_lossy().to_string(),
            destination_path: destination.to_string_lossy().to_string(),
            destination_relative_path: destination.strip_prefix(&root)
                .unwrap_or(destination)
                .to_string_lossy().to_string(),
        }
    }).collect();
    let organized_directory = organized_parent(&root, &moved_pairs)
        .to_string_lossy().to_string();

    Ok(ExecutionResult {
        status: "COMPLETE".to_string(),
        moved_files,
        organized_directory,
        manifest_path: Some(manifest_path.to_string_lossy().to_string()),
        moved: moved_pairs.len(),
        already_organized: plan.stats.already_organized,
        rolled_back: 0,
        old_folders_removed,
        old_folders_retained,
        remaining_legacy_files,
        remaining_legacy_examples,
        remaining_legacy_details,
        errors: cleanup_warnings,
        analysis_before: plan.analysis_before.clone(),
        execution: plan.analysis_before.clone().with_plan(&plan.stats, remaining_legacy_files),
        analysis_current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organized_folder_is_common_parent_of_actual_destinations() {
        let root = std::env::temp_dir().join("S3CC-Test-Mods");
        let packages = root.join("Packages");
        let a = (root.join("Old/a.package"),
                 packages.join("CAS/Hair/a.package"),
                 "a".repeat(64), 10u64);
        let b = (root.join("Old/b.package"),
                 packages.join("Scripts/Jogabilidade/Author/b.package"),
                 "b".repeat(64), 20u64);
        assert_eq!(organized_parent(&root, &[a.clone(), b.clone()]), packages);
        assert_eq!(organized_parent(&root, &[a]), packages.join("CAS/Hair"));
        let overrides = (root.join("Overrides/x.package"),
                         root.join("Overrides/Scripts/x.package"),
                         "c".repeat(64), 10u64);
        assert_eq!(organized_parent(&root, &[b, overrides]), root);
    }

    #[test]
    fn move_result_serialization_has_reconciliable_paths() {
        let item = OrganizationMove {
            source_path: "Mods/Packages/Old/a.package".into(),
            destination_path: "Mods/Packages/CAS/a.package".into(),
            destination_relative_path: "Packages/CAS/a.package".into(),
        };
        let json = serde_json::to_value(item).unwrap();
        assert!(json["sourcePath"].as_str().unwrap().ends_with("a.package"));
        assert_eq!(json["destinationRelativePath"], "Packages/CAS/a.package");
    }

    #[test]
    fn preflight_rejects_duplicate_ready_destinations_before_any_move() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-preflight-same-destination-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let first = root.join("Legacy").join("first.package");
        let second = root.join("Other").join("second.package");
        let destination = root.join("Packages").join("same.package");
        fs::create_dir_all(first.parent().unwrap()).unwrap();
        fs::create_dir_all(second.parent().unwrap()).unwrap();
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();

        let items = vec![
            PlanItem {
                id: "first".into(),
                name: "first.package".into(),
                source_path: first.to_string_lossy().into_owned(),
                source_relative_path: "Legacy\\first.package".into(),
                destination_path: Some(destination.to_string_lossy().into_owned()),
                destination_relative_path: Some("Packages\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 5,
                warnings: vec![],
            },
            PlanItem {
                id: "second".into(),
                name: "second.package".into(),
                source_path: second.to_string_lossy().into_owned(),
                source_relative_path: "Other\\second.package".into(),
                destination_path: Some(
                    destination
                        .to_string_lossy()
                        .replace("same.package", "SAME.PACKAGE"),
                ),
                destination_relative_path: Some("packages\\SAME.PACKAGE".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("B".repeat(64)),
                size: 6,
                warnings: vec![],
            },
        ];
        let ready = ready_items(&items);
        let error = validate_ready_execution_paths(&root, &ready).unwrap_err();
        assert!(error.contains("same destination"));
        assert!(first.is_file() && second.is_file());
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preflight_rejects_verbatim_source_destination_overlap() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-preflight-source-overlap-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let source_a = root.join("Legacy").join("a.package");
        let source_b = root.join("Packages").join("b.package");
        fs::create_dir_all(source_a.parent().unwrap()).unwrap();
        fs::create_dir_all(source_b.parent().unwrap()).unwrap();
        fs::write(&source_a, b"a").unwrap();
        fs::write(&source_b, b"b").unwrap();
        let verbatim_source_b = format!(r"\\?\{}", source_b.display());
        let destination_b = root.join("Canonical").join("b.package");
        let items = vec![
            PlanItem {
                id: "a".into(),
                name: "a.package".into(),
                source_path: source_a.to_string_lossy().into_owned(),
                source_relative_path: "Legacy\\a.package".into(),
                destination_path: Some(source_b.to_string_lossy().into_owned()),
                destination_relative_path: Some("Packages\\b.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 1,
                warnings: vec![],
            },
            PlanItem {
                id: "b".into(),
                name: "b.package".into(),
                source_path: verbatim_source_b,
                source_relative_path: "Packages\\b.package".into(),
                destination_path: Some(destination_b.to_string_lossy().into_owned()),
                destination_relative_path: Some("Canonical\\b.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("B".repeat(64)),
                size: 1,
                warnings: vec![],
            },
        ];
        let ready = ready_items(&items);
        let error = validate_ready_execution_paths(&root, &ready).unwrap_err();
        assert!(error.contains("source"));
        assert!(source_a.is_file() && source_b.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_leftover_sidecars_without_deleting_them() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-legacy-remaining-{}-{}", std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let old = root.join("Packages").join("Legacy");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("picture.png"), b"preserve").unwrap();
        fs::write(old.join("extra.package.disabled"), b"preserve").unwrap();
        fs::create_dir_all(old.join("Nested Legacy").join("Sliders")).unwrap();
        fs::write(
            old.join("Nested Legacy").join("Sliders").join("nested.package"),
            b"preserve nested content",
        )
        .unwrap();
        let source = old.join("original.package");
        let moved = vec![(source, root.join("Packages/Sliders/original.package"), String::new(), 0)];
        let (count, examples) = remaining_legacy_sources(&root, &moved);
        assert_eq!(count, 3);
        assert_eq!(examples.len(), 3);
        assert!(old.join("picture.png").exists());
        assert!(old.join("extra.package.disabled").exists());
        assert!(examples.iter().any(|path| path.ends_with("Nested Legacy\\Sliders\\nested.package")));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn does_not_count_a_verified_destination_as_a_legacy_remainder() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-legacy-destination-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let old = root.join("Packages").join("Substituições");
        let destination = old.join("CAS").join("replacement.package");
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&destination, b"moved").unwrap();
        let source = old.join("replacement.package");
        let moved = vec![(source, destination, String::new(), 5u64)];
        let details = remaining_legacy_details(&root, &moved, &[]);
        assert!(details.is_empty(), "destination must not be counted as legacy content: {details:?}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn removes_every_empty_subfolder_after_organization_and_preserves_nonempty_ones() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mods = std::env::temp_dir()
            .join(format!("s3cc-empty-folders-{}-{nonce}", std::process::id()))
            .join("Mods");
        let packages = mods.join("Packages");
        let overrides = mods.join("Overrides");
        let old = packages.join("CAS").join("Sliders");
        let orphan = packages.join("Custom Empty Folder").join("Unused");
        let retained = packages.join("Legacy").join("Jonha");
        let legacy = mods.join("CAS").join("Sliders");
        let override_old = overrides.join("Old Tuning");
        for folder in [&old, &orphan, &retained, &legacy, &override_old] {
            std::fs::create_dir_all(folder).unwrap();
        }
        std::fs::write(retained.join("Jonha_BASE.package"), b"pending review").unwrap();
        // Files and any folder containing them must be retained, including
        // non-package sidecars and scripts.
        std::fs::write(packages.join("keep.txt"), b"never delete content").unwrap();
        let moved = vec![
            (old.join("moved.package"), packages.join("Sliders/moved.package"), String::new(), 0),
            (retained.join("moved.package"), packages.join("Sliders/other.package"), String::new(), 0),
            (legacy.join("moved.package"), packages.join("Sliders/migrated.package"), String::new(), 0),
            (override_old.join("patch.package"), overrides.join("Tuning/patch.package"), String::new(), 0),
        ];

        let (removed, preserved, warnings) =
            cleanup_empty_directories_after_organization(&mods, &moved);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(removed, 7); // 5 old-source directories + 2 unrelated empty folders
        assert_eq!(preserved, 2); // Legacy/Jonha and its parent still contain CC
        assert!(mods.is_dir() && packages.is_dir() && overrides.is_dir());
        assert!(retained.join("Jonha_BASE.package").is_file());
        assert!(packages.join("keep.txt").is_file());
        assert!(!old.exists() && !orphan.exists() && !legacy.exists());
        assert!(!override_old.exists() && !packages.join("Custom Empty Folder").exists());
        std::fs::remove_dir_all(mods.parent().unwrap()).unwrap();
    }

    #[test]
    fn copied_mods_root_keeps_loading_branches_while_cleaning_empty_legacy() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir()
            .join(format!("s3cc-copied-cleanup-{}-{nonce}",std::process::id()));
        let root = base.join("Mods - Copia");
        let packages = root.join("Packages");
        let overrides = root.join("Overrides");
        let legacy = root.join("#+18").join("Rigged");
        let old = packages.join("Old Categorization").join("Empty");
        for path in [&packages,&overrides,&legacy,&old] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(root.join("Resource.cfg"), b"Priority 500\nPackedFile Packages/*.package\n").unwrap();
        let (removed, _, warnings) = cleanup_empty_directories_after_organization(&root, &[]);
        assert!(removed >= 4, "removed only {removed}");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(root.is_dir() && packages.is_dir() && overrides.is_dir());
        assert!(!legacy.exists() && !old.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn cleanup_preserves_empty_manual_model_directories() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir()
            .join(format!("s3cc-protected-empty-{}-{nonce}", std::process::id()));
        let root = base.join("Mods - Copia");
        let canonical = root.join("Packages").join("Cabelos e Pêlos");
        let legacy = root.join("Packages").join("#6 Hair").join("Old");
        fs::create_dir_all(&canonical).unwrap();
        fs::create_dir_all(&legacy).unwrap();

        let (removed, retained, warnings) =
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &[],
                None,
                &["Packages\\Cabelos e Pêlos".into()],
            );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(removed >= 2);
        assert_eq!(retained, 0);
        assert!(canonical.is_dir());
        assert!(!legacy.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn selected_branch_root_survives_and_only_its_empty_descendants_are_removed() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mods = std::env::temp_dir()
            .join(format!("s3cc-selected-root-{}-{nonce}", std::process::id()))
            .join("Mods");
        let packages = mods.join("Packages");
        let overrides = mods.join("Overrides");
        std::fs::create_dir_all(packages.join("Empty").join("Nested")).unwrap();
        std::fs::create_dir_all(overrides.join("Unrelated Empty")).unwrap();
        let (removed, retained, warnings) =
            cleanup_empty_directories_after_organization(&packages, &[]);
        assert_eq!((removed, retained), (2, 0));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(packages.is_dir());
        assert!(!packages.join("Empty").exists());
        assert!(overrides.join("Unrelated Empty").is_dir());
        std::fs::remove_dir_all(mods.parent().unwrap()).unwrap();
    }

    #[test]
    fn cleanup_ignores_expected_missing_directory_after_duplicate_scope_visit() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir()
            .join(format!("s3cc-cleanup-missing-{}-{nonce}", std::process::id()));
        let root = base.join("Packages");
        let legacy = root.join("Jonas").join("Empty");
        std::fs::create_dir_all(&legacy).unwrap();
        let scopes = vec![legacy.clone(), legacy.clone()];

        let (removed, _, warnings) =
            cleanup_empty_directories_after_organization_scoped(&root, &[], Some(&scopes));

        assert_eq!(removed, 2);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!legacy.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn empty_folder_cleanup_never_follows_directory_symlinks() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir()
            .join(format!("s3cc-cleanup-symlink-{}-{nonce}", std::process::id()));
        let mods = base.join("Mods");
        let target = base.join("External").join("Preserve");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        symlink(base.join("External"), mods.join("Packages").join("External Link")).unwrap();
        let (removed, _, warnings) =
            cleanup_empty_directories_after_organization(&mods, &[]);
        assert_eq!(removed, 0);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(target.is_dir());
        assert!(mods.join("Packages/External Link").exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn only_ready_items_are_written_to_manifest() {
        let items = vec![
            PlanItem {
                id: "1".into(),
                name: "a.package".into(),
                source_path: r"C:\Mods\a.package".into(),
                source_relative_path: "a.package".into(),
                destination_path: Some(r"C:\Mods\CAS\a.package".into()),
                destination_relative_path: Some(r"CAS\a.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test classification".into()),
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 10,
                warnings: vec![],
            },
            PlanItem {
                id: "2".into(),
                name: "b.package".into(),
                source_path: r"C:\Mods\b.package".into(),
                source_relative_path: "b.package".into(),
                destination_path: Some(r"C:\Mods\CAS\b.package".into()),
                destination_relative_path: Some(r"CAS\b.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test classification".into()),
                plan_status: "already_organized".into(),
                sha256: Some("B".repeat(64)),
                size: 20,
                warnings: vec![],
            },
        ];

        let mut fallback = items[1].clone();
        fallback.plan_status = "ready_uncategorized".into();
        assert_eq!(ready_items(&[items[0].clone(), fallback]).len(), 2);
    }

    #[test]
    fn execute_frozen_external_plan_when_requested() {
        let Ok(plan_path) = std::env::var("S3CC_EXECUTE_PLAN_JSON") else {
            return;
        };
        let data = fs::read(&plan_path).expect("read frozen organization plan");
        let plan = serde_json::from_slice::<OrganizationPlan>(&data)
            .expect("deserialize frozen organization plan");
        assert!(plan.can_execute, "frozen residual plan must be executable");
        let result = execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
            .expect("execute frozen residual plan");
        if let Ok(output) = std::env::var("S3CC_EXECUTE_RESULT_JSON") {
            let output = PathBuf::from(output);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        }
    }
}
