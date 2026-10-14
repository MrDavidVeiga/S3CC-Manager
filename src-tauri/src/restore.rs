use crate::{
    workspace::ensure_writable,
    i18n::AppLanguage,
    manifest::{read_manifest, replace_manifest_atomic, sha256_file, RestoreEntry, RestoreManifest},
    resource_cfg_update::restore_cfg_if_safe,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct CurrentPackage {
    absolute: PathBuf,
    relative: PathBuf,
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePlanItem {
    pub kind: String,
    pub status: String,
    pub source_path: Option<String>,
    pub source_relative_path: Option<String>,
    pub destination_path: Option<String>,
    pub destination_relative_path: Option<String>,
    pub sha256: String,
    pub size: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStats {
    pub tracked: usize,
    pub ready_restore: usize,
    pub already_restored: usize,
    pub missing: usize,
    pub changed: usize,
    pub ambiguous: usize,
    pub collisions: usize,
    pub new_files: usize,
    pub ready_new: usize,
    pub already_uncategorized: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePlan {
    pub manifest_path: String,
    pub manifest_status: String,
    pub root: String,
    pub selected_root: Option<String>,
    pub root_matches_selected: bool,
    pub items: Vec<RestorePlanItem>,
    pub stats: RestoreStats,
    pub can_execute: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreExecutionResult {
    pub status: String,
    pub manifest_path: String,
    pub restored: usize,
    pub new_files_relocated: usize,
    pub rolled_back: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

fn is_package(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    name.ends_with(".package")
        || name.ends_with(".package.disabled")
        || name.ends_with(".package.duplicated")
}

fn relative_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\\")
        .to_ascii_lowercase()
}

fn same_relative_path(left: &Path, right: &Path) -> bool {
    relative_key(left) == relative_key(right)
}

fn safe_relative(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }

    let text = path.to_string_lossy();
    if text.starts_with('/') || text.starts_with('\\') {
        return false;
    }

    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return false;
    }

    let mut saw_component = false;
    for component in text.split(['/', '\\']) {
        if component.is_empty() || component == "." || component == ".." {
            return false;
        }
        if component.chars().any(|ch| ch == '\0') {
            return false;
        }
        saw_component = true;
    }

    saw_component
}

fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
    candidate
        .components()
        .zip(root.components())
        .all(|(candidate_component, root_component)| candidate_component == root_component)
        && candidate.components().count() >= root.components().count()
}

fn scan_current_packages(
    root: &Path,
    tracked_entries: &[RestoreEntry],
) -> Result<Vec<CurrentPackage>, String> {
    let mut packages = Vec::new();

    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = entry.map_err(|error| format!("Could not scan restore root: {error}"))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let relative_before_canonical = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| format!("Could not make relative path for {}", entry.path().display()))?;
        let is_manifest_tracked = tracked_entries.iter().any(|tracked| {
            same_relative_path(&tracked.original_relative_path, relative_before_canonical)
                || same_relative_path(&tracked.organized_relative_path, relative_before_canonical)
        });
        // Restore must recognize inactive duplicate markers and non-package
        // companions that this transaction explicitly recorded, while still
        // ignoring unrelated user documents during the "new files" pass.
        if !is_package(entry.path()) && !is_manifest_tracked {
            continue;
        }

        let absolute = entry
            .path()
            .canonicalize()
            .map_err(|error| format!("Could not resolve {}: {error}", entry.path().display()))?;

        if !path_is_within_root(root, &absolute) {
            return Err(format!(
                "Package escaped restore root through filesystem indirection: {}",
                entry.path().display()
            ));
        }

        let relative = absolute
            .strip_prefix(root)
            .map_err(|_| format!("Could not make relative path for {}", absolute.display()))?
            .to_path_buf();

        let (sha256, size) = sha256_file(&absolute)
            .map_err(|error| format!("Could not hash {}: {error}", absolute.display()))?;

        packages.push(CurrentPackage {
            absolute,
            relative,
            sha256,
            size,
        });
    }

    packages.sort_by_key(|item| relative_key(&item.relative));
    Ok(packages)
}

fn identity_matches(file: &CurrentPackage, hash: &str, size: u64) -> bool {
    file.size == size && file.sha256.eq_ignore_ascii_case(hash)
}

fn destination_for_new(
    manifest: &RestoreManifest,
    current_language: AppLanguage,
    relative: &Path,
) -> PathBuf {
    manifest.uncategorized_destination(current_language, relative)
}

fn manifest_ready_for_restore(status: &str) -> bool {
    matches!(status, "COMPLETE" | "RESTORED")
}

#[tauri::command]
pub fn preview_restore(
    manifest_path: String,
    current_language: AppLanguage,
    expected_root: Option<String>,
) -> Result<RestorePlan, String> {
    let manifest_file = PathBuf::from(manifest_path.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve restore manifest: {error}"))?;
    let manifest = read_manifest(&manifest_file)?;

    if !manifest_ready_for_restore(&manifest.status) {
        return Err(format!(
            "Manifest status '{}' is not safe for automatic restore.",
            manifest.status
        ));
    }

    let root = manifest
        .root
        .canonicalize()
        .map_err(|error| format!("Could not resolve manifest root {}: {error}", manifest.root.display()))?;

    if !root.is_dir() {
        return Err(format!("Manifest root is not a directory: {}", root.display()));
    }

    let selected_root = expected_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| {
            path.canonicalize()
                .map_err(|error| format!("Could not resolve selected Mods root: {error}"))
        })
        .transpose()?;
    let root_matches_selected = selected_root
        .as_ref()
        .map(|selected| selected == &root)
        .unwrap_or(false);

    for entry in &manifest.entries {
        if !safe_relative(&entry.original_relative_path)
            || !safe_relative(&entry.organized_relative_path)
        {
            return Err("Manifest contains an unsafe relative path.".to_string());
        }
    }
    for directory in &manifest.created_directories {
        if !safe_relative(directory) {
            return Err("Manifest contains an unsafe created_dir path.".to_string());
        }
    }

    let current = scan_current_packages(&root, &manifest.entries)?;
    let by_relative = current
        .iter()
        .enumerate()
        .map(|(index, file)| (relative_key(&file.relative), index))
        .collect::<HashMap<_, _>>();

    let mut used = HashSet::<usize>::new();
    let mut items = Vec::new();
    let mut stats = RestoreStats {
        tracked: manifest.entries.len(),
        ..RestoreStats::default()
    };

    for entry in &manifest.entries {
        let original_key = relative_key(&entry.original_relative_path);
        let organized_key = relative_key(&entry.organized_relative_path);
        let original_index = by_relative.get(&original_key).copied();
        let organized_index = by_relative.get(&organized_key).copied();

        let destination = root.join(&entry.original_relative_path);

        // Baseline files that were not moved by the organization operation are
        // recorded with original == organized. They are part of the old structure,
        // not "new files" discovered during restore.
        if original_key == organized_key {
            if let Some(index) = original_index {
                let file = &current[index];
                used.insert(index);

                if identity_matches(file, &entry.sha256, entry.size) {
                    stats.already_restored += 1;
                    items.push(RestorePlanItem {
                        kind: "tracked".to_string(),
                        status: "already_restored".to_string(),
                        source_path: Some(file.absolute.to_string_lossy().to_string()),
                        source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                        destination_path: Some(file.absolute.to_string_lossy().to_string()),
                        destination_relative_path: Some(file.relative.to_string_lossy().to_string()),
                        sha256: entry.sha256.clone(),
                        size: entry.size,
                        warnings: Vec::new(),
                    });
                } else {
                    stats.changed += 1;
                    stats.blocked += 1;
                    items.push(RestorePlanItem {
                        kind: "tracked".to_string(),
                        status: "changed".to_string(),
                        source_path: Some(file.absolute.to_string_lossy().to_string()),
                        source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                        destination_path: Some(file.absolute.to_string_lossy().to_string()),
                        destination_relative_path: Some(file.relative.to_string_lossy().to_string()),
                        sha256: entry.sha256.clone(),
                        size: entry.size,
                        warnings: vec![
                            "A pre-existing package that was not moved has changed since organization."
                                .to_string(),
                        ],
                    });
                }
            } else {
                stats.missing += 1;
                stats.blocked += 1;
                items.push(RestorePlanItem {
                    kind: "tracked".to_string(),
                    status: "missing".to_string(),
                    source_path: None,
                    source_relative_path: None,
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(
                        entry.original_relative_path.to_string_lossy().to_string(),
                    ),
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                    warnings: vec![
                        "A pre-existing package that was not moved is no longer present."
                            .to_string(),
                    ],
                });
            }
            continue;
        }

        if let Some(index) = organized_index {
            let file = &current[index];
            if identity_matches(file, &entry.sha256, entry.size) {
                used.insert(index);

                if let Some(original_index) = original_index {
                    if original_index != index {
                        let original_file = &current[original_index];
                        stats.collisions += 1;
                        stats.blocked += 1;
                        items.push(RestorePlanItem {
                            kind: "tracked".to_string(),
                            status: if identity_matches(original_file, &entry.sha256, entry.size) {
                                "collision_same_content".to_string()
                            } else {
                                "collision_different_content".to_string()
                            },
                            source_path: Some(file.absolute.to_string_lossy().to_string()),
                            source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                            destination_path: Some(destination.to_string_lossy().to_string()),
                            destination_relative_path: Some(
                                entry.original_relative_path.to_string_lossy().to_string(),
                            ),
                            sha256: entry.sha256.clone(),
                            size: entry.size,
                            warnings: vec![
                                "Original destination is already occupied; automatic restore will not overwrite it."
                                    .to_string(),
                            ],
                        });
                        continue;
                    }
                }

                stats.ready_restore += 1;
                items.push(RestorePlanItem {
                    kind: "tracked".to_string(),
                    status: "ready_restore".to_string(),
                    source_path: Some(file.absolute.to_string_lossy().to_string()),
                    source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(
                        entry.original_relative_path.to_string_lossy().to_string(),
                    ),
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                    warnings: Vec::new(),
                });
                continue;
            }

            stats.changed += 1;
            stats.blocked += 1;
            items.push(RestorePlanItem {
                kind: "tracked".to_string(),
                status: "changed".to_string(),
                source_path: Some(file.absolute.to_string_lossy().to_string()),
                source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(
                    entry.original_relative_path.to_string_lossy().to_string(),
                ),
                sha256: entry.sha256.clone(),
                size: entry.size,
                warnings: vec![
                    "The file at the organized path no longer matches the manifest identity."
                        .to_string(),
                ],
            });
            continue;
        }

        if let Some(index) = original_index {
            let file = &current[index];
            if identity_matches(file, &entry.sha256, entry.size) {
                used.insert(index);
                stats.already_restored += 1;
                items.push(RestorePlanItem {
                    kind: "tracked".to_string(),
                    status: "already_restored".to_string(),
                    source_path: Some(file.absolute.to_string_lossy().to_string()),
                    source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    destination_path: Some(file.absolute.to_string_lossy().to_string()),
                    destination_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                    warnings: Vec::new(),
                });
                continue;
            }
        }

        let matches = current
            .iter()
            .enumerate()
            .filter(|(index, file)| {
                !used.contains(index) && identity_matches(file, &entry.sha256, entry.size)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        if matches.len() == 1 {
            let index = matches[0];
            let file = &current[index];
            used.insert(index);

            if destination.exists() {
                stats.collisions += 1;
                stats.blocked += 1;
                items.push(RestorePlanItem {
                    kind: "tracked".to_string(),
                    status: "collision_different_content".to_string(),
                    source_path: Some(file.absolute.to_string_lossy().to_string()),
                    source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(
                        entry.original_relative_path.to_string_lossy().to_string(),
                    ),
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                    warnings: vec![
                        "Tracked file was found at another path, but its original destination is occupied."
                            .to_string(),
                    ],
                });
            } else {
                stats.ready_restore += 1;
                items.push(RestorePlanItem {
                    kind: "tracked".to_string(),
                    status: "ready_restore".to_string(),
                    source_path: Some(file.absolute.to_string_lossy().to_string()),
                    source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(
                        entry.original_relative_path.to_string_lossy().to_string(),
                    ),
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                    warnings: vec![
                        "Tracked file was located by SHA-256 because it is no longer at the organized path."
                            .to_string(),
                    ],
                });
            }
        } else if matches.is_empty() {
            stats.missing += 1;
            stats.blocked += 1;
            items.push(RestorePlanItem {
                kind: "tracked".to_string(),
                status: "missing".to_string(),
                source_path: None,
                source_relative_path: None,
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(
                    entry.original_relative_path.to_string_lossy().to_string(),
                ),
                sha256: entry.sha256.clone(),
                size: entry.size,
                warnings: vec!["Tracked file could not be found by path or identity.".to_string()],
            });
        } else {
            stats.ambiguous += 1;
            stats.blocked += 1;
            items.push(RestorePlanItem {
                kind: "tracked".to_string(),
                status: "ambiguous".to_string(),
                source_path: None,
                source_relative_path: None,
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(
                    entry.original_relative_path.to_string_lossy().to_string(),
                ),
                sha256: entry.sha256.clone(),
                size: entry.size,
                warnings: vec![format!(
                    "{} current files match this tracked identity.",
                    matches.len()
                )],
            });
        }
    }

    let uncategorized_root_name = current_language.not_categorized_folder().to_ascii_lowercase();

    for (index, file) in current.iter().enumerate() {
        if used.contains(&index) {
            continue;
        }

        stats.new_files += 1;

        let first_component = file
            .relative
            .components()
            .find_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
                _ => None,
            });

        if first_component.as_deref() == Some(uncategorized_root_name.as_str()) {
            stats.already_uncategorized += 1;
            items.push(RestorePlanItem {
                kind: "new".to_string(),
                status: "already_uncategorized".to_string(),
                source_path: Some(file.absolute.to_string_lossy().to_string()),
                source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                destination_path: Some(file.absolute.to_string_lossy().to_string()),
                destination_relative_path: Some(file.relative.to_string_lossy().to_string()),
                sha256: file.sha256.clone(),
                size: file.size,
                warnings: Vec::new(),
            });
            continue;
        }

        let destination_relative = destination_for_new(&manifest, current_language, &file.relative);
        let destination = root.join(&destination_relative);
        let destination_key = relative_key(&destination_relative);

        if let Some(other_index) = by_relative.get(&destination_key).copied() {
            if other_index != index {
                stats.collisions += 1;
                stats.blocked += 1;
                let other = &current[other_index];
                items.push(RestorePlanItem {
                    kind: "new".to_string(),
                    status: if identity_matches(other, &file.sha256, file.size) {
                        "collision_same_content".to_string()
                    } else {
                        "collision_different_content".to_string()
                    },
                    source_path: Some(file.absolute.to_string_lossy().to_string()),
                    source_relative_path: Some(file.relative.to_string_lossy().to_string()),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(
                        destination_relative.to_string_lossy().to_string(),
                    ),
                    sha256: file.sha256.clone(),
                    size: file.size,
                    warnings: vec![
                        "A new file cannot be relocated because its Not Categorized destination is occupied."
                            .to_string(),
                    ],
                });
                continue;
            }
        }

        stats.ready_new += 1;
        items.push(RestorePlanItem {
            kind: "new".to_string(),
            status: "ready_new".to_string(),
            source_path: Some(file.absolute.to_string_lossy().to_string()),
            source_relative_path: Some(file.relative.to_string_lossy().to_string()),
            destination_path: Some(destination.to_string_lossy().to_string()),
            destination_relative_path: Some(destination_relative.to_string_lossy().to_string()),
            sha256: file.sha256.clone(),
            size: file.size,
            warnings: Vec::new(),
        });
    }

    items.sort_by_key(|item| {
        (
            item.kind.clone(),
            item.source_relative_path.clone().unwrap_or_default().to_ascii_lowercase(),
        )
    });

    Ok(RestorePlan {
        manifest_path: manifest_file.to_string_lossy().to_string(),
        manifest_status: manifest.status,
        root: root.to_string_lossy().to_string(),
        selected_root: selected_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        root_matches_selected,
        can_execute: root_matches_selected
            && stats.blocked == 0
            && (stats.ready_restore + stats.ready_new) > 0,
        stats,
        items,
    })
}

fn verify_identity(path: &Path, hash: &str, size: u64) -> Result<(), String> {
    let (actual_hash, actual_size) = sha256_file(path)
        .map_err(|error| format!("Could not hash {}: {error}", path.display()))?;

    if actual_size != size || !actual_hash.eq_ignore_ascii_case(hash) {
        return Err(format!(
            "Identity changed for {} during restore.",
            path.display()
        ));
    }

    Ok(())
}

fn remove_organizer_directories(root: &Path, manifest: &RestoreManifest) {
    let mut directories = manifest
        .created_directories
        .iter()
        .filter(|relative| safe_relative(relative))
        .map(|relative| root.join(relative))
        .collect::<Vec<_>>();

    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for directory in directories {
        if directory.is_dir() {
            let _ = fs::remove_dir(&directory);
        }
    }
}

pub fn execute_restore(
    manifest_path: String,
    current_language: AppLanguage,
    expected_root: Option<String>,
    confirmed: bool,
) -> Result<RestoreExecutionResult, String> {
    if !confirmed {
        return Err("Restore requires explicit confirmation.".to_string());
    }
    let plan = preview_restore(manifest_path.clone(), current_language, expected_root)?;

    if !plan.can_execute {
        return Err(format!(
            "Restore blocked by preflight: root_match={}, {} blocked item(s), {} collision(s), {} missing, {} changed, {} ambiguous.",
            plan.root_matches_selected,
            plan.stats.blocked,
            plan.stats.collisions,
            plan.stats.missing,
            plan.stats.changed,
            plan.stats.ambiguous
        ));
    }

    let manifest_file = PathBuf::from(&plan.manifest_path);
    let mut manifest = read_manifest(&manifest_file)?;
    let root = PathBuf::from(&plan.root);
    ensure_writable(&root)?;

    let actionable = plan
        .items
        .iter()
        .filter(|item| matches!(item.status.as_str(), "ready_restore" | "ready_new"))
        .cloned()
        .collect::<Vec<_>>();

    let mut moved: Vec<(PathBuf, PathBuf, String, u64, String)> = Vec::new();
    let mut errors = Vec::new();

    for item in actionable {
        let source = PathBuf::from(
            item.source_path
                .as_ref()
                .ok_or_else(|| "Restore action has no source.".to_string())?,
        );
        let destination = PathBuf::from(
            item.destination_path
                .as_ref()
                .ok_or_else(|| "Restore action has no destination.".to_string())?,
        );

        let step = (|| -> Result<(), String> {
            if !source.is_file() {
                return Err(format!("Restore source disappeared: {}", source.display()));
            }
            if destination.exists() {
                return Err(format!(
                    "Restore destination appeared after preflight: {}",
                    destination.display()
                ));
            }

            verify_identity(&source, &item.sha256, item.size)?;

            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
            }

            fs::rename(&source, &destination).map_err(|error| {
                format!(
                    "Could not restore {} -> {}: {error}",
                    source.display(),
                    destination.display()
                )
            })?;

            verify_identity(&destination, &item.sha256, item.size)?;
            Ok(())
        })();

        match step {
            Ok(()) => moved.push((
                source,
                destination,
                item.sha256.clone(),
                item.size,
                item.kind.clone(),
            )),
            Err(error) => {
                errors.push(error);
                break;
            }
        }
    }

    if !errors.is_empty() {
        let mut rollback_errors = Vec::new();
        let mut rolled_back = 0usize;

        for (source, destination, hash, size, _) in moved.iter().rev() {
            let rollback = (|| -> Result<(), String> {
                if source.exists() {
                    return Err(format!(
                        "Restore rollback source is occupied: {}",
                        source.display()
                    ));
                }
                if !destination.is_file() {
                    return Err(format!(
                        "Restore rollback file disappeared: {}",
                        destination.display()
                    ));
                }

                verify_identity(destination, hash, *size)?;

                if let Some(parent) = source.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        format!("Could not recreate {}: {error}", parent.display())
                    })?;
                }

                fs::rename(destination, source).map_err(|error| {
                    format!(
                        "Could not rollback restore {} -> {}: {error}",
                        destination.display(),
                        source.display()
                    )
                })?;

                verify_identity(source, hash, *size)?;
                Ok(())
            })();

            match rollback {
                Ok(()) => rolled_back += 1,
                Err(error) => rollback_errors.push(error),
            }
        }

        if !rollback_errors.is_empty() {
            manifest.status = "RESTORE_ROLLBACK_INCOMPLETE".to_string();
            let _ = replace_manifest_atomic(&manifest_file, &manifest);
        }

        errors.extend(rollback_errors);

        return Ok(RestoreExecutionResult {
            status: if manifest.status == "RESTORE_ROLLBACK_INCOMPLETE" {
                manifest.status.clone()
            } else {
                "ROLLED_BACK".to_string()
            },
            manifest_path: plan.manifest_path,
            restored: moved
                .iter()
                .filter(|(_, _, _, _, kind)| kind == "tracked")
                .count(),
            new_files_relocated: moved
                .iter()
                .filter(|(_, _, _, _, kind)| kind == "new")
                .count(),
            rolled_back,
            errors,
            warnings: Vec::new(),
        });
    }

    remove_organizer_directories(&root, &manifest);

    // Recover Resource.cfg only if the old rules still cover every live
    // package. Never hide newly added CC or replace a user's later edits.
    let mut warnings = Vec::new();
    if let Some(cfg) = &manifest.resource_cfg_restore {
        match restore_cfg_if_safe(&root, cfg) {
            Ok(Some(warning)) => warnings.push(warning),
            Ok(None) => {}
            Err(error) => warnings.push(format!("Resource.cfg was not reverted: {error}")),
        }
    }

    manifest.status = "RESTORED".to_string();
    replace_manifest_atomic(&manifest_file, &manifest)?;

    Ok(RestoreExecutionResult {
        status: "RESTORED".to_string(),
        manifest_path: plan.manifest_path,
        restored: moved
            .iter()
            .filter(|(_, _, _, _, kind)| kind == "tracked")
            .count(),
        new_files_relocated: moved
            .iter()
            .filter(|(_, _, _, _, kind)| kind == "new")
            .count(),
        rolled_back: 0,
        errors: Vec::new(),
        warnings,
    })
}

#[tauri::command(rename = "execute_restore")]
pub async fn execute_restore_async(
    manifest_path: String,
    current_language: AppLanguage,
    expected_root: Option<String>,
    confirmed: bool,
) -> Result<RestoreExecutionResult, String> {
    crate::operation::run_mutating_blocking("restore", "restoring", move || {
        execute_restore(manifest_path, current_language, expected_root, confirmed)
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_manifest_paths_cannot_escape_root() {
        assert!(safe_relative(Path::new(r"CAS\Hair\x.package")));
        assert!(safe_relative(Path::new("CAS/Hair/x.package")));
        assert!(!safe_relative(Path::new(r"..\x.package")));
        assert!(!safe_relative(Path::new("../x.package")));
        assert!(!safe_relative(Path::new(r"\x.package")));
        assert!(!safe_relative(Path::new(r"C:\x.package")));
    }

    #[test]
    fn only_complete_or_restored_manifests_can_be_previewed() {
        assert!(manifest_ready_for_restore("COMPLETE"));
        assert!(manifest_ready_for_restore("RESTORED"));
        assert!(!manifest_ready_for_restore("PENDING"));
        assert!(!manifest_ready_for_restore("ROLLBACK_INCOMPLETE"));
    }
}
