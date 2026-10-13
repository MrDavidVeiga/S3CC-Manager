use crate::operation;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const STORE_VERSION: u32 = 1;
const INTENTIONAL_MARK: &str = "intentional_override";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDecisionRecord {
    pub decision_key: String,
    pub mark: String,
    pub left_sha256: String,
    pub right_sha256: String,
    pub left_relative_path: String,
    pub right_relative_path: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConflictDecisionStore {
    version: u32,
    decisions: BTreeMap<String, ConflictDecisionRecord>,
}

impl Default for ConflictDecisionStore {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            decisions: BTreeMap::new(),
        }
    }
}

fn store_path(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("S3CC Organizer")
        .join("Review")
        .join("conflict-decisions-v1.json")
}

fn load_store(root: &Path) -> ConflictDecisionStore {
    let path = store_path(root);
    let Ok(text) = fs::read_to_string(path) else {
        return ConflictDecisionStore::default();
    };
    let Ok(store) = serde_json::from_str::<ConflictDecisionStore>(&text) else {
        return ConflictDecisionStore::default();
    };
    if store.version != STORE_VERSION {
        return ConflictDecisionStore::default();
    }
    store
}

fn save_store(root: &Path, store: &ConflictDecisionStore) -> Result<(), String> {
    let path = store_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| format!("Review-store path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create review directory {}: {error}", parent.display()))?;

    let temp = parent.join(".conflict-decisions-v1.json.tmp");
    let data = serde_json::to_vec_pretty(store)
        .map_err(|error| format!("Could not serialize conflict decisions: {error}"))?;
    fs::write(&temp, data)
        .map_err(|error| format!("Could not write review-store temp {}: {error}", temp.display()))?;

    #[cfg(windows)]
    {
        // A plain remove_file followed by rename could destroy existing
        // decisions if the second operation failed. Keep a sibling backup
        // until the replacement is successfully installed.
        let backup = parent.join(".conflict-decisions-v1.json.backup");
        if backup.exists() {
            return Err(format!(
                "Conflict decision recovery backup exists at {}; inspect it before continuing.",
                backup.display()
            ));
        }
        let had_previous = path.exists();
        if had_previous {
            fs::rename(&path, &backup)
                .map_err(|error| format!("Could not back up review store {}: {error}", path.display()))?;
        }
        if let Err(error) = fs::rename(&temp, &path) {
            if had_previous {
                fs::rename(&backup, &path).map_err(|restore_error| format!(
                    "Review-store install failed: {error}; original remains at {} (restore failed: {restore_error})",
                    backup.display()
                ))?;
            }
            return Err(format!("Could not install review store {}: {error}", path.display()));
        }
        if had_previous {
            let _ = fs::remove_file(&backup);
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        fs::rename(&temp, &path)
            .map_err(|error| format!("Could not commit review store {}: {error}", path.display()))
    }
}

fn normalized_hash_pair(left: &str, right: &str) -> Result<[String; 2], String> {
    for hash in [left, right] {
        if hash.len() != 64 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err("Conflict decision contains an invalid SHA-256.".to_string());
        }
    }
    let mut hashes = [left.to_ascii_uppercase(), right.to_ascii_uppercase()];
    hashes.sort();
    Ok(hashes)
}

fn expected_decision_key(left: &str, right: &str) -> Result<String, String> {
    let hashes = normalized_hash_pair(left, right)?;
    let digest = format!(
        "{:X}",
        Sha256::digest(format!("{}|{}", hashes[0], hashes[1]).as_bytes())
    );
    Ok(format!("decision:{}", &digest[..24]))
}

fn canonical_root(folder: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve Mods root: {error}"))?;
    if !root.is_dir() {
        return Err(format!("Selected root is not a directory: {}", root.display()));
    }
    Ok(root)
}

#[tauri::command]
pub fn load_conflict_decisions(folder: String) -> Result<Vec<ConflictDecisionRecord>, String> {
    let root = canonical_root(&folder)?;
    Ok(load_store(&root).decisions.into_values().collect())
}

#[tauri::command]
pub fn set_conflict_decision(
    folder: String,
    decision_key: String,
    mark: Option<String>,
    left_sha256: String,
    right_sha256: String,
    left_relative_path: String,
    right_relative_path: String,
) -> Result<Vec<ConflictDecisionRecord>, String> {
    let root = canonical_root(&folder)?;
    if decision_key.trim().is_empty() {
        return Err("Decision key is empty.".to_string());
    }

    let mut store = load_store(&root);

    match mark.as_deref() {
        None => {
            store.decisions.remove(&decision_key);
        }
        Some(INTENTIONAL_MARK) => {
            let expected_key = expected_decision_key(&left_sha256, &right_sha256)?;
            if decision_key != expected_key {
                return Err("Conflict decision key does not match the package hashes.".to_string());
            }
            let hashes = normalized_hash_pair(&left_sha256, &right_sha256)?;
            let mut paths = [left_relative_path, right_relative_path];
            paths.sort();

            store.decisions.insert(
                decision_key.clone(),
                ConflictDecisionRecord {
                    decision_key,
                    mark: INTENTIONAL_MARK.to_string(),
                    left_sha256: hashes[0].clone(),
                    right_sha256: hashes[1].clone(),
                    left_relative_path: paths[0].clone(),
                    right_relative_path: paths[1].clone(),
                    updated_at: Local::now().to_rfc3339(),
                },
            );
        }
        Some(other) => {
            return Err(format!("Unsupported persistent conflict mark: {other}"));
        }
    }

    let scope = root.to_string_lossy().to_string();
    operation::run_mutating_sync("review_write", "saving", &[scope.as_str()], || {
        save_store(&root, &store)
    })?;
    Ok(store.decisions.into_values().collect())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDecisionChange {
    pub decision_key: String,
    pub mark: Option<String>,
    pub left_sha256: String,
    pub right_sha256: String,
    pub left_relative_path: String,
    pub right_relative_path: String,
}

fn apply_bulk_changes(
    store: &mut ConflictDecisionStore,
    changes: Vec<ConflictDecisionChange>,
) -> Result<(), String> {
    // Validate the *whole* batch before modifying the store. A single
    // incorrect key must never persist a partial set of review decisions.
    if changes.len() > 10_000 {
        return Err("Too many conflict review decisions in one batch.".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut updates = Vec::with_capacity(changes.len());
    for change in changes {
        if !seen.insert(change.decision_key.clone()) {
            return Err("Conflict review batch contains a repeated decision key.".into());
        }
        if change.decision_key != expected_decision_key(&change.left_sha256, &change.right_sha256)? {
            return Err("Conflict review decision key does not match package hashes.".into());
        }
        if change.left_relative_path.is_empty() || change.right_relative_path.is_empty() {
            return Err("Conflict decision is missing package paths.".into());
        }
        match change.mark.as_deref() {
            None => updates.push((change.decision_key, None)),
            Some(INTENTIONAL_MARK) => {
                let hashes = normalized_hash_pair(&change.left_sha256, &change.right_sha256)?;
                let mut paths = [change.left_relative_path, change.right_relative_path];
                paths.sort();
                let record = ConflictDecisionRecord {
                    decision_key: change.decision_key.clone(),
                    mark: INTENTIONAL_MARK.into(),
                    left_sha256: hashes[0].clone(),
                    right_sha256: hashes[1].clone(),
                    left_relative_path: paths[0].clone(),
                    right_relative_path: paths[1].clone(),
                    updated_at: Local::now().to_rfc3339(),
                };
                updates.push((change.decision_key, Some(record)));
            }
            Some(other) => return Err(format!("Unsupported bulk review mark: {other}")),
        }
    }
    for (key, record) in updates {
        if let Some(record) = record {
            store.decisions.insert(key, record);
        } else {
            store.decisions.remove(&key);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_conflict_decisions_bulk(
    folder: String,
    changes: Vec<ConflictDecisionChange>,
) -> Result<Vec<ConflictDecisionRecord>, String> {
    let scope = folder.clone();
    operation::run_mutating_blocking_scoped(
        "review_write",
        "saving",
        &[scope.as_str()],
        move || {
        let root = canonical_root(&folder)?;
        if changes.is_empty() {
            return Ok(load_store(&root).decisions.into_values().collect());
        }
        let mut store = load_store(&root);
        apply_bulk_changes(&mut store, changes)?;
        // One persistence operation rather than N reads and writes.
        save_store(&root, &store)?;
        Ok(store.decisions.into_values().collect())
        },
    ).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_validation_is_atomic_and_deduplicates_no_decisions() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let key = expected_decision_key(&a, &b).unwrap();
        let valid = ConflictDecisionChange {
            decision_key: key.clone(),
            mark: Some(INTENTIONAL_MARK.into()),
            left_sha256: a.clone(), right_sha256: b.clone(),
            left_relative_path: "Packages/a.package".into(),
            right_relative_path: "Packages/b.package".into(),
        };
        let mut store = ConflictDecisionStore::default();
        assert!(apply_bulk_changes(&mut store, vec![
            valid.clone(),
            ConflictDecisionChange { decision_key: "wrong".into(), ..valid.clone() },
        ]).is_err());
        assert!(store.decisions.is_empty());
        apply_bulk_changes(&mut store, vec![valid.clone()]).unwrap();
        assert_eq!(store.decisions.len(), 1);
        assert_eq!(store.decisions.get(&key).unwrap().mark, INTENTIONAL_MARK);
        let duplicate = vec![valid.clone(), valid.clone()];
        assert!(apply_bulk_changes(&mut store, duplicate).is_err());
        assert_eq!(store.decisions.len(), 1);
        let clear = ConflictDecisionChange { mark: None, ..valid };
        apply_bulk_changes(&mut store, vec![clear]).unwrap();
        assert!(store.decisions.is_empty());
    }

    #[test]
    fn bulk_review_scales_without_dropping_decisions() {
        let mut changes = Vec::new();
        for index in 1..=1_200u32 {
            let left = format!("{index:064x}");
            let right = "f".repeat(64);
            changes.push(ConflictDecisionChange {
                decision_key: expected_decision_key(&left, &right).unwrap(),
                mark: Some(INTENTIONAL_MARK.into()),
                left_sha256: left,
                right_sha256: right.clone(),
                left_relative_path: format!("Packages/Mod{index}.package"),
                right_relative_path: "Packages/Core.package".into(),
            });
        }
        let mut store = ConflictDecisionStore::default();
        apply_bulk_changes(&mut store, changes).unwrap();
        assert_eq!(store.decisions.len(), 1_200);
    }

    #[test]
    fn store_path_is_outside_packages_tree() {
        let root = Path::new(r"C:\Mods\Packages");
        let path = store_path(root).to_string_lossy().to_string();
        assert!(path.contains("S3CC Organizer"));
        assert!(path.contains("Review"));
    }
}
