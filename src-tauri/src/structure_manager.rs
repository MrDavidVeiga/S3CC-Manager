use crate::{operation, workspace::ensure_writable};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureEntry {
    pub name: String,
    pub path: String,
    pub relative_path: String,
    pub is_directory: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureListing {
    pub root: String,
    pub current_relative_path: String,
    pub parent_relative_path: Option<String>,
    pub entries: Vec<StructureEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureDirectory {
    pub name: String,
    pub relative_path: String,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualOperationRecord {
    pub created_at: String,
    pub operation: String,
    pub source_relative_path: Option<String>,
    pub destination_relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureActionResult {
    pub operation: String,
    pub source_relative_path: Option<String>,
    pub destination_relative_path: String,
}

fn canonical_root(folder: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve selected root: {error}"))?;
    if !root.is_dir() {
        return Err(format!("Selected root is not a directory: {}", root.display()));
    }
    Ok(root)
}

fn normalize_relative(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Ok(PathBuf::new());
    }

    let normalized = trimmed.replace('\\', "/");
    let candidate = Path::new(&normalized);
    if candidate.is_absolute() {
        return Err("Absolute paths are not allowed.".to_string());
    }

    let mut result = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(value) => result.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Path traversal is not allowed.".to_string())
            }
        }
    }
    Ok(result)
}

fn validate_component(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err("Folder name is empty or invalid.".to_string());
    }
    if trimmed.chars().any(|ch| matches!(ch, '/' | '\\' | '\0')) {
        return Err("Folder name must be a single path component.".to_string());
    }

    #[cfg(windows)]
    {
        if trimmed.ends_with(' ') || trimmed.ends_with('.') {
            return Err("Windows folder names cannot end with a space or dot.".to_string());
        }
        if trimmed
            .chars()
            .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        {
            return Err("Folder name contains characters invalid on Windows.".to_string());
        }
        let stem = trimmed
            .trim_end_matches('.')
            .split('.')
            .next()
            .unwrap_or(trimmed)
            .to_ascii_uppercase();
        let reserved = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5",
            "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5",
            "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        if reserved.contains(&stem.as_str()) {
            return Err("Folder name is reserved on Windows.".to_string());
        }
    }

    Ok(())
}

fn relative_text(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

fn validate_existing_destination_chain(
    root: &Path,
    parent: &Path,
    nested: &Path,
) -> Result<(), String> {
    let mut current = parent.to_path_buf();
    for component in nested.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        if !current.exists() {
            continue;
        }

        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Could not inspect {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Folder creation cannot traverse a symbolic link: {}",
                current.display()
            ));
        }

        let canonical = current
            .canonicalize()
            .map_err(|error| format!("Could not resolve {}: {error}", current.display()))?;
        if !canonical.starts_with(root) {
            return Err("Folder creation would leave the selected root.".to_string());
        }
    }
    Ok(())
}

fn resolve_existing(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", path.display()))?;
    if !canonical.starts_with(root) {
        return Err("Path resolves outside the selected root.".to_string());
    }
    Ok(canonical)
}

fn log_path(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("S3CC Organizer")
        .join("Manual Operations")
        .join("manual-operations-v1.jsonl")
}

pub(crate) fn append_log(root: &Path, record: &ManualOperationRecord) -> Result<(), String> {
    let path = log_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| "Manual-operation log has no parent.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create manual-operation log directory: {error}"))?;
    let mut line = serde_json::to_string(record)
        .map_err(|error| format!("Could not serialize manual-operation record: {error}"))?;
    line.push('\n');

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open manual-operation log {}: {error}", path.display()))?;
    file.write_all(line.as_bytes())
        .map_err(|error| format!("Could not write manual-operation log {}: {error}", path.display()))
}

#[tauri::command]
pub fn list_structure(folder: String, relative_path: String) -> Result<StructureListing, String> {
    let root = canonical_root(&folder)?;
    let relative = normalize_relative(&relative_path)?;
    let current = resolve_existing(&root, &relative)?;

    if !current.is_dir() {
        return Err(format!("Not a directory: {}", current.display()));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&current)
        .map_err(|error| format!("Could not list {}: {error}", current.display()))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let entry_relative = path.strip_prefix(&root).unwrap_or(&path);
        let size = if file_type.is_file() {
            entry.metadata().map(|metadata| metadata.len()).unwrap_or(0)
        } else {
            0
        };
        entries.push(StructureEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
            relative_path: relative_text(entry_relative),
            is_directory: file_type.is_dir(),
            size,
        });
    }

    entries.sort_by(|left, right| {
        right
            .is_directory
            .cmp(&left.is_directory)
            .then_with(|| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()))
    });

    let parent_relative_path = relative.parent().map(relative_text);

    Ok(StructureListing {
        root: root.to_string_lossy().to_string(),
        current_relative_path: relative_text(&relative),
        parent_relative_path,
        entries,
    })
}

#[tauri::command]
pub fn list_structure_directories(folder: String) -> Result<Vec<StructureDirectory>, String> {
    let root = canonical_root(&folder)?;
    let mut directories = vec![StructureDirectory {
        name: "\\".to_string(),
        relative_path: String::new(),
        depth: 0,
    }];

    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_dir() || entry.path_is_symlink() {
            continue;
        }
        let relative = entry.path().strip_prefix(&root).unwrap_or(entry.path());
        directories.push(StructureDirectory {
            name: entry.file_name().to_string_lossy().to_string(),
            relative_path: relative_text(relative),
            depth: relative.components().count(),
        });
    }

    directories.sort_by_key(|item| item.relative_path.to_ascii_lowercase());
    Ok(directories)
}

#[tauri::command]
fn create_structure_folder_core(
    folder: String,
    parent_relative_path: String,
    nested_path: String,
) -> Result<StructureActionResult, String> {
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let parent_relative = normalize_relative(&parent_relative_path)?;
    let parent = resolve_existing(&root, &parent_relative)?;
    if !parent.is_dir() {
        return Err("Selected parent is not a directory.".to_string());
    }

    let nested = normalize_relative(&nested_path)?;
    if nested.as_os_str().is_empty() {
        return Err("New folder path is empty.".to_string());
    }
    for component in nested.components() {
        if let Component::Normal(name) = component {
            validate_component(&name.to_string_lossy())?;
        }
    }

    validate_existing_destination_chain(&root, &parent, &nested)?;

    let destination = parent.join(&nested);
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()));
    }
    if !destination.starts_with(&root) {
        return Err("Destination is outside the selected root.".to_string());
    }

    let mut current = parent.clone();
    let mut created = Vec::<PathBuf>::new();
    for component in nested.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        if current.exists() {
            continue;
        }
        if let Err(error) = fs::create_dir(&current) {
            for created_path in created.iter().rev() {
                let _ = fs::remove_dir(created_path);
            }
            return Err(format!("Could not create {}: {error}", current.display()));
        }
        created.push(current.clone());
    }

    let destination_relative = relative_text(destination.strip_prefix(&root).unwrap_or(&destination));
    let _ = append_log(
        &root,
        &ManualOperationRecord {
            created_at: Local::now().to_rfc3339(),
            operation: "create_folder".to_string(),
            source_relative_path: None,
            destination_relative_path: Some(destination_relative.clone()),
        },
    );

    Ok(StructureActionResult {
        operation: "create_folder".to_string(),
        source_relative_path: None,
        destination_relative_path: destination_relative,
    })
}

#[tauri::command(rename = "create_structure_folder")]
pub fn create_structure_folder(
    folder: String,
    parent_relative_path: String,
    nested_path: String,
) -> Result<StructureActionResult, String> {
    let scope = folder.clone();
    operation::run_mutating_sync(
        "structure_write",
        "creating_folder",
        &[scope.as_str()],
        move || create_structure_folder_core(folder, parent_relative_path, nested_path),
    )
}

#[tauri::command]
fn move_structure_path_core(
    folder: String,
    source_relative_path: String,
    target_parent_relative_path: String,
) -> Result<StructureActionResult, String> {
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let source_relative = normalize_relative(&source_relative_path)?;
    if source_relative.as_os_str().is_empty() {
        return Err("The selected root cannot be moved.".to_string());
    }
    let source = resolve_existing(&root, &source_relative)?;

    let target_relative = normalize_relative(&target_parent_relative_path)?;
    let target_parent = resolve_existing(&root, &target_relative)?;
    if !target_parent.is_dir() {
        return Err("Move destination is not a directory.".to_string());
    }

    if source.is_dir() && target_parent.starts_with(&source) {
        return Err("A folder cannot be moved into itself or one of its descendants.".to_string());
    }

    let name = source
        .file_name()
        .ok_or_else(|| "Source has no file name.".to_string())?;
    let destination = target_parent.join(name);
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()));
    }

    fs::rename(&source, &destination)
        .map_err(|error| format!("Could not move {}: {error}", source.display()))?;

    let destination_relative = relative_text(destination.strip_prefix(&root).unwrap_or(&destination));
    let _ = append_log(
        &root,
        &ManualOperationRecord {
            created_at: Local::now().to_rfc3339(),
            operation: "move".to_string(),
            source_relative_path: Some(relative_text(&source_relative)),
            destination_relative_path: Some(destination_relative.clone()),
        },
    );

    Ok(StructureActionResult {
        operation: "move".to_string(),
        source_relative_path: Some(relative_text(&source_relative)),
        destination_relative_path: destination_relative,
    })
}

#[tauri::command(rename = "move_structure_path")]
pub fn move_structure_path(
    folder: String,
    source_relative_path: String,
    target_parent_relative_path: String,
) -> Result<StructureActionResult, String> {
    let scope = folder.clone();
    operation::run_mutating_sync(
        "structure_write",
        "moving_path",
        &[scope.as_str()],
        move || move_structure_path_core(folder, source_relative_path, target_parent_relative_path),
    )
}

#[tauri::command]
fn rename_structure_folder_core(
    folder: String,
    source_relative_path: String,
    new_name: String,
) -> Result<StructureActionResult, String> {
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let source_relative = normalize_relative(&source_relative_path)?;
    if source_relative.as_os_str().is_empty() {
        return Err("The selected root cannot be renamed.".to_string());
    }
    let source = resolve_existing(&root, &source_relative)?;
    if !source.is_dir() {
        return Err("Only folders can be renamed by this action.".to_string());
    }

    validate_component(&new_name)?;
    let parent = source
        .parent()
        .ok_or_else(|| "Folder has no parent.".to_string())?;
    let destination = parent.join(new_name.trim());
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()));
    }

    fs::rename(&source, &destination)
        .map_err(|error| format!("Could not rename {}: {error}", source.display()))?;

    let destination_relative = relative_text(destination.strip_prefix(&root).unwrap_or(&destination));
    let _ = append_log(
        &root,
        &ManualOperationRecord {
            created_at: Local::now().to_rfc3339(),
            operation: "rename_folder".to_string(),
            source_relative_path: Some(relative_text(&source_relative)),
            destination_relative_path: Some(destination_relative.clone()),
        },
    );

    Ok(StructureActionResult {
        operation: "rename_folder".to_string(),
        source_relative_path: Some(relative_text(&source_relative)),
        destination_relative_path: destination_relative,
    })
}

#[tauri::command(rename = "rename_structure_folder")]
pub fn rename_structure_folder(
    folder: String,
    source_relative_path: String,
    new_name: String,
) -> Result<StructureActionResult, String> {
    let scope = folder.clone();
    operation::run_mutating_sync(
        "structure_write",
        "renaming_folder",
        &[scope.as_str()],
        move || rename_structure_folder_core(folder, source_relative_path, new_name),
    )
}

#[tauri::command]
pub fn list_manual_operations(folder: String) -> Result<Vec<ManualOperationRecord>, String> {
    let root = canonical_root(&folder)?;
    let path = log_path(&root);
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };

    let mut records = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<ManualOperationRecord>(line) {
            records.push(record);
        }
    }
    Ok(records)
}

#[tauri::command]
fn undo_last_manual_operation_core(folder: String) -> Result<StructureActionResult, String> {
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let records = list_manual_operations(folder.clone())?;

    let undone = records
        .iter()
        .filter(|record| record.operation.starts_with("undo:"))
        .filter_map(|record| record.operation.strip_prefix("undo:"))
        .collect::<std::collections::HashSet<_>>();

    let original = records
        .iter()
        .rev()
        .find(|record| {
            !record.operation.starts_with("undo:")
                && !undone.contains(record.created_at.as_str())
        })
        .cloned()
        .ok_or_else(|| "No reversible manual operation is available.".to_string())?;

    let (source_relative, destination_relative) = match original.operation.as_str() {
        "create_folder" => {
            let destination_relative = original
                .destination_relative_path
                .clone()
                .ok_or_else(|| "Create-folder history is missing its destination.".to_string())?;
            let destination = root.join(&destination_relative);
            if !destination.is_dir() {
                return Err("The created folder no longer exists.".to_string());
            }
            let mut entries = fs::read_dir(&destination)
                .map_err(|error| format!("Could not inspect folder before undo: {error}"))?;
            if entries.next().is_some() {
                return Err("The created folder is no longer empty, so undo is blocked.".to_string());
            }
            fs::remove_dir(&destination)
                .map_err(|error| format!("Could not undo folder creation: {error}"))?;
            (Some(destination_relative.clone()), String::new())
        }
        "remove_empty_folder" => {
            let source_relative = original
                .source_relative_path
                .clone()
                .ok_or_else(|| "Empty-folder history is missing its path.".to_string())?;
            let destination = root.join(&source_relative);
            if destination.exists() {
                return Err("The removed empty folder path is already occupied.".to_string());
            }
            fs::create_dir(&destination)
                .map_err(|error| format!("Could not recreate empty folder: {error}"))?;
            (None, source_relative)
        }
        "move" | "rename_folder" => {
            let source_relative = original
                .source_relative_path
                .clone()
                .ok_or_else(|| "History is missing the original path.".to_string())?;
            let destination_relative = original
                .destination_relative_path
                .clone()
                .ok_or_else(|| "History is missing the destination path.".to_string())?;
            let current = root.join(&destination_relative);
            let previous = root.join(&source_relative);
            if !current.exists() {
                return Err("The moved/renamed item no longer exists at its recorded destination.".to_string());
            }
            if previous.exists() {
                return Err("The original path is occupied, so undo would overwrite content.".to_string());
            }
            if let Some(parent) = previous.parent() {
                if !parent.exists() {
                    return Err("The original parent folder no longer exists.".to_string());
                }
            }
            fs::rename(&current, &previous)
                .map_err(|error| format!("Could not undo manual operation: {error}"))?;
            (Some(destination_relative), source_relative)
        }
        _ => return Err(format!("Operation '{}' is not reversible.", original.operation)),
    };

    let record = ManualOperationRecord {
        created_at: Local::now().to_rfc3339(),
        operation: format!("undo:{}", original.created_at),
        source_relative_path: source_relative.clone(),
        destination_relative_path: if destination_relative.is_empty() {
            None
        } else {
            Some(destination_relative.clone())
        },
    };
    let _ = append_log(&root, &record);

    Ok(StructureActionResult {
        operation: "undo".to_string(),
        source_relative_path: source_relative,
        destination_relative_path: destination_relative,
    })
}

#[tauri::command(rename = "undo_last_manual_operation")]
pub fn undo_last_manual_operation(folder: String) -> Result<StructureActionResult, String> {
    let scope = folder.clone();
    operation::run_mutating_sync(
        "structure_write",
        "undoing_operation",
        &[scope.as_str()],
        move || undo_last_manual_operation_core(folder),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("s3cc-organizer-structure-{nonce}"));
        let root = base.join("Packages");
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: &Path) {
        if let Some(base) = root.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }

    #[test]
    fn rejects_traversal() {
        assert!(normalize_relative("../outside").is_err());
        assert!(normalize_relative("safe/folder").is_ok());
        assert!(normalize_relative(r"safe\folder").is_ok());
    }

    #[test]
    fn manual_log_is_outside_selected_tree() {
        let root = Path::new(r"C:\Mods\Packages");
        assert!(log_path(root)
            .to_string_lossy()
            .contains("S3CC Organizer"));
    }

    #[test]
    fn create_move_and_rename_workflow() {
        let root = temp_root();
        let root_text = root.to_string_lossy().to_string();

        create_structure_folder(
            root_text.clone(),
            String::new(),
            r"Creator\Hair".to_string(),
        )
        .unwrap();
        assert!(root.join("Creator").join("Hair").is_dir());

        let source = root.join("Sample.package");
        fs::write(&source, b"package").unwrap();

        move_structure_path(
            root_text.clone(),
            "Sample.package".to_string(),
            r"Creator\Hair".to_string(),
        )
        .unwrap();
        assert!(!source.exists());
        assert!(root
            .join("Creator")
            .join("Hair")
            .join("Sample.package")
            .is_file());

        rename_structure_folder(
            root_text.clone(),
            r"Creator\Hair".to_string(),
            "Female Hair".to_string(),
        )
        .unwrap();
        assert!(root
            .join("Creator")
            .join("Female Hair")
            .join("Sample.package")
            .is_file());

        let records = list_manual_operations(root_text).unwrap();
        assert_eq!(records.len(), 3);
        cleanup(&root);
    }

    #[test]
    fn move_refuses_existing_destination() {
        let root = temp_root();
        let root_text = root.to_string_lossy().to_string();
        fs::create_dir_all(root.join("Target")).unwrap();
        fs::write(root.join("Same.package"), b"source").unwrap();
        fs::write(root.join("Target").join("Same.package"), b"destination").unwrap();

        let result = move_structure_path(
            root_text,
            "Same.package".to_string(),
            "Target".to_string(),
        );
        assert!(result.is_err());
        assert_eq!(fs::read(root.join("Same.package")).unwrap(), b"source");
        assert_eq!(
            fs::read(root.join("Target").join("Same.package")).unwrap(),
            b"destination"
        );
        cleanup(&root);
    }
}
