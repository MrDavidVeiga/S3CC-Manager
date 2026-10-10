//! Persistent, content-based evidence learned from the manual model.
//!
//! The index is deliberately kept in the user's application cache, never in
//! the repository or inside a Mods tree.  It records complete DBPF resource
//! signatures (TGI plus decoded payload hash) and two weaker signatures.  A
//! weaker signature is only used together with a validated correction token;
//! a filename token alone can never create a Fix or Override classification.

use crate::cache::{self, CachedPackage, CachedResource};
use crate::dbpf::Package;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct DbpfResourceSignature {
    pub type_id: u32,
    pub group: u32,
    pub instance: u64,
    pub payload_sha256: String,
    pub payload_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixSignatureRecord {
    /// All TGI and decoded resource payloads, sorted deterministically.
    pub signature_key: String,
    /// Resource type plus decoded payload, ignoring TGI relocation.
    pub semantic_signature_key: String,
    /// Resource type/TGI shape, without payload hashes.
    pub shape_signature_key: String,
    pub resources: Vec<DbpfResourceSignature>,
    pub canonical_destination: String,
    pub loading_branch: String,
    pub model_relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixSignatureIndex {
    pub version: u32,
    pub generated_at: String,
    pub reference_root: String,
    pub records: Vec<FixSignatureRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixSignatureIndexStatus {
    pub path: String,
    pub available: bool,
    pub version: Option<u32>,
    pub records: usize,
    pub generated_at: Option<String>,
    pub reference_root: Option<String>,
    pub can_rebuild: bool,
}

#[derive(Debug, Clone)]
pub struct LearnedFixClassification {
    pub folder_parts: Vec<String>,
    pub detected_from: Vec<String>,
    pub technical_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchKind {
    Exact,
    Semantic,
    ShapeWithValidatedName,
}

fn normalized_parts(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn lower(value: &str) -> String {
    value.replace('/', "\\").to_ascii_lowercase()
}

fn has_fix_reference_component(path: &Path) -> bool {
    normalized_parts(path).iter().any(|part| {
        let value = lower(part);
        value == "correções"
            || value == "correcoes"
            || value == "corrections"
            || value == "fixes"
            || value.contains("eps fixes")
    })
}

fn resource_signature(resource: &CachedResource) -> DbpfResourceSignature {
    DbpfResourceSignature {
        type_id: resource.type_id,
        group: resource.group,
        instance: resource.instance,
        payload_sha256: resource.payload_sha256.to_ascii_uppercase(),
        payload_size: resource.payload_size,
    }
}

fn sorted_resources(resources: &[DbpfResourceSignature]) -> Vec<DbpfResourceSignature> {
    let mut sorted = resources.to_vec();
    sorted.sort();
    sorted
}

fn key_from_parts(parts: impl IntoIterator<Item = String>) -> String {
    let joined = parts.into_iter().collect::<Vec<_>>().join("|");
    format!("{:X}", Sha256::digest(joined.as_bytes()))
}

fn keys(resources: &[DbpfResourceSignature]) -> (String, String, String) {
    let sorted = sorted_resources(resources);
    let exact = sorted.iter().map(|resource| {
        format!(
            "{:08X}:{:08X}:{:016X}:{}:{}",
            resource.type_id,
            resource.group,
            resource.instance,
            resource.payload_sha256,
            resource.payload_size
        )
    });
    let semantic = sorted.iter().map(|resource| {
        format!(
            "{:08X}:{}:{}",
            resource.type_id, resource.payload_sha256, resource.payload_size
        )
    });
    let shape = sorted.iter().map(|resource| {
        format!(
            "{:08X}:{:08X}:{:016X}",
            resource.type_id, resource.group, resource.instance
        )
    });
    (
        key_from_parts(exact),
        key_from_parts(semantic),
        key_from_parts(shape),
    )
}

pub fn package_resources(package: &Package) -> Result<Vec<DbpfResourceSignature>, String> {
    let mut resources = Vec::with_capacity(package.entries.len());
    for entry in &package.entries {
        let data = package
            .data(entry)
            .map_err(|error| format!("Could not decode {}: {error}", entry.key_string()))?;
        resources.push(DbpfResourceSignature {
            type_id: entry.type_id,
            group: entry.group,
            instance: entry.instance,
            payload_sha256: format!("{:X}", Sha256::digest(&data)),
            payload_size: data.len(),
        });
    }
    Ok(sorted_resources(&resources))
}

fn record_from_cached(
    cached: &CachedPackage,
    branch: &str,
    destination: &str,
    model_relative_path: &str,
) -> Option<FixSignatureRecord> {
    if cached.parse_error.is_some() || cached.resources.is_empty() {
        return None;
    }
    let resources = cached
        .resources
        .iter()
        .map(resource_signature)
        .collect::<Vec<_>>();
    let (signature_key, semantic_signature_key, shape_signature_key) = keys(&resources);
    Some(FixSignatureRecord {
        signature_key,
        semantic_signature_key,
        shape_signature_key,
        resources: sorted_resources(&resources),
        canonical_destination: destination
            .replace('/', "\\")
            .trim_matches('\\')
            .to_string(),
        loading_branch: branch.to_string(),
        model_relative_path: model_relative_path.replace('/', "\\"),
    })
}

fn cache_root() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Veiga's S3CC Manager")
        .join("Cache")
}

pub fn index_path() -> PathBuf {
    cache_root().join("fix-signatures-v1.json")
}

pub fn save_persistent_index(index: &FixSignatureIndex) -> Result<(), String> {
    let path = index_path();
    let parent = path
        .parent()
        .ok_or_else(|| "Fix signature index has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create fix signature cache: {error}"))?;
    let temp = parent.join(".fix-signatures-v1.json.tmp");
    let data = serde_json::to_vec_pretty(index)
        .map_err(|error| format!("Could not serialize fix signature index: {error}"))?;
    fs::write(&temp, data)
        .map_err(|error| format!("Could not write fix signature index: {error}"))?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not replace fix signature index: {error}"))?;
    }
    fs::rename(&temp, &path)
        .map_err(|error| format!("Could not commit fix signature index: {error}"))
}

pub fn load_persistent_index() -> Option<FixSignatureIndex> {
    let text = fs::read_to_string(index_path()).ok()?;
    let index = serde_json::from_str::<FixSignatureIndex>(&text).ok()?;
    (index.version == INDEX_VERSION && !index.records.is_empty()).then_some(index)
}

/// Return an explicit, read-only status so the UI can verify whether scans
/// are using the persistent model evidence instead of a test-only cache.
pub fn persistent_index_status() -> FixSignatureIndexStatus {
    let path = index_path();
    let index = load_persistent_index();
    FixSignatureIndexStatus {
        path: path.to_string_lossy().to_string(),
        available: index.is_some(),
        version: index.as_ref().map(|value| value.version),
        records: index.as_ref().map(|value| value.records.len()).unwrap_or(0),
        generated_at: index.as_ref().map(|value| value.generated_at.clone()),
        reference_root: index.as_ref().map(|value| value.reference_root.clone()),
        can_rebuild: true,
    }
}

fn discover_branches(root: &Path) -> Vec<(String, PathBuf)> {
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name == "packages" {
        return vec![("Packages".to_string(), root.to_path_buf())];
    }
    if name == "overrides" {
        return vec![("Overrides".to_string(), root.to_path_buf())];
    }
    ["Packages", "Overrides"]
        .into_iter()
        .map(|branch| (branch.to_string(), root.join(branch)))
        .filter(|(_, path)| path.is_dir())
        .collect()
}

fn package_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("package"))
        })
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    paths
}

pub fn build_fix_signature_index(reference_root: &Path) -> Result<FixSignatureIndex, String> {
    let root = reference_root
        .canonicalize()
        .map_err(|error| format!("Could not resolve reference model: {error}"))?;
    if !root.is_dir() {
        return Err(format!(
            "Reference model is not a directory: {}",
            root.display()
        ));
    }

    let mut fingerprint_cache = cache::load_cache(&root);
    let mut records = Vec::new();
    for (branch, branch_root) in discover_branches(&root) {
        for path in package_files(&branch_root) {
            let relative = path
                .strip_prefix(&branch_root)
                .unwrap_or(&path)
                .to_path_buf();
            let Some(parent) = relative.parent() else {
                continue;
            };
            if !has_fix_reference_component(parent) {
                continue;
            }
            let Some(destination) = parent.to_str() else {
                continue;
            };
            let (cached, _) = cache::get_or_build(&path, &mut fingerprint_cache)?;
            let relative_path = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy();
            if let Some(record) = record_from_cached(&cached, &branch, destination, &relative_path)
            {
                records.push(record);
            }
        }
    }
    cache::save_cache(&root, &fingerprint_cache)?;
    records.sort_by(|left, right| {
        left.loading_branch
            .cmp(&right.loading_branch)
            .then(left.canonical_destination.cmp(&right.canonical_destination))
            .then(left.model_relative_path.cmp(&right.model_relative_path))
    });
    records.dedup_by(|left, right| {
        left.signature_key == right.signature_key
            && left.loading_branch == right.loading_branch
            && left
                .canonical_destination
                .eq_ignore_ascii_case(&right.canonical_destination)
    });

    let generated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string());
    Ok(FixSignatureIndex {
        version: INDEX_VERSION,
        generated_at,
        reference_root: root.to_string_lossy().to_string(),
        records,
    })
}

fn validated_fix_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    normalized.split_whitespace().any(|token| {
        matches!(
            token,
            "fix"
                | "fixed"
                | "fixes"
                | "correction"
                | "corrections"
                | "patch"
                | "patched"
                | "bugfix"
                | "hotfix"
        )
    })
}

fn best_match<'a>(
    index: &'a FixSignatureIndex,
    resources: &[DbpfResourceSignature],
    branch: &str,
    name: &str,
) -> Option<(MatchKind, &'a FixSignatureRecord)> {
    let (exact, semantic, shape) = keys(resources);
    let candidates = index
        .records
        .iter()
        .filter(|record| {
            record.loading_branch.eq_ignore_ascii_case(branch) && record.signature_key == exact
        })
        .collect::<Vec<_>>();
    if let Some(first) = candidates.first() {
        if candidates.iter().all(|record| {
            record
                .canonical_destination
                .eq_ignore_ascii_case(&first.canonical_destination)
        }) {
            return Some((MatchKind::Exact, *first));
        }
    }

    let candidates = index
        .records
        .iter()
        .filter(|record| {
            record.loading_branch.eq_ignore_ascii_case(branch)
                && record.semantic_signature_key == semantic
        })
        .collect::<Vec<_>>();
    if let Some(first) = candidates.first() {
        if candidates.iter().all(|record| {
            record
                .canonical_destination
                .eq_ignore_ascii_case(&first.canonical_destination)
        }) {
            return Some((MatchKind::Semantic, *first));
        }
    }

    if !validated_fix_name(name) {
        return None;
    }
    let candidates = index
        .records
        .iter()
        .filter(|record| {
            record.loading_branch.eq_ignore_ascii_case(branch)
                && record.shape_signature_key == shape
        })
        .collect::<Vec<_>>();
    let first = candidates.first()?;
    if candidates.iter().all(|record| {
        record
            .canonical_destination
            .eq_ignore_ascii_case(&first.canonical_destination)
    }) {
        Some((MatchKind::ShapeWithValidatedName, *first))
    } else {
        None
    }
}

pub fn learned_fix_classification(
    package: &Package,
    index: Option<&FixSignatureIndex>,
    name: &str,
    source_is_override: bool,
) -> Option<LearnedFixClassification> {
    let index = index?;
    let resources = package_resources(package).ok()?;
    let branch = if source_is_override {
        "Overrides"
    } else {
        "Packages"
    };
    let (match_kind, record) = best_match(index, &resources, branch, name)?;
    let parts = record
        .canonical_destination
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    let evidence = match match_kind {
        MatchKind::Exact => "FixSignatureExact",
        MatchKind::Semantic => "FixSignatureSemantic",
        MatchKind::ShapeWithValidatedName => "FixSignatureShapeAndValidatedName",
    };
    let branch_evidence = if source_is_override {
        "OverrideVerified"
    } else {
        "PackagesFixVerified"
    };
    Some(LearnedFixClassification {
        folder_parts: parts.clone(),
        detected_from: vec![
            "ManualModelReference".to_string(),
            evidence.to_string(),
            branch_evidence.to_string(),
        ],
        technical_reason: format!(
            "Manual model DBPF signature matched by {evidence}; canonical {branch} destination is '{}'. Loading branch is preserved from the source; a Packages package is never promoted to Overrides by its filename.",
            parts.join("\\")
        ),
    })
}

#[tauri::command]
pub fn build_persistent_fix_signature_index(
    reference_root: String,
) -> Result<FixSignatureIndex, String> {
    let index = build_fix_signature_index(Path::new(reference_root.trim()))?;
    save_persistent_index(&index)?;
    Ok(index)
}

#[tauri::command]
pub fn get_persistent_fix_signature_index_status() -> FixSignatureIndexStatus {
    persistent_index_status()
}

#[tauri::command]
pub fn rebuild_persistent_fix_signature_index(
    reference_root: String,
) -> Result<FixSignatureIndex, String> {
    build_persistent_fix_signature_index(reference_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(type_id: u32, group: u32, instance: u64, hash: &str) -> DbpfResourceSignature {
        DbpfResourceSignature {
            type_id,
            group,
            instance,
            payload_sha256: hash.to_string(),
            payload_size: hash.len(),
        }
    }

    fn record(resources: Vec<DbpfResourceSignature>, destination: &str) -> FixSignatureRecord {
        let (signature_key, semantic_signature_key, shape_signature_key) = keys(&resources);
        FixSignatureRecord {
            signature_key,
            semantic_signature_key,
            shape_signature_key,
            resources,
            canonical_destination: destination.to_string(),
            loading_branch: "Packages".to_string(),
            model_relative_path: format!("Packages\\{destination}\\reference.package"),
        }
    }

    #[test]
    fn index_records_tgi_and_payload_hashes() {
        let resources = vec![resource(1, 2, 3, "AABB")];
        let index = FixSignatureIndex {
            version: INDEX_VERSION,
            generated_at: "0".to_string(),
            reference_root: "model".to_string(),
            records: vec![record(resources.clone(), "Correções\\Gameplay")],
        };
        assert_eq!(index.records[0].resources, resources);
        assert_ne!(
            index.records[0].signature_key,
            index.records[0].shape_signature_key
        );
    }

    #[test]
    fn shape_learning_requires_fix_evidence_and_never_override_token_alone() {
        assert!(validated_fix_name("creator_patch.package"));
        assert!(!validated_fix_name("creator_override.package"));

        let resources = vec![resource(1, 2, 3, "AABB")];
        let index = FixSignatureIndex {
            version: INDEX_VERSION,
            generated_at: "0".to_string(),
            reference_root: "model".to_string(),
            records: vec![record(resources, "Correções\\Gameplay")],
        };
        let (kind, _) = best_match(
            &index,
            &[resource(1, 2, 3, "DIFFERENT")],
            "Packages",
            "patch.package",
        )
        .unwrap();
        assert_eq!(kind, MatchKind::ShapeWithValidatedName);
        assert!(best_match(
            &index,
            &[resource(1, 2, 3, "DIFFERENT")],
            "Packages",
            "override.package"
        )
        .is_none());
    }

    #[test]
    fn override_records_are_branch_scoped() {
        let resources = vec![resource(1, 2, 3, "AABB")];
        let mut override_record = record(resources.clone(), "Fixes");
        override_record.loading_branch = "Overrides".to_string();
        let index = FixSignatureIndex {
            version: INDEX_VERSION,
            generated_at: "0".to_string(),
            reference_root: "model".to_string(),
            records: vec![override_record],
        };
        assert!(best_match(&index, &resources, "Packages", "fix.package").is_none());
        assert!(best_match(&index, &resources, "Overrides", "anything.package").is_some());
    }

    #[test]
    fn legitimate_override_fix_reference_keeps_branch_and_relative_destination() {
        let cached = CachedPackage {
            path: "model\\Overrides\\Fixes\\reference.package".to_string(),
            size: 12,
            modified_ns: 0,
            file_sha256: "FILE".to_string(),
            dbpf_major: Some(2),
            dbpf_minor: Some(0),
            resources: vec![CachedResource {
                type_id: 0x073F_AA07,
                group: 0,
                instance: 1,
                payload_sha256: "PAYLOAD".to_string(),
                payload_size: 7,
                file_size: 7,
                mem_size: 7,
                compressed: 0,
            }],
            parse_error: None,
        };
        let record = record_from_cached(
            &cached,
            "Overrides",
            "Fixes",
            "Overrides\\Fixes\\reference.package",
        )
        .unwrap();
        assert_eq!(record.loading_branch, "Overrides");
        assert_eq!(record.canonical_destination, "Fixes");
    }

    #[test]
    fn build_reference_index_when_requested_by_validation_environment() {
        let Ok(root) = std::env::var("S3CC_FIX_REFERENCE_ROOT") else {
            return;
        };
        let index = build_fix_signature_index(Path::new(&root)).unwrap();
        assert!(
            !index.records.is_empty(),
            "reference model did not expose fix packages"
        );
        save_persistent_index(&index).unwrap();
        println!(
            "reference_fix_records={} index_path={}",
            index.records.len(),
            index_path().display()
        );
    }
}
