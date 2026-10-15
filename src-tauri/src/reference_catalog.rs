//! Persistent, metadata-only reference catalogs for The Sims 3 Store and the
//! installed game's official DBPF archives.
//!
//! Catalogs contain no copied package bytes.  They retain file/resource hashes
//! and provenance so the scanner can distinguish an exact Store match, a
//! technical resource-set match, a possible replacement, and an ambiguous
//! partial overlap without treating a shared TGI as proof by itself.

use crate::{cache, dbpf::Package};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use walkdir::WalkDir;

pub const CATALOG_VERSION: u32 = 1;
const STORE_LABEL: &str = "Sims 3 Store";
const OFFICIAL_LABEL: &str = "The Sims 3 Official Resources";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceResource {
    pub type_id: u32,
    pub group: u32,
    pub instance: u64,
    pub payload_sha256: String,
    pub payload_size: usize,
}

impl ReferenceResource {
    fn key(&self) -> String {
        format!(
            "{:08X}:{:08X}:{:016X}",
            self.type_id, self.group, self.instance
        )
    }

    fn technical_key(&self) -> String {
        format!("{}:{}", self.key(), self.payload_sha256)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePackageReference {
    pub relative_path: String,
    pub category_parts: Vec<String>,
    pub name: String,
    pub file_size: u64,
    pub sha256: String,
    pub dbpf_major: Option<u32>,
    pub dbpf_minor: Option<u32>,
    pub resources: Vec<ReferenceResource>,
    pub resource_types: Vec<u32>,
    pub resource_count: usize,
    pub tgi_signature: String,
    pub technical_signature: String,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialArchiveReference {
    pub relative_path: String,
    pub name: String,
    pub product: String,
    pub version: Option<String>,
    pub file_size: u64,
    pub sha256: String,
    pub dbpf_major: Option<u32>,
    pub dbpf_minor: Option<u32>,
    pub resources: Vec<ReferenceResource>,
    pub resource_count: usize,
    pub resource_type_counts: BTreeMap<u32, usize>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreCatalog {
    pub version: u32,
    pub generated_at: String,
    pub source_label: String,
    pub source_root: String,
    pub directories: Vec<String>,
    pub packages: Vec<StorePackageReference>,
    pub package_count: usize,
    pub resource_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialCatalog {
    pub version: u32,
    pub generated_at: String,
    pub source_label: String,
    pub source_root: String,
    pub archives: Vec<OfficialArchiveReference>,
    pub archive_count: usize,
    pub resource_count: usize,
    pub products: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEvidence {
    pub store_match_kind: Option<String>,
    pub store_confidence: Option<String>,
    pub store_reference_paths: Vec<String>,
    pub store_category_parts: Vec<String>,
    pub official_overlap_kind: Option<String>,
    pub official_confidence: Option<String>,
    pub official_occurrences: Vec<String>,
    pub shared_tgi_count: usize,
    pub identical_resource_count: usize,
    pub changed_resource_count: usize,
    pub unmatched_resource_count: usize,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceCatalogStatus {
    pub store_catalog_path: String,
    pub official_catalog_path: String,
    pub store_available: bool,
    pub official_available: bool,
    pub store_packages: usize,
    pub store_resources: usize,
    pub official_archives: usize,
    pub official_resources: usize,
    pub store_version: Option<u32>,
    pub official_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceCatalogBuildResult {
    pub status: ReferenceCatalogStatus,
    pub store_catalog_path: String,
    pub official_catalog_path: String,
    pub store_packages: usize,
    pub store_resources: usize,
    pub official_archives: usize,
    pub official_resources: usize,
    pub store_directories: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceCatalogSet {
    pub store: Option<StoreCatalog>,
    pub official: Option<OfficialCatalog>,
}

fn data_root() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Veiga's S3CC Manager")
        .join("Data")
        .join("Reference Catalogs")
}

pub fn store_catalog_path() -> PathBuf {
    data_root().join("Sims 3 Store").join("catalog-v1.json")
}

pub fn official_catalog_path() -> PathBuf {
    data_root()
        .join("Official Game Resources")
        .join("catalog-v1.json")
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("catalog path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).context("serialize reference catalog")?;
    fs::write(&temporary, bytes).with_context(|| format!("write {}", temporary.display()))?;
    if path.exists() {
        let backup = path.with_extension("json.bak");
        let _ = fs::copy(path, &backup);
    }
    fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn read_catalog<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn catalog_cache() -> &'static Mutex<Option<ReferenceCatalogSet>> {
    static CACHE: OnceLock<Mutex<Option<ReferenceCatalogSet>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_catalog_cache() {
    if let Ok(mut cache) = catalog_cache().lock() {
        *cache = None;
    }
}

pub fn load_catalogs() -> ReferenceCatalogSet {
    if let Ok(cache) = catalog_cache().lock() {
        if let Some(value) = cache.as_ref() {
            return value.clone();
        }
    }
    let value = ReferenceCatalogSet {
        store: read_catalog(&store_catalog_path()),
        official: read_catalog(&official_catalog_path()),
    };
    if let Ok(mut cache) = catalog_cache().lock() {
        *cache = Some(value.clone());
    }
    value
}

pub fn get_status() -> ReferenceCatalogStatus {
    let store_path = store_catalog_path();
    let official_path = official_catalog_path();
    let catalogs = load_catalogs();
    ReferenceCatalogStatus {
        store_catalog_path: store_path.to_string_lossy().to_string(),
        official_catalog_path: official_path.to_string_lossy().to_string(),
        store_available: catalogs.store.is_some(),
        official_available: catalogs.official.is_some(),
        store_packages: catalogs
            .store
            .as_ref()
            .map_or(0, |value| value.package_count),
        store_resources: catalogs
            .store
            .as_ref()
            .map_or(0, |value| value.resource_count),
        official_archives: catalogs
            .official
            .as_ref()
            .map_or(0, |value| value.archive_count),
        official_resources: catalogs
            .official
            .as_ref()
            .map_or(0, |value| value.resource_count),
        store_version: catalogs.store.as_ref().map(|value| value.version),
        official_version: catalogs.official.as_ref().map(|value| value.version),
    }
}

fn normalized_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('/', "\\")
}

fn resource_from_cached(resource: &cache::CachedResource) -> ReferenceResource {
    ReferenceResource {
        type_id: resource.type_id,
        group: resource.group,
        instance: resource.instance,
        payload_sha256: resource.payload_sha256.clone(),
        payload_size: resource.payload_size,
    }
}

fn signatures(resources: &[ReferenceResource]) -> (String, String) {
    let mut tgi = resources
        .iter()
        .map(ReferenceResource::key)
        .collect::<Vec<_>>();
    let mut technical = resources
        .iter()
        .map(ReferenceResource::technical_key)
        .collect::<Vec<_>>();
    tgi.sort_unstable();
    technical.sort_unstable();
    (digest_lines(&tgi), digest_lines(&technical))
}

fn digest_lines(lines: &[String]) -> String {
    let mut digest = Sha256::new();
    for line in lines {
        digest.update(line.as_bytes());
        digest.update([0]);
    }
    format!("{:X}", digest.finalize())
}

fn category_parts(relative: &str) -> Vec<String> {
    let mut parts = relative
        .replace('/', "\\")
        .split('\\')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let _ = parts.pop();
    parts
}

fn is_reference_file(path: &Path, official: bool) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if official {
        matches!(extension.as_str(), "package" | "dbc" | "ebc" | "world")
    } else {
        extension == "package"
    }
}

fn catalog_entry(
    path: &Path,
    cache_index: &mut cache::FingerprintCache,
) -> Result<cache::CachedPackage, String> {
    cache::get_or_build(path, cache_index).map(|(entry, _)| entry)
}

fn store_reference(
    path: &Path,
    root: &Path,
    cache_index: &mut cache::FingerprintCache,
) -> StorePackageReference {
    let relative = normalized_relative(root, path);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    match catalog_entry(path, cache_index) {
        Ok(entry) => {
            let resources = entry
                .resources
                .iter()
                .map(resource_from_cached)
                .collect::<Vec<_>>();
            let resource_types = resources
                .iter()
                .map(|value| value.type_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let (tgi_signature, technical_signature) = signatures(&resources);
            StorePackageReference {
                relative_path: relative,
                category_parts: category_parts(&normalized_relative(root, path)),
                name,
                file_size: entry.size,
                sha256: entry.file_sha256,
                dbpf_major: entry.dbpf_major,
                dbpf_minor: entry.dbpf_minor,
                resource_count: resources.len(),
                resources,
                resource_types,
                tgi_signature,
                technical_signature,
                parse_error: entry.parse_error,
            }
        }
        Err(error) => StorePackageReference {
            relative_path: relative,
            category_parts: category_parts(&normalized_relative(root, path)),
            name,
            file_size: fs::metadata(path).map(|value| value.len()).unwrap_or(0),
            sha256: String::new(),
            dbpf_major: None,
            dbpf_minor: None,
            resources: Vec::new(),
            resource_types: Vec::new(),
            resource_count: 0,
            tgi_signature: String::new(),
            technical_signature: String::new(),
            parse_error: Some(error),
        },
    }
}

fn product_for(relative: &str) -> String {
    let first = relative.split(['\\', '/']).next().unwrap_or_default();
    let upper = first.to_ascii_uppercase();
    if upper.starts_with("EP") || upper.starts_with("SP") {
        return format!("{} {}", &upper[..2], &upper[2..]);
    }
    if first.eq_ignore_ascii_case("game") || first.eq_ignore_ascii_case("gamedata") {
        return "Base Game".to_string();
    }
    if first.is_empty() {
        "Unknown".to_string()
    } else {
        first.to_string()
    }
}

fn official_reference(
    path: &Path,
    root: &Path,
    cache_index: &mut cache::FingerprintCache,
) -> OfficialArchiveReference {
    let relative = normalized_relative(root, path);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    match catalog_entry(path, cache_index) {
        Ok(entry) => {
            let resources = entry
                .resources
                .iter()
                .map(resource_from_cached)
                .collect::<Vec<_>>();
            let resource_type_counts =
                resources
                    .iter()
                    .fold(BTreeMap::new(), |mut counts, resource| {
                        *counts.entry(resource.type_id).or_insert(0) += 1;
                        counts
                    });
            OfficialArchiveReference {
                relative_path: relative.clone(),
                name,
                product: product_for(&relative),
                version: entry
                    .dbpf_major
                    .zip(entry.dbpf_minor)
                    .map(|(major, minor)| format!("{major}.{minor}")),
                file_size: entry.size,
                sha256: entry.file_sha256,
                dbpf_major: entry.dbpf_major,
                dbpf_minor: entry.dbpf_minor,
                resource_count: resources.len(),
                resources,
                resource_type_counts,
                parse_error: entry.parse_error,
            }
        }
        Err(error) => OfficialArchiveReference {
            relative_path: relative.clone(),
            name,
            product: product_for(&relative),
            version: None,
            file_size: fs::metadata(path).map(|value| value.len()).unwrap_or(0),
            sha256: String::new(),
            dbpf_major: None,
            dbpf_minor: None,
            resources: Vec::new(),
            resource_count: 0,
            resource_type_counts: BTreeMap::new(),
            parse_error: Some(error),
        },
    }
}

pub fn build_store_catalog(source: &Path) -> Result<(StoreCatalog, usize, Vec<String>)> {
    if !source.is_dir() {
        anyhow::bail!("Store source is not a directory: {}", source.display());
    }
    let mut directories = Vec::new();
    let mut files = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.with_context(|| format!("walk {}", source.display()))?;
        let path = entry.path();
        if entry.file_type().is_dir() {
            if path != source {
                directories.push(normalized_relative(source, path));
            }
        } else if entry.file_type().is_file() && is_reference_file(path, false) {
            files.push(path.to_path_buf());
        }
    }
    directories.sort_unstable();
    files.sort_unstable();
    let mut cache_index = cache::load_cache(source);
    let mut errors = Vec::new();
    let packages = files
        .iter()
        .map(|path| {
            let reference = store_reference(path, source, &mut cache_index);
            if let Some(error) = reference.parse_error.as_ref() {
                errors.push(format!("{}: {}", reference.relative_path, error));
            }
            reference
        })
        .collect::<Vec<_>>();
    let resource_count = packages.iter().map(|value| value.resource_count).sum();
    cache::save_cache(source, &cache_index).map_err(anyhow::Error::msg)?;
    Ok((
        StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            source_label: STORE_LABEL.to_string(),
            source_root: source.to_string_lossy().to_string(),
            directories,
            package_count: packages.len(),
            packages,
            resource_count,
        },
        files.len(),
        errors,
    ))
}

pub fn build_official_catalog(source: &Path) -> Result<(OfficialCatalog, usize, Vec<String>)> {
    if !source.is_dir() {
        anyhow::bail!("Official source is not a directory: {}", source.display());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.with_context(|| format!("walk {}", source.display()))?;
        if entry.file_type().is_file() && is_reference_file(entry.path(), true) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort_unstable();
    let mut cache_index = cache::load_cache(source);
    let mut errors = Vec::new();
    let archives = files
        .iter()
        .map(|path| {
            let reference = official_reference(path, source, &mut cache_index);
            if let Some(error) = reference.parse_error.as_ref() {
                errors.push(format!("{}: {}", reference.relative_path, error));
            }
            reference
        })
        .collect::<Vec<_>>();
    let resource_count = archives.iter().map(|value| value.resource_count).sum();
    let products = archives
        .iter()
        .map(|value| value.product.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    cache::save_cache(source, &cache_index).map_err(anyhow::Error::msg)?;
    Ok((
        OfficialCatalog {
            version: CATALOG_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            source_label: OFFICIAL_LABEL.to_string(),
            source_root: source.to_string_lossy().to_string(),
            archive_count: archives.len(),
            archives,
            resource_count,
            products,
        },
        files.len(),
        errors,
    ))
}

pub fn build_reference_catalogs_sync(
    store_root: &Path,
    official_root: &Path,
) -> Result<ReferenceCatalogBuildResult> {
    let (store, _, mut errors) = build_store_catalog(store_root)?;
    let (official, _, official_errors) = build_official_catalog(official_root)?;
    errors.extend(official_errors);
    write_json_atomic(&store_catalog_path(), &store)?;
    write_json_atomic(&official_catalog_path(), &official)?;
    invalidate_catalog_cache();
    Ok(ReferenceCatalogBuildResult {
        status: get_status(),
        store_catalog_path: store_catalog_path().to_string_lossy().to_string(),
        official_catalog_path: official_catalog_path().to_string_lossy().to_string(),
        store_packages: store.package_count,
        store_resources: store.resource_count,
        official_archives: official.archive_count,
        official_resources: official.resource_count,
        store_directories: store.directories.len(),
        errors,
    })
}

fn package_resources(package: &Package) -> Result<Vec<ReferenceResource>> {
    package
        .entries
        .iter()
        .map(|entry| {
            let data = package
                .data(entry)
                .with_context(|| format!("read {}", entry.key_string()))?;
            let mut digest = Sha256::new();
            digest.update(&data);
            Ok(ReferenceResource {
                type_id: entry.type_id,
                group: entry.group,
                instance: entry.instance,
                payload_sha256: format!("{:X}", digest.finalize()),
                payload_size: data.len(),
            })
        })
        .collect()
}

fn compare_package(
    package: &Package,
    store: Option<&StoreCatalog>,
    official: Option<&OfficialCatalog>,
) -> Option<ReferenceEvidence> {
    let resources = package_resources(package).ok()?;
    let (_, technical_signature) = signatures(&resources);
    let file_hash = fs::read(&package.path).ok().map(|bytes| {
        let mut digest = Sha256::new();
        digest.update(bytes);
        format!("{:X}", digest.finalize())
    });
    let mut result = ReferenceEvidence::default();

    if let Some(store) = store {
        let exact = file_hash
            .as_ref()
            .map(|hash| {
                store
                    .packages
                    .iter()
                    .filter(|value| !value.sha256.is_empty() && &value.sha256 == hash)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let technical = store
            .packages
            .iter()
            .filter(|value| {
                !value.technical_signature.is_empty()
                    && value.technical_signature == technical_signature
            })
            .collect::<Vec<_>>();
        let matches = if !exact.is_empty() {
            exact
        } else {
            technical.clone()
        };
        if !matches.is_empty() {
            let categories = matches
                .iter()
                .map(|value| value.category_parts.clone())
                .collect::<BTreeSet<_>>();
            result.store_match_kind = Some(
                if file_hash
                    .as_ref()
                    .is_some_and(|hash| matches.iter().any(|value| &value.sha256 == hash))
                {
                    "exact_package"
                } else {
                    "technical_exact"
                }
                .to_string(),
            );
            result.store_confidence = Some(
                if categories.len() == 1 {
                    "confirmed"
                } else {
                    "ambiguous"
                }
                .to_string(),
            );
            result.store_reference_paths = matches
                .iter()
                .map(|value| value.relative_path.clone())
                .collect();
            if categories.len() == 1 {
                result.store_category_parts = categories.into_iter().next().unwrap_or_default();
                result.evidence.push(
                    "Store package/resource-set identity matches a single category.".to_string(),
                );
            } else {
                result.evidence.push("Store identity matches multiple categories; no category was selected automatically.".to_string());
            }
        } else {
            let current_tgis = resources
                .iter()
                .map(ReferenceResource::key)
                .collect::<BTreeSet<_>>();
            let shared = store
                .packages
                .iter()
                .filter_map(|value| {
                    let reference_tgis = value
                        .resources
                        .iter()
                        .map(ReferenceResource::key)
                        .collect::<BTreeSet<_>>();
                    let count = current_tgis.intersection(&reference_tgis).count();
                    (count > 0).then_some(count)
                })
                .max()
                .unwrap_or(0);
            if shared > 0 {
                result.store_match_kind = Some("partial_resource_overlap".to_string());
                result.store_confidence = Some("review".to_string());
                result.shared_tgi_count = shared;
                result.evidence.push(
                    "Store overlap is partial; a shared TGI alone is not a package identity."
                        .to_string(),
                );
            }
        }
    }

    if let Some(official) = official {
        let mut official_by_tgi: HashMap<
            String,
            Vec<(&ReferenceResource, &OfficialArchiveReference)>,
        > = HashMap::new();
        for archive in &official.archives {
            for resource in &archive.resources {
                official_by_tgi
                    .entry(resource.key())
                    .or_default()
                    .push((resource, archive));
            }
        }
        let mut shared = 0;
        let mut identical = 0;
        let mut changed = 0;
        let mut unmatched = 0;
        let mut occurrences = BTreeSet::new();
        for resource in &resources {
            if let Some(candidates) = official_by_tgi.get(&resource.key()) {
                shared += 1;
                let mut same = false;
                for (candidate, archive) in candidates {
                    occurrences.insert(format!("{}|{}", archive.relative_path, resource.key()));
                    same |= candidate.payload_sha256 == resource.payload_sha256;
                }
                if same {
                    identical += 1;
                } else {
                    changed += 1;
                }
            } else {
                unmatched += 1;
            }
        }
        if shared > 0 {
            result.official_overlap_kind = Some(
                if changed > 0 && unmatched > 0 {
                    "mixed_official_and_new"
                } else if changed > 0 {
                    "replacement_candidate"
                } else if identical == shared {
                    "official_copy"
                } else {
                    "official_partial_overlap"
                }
                .to_string(),
            );
            result.official_confidence =
                Some(if changed > 0 { "evidence" } else { "confirmed" }.to_string());
            result.official_occurrences = occurrences.into_iter().collect();
            result.shared_tgi_count = result.shared_tgi_count.max(shared);
            result.identical_resource_count = identical;
            result.changed_resource_count = changed;
            result.unmatched_resource_count = unmatched;
            result.evidence.push(format!("Official resources: {shared} shared TGI(s), {identical} identical payload(s), {changed} changed payload(s), {unmatched} package resource(s) without an official occurrence."));
        }
    }

    (result.store_match_kind.is_some() || result.official_overlap_kind.is_some()).then_some(result)
}

pub fn analyze_package(package: &Package) -> Option<ReferenceEvidence> {
    let catalogs = load_catalogs();
    (catalogs.store.is_some() || catalogs.official.is_some())
        .then(|| compare_package(package, catalogs.store.as_ref(), catalogs.official.as_ref()))
        .flatten()
}

#[tauri::command]
pub fn get_reference_catalog_status() -> ReferenceCatalogStatus {
    get_status()
}

#[tauri::command]
pub fn build_reference_catalogs(
    store_root: String,
    official_root: String,
) -> Result<ReferenceCatalogBuildResult, String> {
    build_reference_catalogs_sync(
        Path::new(store_root.trim()),
        Path::new(official_root.trim()),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(path: &str, payload: &str) -> ReferenceResource {
        let mut digest = Sha256::new();
        digest.update(vec![0u8; payload.len()]);
        ReferenceResource {
            type_id: 1,
            group: 2,
            instance: path.parse().unwrap_or(3),
            payload_sha256: format!("{:X}", digest.finalize()),
            payload_size: payload.len(),
        }
    }

    fn store(
        path: &str,
        category: &[&str],
        resources: Vec<ReferenceResource>,
    ) -> StorePackageReference {
        let (tgi_signature, technical_signature) = signatures(&resources);
        StorePackageReference {
            relative_path: path.to_string(),
            category_parts: category.iter().map(|value| value.to_string()).collect(),
            name: path.to_string(),
            file_size: 1,
            sha256: String::new(),
            dbpf_major: Some(2),
            dbpf_minor: Some(0),
            resource_count: resources.len(),
            resource_types: vec![1],
            resources,
            tgi_signature,
            technical_signature,
            parse_error: None,
        }
    }

    #[test]
    fn technical_store_identity_confirms_single_category() {
        let resources = vec![reference("10", "shirt")];
        let entry = store(
            "CAS\\Roupas\\shirt.package",
            &["CAS", "Roupas"],
            resources.clone(),
        );
        let catalog = StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: STORE_LABEL.to_string(),
            source_root: String::new(),
            directories: vec![],
            package_count: 1,
            packages: vec![entry],
            resource_count: 1,
        };
        let package = package_fixture(resources);
        let evidence = compare_package(&package, Some(&catalog), None).unwrap();
        assert_eq!(
            evidence.store_match_kind.as_deref(),
            Some("technical_exact")
        );
        assert_eq!(evidence.store_confidence.as_deref(), Some("confirmed"));
    }

    #[test]
    fn same_technical_identity_in_two_categories_stays_ambiguous() {
        let resources = vec![reference("10", "shirt")];
        let catalog = StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: STORE_LABEL.to_string(),
            source_root: String::new(),
            directories: vec![],
            package_count: 2,
            packages: vec![
                store("CAS\\A\\one.package", &["CAS", "A"], resources.clone()),
                store("CAS\\B\\two.package", &["CAS", "B"], resources.clone()),
            ],
            resource_count: 2,
        };
        let package = package_fixture(resources);
        let evidence = compare_package(&package, Some(&catalog), None).unwrap();
        assert_eq!(evidence.store_confidence.as_deref(), Some("ambiguous"));
        assert!(evidence.store_category_parts.is_empty());
    }

    #[test]
    fn official_same_tgi_different_payload_is_replacement_evidence() {
        let current = reference("10", "replacement");
        let original = reference("10", "official");
        let archive = OfficialArchiveReference {
            relative_path: "EP01\\Game.package".to_string(),
            name: "Game.package".to_string(),
            product: "EP 01".to_string(),
            version: Some("2.0".to_string()),
            file_size: 1,
            sha256: String::new(),
            dbpf_major: Some(2),
            dbpf_minor: Some(0),
            resources: vec![original],
            resource_count: 1,
            resource_type_counts: BTreeMap::from([(1, 1)]),
            parse_error: None,
        };
        let catalog = OfficialCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: OFFICIAL_LABEL.to_string(),
            source_root: String::new(),
            archives: vec![archive],
            archive_count: 1,
            resource_count: 1,
            products: vec!["EP 01".to_string()],
        };
        let package = package_fixture(vec![current]);
        let evidence = compare_package(&package, None, Some(&catalog)).unwrap();
        assert_eq!(
            evidence.official_overlap_kind.as_deref(),
            Some("replacement_candidate")
        );
        assert_eq!(evidence.changed_resource_count, 1);
    }

    #[test]
    fn partial_shared_tgi_never_confirms_store_category() {
        let catalog_resources = vec![reference("10", "one"), reference("11", "two")];
        let current_resources = vec![reference("10", "different")];
        let catalog = StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: STORE_LABEL.to_string(),
            source_root: String::new(),
            directories: vec![],
            package_count: 1,
            packages: vec![store(
                "CAS\\A\\one.package",
                &["CAS", "A"],
                catalog_resources,
            )],
            resource_count: 2,
        };
        let package = package_fixture(current_resources);
        let evidence = compare_package(&package, Some(&catalog), None).unwrap();
        assert_eq!(
            evidence.store_match_kind.as_deref(),
            Some("partial_resource_overlap")
        );
        assert_eq!(evidence.store_confidence.as_deref(), Some("review"));
        assert!(evidence.store_category_parts.is_empty());
    }

    fn package_fixture(resources: Vec<ReferenceResource>) -> Package {
        let path =
            std::env::temp_dir().join(format!("s3cc-reference-{}.package", std::process::id()));
        let bytes = dbpf_fixture_bytes(&resources);
        fs::write(&path, bytes).unwrap();
        Package::load(path).unwrap()
    }

    fn dbpf_fixture_bytes(resources: &[ReferenceResource]) -> Vec<u8> {
        // Minimal uncompressed DBPF with a fixed-size index. Payload bytes are
        // the resource instance number, which is enough for these comparison
        // tests and exercises the real Package loader/data path.
        let index_offset = 96u32;
        let index_size = (4 + resources.len() * 32) as u32;
        let mut bytes = vec![0u8; index_offset as usize + index_size as usize];
        bytes[0..4].copy_from_slice(&0x4650_4244u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&(resources.len() as u32).to_le_bytes());
        bytes[40..44].copy_from_slice(&index_offset.to_le_bytes());
        bytes[44..48].copy_from_slice(&index_size.to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        bytes[64..68].copy_from_slice(&index_offset.to_le_bytes());
        bytes[index_offset as usize..index_offset as usize + 4]
            .copy_from_slice(&0u32.to_le_bytes());
        let mut payload_offset = index_offset + index_size;
        for (index, resource) in resources.iter().enumerate() {
            let offset = index_offset as usize + 4 + index * 32;
            bytes[offset..offset + 4].copy_from_slice(&resource.type_id.to_le_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&resource.group.to_le_bytes());
            bytes[offset + 8..offset + 12]
                .copy_from_slice(&((resource.instance >> 32) as u32).to_le_bytes());
            bytes[offset + 12..offset + 16]
                .copy_from_slice(&(resource.instance as u32).to_le_bytes());
            bytes[offset + 16..offset + 20].copy_from_slice(&payload_offset.to_le_bytes());
            bytes[offset + 20..offset + 24]
                .copy_from_slice(&(resource.payload_size as u32).to_le_bytes());
            bytes[offset + 24..offset + 28]
                .copy_from_slice(&(resource.payload_size as u32).to_le_bytes());
            bytes[offset + 28..offset + 32].copy_from_slice(&0u32.to_le_bytes());
            bytes.extend(std::iter::repeat_n(0u8, resource.payload_size));
            payload_offset += resource.payload_size as u32;
        }
        bytes
    }
}
