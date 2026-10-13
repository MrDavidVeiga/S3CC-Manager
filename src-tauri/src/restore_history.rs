use crate::manifest::read_manifest;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreHistoryItem {
    pub path: String,
    pub file_name: String,
    pub status: String,
    pub created_at: String,
    pub modified_unix_ms: u64,
    pub files: usize,
    pub root: Option<String>,
    pub matches_selected_root: bool,
    pub valid: bool,
    pub error: Option<String>,
}

fn manifests_dir(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("S3CC Manager")
        .join("Restore Manifests")
}

fn legacy_manifests_dir(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("S3CC Organizer")
        .join("Restore Manifests")
}

fn collect_directory(
    directory: &Path,
    root: &Path,
    items: &mut Vec<RestoreHistoryItem>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(directory)
        .map_err(|error| format!("Could not list {}: {error}", directory.display()))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("txt"))
                .unwrap_or(false)
        {
            continue;
        }

        let metadata = fs::metadata(&path).ok();
        let modified_unix_ms = metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);

        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("manifest.txt")
            .to_string();

        match read_manifest(&path) {
            Ok(manifest) => {
                let manifest_root = manifest.root.canonicalize().ok();
                items.push(RestoreHistoryItem {
                    path: path.to_string_lossy().to_string(),
                    file_name,
                    status: manifest.status,
                    created_at: manifest.created_at,
                    modified_unix_ms,
                    files: manifest.entries.len(),
                    root: Some(manifest.root.to_string_lossy().to_string()),
                    matches_selected_root: manifest_root
                        .as_ref()
                        .map(|manifest_root| manifest_root == root)
                        .unwrap_or(false),
                    valid: true,
                    error: None,
                });
            }
            Err(error) => items.push(RestoreHistoryItem {
                path: path.to_string_lossy().to_string(),
                file_name,
                status: "INVALID".to_string(),
                created_at: String::new(),
                modified_unix_ms,
                files: 0,
                root: None,
                matches_selected_root: false,
                valid: false,
                error: Some(error),
            }),
        }
    }

    Ok(())
}

#[tauri::command]
pub fn list_restore_history(folder: String) -> Result<Vec<RestoreHistoryItem>, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve restore root: {error}"))?;

    let mut items = Vec::new();
    collect_directory(&manifests_dir(&root), &root, &mut items)?;
    collect_directory(&legacy_manifests_dir(&root), &root, &mut items)?;

    items.sort_by_key(|item| std::cmp::Reverse(item.modified_unix_ms));
    Ok(items)
}

/// Remove only a manifest indexed in this Mods root's managed Restore History.
/// Packages and their directory structure are never touched.
#[tauri::command]
pub fn remove_restore_history(
    folder: String,
    manifest_path: String,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("Removing a restore record requires explicit confirmation.".to_string());
    }
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve restore root: {error}"))?;
    if !root.is_dir() {
        return Err("Selected Mods root is not a directory.".to_string());
    }

    let requested = PathBuf::from(manifest_path.trim());
    let metadata = fs::symlink_metadata(&requested)
        .map_err(|error| format!("Could not inspect restore manifest: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Only regular restore manifest files may be removed.".to_string());
    }

    let canonical = requested.canonicalize()
        .map_err(|error| format!("Could not resolve restore manifest: {error}"))?;
    if !canonical
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("txt"))
        .unwrap_or(false)
    {
        return Err("Only .txt restore manifests may be removed.".to_string());
    }

    // Disallow arbitrary .txt files, including paths elsewhere on the disk.
    // Both current and legacy managed manifest directories are supported.
    let authorized_directory = [manifests_dir(&root), legacy_manifests_dir(&root)]
        .iter()
        .filter_map(|directory| directory.canonicalize().ok())
        .any(|directory| canonical.parent() == Some(directory.as_path()));
    if !authorized_directory {
        return Err("This file is not in a managed Restore Manifests directory.".to_string());
    }

    let known = list_restore_history(root.to_string_lossy().into_owned())?;
    let indexed = known.iter().find(|item| Path::new(&item.path) == requested);
    let Some(indexed) = indexed else {
        return Err("This manifest is not listed in the selected Mods root's history.".to_string());
    };
    if indexed.valid && !indexed.matches_selected_root {
        return Err("This manifest belongs to a different Mods root.".to_string());
    }

    crate::workspace::ensure_writable(&root)?;
    let scope = canonical.to_string_lossy().to_string();
    operation::run_mutating_sync("restore_history_write", "removing", &[scope.as_str()], || {
        fs::remove_file(&canonical)
            .map_err(|error| format!("Could not remove restore manifest: {error}"))
    })
}

use crate::operation;
