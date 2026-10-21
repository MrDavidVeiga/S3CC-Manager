use crate::{
    category_identity::stable_empty_roots,
    i18n::AppLanguage,
    manifest::{
        replace_manifest_atomic, sha256_file, write_manifest_atomic, RestoreEntry, RestoreManifest,
    },
    operation,
    planner::{
        build_organization_plan_with_cfg_core, normalized_path_key, OrganizationAnalysisSnapshot,
        OrganizationPlan, PlanItem,
    },
    resource_cfg::{package_priority, parse_resource_cfg},
    resource_cfg_update::{apply_resource_cfg_update, rollback_resource_cfg_update},
    unified_analysis,
    workspace::ensure_writable,
};
use chrono::{Local, SecondsFormat};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    time::Instant,
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
    #[serde(default)]
    pub analysis_reconciled: bool,
    /// Complete post-rename views from the shared analysis. Returning this
    /// prevents the frontend from retaining stale priority, severity or
    /// relation IDs while avoiding a second DBPF scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciled_analysis: Option<unified_analysis::UnifiedAnalysis>,
    #[serde(default)]
    pub timings: ExecutionTimings,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimings {
    pub total_ms: u128,
    pub preflight_ms: u128,
    pub manifest_ms: u128,
    pub resource_cfg_ms: u128,
    pub moving_ms: u128,
    pub integrity_ms: u128,
    pub cleanup_ms: u128,
    pub cache_reconcile_ms: u128,
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
            let Some(parent) = common.parent() else {
                return root.to_path_buf();
            };
            common = parent.to_path_buf();
        }
    }
    if !common.starts_with(root) {
        return root.to_path_buf();
    }
    common
}

fn make_manifest_path(root: &Path) -> Result<PathBuf, String> {
    let base = std::env::var_os("S3CC_RESTORE_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.parent()
                .unwrap_or(root)
                .join("S3CC Manager")
                .join("Restore Manifests")
        });

    fs::create_dir_all(&base).map_err(|error| {
        format!(
            "Could not create restore manifest directory {}: {error}",
            base.display()
        )
    })?;

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

        sources.entry(source_key).or_default().push(*item);

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

    for (source_key, peers) in sources.iter().filter(|(_, peers)| peers.len() > 1) {
        let names = peers
            .iter()
            .map(|peer| peer.name.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!(
            "Plan contains multiple ready operations for the same source ({source_key}): {names}. No files were moved."
        ));
    }

    for item in ready {
        let destination = Path::new(item.destination_path.as_deref().unwrap());
        let destination_key = normalized_path_key(root, destination);
        if let Some(peers) = sources.get(&destination_key) {
            if peers.iter().any(|peer| {
                normalized_path_key(root, Path::new(&peer.source_path)) != destination_key
            }) {
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

fn incremental_current_snapshot(
    plan: &OrganizationPlan,
    remaining_legacy_files: usize,
) -> OrganizationAnalysisSnapshot {
    let mut snapshot = plan.analysis_before.clone();
    snapshot.phase = "analysis_current".to_string();
    snapshot.with_plan(&plan.stats, remaining_legacy_files)
}

fn package_extension(path: &Path) -> bool {
    let lower = path
        .file_name()
        .map(|v| v.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    lower.ends_with(".package")
        || lower.ends_with(".package.disabled")
        || lower.ends_with(".package.duplicated")
}

fn snapshot_extension(path: &Path) -> bool {
    if package_extension(path) {
        return true;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff"
            )
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
            .map_err(|error| {
                format!(
                    "Could not resolve planned source {}: {error}",
                    item.source_path
                )
            })?;
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
            let entry =
                entry.map_err(|error| format!("Could not snapshot package tree: {error}"))?;
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

        let (sha256, size) = sha256_file(&absolute).map_err(|error| {
            format!(
                "Could not hash snapshot file {}: {error}",
                absolute.display()
            )
        })?;

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
    let (hash, size) =
        sha256_file(path).map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
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
    let protected = default_stable_empty_directories(root);
    cleanup_empty_directories_after_organization_with_protected(root, moved_pairs, None, &protected)
}

fn cleanup_empty_directories_after_organization_scoped(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    scope_roots: Option<&[PathBuf]>,
) -> (usize, usize, Vec<String>) {
    let protected = default_stable_empty_directories(root);
    cleanup_empty_directories_after_organization_with_protected(
        root,
        moved_pairs,
        scope_roots,
        &protected,
    )
}

/// Compatibility helpers used by targeted cleanup tests do not have a plan
/// carrying its active language.  PT is the historical Manager default. Real
/// plans pass their explicit, language-specific protected list below.
fn default_stable_empty_directories(root: &Path) -> Vec<String> {
    let prefix = if crate::resource_cfg::is_mods_layout_root(root) {
        "Packages\\"
    } else {
        ""
    };
    stable_empty_roots(AppLanguage::Pt)
        .into_iter()
        .map(|name| format!("{prefix}{name}"))
        .collect()
}

fn cleanup_remove_diagnostic(folder: &Path, error: &std::io::Error) -> String {
    let readonly = fs::metadata(folder)
        .map(|metadata| metadata.permissions().readonly())
        .unwrap_or(false);
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        if readonly {
            return format!(
                "Windows denied removal of verified-empty legacy folder {} because it is read-only; it was preserved without changing attributes or ACLs: {error}",
                folder.display()
            );
        }
        return format!(
            "Windows denied removal of verified-empty legacy folder {} after the Manager released its enumeration handle; it may be held by another process or restricted by its ACL. The folder was preserved and no privilege or ACL change was attempted: {error}",
            folder.display()
        );
    }
    format!(
        "Could not remove verified-empty legacy folder {}: {error}",
        folder.display()
    )
}

#[cfg(windows)]
fn windows_file_attributes(folder: &Path) -> Result<u32, std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetFileAttributesW, INVALID_FILE_ATTRIBUTES};

    let mut path = folder.as_os_str().encode_wide().collect::<Vec<_>>();
    path.push(0);
    let attributes = unsafe { GetFileAttributesW(PCWSTR(path.as_ptr())) };
    if attributes == INVALID_FILE_ATTRIBUTES {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(attributes)
    }
}

#[cfg(windows)]
fn set_windows_file_attributes(folder: &Path, attributes: u32) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        SetFileAttributesW, FILE_FLAGS_AND_ATTRIBUTES,
    };

    let mut path = folder.as_os_str().encode_wide().collect::<Vec<_>>();
    path.push(0);
    unsafe {
        SetFileAttributesW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(attributes),
        )
    }
    .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn windows_is_reparse_point(folder: &Path) -> Result<bool, std::io::Error> {
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    Ok(windows_file_attributes(folder)? & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0)
}

fn cleanup_empty_directories_after_organization_with_protected(
    root: &Path,
    _moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    scope_roots: Option<&[PathBuf]>,
    protected_relative_directories: &[String],
) -> (usize, usize, Vec<String>) {
    let is_protected_empty_directory = |path: &Path| {
        protected_relative_directories.iter().any(|relative| {
            normalized_path_key(root, path) == normalized_path_key(root, Path::new(relative))
        })
    };
    let is_loading_branch_root = |path: &Path| {
        (path == root || path.parent() == Some(root))
            && path.file_name().is_some_and(|name| {
                name.to_string_lossy().eq_ignore_ascii_case("Packages")
                    || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
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
        let full_tree_scope = scope_roots.is_some_and(|roots| {
            roots.iter().any(|root_path| {
                normalized_path_key(root, root_path) == normalized_path_key(root, &walk_root)
            })
        });
        if scope_roots.is_none() || full_tree_scope {
            let minimum_depth = usize::from(walk_root == root);
            for entry in WalkDir::new(&walk_root)
                .follow_links(false)
                .min_depth(minimum_depth)
                .into_iter()
            {
                match entry {
                    Ok(entry) if entry.file_type().is_dir() && !entry.file_type().is_symlink() => {
                        let path = entry.path();
                        #[cfg(windows)]
                        if match windows_is_reparse_point(path) {
                            Ok(is_reparse_point) => is_reparse_point,
                            Err(error) => {
                                warnings.push(format!(
                                    "Could not verify whether legacy folder {} is a junction or reparse point: {error}",
                                    path.display()
                                ));
                                true
                            }
                        } {
                            continue;
                        }
                        if is_protected_empty_directory(path) {
                            continue;
                        }
                        // These are the two loading branch roots. They must survive
                        // even if they contain zero packages after organization.
                        if is_loading_branch_root(path) {
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
            if !is_protected_empty_directory(&walk_root)
                && walk_root != root
                && !is_loading_branch_root(&walk_root)
            {
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
                if !is_protected_empty_directory(folder)
                    && folder != root
                    && !is_loading_branch_root(folder)
                {
                    candidates.push(folder.to_path_buf());
                }
                ancestor = folder.parent();
            }
        }
    }

    // Remove children before their parents. As each descendant disappears,
    // another formerly nonempty parent can become empty in the same pass.
    candidates.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    candidates.dedup();
    let mut removed = 0;
    let mut retained_candidates = candidates.clone();
    for folder in candidates {
        if is_protected_empty_directory(&folder) {
            continue;
        }
        // Do not retain a ReadDir handle while calling remove_dir. Windows
        // can otherwise report AccessDenied for a directory the Manager has
        // itself just enumerated. The second check also prevents deletion if
        // another process placed a file in the folder after planning.
        let empty_now = match fs::read_dir(&folder) {
            Ok(mut entries) => entries.next().is_none(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                warnings.push(format!(
                    "Could not inspect legacy folder {} before cleanup: {error}",
                    folder.display()
                ));
                continue;
            }
        };
        if !empty_now {
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
                #[cfg(windows)]
                if error.kind() == std::io::ErrorKind::PermissionDenied {
                    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_READONLY;

                    match windows_file_attributes(&folder) {
                        Ok(attributes) if attributes & FILE_ATTRIBUTE_READONLY.0 != 0 => {
                            // Windows treats FILE_ATTRIBUTE_READONLY on a
                            // directory as a removal restriction. Clear only
                            // that bit on this already-verified empty folder;
                            // ACLs, files, and every other attribute remain
                            // untouched.
                            let writable_attributes =
                                attributes & !FILE_ATTRIBUTE_READONLY.0;
                            match set_windows_file_attributes(
                                &folder,
                                writable_attributes,
                            ) {
                                Ok(()) => match fs::remove_dir(&folder) {
                                    Ok(()) => {
                                        removed += 1;
                                        continue;
                                    }
                                    Err(retry_error)
                                        if retry_error.kind()
                                            == std::io::ErrorKind::NotFound =>
                                    {
                                        // Another actor removed it after
                                        // the attribute update. This is the
                                        // expected idempotent outcome.
                                        continue;
                                    }
                                    Err(retry_error) => {
                                        let restore_error =
                                            set_windows_file_attributes(&folder, attributes)
                                                .err();
                                        let restore_note = restore_error
                                            .map(|restore| {
                                                format!(
                                                    "; restoring original attributes also failed: {restore}"
                                                )
                                            })
                                            .unwrap_or_default();
                                        warnings.push(format!(
                                            "Could not remove verified-empty legacy folder {} after clearing only FILE_ATTRIBUTE_READONLY: {retry_error}{restore_note}",
                                            folder.display()
                                        ));
                                        continue;
                                    }
                                },
                                Err(attribute_error) => {
                                    warnings.push(format!(
                                        "Could not remove verified-empty legacy folder {}: clearing only FILE_ATTRIBUTE_READONLY failed: {attribute_error}; original removal error: {error}",
                                        folder.display()
                                    ));
                                    continue;
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(attribute_error) => {
                            warnings.push(format!(
                                "Could not inspect attributes for verified-empty legacy folder {} after access denied: {attribute_error}; original removal error: {error}",
                                folder.display()
                            ));
                            continue;
                        }
                    }
                }
                // A nonempty directory is expected and must be kept intact.
                // Surface only failures concerning an actually empty folder,
                // including a genuine Windows AccessDenied after the handle
                // from the preflight was released.
                match fs::read_dir(&folder) {
                    Ok(mut contents) => {
                        if contents.next().is_none() {
                            warnings.push(cleanup_remove_diagnostic(&folder, &error));
                        }
                    }
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

    retained_candidates.sort();
    retained_candidates.dedup();
    let retained = retained_candidates
        .into_iter()
        .filter(|path| {
            path.is_dir() && !is_protected_empty_directory(path) && !is_loading_branch_root(path)
        })
        .count();
    (removed, retained, warnings)
}

/// Full legacy cleanup is directory-only and never opens DBPF files.  For a
/// Mods root it is intentionally limited to the two game loading branches;
/// DCCache, logs and unrelated sibling folders are outside Organizer scope.
fn universal_cleanup_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = [root.join("Packages"), root.join("Overrides")]
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.push(root.to_path_buf());
    }
    roots
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
    remaining_legacy_details_with_context(root, moved_pairs, plan_items, &[])
}

/// Reconcile the physical tree after an organization independently of which
/// folders were selected for that transaction. The old implementation only
/// walked parents of moved/blocked plan items, so cleanup-only runs could
/// report zero legacy files while an older unselected folder still contained
/// content. This inventory is directory/file-only: it never opens or hashes
/// DBPF files.
fn remaining_legacy_details_with_context(
    root: &Path,
    moved_pairs: &[(PathBuf, PathBuf, String, u64)],
    plan_items: &[PlanItem],
    protected_relative_directories: &[String],
) -> Vec<LegacyRemainder> {
    let moved_destinations = moved_pairs
        .iter()
        .filter_map(|(_, destination, _, _)| destination.strip_prefix(root).ok())
        .map(|path| {
            path.to_string_lossy()
                .replace('/', "\\")
                .to_ascii_lowercase()
        })
        .collect::<BTreeSet<_>>();
    let package_root = if crate::resource_cfg::is_mods_layout_root(root) {
        root.join("Packages")
    } else {
        root.to_path_buf()
    };
    let inventory_roots = if package_root.is_dir() {
        vec![package_root]
    } else {
        vec![root.to_path_buf()]
    };
    let is_under_relative_directory = |path: &Path, relative: &str| {
        let candidate = normalized_path_key(root, path);
        let directory = normalized_path_key(root, Path::new(relative));
        candidate == directory || candidate.starts_with(&format!("{directory}\\"))
    };
    let is_canonical_or_stable = |path: &Path| {
        if protected_relative_directories
            .iter()
            .any(|relative| is_under_relative_directory(path, relative))
        {
            return true;
        }

        // Only the exact active roots are protected above. Historical aliases
        // such as `Sem Categoria` or `#Duplicates`, and similarly named
        // folders nested below a creator directory, are legacy input and must
        // be reconciled or reported rather than silently treated as canonical.
        false
    };
    let item_by_source = plan_items
        .iter()
        .map(|item| (normalized_path_key(root, Path::new(&item.source_path)), item))
        .collect::<HashMap<_, _>>();
    let moved_sources = moved_pairs
        .iter()
        .map(|(source, _, _, _)| normalized_path_key(root, source))
        .collect::<BTreeSet<_>>();

    let mut remaining = BTreeSet::new();
    let mut details = Vec::new();
    for inventory_root in inventory_roots {
        for entry in WalkDir::new(inventory_root)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(relative) = entry.path().strip_prefix(root) {
                    let relative = relative.to_string_lossy().replace('/', "\\");
                    let path = entry.path();
                    if crate::resource_cfg::is_mods_layout_root(root)
                        && relative
                            .split('\\')
                            .next()
                            .is_some_and(|part| part.eq_ignore_ascii_case("Overrides"))
                    {
                        continue;
                    }
                    // A destination can legitimately remain below the same
                    // ancestor as its old source (for example
                    // Packages\\Substituições -> Packages\\Substituições\\CAS).
                    // It is current content, not a legacy remainder.
                    if moved_destinations.contains(&relative.to_ascii_lowercase())
                        || is_canonical_or_stable(path)
                        || moved_sources.contains(&normalized_path_key(root, path))
                    {
                        continue;
                    }
                    if !remaining.insert(relative.clone()) {
                        continue;
                    }
                    let plan = item_by_source
                        .get(&normalized_path_key(root, path))
                        .copied()
                        .or_else(|| {
                            plan_items.iter().find(|item| {
                                same_relative_path(&item.source_relative_path, &relative)
                            })
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
    let operation_run = operation::begin_mutating("organize", "planning")?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        execute_organization_core(
            folder,
            language,
            selected_paths,
            update_resource_cfg,
            expected_cfg_hash,
            expected_cfg_rules,
        )
    })
    .await
    .map_err(|error| format!("Organization worker failed: {error}"))
    .and_then(|output| output);
    match &result {
        Ok(output)
            if output.status == "COMPLETE"
                || output.status == "COMPLETE_WITH_WARNINGS"
                || output.status == "NO_CHANGES" =>
        {
            operation::finish(&operation_run, "complete", None)
        }
        Ok(output) => operation::finish(&operation_run, "failed", Some(output.status.clone())),
        Err(error) => operation::finish(&operation_run, "failed", Some(error.clone())),
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
    let plan = build_organization_plan_with_cfg_core(
        folder,
        language,
        selected_paths,
        update_resource_cfg,
    )?;
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
    let operation_run = operation::begin_mutating("organize", "executing_plan")?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        execute_organization_plan_core(
            plan,
            language,
            update_resource_cfg,
            expected_cfg_hash,
            expected_cfg_rules,
        )
    })
    .await
    .map_err(|error| format!("Organization worker failed: {error}"))
    .and_then(|output| output);
    match &result {
        Ok(output)
            if output.status == "COMPLETE"
                || output.status == "COMPLETE_WITH_WARNINGS"
                || output.status == "NO_CHANGES" =>
        {
            operation::finish(&operation_run, "complete", None)
        }
        Ok(output) => operation::finish(&operation_run, "failed", Some(output.status.clone())),
        Err(error) => operation::finish(&operation_run, "failed", Some(error.clone())),
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
    let total_started = Instant::now();
    let mut timings = ExecutionTimings::default();
    let ready = ready_items(&plan.items);
    let preflight_started = Instant::now();
    validate_ready_execution_paths(&root, &ready)?;
    timings.preflight_ms = preflight_started.elapsed().as_millis();
    let move_total = ready.len();
    operation::update("organize", 0, None, "preparing");

    if ready.is_empty() {
        // A previous organization may already have emptied legacy category
        // directories. Clean only genuinely empty ones, even if no new move
        // was necessary. Never remove files or the Packages/Overrides roots.
        let cleanup_roots = universal_cleanup_roots(&root);
        let cleanup_started = Instant::now();
        let (removed, retained, cleanup_warnings) =
            cleanup_empty_directories_after_organization_with_protected(
                &root,
                &[],
                Some(&cleanup_roots),
                &plan.protected_empty_directories,
            );
        timings.cleanup_ms = cleanup_started.elapsed().as_millis();
        timings.total_ms = total_started.elapsed().as_millis();
        let remaining_legacy_details = remaining_legacy_details_with_context(
            &root,
            &[],
            &plan.items,
            &plan.protected_empty_directories,
        );
        let remaining_legacy_files = remaining_legacy_details.len();
        let remaining_legacy_examples = remaining_legacy_details
            .iter()
            .take(12)
            .map(|detail| detail.relative_path.clone())
            .collect();
        let analysis_current = Some(incremental_current_snapshot(&plan, remaining_legacy_files));
        return Ok(ExecutionResult {
            status: if cleanup_warnings.is_empty() {
                "NO_CHANGES".to_string()
            } else {
                "COMPLETE_WITH_WARNINGS".to_string()
            },
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
            execution: plan
                .analysis_before
                .clone()
                .with_plan(&plan.stats, remaining_legacy_files),
            analysis_current,
            analysis_reconciled: true,
            reconciled_analysis: None,
            timings,
        });
    }

    let manifest_started = Instant::now();
    let manifest_path = make_manifest_path(&root)?;
    // The verified plan already carries every ready source identity. A
    // transaction needs a durable rollback record for files it can mutate,
    // not a second full-library hash pass before the first rename.
    let snapshot = snapshot_entries(&root, &ready, true)?;
    operation::set_total("organize", move_total);
    operation::update("organize", 0, None, "preparing");
    let mut manifest =
        manifest_from_snapshot(&root, language, snapshot, &plan.directories_to_create);
    // Persist recovery information before touching Resource.cfg or CC files.
    write_manifest_atomic(&manifest_path, &manifest)?;
    timings.manifest_ms = manifest_started.elapsed().as_millis();

    let resource_cfg_started = Instant::now();
    let mut applied_cfg = None;
    if update_resource_cfg {
        let change = plan
            .resource_cfg_update
            .as_ref()
            .ok_or("Resource.cfg opt-in requires a valid Mods/Packages or Mods/Overrides root.")?;
        if expected_cfg_hash.as_deref() != Some(change.original_hash.as_str())
            || expected_cfg_rules != change.added_rules
        {
            manifest.status = "ROLLED_BACK".to_string();
            let _ = replace_manifest_atomic(&manifest_path, &manifest);
            return Err(
                "Resource.cfg preview is stale; review the new plan before organizing.".into(),
            );
        }
        applied_cfg = apply_resource_cfg_update(change)?;

        // Every updated target must be loadable before the first package move.
        // A parsing / IO failure here must also roll the newly installed cfg back.
        let cfg_validation = (|| -> Result<(), String> {
            let cfg_path = PathBuf::from(&change.path);
            let parsed = parse_resource_cfg(&cfg_path)?;
            let cfg_dir = cfg_path.parent().ok_or("Resource.cfg parent missing")?;
            let uncovered = ready
                .iter()
                .filter(|item| {
                    // .package.disabled remains disabled even after relocation;
                    // Resource.cfg intentionally has no PackedFile rule for it.
                    package_extension(Path::new(&item.source_path))
                        && item.source_path.to_ascii_lowercase().ends_with(".package")
                        && item.destination_path.as_ref().is_none_or(|path| {
                            package_priority(&parsed, cfg_dir, Path::new(path)).is_none()
                        })
                })
                .count();
            if uncovered != 0 {
                return Err(format!(
                    "Updated Resource.cfg misses {uncovered} planned packages."
                ));
            }
            Ok(())
        })();
        if let Err(error) = cfg_validation {
            if let Some(applied) = applied_cfg.take() {
                rollback_resource_cfg_update(applied)?;
            }
            manifest.status = "ROLLED_BACK".to_string();
            let _ = replace_manifest_atomic(&manifest_path, &manifest);
            return Err(format!(
                "{error} No packages were moved; Resource.cfg was restored."
            ));
        }

        if let Some(applied) = &applied_cfg {
            manifest.resource_cfg_restore = Some(applied.restore_snapshot(change));
            if let Err(error) = replace_manifest_atomic(&manifest_path, &manifest) {
                if let Some(applied) = applied_cfg.take() {
                    rollback_resource_cfg_update(applied)?;
                }
                return Err(format!(
                    "Could not persist Resource.cfg recovery metadata: {error}"
                ));
            }
        }
    }
    timings.resource_cfg_ms = resource_cfg_started.elapsed().as_millis();

    operation::set_total("organize", move_total);
    operation::update("organize", 0, None, "moving");
    let moving_started = Instant::now();
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
                return Err(format!(
                    "Source disappeared before move: {}",
                    source.display()
                ));
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
                moved_pairs.push((source, destination, expected_hash.clone(), item.size));
                operation::update("organize", moved_pairs.len(), None, "moving");
                #[cfg(test)]
                if moved_pairs.len() == 1
                    && std::env::var_os("S3CC_TEST_ABORT_AFTER_FIRST_MOVE").is_some()
                {
                    errors.push("test-injected failure after first move".to_string());
                    break;
                }
            }
            Err(error) => {
                errors.push(error);
                break;
            }
        }
    }
    timings.moving_ms = moving_started.elapsed().as_millis();

    // A rename is atomic, but the transaction still performs an explicit
    // destination identity check before any legacy-folder cleanup. If an
    // external process changed a moved file, the existing rollback path below
    // restores every verified pair and no cleanup is attempted.
    if errors.is_empty() {
        let integrity_started = Instant::now();
        operation::set_total("organize", moved_pairs.len().max(1));
        operation::update("organize", 0, None, "integrity_check");
        for (index, (_, destination, expected_hash, expected_size)) in
            moved_pairs.iter().enumerate()
        {
            if let Err(error) = verify_identity(destination, expected_hash, *expected_size) {
                errors.push(format!(
                    "Post-move integrity verification failed for {}: {error}",
                    destination.display()
                ));
                break;
            }
            operation::update("organize", index + 1, None, "integrity_check");
        }
        timings.integrity_ms = integrity_started.elapsed().as_millis();
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
                        format!(
                            "Could not recreate original folder {}: {error}",
                            parent.display()
                        )
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
            analysis_reconciled: false,
            reconciled_analysis: None,
            timings: ExecutionTimings {
                total_ms: total_started.elapsed().as_millis(),
                ..timings
            },
        });
    }

    operation::set_total("organize", 1);
    operation::update("organize", 0, None, "cleaning");
    let cleanup_started = Instant::now();
    let cleanup_roots = universal_cleanup_roots(&root);
    let (old_folders_removed, old_folders_retained, cleanup_warnings) =
        cleanup_empty_directories_after_organization_with_protected(
            &root,
            &moved_pairs,
            Some(&cleanup_roots),
            &plan.protected_empty_directories,
        );
    timings.cleanup_ms = cleanup_started.elapsed().as_millis();

    let remaining_legacy_details = remaining_legacy_details_with_context(
        &root,
        &moved_pairs,
        &plan.items,
        &plan.protected_empty_directories,
    );
    let remaining_legacy_files = remaining_legacy_details.len();
    let remaining_legacy_examples = remaining_legacy_details
        .iter()
        .take(12)
        .map(|detail| detail.relative_path.clone())
        .collect();

    manifest.status = if cleanup_warnings.is_empty() {
        "COMPLETE".to_string()
    } else {
        "COMPLETE_WITH_WARNINGS".to_string()
    };
    replace_manifest_atomic(&manifest_path, &manifest)?;
    operation::update("organize", 1, None, "complete");

    let cache_reconcile_started = Instant::now();
    let moved_path_pairs = moved_pairs
        .iter()
        .map(|(source, destination, _, _)| (source.clone(), destination.clone()))
        .collect::<Vec<_>>();
    let reconciled_analysis = match unified_analysis::reconcile_organization_moves(
        &root,
        language,
        &moved_path_pairs,
    ) {
        Ok(reconciled) => reconciled,
        Err(error) => {
            errors.push(format!(
                "Organization completed, but shared analysis needs refresh: {error}"
            ));
            None
        }
    };
    let analysis_reconciled = moved_path_pairs.is_empty() || reconciled_analysis.is_some();
    timings.cache_reconcile_ms = cache_reconcile_started.elapsed().as_millis();
    timings.total_ms = total_started.elapsed().as_millis();
    let analysis_current = Some(incremental_current_snapshot(&plan, remaining_legacy_files));
    let moved_files = moved_pairs
        .iter()
        .map(|(source, destination, _, _)| OrganizationMove {
            source_path: source.to_string_lossy().to_string(),
            destination_path: destination.to_string_lossy().to_string(),
            destination_relative_path: destination
                .strip_prefix(&root)
                .unwrap_or(destination)
                .to_string_lossy()
                .to_string(),
        })
        .collect();
    let organized_directory = organized_parent(&root, &moved_pairs)
        .to_string_lossy()
        .to_string();

    Ok(ExecutionResult {
        status: manifest.status.clone(),
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
        execution: plan
            .analysis_before
            .clone()
            .with_plan(&plan.stats, remaining_legacy_files),
        analysis_current,
        analysis_reconciled,
        reconciled_analysis,
        timings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organized_folder_is_common_parent_of_actual_destinations() {
        let root = std::env::temp_dir().join("S3CC-Test-Mods");
        let packages = root.join("Packages");
        let a = (
            root.join("Old/a.package"),
            packages.join("CAS/Hair/a.package"),
            "a".repeat(64),
            10u64,
        );
        let b = (
            root.join("Old/b.package"),
            packages.join("Scripts/Jogabilidade/Author/b.package"),
            "b".repeat(64),
            20u64,
        );
        assert_eq!(organized_parent(&root, &[a.clone(), b.clone()]), packages);
        assert_eq!(organized_parent(&root, &[a]), packages.join("CAS/Hair"));
        let overrides = (
            root.join("Overrides/x.package"),
            root.join("Overrides/Scripts/x.package"),
            "c".repeat(64),
            10u64,
        );
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
    fn preflight_rejects_duplicate_ready_sources_before_any_move() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-preflight-same-source-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let source = root
            .join("Packages")
            .join("#Duplicados")
            .join("legacy.package");
        let destination_a = root.join("Packages").join("Sliders").join("legacy.package");
        let destination_b = root
            .join("Packages")
            .join("#Duplicados")
            .join("legacy.package.duplicated");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"source bytes").unwrap();

        let items = vec![
            PlanItem {
                id: "normal-selection".into(),
                name: "legacy.package".into(),
                source_path: source.to_string_lossy().into_owned(),
                source_relative_path: "Packages\\#Duplicados\\legacy.package".into(),
                destination_path: Some(destination_a.to_string_lossy().into_owned()),
                destination_relative_path: Some("Packages\\Sliders\\legacy.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 12,
                warnings: vec![],
            },
            PlanItem {
                id: "nested-migration".into(),
                name: "legacy.package".into(),
                source_path: format!(r"\\?\{}", source.display()),
                source_relative_path: "Packages\\#Duplicados\\legacy.package".into(),
                destination_path: Some(destination_b.to_string_lossy().into_owned()),
                destination_relative_path: Some(
                    "Packages\\#Duplicados\\legacy.package.duplicated".into(),
                ),
                classification_status: "duplicate_quarantine".into(),
                classification_reason: None,
                plan_status: "ready_duplicate_quarantine_migration".into(),
                sha256: Some("A".repeat(64)),
                size: 12,
                warnings: vec![],
            },
        ];

        let ready = ready_items(&items);
        let error = validate_ready_execution_paths(&root, &ready).unwrap_err();
        assert!(error.contains("same source"));
        assert!(source.is_file());
        assert!(!destination_a.exists() && !destination_b.exists());
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
            "s3cc-legacy-remaining-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let old = root.join("Packages").join("Legacy");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("picture.png"), b"preserve").unwrap();
        fs::write(old.join("extra.package.disabled"), b"preserve").unwrap();
        fs::create_dir_all(old.join("Nested Legacy").join("Sliders")).unwrap();
        fs::write(
            old.join("Nested Legacy")
                .join("Sliders")
                .join("nested.package"),
            b"preserve nested content",
        )
        .unwrap();
        let source = old.join("original.package");
        let moved = vec![(
            source,
            root.join("Packages/Sliders/original.package"),
            String::new(),
            0,
        )];
        let (count, examples) = remaining_legacy_sources(&root, &moved);
        assert_eq!(count, 3);
        assert_eq!(examples.len(), 3);
        assert!(old.join("picture.png").exists());
        assert!(old.join("extra.package.disabled").exists());
        assert!(examples
            .iter()
            .any(|path| path.ends_with("Nested Legacy\\Sliders\\nested.package")));
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
        assert!(
            details.is_empty(),
            "destination must not be counted as legacy content: {details:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn residual_inventory_reports_unselected_legacy_files_but_not_canonical_review_content() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-residual-inventory-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let legacy = root.join("#4 Hair").join("Female").join("Adult");
        let canonical = root.join("Cabelos e Pêlos").join("Pelos Corporais");
        let review = root.join("#Sem Categoria");
        let review_alias = root.join("Sem Categoria");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&canonical).unwrap();
        fs::create_dir_all(&review).unwrap();
        fs::create_dir_all(&review_alias).unwrap();
        fs::write(legacy.join("unselected.package"), b"legacy").unwrap();
        fs::write(canonical.join("keeper.package"), b"canonical").unwrap();
        fs::write(review.join("unknown.package"), b"review").unwrap();
        fs::write(review_alias.join("alias.package"), b"review alias").unwrap();

        let details = remaining_legacy_details_with_context(
            &root,
            &[],
            &[],
            &[
                "Cabelos e Pêlos".to_string(),
                "#Sem Categoria".to_string(),
            ],
        );
        assert_eq!(details.len(), 2);
        assert!(details.iter().any(|detail| {
            detail.relative_path == "#4 Hair\\Female\\Adult\\unselected.package"
        }));
        assert!(details
            .iter()
            .any(|detail| detail.relative_path == "Sem Categoria\\alias.package"));
        assert_eq!(details[0].reason, "not_selected");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nested_review_alias_is_not_canonicalized_by_folder_name() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-nested-review-alias-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let nested = root.join("Anthon").join("Sem Categoria");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("package.package"), b"legacy").unwrap();

        let details = remaining_legacy_details_with_context(
            &root,
            &[],
            &[],
            &["#Sem Categoria".to_string()],
        );

        assert_eq!(details.len(), 1);
        assert_eq!(
            details[0].relative_path,
            "Anthon\\Sem Categoria\\package.package"
        );
        assert_eq!(details[0].reason, "not_selected");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removes_every_empty_subfolder_after_organization_and_preserves_nonempty_ones() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
            (
                old.join("moved.package"),
                packages.join("Sliders/moved.package"),
                String::new(),
                0,
            ),
            (
                retained.join("moved.package"),
                packages.join("Sliders/other.package"),
                String::new(),
                0,
            ),
            (
                legacy.join("moved.package"),
                packages.join("Sliders/migrated.package"),
                String::new(),
                0,
            ),
            (
                override_old.join("patch.package"),
                overrides.join("Tuning/patch.package"),
                String::new(),
                0,
            ),
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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-copied-cleanup-{}-{nonce}",
            std::process::id()
        ));
        let root = base.join("Mods - Copia");
        let packages = root.join("Packages");
        let overrides = root.join("Overrides");
        let legacy = root.join("#+18").join("Rigged");
        let old = packages.join("Old Categorization").join("Empty");
        for path in [&packages, &overrides, &legacy, &old] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(
            root.join("Resource.cfg"),
            b"Priority 500\nPackedFile Packages/*.package\n",
        )
        .unwrap();
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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-protected-empty-{}-{nonce}",
            std::process::id()
        ));
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
    fn cleanup_removes_empty_translated_alias_but_keeps_pt_canonical_root() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-cleanup-category-alias-{}-{}",
            std::process::id(),
            chrono::Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let packages = base.join("Packages");
        let alias = packages.join("#Duplicates");
        let canonical = packages.join("#Duplicados");
        fs::create_dir_all(&alias).unwrap();
        fs::create_dir_all(&canonical).unwrap();

        let (removed, _, warnings) =
            cleanup_empty_directories_after_organization(&packages, &[]);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(removed >= 1);
        assert!(!alias.exists(), "historical alias was retained");
        assert!(canonical.is_dir(), "active canonical root was removed");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn cleanup_removes_legacy_hair_root_but_preserves_nested_canonical_family() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-cleanup-hair-alias-{}-{}",
            std::process::id(),
            chrono::Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let packages = base.join("Packages");
        let legacy = packages.join("Cabelos").join("Masculino").join("Criança");
        let canonical = packages.join("Cabelos e Pêlos").join("Cabelos").join("Masculino");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&canonical).unwrap();

        let (removed, _, warnings) =
            cleanup_empty_directories_after_organization_with_protected(
                &packages,
                &[],
                None,
                &[
                    "Cabelos e Pêlos".into(),
                    "Cabelos e Pêlos\\Cabelos\\Masculino".into(),
                ],
            );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(removed >= 3);
        assert!(!packages.join("Cabelos").exists());
        assert!(canonical.is_dir());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn no_move_plan_cleans_selected_legacy_root_but_keeps_canonical_empty_tree() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "s3cc-no-move-cleanup-{}-{nonce}",
                std::process::id()
            ));
        let root = base.join("Packages");
        let legacy = root.join("#6 Sliders").join("Old");
        let canonical = root.join("Sliders");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&canonical).unwrap();

        let plan = OrganizationPlan {
            root: root.display().to_string(),
            items: Vec::new(),
            stats: crate::planner::PlanStats::default(),
            directories_to_create: Vec::new(),
            protected_empty_directories: vec!["Sliders".into()],
            cleanup_roots: vec!["#6 Sliders".into()],
            manifest_preview: String::new(),
            resource_cfg_update: None,
            can_execute: true,
            analysis_before: OrganizationAnalysisSnapshot::from_scan(
                "analysis_before",
                &crate::scanner::ScanStats::default(),
            ),
        };

        let result = execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
            .expect("empty cleanup plan");
        assert_eq!(result.status, "NO_CHANGES");
        assert!(result.old_folders_removed >= 2);
        assert!(!legacy.parent().unwrap().exists());
        assert!(canonical.is_dir());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn selected_branch_root_survives_and_only_its_empty_descendants_are_removed() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-cleanup-missing-{}-{nonce}",
            std::process::id()
        ));
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

    #[cfg(windows)]
    #[test]
    fn readonly_empty_legacy_directory_is_removed_by_clearing_only_readonly() {
        use std::time::{SystemTime, UNIX_EPOCH};
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_READONLY;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-readonly-cleanup-{}-{nonce}",
            std::process::id()
        ));
        let root = base.join("Mods");
        let targets = [
            "Packages\\#1 Face\\Make Up\\Blush",
            "Packages\\#1 Face\\Make Up\\Eyeliner",
            "Packages\\#1 Face\\Make Up\\Lipstick",
            "Packages\\#1 Face\\Make Up\\Moles",
            "Packages\\#4 Hair\\Female\\Adult",
            "Packages\\#4 Hair\\Female\\Child",
            "Packages\\#4 Hair\\Female\\Toddler",
            "Packages\\#1 Face\\Eyes",
            "Packages\\#4 Hair\\Eyebrows",
            "Packages\\#7 Objects\\Store",
        ];
        for relative in targets {
            let folder = root.join(relative);
            fs::create_dir_all(&folder).unwrap();
            let original = windows_file_attributes(&folder).unwrap();
            set_windows_file_attributes(&folder, original | FILE_ATTRIBUTE_READONLY.0).unwrap();
            assert!(
                fs::remove_dir(&folder).is_err(),
                "the read-only probe must fail before the targeted retry for {relative}"
            );
        }

        let (removed, _, warnings) = cleanup_empty_directories_after_organization(&root, &[]);
        assert!(removed >= targets.len());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(targets
            .iter()
            .all(|relative| !root.join(relative).exists()));
        assert!(root.join("Packages").is_dir());
        fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn empty_folder_cleanup_never_follows_directory_symlinks() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-cleanup-symlink-{}-{nonce}",
            std::process::id()
        ));
        let mods = base.join("Mods");
        let target = base.join("External").join("Preserve");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        symlink(
            base.join("External"),
            mods.join("Packages").join("External Link"),
        )
        .unwrap();
        let (removed, _, warnings) = cleanup_empty_directories_after_organization(&mods, &[]);
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
    fn fixture_organization_moves_once_and_removes_empty_legacy_tree() {
        let base = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "s3cc-fixture-organization-{}-{}",
                std::process::id(),
                Local::now().timestamp_nanos_opt().unwrap()
            ));
        let root = base.join("Packages");
        let source = root.join("Legacy").join("one.package");
        let destination = root.join("Sliders").join("one.package");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(&source, b"fixture package bytes").unwrap();
        let root = root.canonicalize().unwrap();
        let source = source.canonicalize().unwrap();
        let destination = root.join("Sliders").join("one.package");
        let (hash, size) = sha256_file(&source).unwrap();

        let plan = OrganizationPlan {
            root: root.to_string_lossy().into_owned(),
            items: vec![PlanItem {
                id: "one".into(),
                name: "one.package".into(),
                source_path: source.to_string_lossy().into_owned(),
                source_relative_path: "Legacy\\one.package".into(),
                destination_path: Some(destination.to_string_lossy().into_owned()),
                destination_relative_path: Some("Sliders\\one.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("isolated fixture".into()),
                plan_status: "ready".into(),
                sha256: Some(hash.clone()),
                size,
                warnings: Vec::new(),
            }],
            stats: crate::planner::PlanStats {
                selected: 1,
                ready: 1,
                empty_folders_to_clean: 1,
                ..Default::default()
            },
            directories_to_create: vec!["Sliders".into()],
            protected_empty_directories: vec!["Sliders".into()],
            cleanup_roots: vec!["Legacy".into()],
            manifest_preview: String::new(),
            resource_cfg_update: None,
            can_execute: true,
            analysis_before: OrganizationAnalysisSnapshot::from_scan(
                "analysis_before",
                &crate::scanner::ScanStats::default(),
            ),
        };

        let result = execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
            .expect("fixture organization should complete");
        assert_eq!(result.status, "COMPLETE");
        assert_eq!(result.moved, 1);
        assert!(!source.exists());
        assert!(destination.is_file());
        assert_eq!(sha256_file(&destination).unwrap(), (hash, size));
        assert!(!root.join("Legacy").exists());
        assert!(root.join("Sliders").is_dir());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn stale_fixture_source_is_rejected_before_any_move() {
        let base = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "s3cc-fixture-stale-source-{}-{}",
                std::process::id(),
                Local::now().timestamp_nanos_opt().unwrap()
            ));
        let root = base.join("Packages");
        let source = root.join("Legacy").join("changed.package");
        let destination = root.join("Sliders").join("changed.package");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"original").unwrap();
        let root = root.canonicalize().unwrap();
        let source = source.canonicalize().unwrap();
        let (expected_hash, expected_size) = sha256_file(&source).unwrap();
        fs::write(&source, b"changed").unwrap();

        let plan = OrganizationPlan {
            root: root.to_string_lossy().into_owned(),
            items: vec![PlanItem {
                id: "changed".into(),
                name: "changed.package".into(),
                source_path: source.to_string_lossy().into_owned(),
                source_relative_path: "Legacy\\changed.package".into(),
                destination_path: Some(destination.to_string_lossy().into_owned()),
                destination_relative_path: Some("Sliders\\changed.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some(expected_hash),
                size: expected_size,
                warnings: Vec::new(),
            }],
            stats: crate::planner::PlanStats {
                selected: 1,
                ready: 1,
                ..Default::default()
            },
            directories_to_create: vec!["Sliders".into()],
            protected_empty_directories: vec!["Sliders".into()],
            cleanup_roots: vec!["Legacy".into()],
            manifest_preview: String::new(),
            resource_cfg_update: None,
            can_execute: true,
            analysis_before: OrganizationAnalysisSnapshot::from_scan(
                "analysis_before",
                &crate::scanner::ScanStats::default(),
            ),
        };

        let error = execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
            .unwrap_err();
        assert!(error.contains("changed between Planner and manifest snapshot"));
        assert!(source.is_file());
        assert!(!destination.exists());
        fs::remove_dir_all(base).unwrap();
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

    #[test]
    fn physical_fixture_exercises_planner_executor_restore_and_mid_transaction_rollback() {
        use std::{
            collections::{BTreeMap, BTreeSet},
            panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
            time::{SystemTime, UNIX_EPOCH},
        };

        fn minimal_dbpf(seed: u8) -> Vec<u8> {
            let mut bytes = vec![0u8; 96];
            bytes[0..4].copy_from_slice(&0x4650_424Du32.to_le_bytes());
            bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
            bytes[8] = seed;
            bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
            bytes
        }

        fn write_package(path: &Path, seed: u8) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, minimal_dbpf(seed)).unwrap();
        }

        // A minimal valid DBPF containing S3SA. It is deliberately placed at
        // the Packages root so this end-to-end fixture proves that a package
        // with real internal classification evidence is not mistaken for an
        // intentional root asset such as sim_export_merged.
        fn write_script_package(path: &Path, seed: u8) {
            const TYPE_S3SA: u32 = 0x073F_AA07;
            const DATA_OFFSET: u32 = 96;
            const INDEX_OFFSET: u32 = DATA_OFFSET + 1;
            let mut bytes = vec![0u8; DATA_OFFSET as usize];
            bytes[0..4].copy_from_slice(b"DBPF");
            bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
            bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
            bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
            bytes[44..48].copy_from_slice(&36u32.to_le_bytes());
            bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
            bytes[64..68].copy_from_slice(&INDEX_OFFSET.to_le_bytes());
            bytes.push(seed);
            bytes.extend_from_slice(&0u32.to_le_bytes()); // index type: no shared fields
            for value in [TYPE_S3SA, 0, 0, seed as u32, DATA_OFFSET, 1, 1, 0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }

        fn inventory(root: &Path) -> BTreeMap<String, usize> {
            let mut result = BTreeMap::new();
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let (hash, size) = sha256_file(entry.path()).unwrap();
                let key = format!("{hash}:{size}");
                *result.entry(key).or_insert(0) += 1;
            }
            result
        }

        fn assert_unique_ready_paths(root: &Path, plan: &OrganizationPlan) {
            let mut sources = BTreeSet::new();
            let mut destinations = BTreeSet::new();
            for item in plan
                .items
                .iter()
                .filter(|item| item.plan_status.starts_with("ready"))
            {
                let source = normalized_path_key(root, Path::new(&item.source_path));
                assert!(sources.insert(source), "duplicate ready source: {item:?}");
                let destination =
                    normalized_path_key(root, Path::new(item.destination_path.as_deref().unwrap()));
                assert!(
                    destinations.insert(destination),
                    "duplicate ready destination: {item:?}"
                );
            }
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("s3cc-physical-e2e-{}-{nonce}", std::process::id()));
        let model = base.join("Model").join("Packages");
        let mods = base.join("Fixture Mods");
        let packages = mods.join("Packages");
        let canonical_relative = PathBuf::from("Sliders")
            .join("Rosto")
            .join("Sobrancelhas")
            .join("Canonical.package");
        let canonical_model = model.join(&canonical_relative);
        let canonical_fixture = packages.join(&canonical_relative);
        let legacy_duplicate = packages.join("#6 Sliders").join("Canonical.package");
        let nested_duplicate = packages
            .join("#Duplicados")
            .join("OldBucket")
            .join("Canonical.package");
        let disabled = packages.join("Legacy").join("Review.package.disabled");
        let conflicting_a = packages.join("Legacy").join("Same.package");
        let conflicting_b = packages.join("Other").join("Same.package");
        let auxiliary = packages.join("Legacy").join("readme.txt");
        let root_script = packages.join("RootGameplay.package");
        let old_empty = packages.join("#6 Sliders").join("Empty").join("Nested");
        let official_empty = packages.join("Sliders").join("Chapéus");
        let root_string = packages.to_string_lossy().into_owned();
        let model_string = model.to_string_lossy().into_owned();
        let index_path = base.join("manual-model-index.json");
        let selected_roots = vec![
            packages.join("#6 Sliders"),
            packages.join("#Duplicados"),
            packages.join("Legacy"),
            packages.join("Other"),
        ];
        let mut selected = selected_roots
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        selected.push(root_script.to_string_lossy().into_owned());

        let test_result =
            catch_unwind(AssertUnwindSafe(|| {
                fs::create_dir_all(&model).unwrap();
                fs::create_dir_all(&packages).unwrap();
                fs::create_dir_all(&official_empty).unwrap();
                fs::create_dir_all(model.join("Sliders").join("Chapéus")).unwrap();
                fs::create_dir_all(model.join("Scripts").join("Jogabilidade")).unwrap();
                fs::create_dir_all(model.join("#Sem Categoria")).unwrap();
                fs::create_dir_all(model.join("#Duplicados")).unwrap();
                write_package(&canonical_model, 0);
                write_package(&canonical_fixture, 0);
                write_package(&legacy_duplicate, 0);
                write_package(&nested_duplicate, 0);
                write_package(&disabled, 1);
                write_package(&conflicting_a, 2);
                write_package(&conflicting_b, 3);
                write_script_package(&root_script, 9);
                fs::write(&auxiliary, b"fixture auxiliary").unwrap();
                fs::create_dir_all(&old_empty).unwrap();

                std::env::set_var("S3CC_MANUAL_MODEL_PACKAGES", &model_string);
                std::env::set_var("S3CC_MANUAL_MODEL_INDEX", &index_path);
                std::env::set_var("S3CC_REFRESH_MANUAL_MODEL", "1");

                let scan = tauri::async_runtime::block_on(crate::scanner::scan_packages(
                    root_string.clone(),
                    AppLanguage::Pt,
                ))
                .expect("physical fixture scan");
                assert_eq!(scan.stats.packages, 7);
                assert!(scan.stats.package_reads >= 7);
                let cached_before = crate::scanner::cached_scan_for(&packages, AppLanguage::Pt)
                    .expect("scanner cache after application scan");

                let plan = crate::planner::build_organization_plan_with_cfg_core(
                    root_string.clone(),
                    AppLanguage::Pt,
                    selected.clone(),
                    false,
                )
                .expect("physical fixture organization plan");
                assert!(plan.can_execute, "plan unexpectedly blocked: {plan:?}");
                assert_unique_ready_paths(&packages, &plan);
                let nested_source_key = normalized_path_key(&packages, &nested_duplicate);
                assert_eq!(
                    plan.items
                        .iter()
                        .filter(|item| {
                            item.plan_status.starts_with("ready")
                                && normalized_path_key(&packages, Path::new(&item.source_path))
                                    == nested_source_key
                        })
                        .count(),
                    1,
                    "nested duplicate source must be executable once"
                );
                assert!(plan.items.iter().any(|item| {
                    normalized_path_key(&packages, Path::new(&item.source_path))
                        == normalized_path_key(&packages, &conflicting_b)
                        && item.plan_status == "collision_different_content"
                }));
                assert!(plan.items.iter().any(|item| {
                    normalized_path_key(&packages, Path::new(&item.source_path))
                        == normalized_path_key(&packages, &root_script)
                        && item.plan_status == "ready"
                        && item.destination_relative_path.as_deref()
                            == Some("Scripts\\Jogabilidade\\RootGameplay.package")
                }), "a classified root package must be routed to its canonical category: {plan:?}");
                // This is the same data path used by Visualizar Plano: a second
                // preview must consume the valid scan cache instead of reopening
                // the DBPF corpus.
                std::env::remove_var("S3CC_REFRESH_MANUAL_MODEL");
                let second_plan = crate::planner::build_organization_plan_with_cfg_core(
                    root_string.clone(),
                    AppLanguage::Pt,
                    selected.clone(),
                    false,
                )
                .expect("second physical fixture plan");
                let cached_after = crate::scanner::cached_scan_for(&packages, AppLanguage::Pt)
                    .expect("scanner cache after two previews");
                assert_eq!(
                    cached_before.stats.package_reads,
                    cached_after.stats.package_reads
                );
                assert_eq!(
                    cached_before.stats.dbpf_load_ms,
                    cached_after.stats.dbpf_load_ms
                );
                assert_unique_ready_paths(&packages, &second_plan);

                let before_inventory = inventory(&packages);
                let planning_ms = plan.stats.planning_ms;
                let result =
                    execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
                        .expect("physical fixture organization");
                assert_eq!(result.status, "COMPLETE");
                println!(
                    "physical fixture timing: planning={}ms moving={}ms cleanup={}ms integrity={}ms total={}ms",
                    planning_ms,
                    result.timings.moving_ms,
                    result.timings.cleanup_ms,
                    result.timings.integrity_ms,
                    result.timings.total_ms,
                );
                assert!(
                    result.errors.is_empty(),
                    "unexpected execution warnings: {:?}",
                    result.errors
                );
                assert!(result.moved > 0);
                assert!(!packages.join("#6 Sliders").exists());
                assert!(
                    official_empty.is_dir(),
                    "canonical empty directory was removed"
                );
                assert!(
                    conflicting_b.is_file(),
                    "different-content collision was overwritten"
                );
                assert!(packages
                    .join("#Sem Categoria")
                    .join("Review.package.disabled")
                    .is_file());
                assert!(packages
                    .join("#Sem Categoria")
                    .join("Arquivos Auxiliares")
                    .join("Legacy")
                    .join("readme.txt")
                    .is_file());
                assert!(packages
                    .join("Sliders")
                    .join("Rosto")
                    .join("Sobrancelhas")
                    .join("Canonical.package")
                    .is_file());
                assert!(packages
                    .join("Scripts")
                    .join("Jogabilidade")
                    .join("RootGameplay.package")
                    .is_file(), "classified package was left at Packages root");
                assert!(!root_script.exists());
                assert!(packages
                    .join("#Duplicados")
                    .join("Canonical.package.duplicated")
                    .is_file());
                assert!(packages.join("#Duplicados").is_dir());
                assert!(
                    !packages.join("#Duplicados").join("OldBucket").exists(),
                    "empty legacy bucket below #Duplicados was retained"
                );
                assert!(walkdir::WalkDir::new(packages.join("#Duplicados"))
                    .follow_links(false)
                    .into_iter()
                    .flatten()
                    .filter(|entry| entry.file_type().is_file())
                    .all(|entry| entry.path().parent()
                        == Some(packages.join("#Duplicados").as_path())));
                assert_eq!(
                    before_inventory.values().sum::<usize>(),
                    inventory(&packages).values().sum::<usize>()
                );

                // No second organization should propose a new executable move.
                crate::scanner::invalidate_latest_scan_for(&packages);
                let second_scan = tauri::async_runtime::block_on(crate::scanner::scan_packages(
                    root_string.clone(),
                    AppLanguage::Pt,
                ))
                .expect("post-organization scan");
                assert!(second_scan.stats.packages >= 5);
                let idempotent_selected = vec![root_string.clone()];
                let idempotent_plan = crate::planner::build_organization_plan_with_cfg_core(
                    root_string.clone(),
                    AppLanguage::Pt,
                    idempotent_selected,
                    false,
                )
                .expect("idempotent physical fixture plan");
                assert_eq!(
                    idempotent_plan
                        .items
                        .iter()
                        .filter(|item| item.plan_status.starts_with("ready"))
                        .count(),
                    0,
                    "second organization proposed new moves: {idempotent_plan:?}"
                );
                assert!(idempotent_plan.items.iter().any(|item| {
                    item.plan_status == "collision_different_content"
                        && normalized_path_key(&packages, Path::new(&item.source_path))
                            == normalized_path_key(&packages, &conflicting_b)
                }));

                let manifest = result.manifest_path.clone().expect("restore manifest");
                let restore_preview = crate::restore::preview_restore(
                    manifest.clone(),
                    AppLanguage::Pt,
                    Some(root_string.clone()),
                )
                .expect("physical fixture restore preview");
                assert!(restore_preview.can_execute);
                let restored = crate::restore::execute_restore(
                    manifest,
                    AppLanguage::Pt,
                    Some(root_string.clone()),
                    true,
                )
                .expect("physical fixture restore");
                assert_eq!(restored.status, "RESTORED");
                assert_eq!(before_inventory, inventory(&packages));
                assert!(legacy_duplicate.is_file());
                assert!(nested_duplicate.is_file());
                assert!(disabled.is_file());
                assert!(auxiliary.is_file());
                assert!(legacy_duplicate.parent().unwrap().is_dir());
                assert!(nested_duplicate.parent().unwrap().is_dir());

                // Reproduce a failure after one real rename. The hook is compiled
                // only into tests; production builds retain the normal executor.
                let rollback_source_a = packages.join("Rollback A").join("a.package");
                let rollback_source_b = packages.join("Rollback B").join("b.package");
                write_package(&rollback_source_a, 7);
                write_package(&rollback_source_b, 8);
                let rollback_root = packages.canonicalize().unwrap();
                let rollback_source_a = rollback_source_a.canonicalize().unwrap();
                let rollback_source_b = rollback_source_b.canonicalize().unwrap();
                let rollback_destination_a = rollback_root.join("Rollback Dest").join("a.package");
                let rollback_destination_b = rollback_root.join("Rollback Dest").join("b.package");
                let (hash_a, size_a) = sha256_file(&rollback_source_a).unwrap();
                let (hash_b, size_b) = sha256_file(&rollback_source_b).unwrap();
                let rollback_plan = OrganizationPlan {
                    root: rollback_root.to_string_lossy().into_owned(),
                    items: vec![
                        PlanItem {
                            id: "rollback-a".into(),
                            name: "a.package".into(),
                            source_path: rollback_source_a.to_string_lossy().into_owned(),
                            source_relative_path: "Rollback A\\a.package".into(),
                            destination_path: Some(
                                rollback_destination_a.to_string_lossy().into_owned(),
                            ),
                            destination_relative_path: Some("Rollback Dest\\a.package".into()),
                            classification_status: "classified".into(),
                            classification_reason: None,
                            plan_status: "ready".into(),
                            sha256: Some(hash_a),
                            size: size_a,
                            warnings: Vec::new(),
                        },
                        PlanItem {
                            id: "rollback-b".into(),
                            name: "b.package".into(),
                            source_path: rollback_source_b.to_string_lossy().into_owned(),
                            source_relative_path: "Rollback B\\b.package".into(),
                            destination_path: Some(
                                rollback_destination_b.to_string_lossy().into_owned(),
                            ),
                            destination_relative_path: Some("Rollback Dest\\b.package".into()),
                            classification_status: "classified".into(),
                            classification_reason: None,
                            plan_status: "ready".into(),
                            sha256: Some(hash_b),
                            size: size_b,
                            warnings: Vec::new(),
                        },
                    ],
                    stats: crate::planner::PlanStats {
                        selected: 2,
                        ready: 2,
                        ..Default::default()
                    },
                    directories_to_create: vec!["Rollback Dest".into()],
                    protected_empty_directories: Vec::new(),
                    cleanup_roots: vec!["Rollback A".into(), "Rollback B".into()],
                    resource_cfg_update: None,
                    manifest_preview: String::new(),
                    can_execute: true,
                    analysis_before: OrganizationAnalysisSnapshot::from_scan(
                        "analysis_before",
                        &crate::scanner::ScanStats::default(),
                    ),
                };
                std::env::set_var("S3CC_TEST_ABORT_AFTER_FIRST_MOVE", "1");
                let rolled_back = execute_organization_plan_core(
                    rollback_plan,
                    AppLanguage::Pt,
                    false,
                    None,
                    Vec::new(),
                )
                .expect("injected rollback should return a transaction result");
                std::env::remove_var("S3CC_TEST_ABORT_AFTER_FIRST_MOVE");
                assert_eq!(rolled_back.status, "ROLLED_BACK");
                assert_eq!(rolled_back.rolled_back, 1);
                assert!(rollback_source_a.is_file() && rollback_source_b.is_file());
                assert!(!rollback_destination_a.exists() && !rollback_destination_b.exists());
                assert_eq!(sha256_file(&rollback_source_a).unwrap().1, size_a);
                assert_eq!(sha256_file(&rollback_source_b).unwrap().1, size_b);
            }));

        std::env::remove_var("S3CC_MANUAL_MODEL_PACKAGES");
        std::env::remove_var("S3CC_MANUAL_MODEL_INDEX");
        std::env::remove_var("S3CC_REFRESH_MANUAL_MODEL");
        std::env::remove_var("S3CC_TEST_ABORT_AFTER_FIRST_MOVE");
        let _ = fs::remove_dir_all(&base);
        if let Err(payload) = test_result {
            resume_unwind(payload);
        }
    }

    // Optional physical validation hook. It is inert unless a plan path is
    // supplied by a directed test, and therefore cannot affect production
    // execution. Keeping the call in the real executor path proves that a
    // serialized UI plan is accepted by the same preflight/move/cleanup code.
    #[test]
    fn execute_physical_plan_from_environment_when_requested() {
        let Ok(plan_path) = std::env::var("S3CC_PHYSICAL_PLAN") else {
            return;
        };
        let data = fs::read(&plan_path).expect("read directed physical plan");
        let plan = serde_json::from_slice::<OrganizationPlan>(&data)
            .expect("deserialize directed physical plan");
        let result = execute_organization_plan_core(plan, AppLanguage::Pt, false, None, Vec::new())
            .expect("execute directed physical plan");
        if let Ok(output) = std::env::var("S3CC_PHYSICAL_RESULT") {
            fs::write(
                output,
                serde_json::to_vec_pretty(&result).expect("serialize directed result"),
            )
            .expect("write directed physical result");
        }
    }
}
