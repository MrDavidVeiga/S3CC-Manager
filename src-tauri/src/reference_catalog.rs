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
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
    time::Instant,
};
use walkdir::WalkDir;

pub const CATALOG_VERSION: u32 = 2;
const STORE_LABEL: &str = "Sims 3 Store";
const OFFICIAL_LABEL: &str = "The Sims 3 Official Resources";
const LOCAL_SOURCE_MARKER: &str = "<local-source>";

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
    pub duration_ms: u128,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceCatalogSet {
    pub store: Option<StoreCatalog>,
    pub official: Option<OfficialCatalog>,
    store_index: StoreQueryIndex,
    official_index: OfficialQueryIndex,
}

#[derive(Debug, Clone, Default)]
struct StoreQueryIndex {
    by_sha256: HashMap<String, Vec<usize>>,
    by_technical_signature: HashMap<String, Vec<usize>>,
    by_tgi: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone)]
struct OfficialOccurrence {
    archive_index: usize,
    resource_index: usize,
}

#[derive(Debug, Clone, Default)]
struct OfficialQueryIndex {
    by_tgi: HashMap<String, Vec<OfficialOccurrence>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CatalogProgress {
    source_label: String,
    source_root: String,
    discovered: usize,
    completed: usize,
    resources: usize,
    bytes_read: u64,
    cache_hits: usize,
    cache_misses: usize,
    current_relative_path: Option<String>,
    started_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OfficialInstallationMetadata {
    source_root: String,
    products: Vec<String>,
    generated_at: String,
}

fn data_root() -> PathBuf {
    if let Some(root) = std::env::var_os("S3CC_MANAGER_DATA_ROOT") {
        return PathBuf::from(root).join("Reference Catalogs");
    }
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
    data_root().join("Sims 3 Store").join("catalog-v2.json")
}

pub fn official_catalog_path() -> PathBuf {
    data_root()
        .join("Official Game Resources")
        .join("catalog-v2.json")
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("catalog path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let temporary = path.with_extension("json.tmp");
    let file =
        File::create(&temporary).with_context(|| format!("write {}", temporary.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer(&mut writer, value).context("serialize reference catalog")?;
    writer
        .flush()
        .with_context(|| format!("flush {}", temporary.display()))?;
    drop(writer);
    if path.exists() {
        let backup = path.with_extension("json.bak");
        let _ = fs::copy(path, &backup);
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("replace {}", path.display()))?;
    }
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // Some Windows security providers deny a rename in an app-data
            // directory even after the writer and destination handle are
            // closed. The fully flushed temporary file is still safe to copy;
            // retain the original backup and remove only this temporary file
            // after the destination copy succeeds.
            fs::copy(&temporary, path).with_context(|| {
                format!(
                    "replace {} after rename failed ({rename_error})",
                    path.display()
                )
            })?;
            fs::remove_file(&temporary)
                .with_context(|| format!("remove temporary {}", temporary.display()))?;
            Ok(())
        }
    }
}

fn read_catalog<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let file = File::open(path).ok()?;
    serde_json::from_reader(BufReader::new(file)).ok()
}

fn catalog_cache() -> &'static RwLock<Option<Arc<ReferenceCatalogSet>>> {
    static CACHE: OnceLock<RwLock<Option<Arc<ReferenceCatalogSet>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

pub fn invalidate_catalog_cache() {
    if let Ok(mut cache) = catalog_cache().write() {
        *cache = None;
    }
}

pub fn load_catalogs() -> Arc<ReferenceCatalogSet> {
    if let Ok(cache) = catalog_cache().read() {
        if let Some(value) = cache.as_ref() {
            return Arc::clone(value);
        }
    }
    let store = read_catalog::<StoreCatalog>(&store_catalog_path())
        .filter(|catalog| catalog.version == CATALOG_VERSION);
    let official = read_catalog::<OfficialCatalog>(&official_catalog_path())
        .filter(|catalog| catalog.version == CATALOG_VERSION);
    let value = Arc::new(ReferenceCatalogSet {
        store_index: build_store_query_index(store.as_ref()),
        official_index: build_official_query_index(official.as_ref()),
        store,
        official,
    });
    if let Ok(mut cache) = catalog_cache().write() {
        *cache = Some(Arc::clone(&value));
    }
    value
}

fn build_store_query_index(catalog: Option<&StoreCatalog>) -> StoreQueryIndex {
    let mut index = StoreQueryIndex::default();
    let Some(catalog) = catalog else { return index };
    for (package_index, package) in catalog.packages.iter().enumerate() {
        if package.parse_error.is_some() || package.resources.is_empty() {
            continue;
        }
        if !package.sha256.is_empty() && package.parse_error.is_none() {
            index
                .by_sha256
                .entry(package.sha256.clone())
                .or_default()
                .push(package_index);
        }
        if !package.technical_signature.is_empty() && package.parse_error.is_none() {
            index
                .by_technical_signature
                .entry(package.technical_signature.clone())
                .or_default()
                .push(package_index);
        }
        for resource in &package.resources {
            index
                .by_tgi
                .entry(resource.key())
                .or_default()
                .push(package_index);
        }
    }
    index
}

fn build_official_query_index(catalog: Option<&OfficialCatalog>) -> OfficialQueryIndex {
    let mut index = OfficialQueryIndex::default();
    let Some(catalog) = catalog else { return index };
    for (archive_index, archive) in catalog.archives.iter().enumerate() {
        if archive.parse_error.is_some() || archive.resources.is_empty() {
            continue;
        }
        for (resource_index, resource) in archive.resources.iter().enumerate() {
            index
                .by_tgi
                .entry(resource.key())
                .or_default()
                .push(OfficialOccurrence {
                    archive_index,
                    resource_index,
                });
        }
    }
    index
}

fn write_progress(path: &Path, progress: &CatalogProgress) {
    if let Err(error) = write_json_atomic(path, progress) {
        eprintln!("reference catalog progress write failed: {error:#}");
    }
}

fn store_progress_path() -> PathBuf {
    data_root()
        .join("Sims 3 Store")
        .join("generation-progress.json")
}

fn official_progress_path() -> PathBuf {
    data_root()
        .join("Official Game Resources")
        .join("generation-progress.json")
}

fn official_metadata_path() -> PathBuf {
    data_root()
        .join("Official Game Resources")
        .join("installation-metadata.json")
}

fn sha256_path(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 128];
    let mut size = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((format!("{:X}", hasher.finalize()), size))
}

#[cfg(test)]
fn rebuild_query_indexes(value: &mut ReferenceCatalogSet) {
    value.store_index = build_store_query_index(value.store.as_ref());
    value.official_index = build_official_query_index(value.official.as_ref());
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

fn signatures(resources: &[ReferenceResource]) -> Option<(String, String)> {
    if resources.is_empty() {
        return None;
    }
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
    Some((digest_lines(&tgi), digest_lines(&technical)))
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

fn collect_reference_files(source: &Path, official: bool) -> Result<Vec<PathBuf>> {
    if !source.is_dir() {
        anyhow::bail!("reference source is not a directory: {}", source.display());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.with_context(|| format!("walk {}", source.display()))?;
        if entry.file_type().is_file() && is_reference_file(entry.path(), official) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort_unstable();
    Ok(files)
}

fn cached_entry_matches(
    path: &Path,
    expected_sha256: &str,
    expected_resource_count: usize,
    expected_parse_error: bool,
    cache_index: &mut cache::FingerprintCache,
) -> bool {
    let Ok((cached, _)) = cache::get_or_build_analysis(path, cache_index) else {
        return false;
    };
    let same_hash =
        expected_sha256.is_empty() || cached.file_sha256.eq_ignore_ascii_case(expected_sha256);
    same_hash
        && cached.resources.len() == expected_resource_count
        && cached.parse_error.is_some() == expected_parse_error
}

fn store_catalog_matches_source(source: &Path, catalog: &StoreCatalog) -> bool {
    let Ok(files) = collect_reference_files(source, false) else {
        return false;
    };
    if files.len() != catalog.package_count || catalog.packages.len() != files.len() {
        return false;
    }
    let expected = catalog
        .packages
        .iter()
        .map(|package| (package.relative_path.clone(), package))
        .collect::<HashMap<_, _>>();
    let mut cache_index = cache::load_cache(source);
    let matches = files.iter().all(|path| {
        let relative = normalized_relative(source, path);
        let Some(package) = expected.get(&relative) else {
            return false;
        };
        cached_entry_matches(
            path,
            &package.sha256,
            package.resource_count,
            package.parse_error.is_some(),
            &mut cache_index,
        )
    });
    let _ = cache::save_cache(source, &cache_index);
    matches
}

fn official_catalog_matches_source(source: &Path, catalog: &OfficialCatalog) -> bool {
    let Ok(files) = collect_reference_files(source, true) else {
        return false;
    };
    if files.len() != catalog.archive_count || catalog.archives.len() != files.len() {
        return false;
    }
    let expected = catalog
        .archives
        .iter()
        .map(|archive| (archive.relative_path.clone(), archive))
        .collect::<HashMap<_, _>>();
    let mut cache_index = cache::load_cache(source);
    let matches = files.iter().all(|path| {
        let relative = normalized_relative(source, path);
        let Some(archive) = expected.get(&relative) else {
            return false;
        };
        cached_entry_matches(
            path,
            &archive.sha256,
            archive.resource_count,
            archive.parse_error.is_some(),
            &mut cache_index,
        )
    });
    let _ = cache::save_cache(source, &cache_index);
    matches
}

fn catalog_entry(
    path: &Path,
    cache_index: &mut cache::FingerprintCache,
) -> Result<(cache::CachedPackage, bool), String> {
    // Catalog generation is read-only. A previously completed cache entry
    // still carries the full SHA-256 and decoded resources; a bounded content
    // probe is sufficient to decide whether that entry may be reused here.
    // Mutating operations never use this path and continue to require a fresh
    // full SHA-256 before they move, quarantine, restore, or deduplicate data.
    cache::get_or_build_analysis(path, cache_index)
}

fn store_reference(
    path: &Path,
    root: &Path,
    cache_index: &mut cache::FingerprintCache,
) -> (StorePackageReference, bool) {
    let relative = normalized_relative(root, path);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    match catalog_entry(path, cache_index) {
        Ok((entry, cache_hit)) => {
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
            let (tgi_signature, technical_signature) = signatures(&resources)
                .map(|(tgi, technical)| (tgi, technical))
                .unwrap_or_default();
            (
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
                },
                cache_hit,
            )
        }
        Err(error) => (
            StorePackageReference {
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
            false,
        ),
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
) -> (OfficialArchiveReference, bool) {
    let relative = normalized_relative(root, path);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    match catalog_entry(path, cache_index) {
        Ok((entry, cache_hit)) => {
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
            (
                OfficialArchiveReference {
                    relative_path: relative.clone(),
                    name,
                    product: product_for(&relative),
                    // DBPF major/minor identify the archive format, not the game
                    // patch or product version. Keep this unknown unless reliable
                    // installation metadata supplies a real version.
                    version: None,
                    file_size: entry.size,
                    sha256: entry.file_sha256,
                    dbpf_major: entry.dbpf_major,
                    dbpf_minor: entry.dbpf_minor,
                    resource_count: resources.len(),
                    resources,
                    resource_type_counts,
                    parse_error: entry.parse_error,
                },
                cache_hit,
            )
        }
        Err(error) => (
            OfficialArchiveReference {
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
            false,
        ),
    }
}

pub fn build_store_catalog(source: &Path) -> Result<(StoreCatalog, usize, Vec<String>)> {
    let mut directories = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.with_context(|| format!("walk {}", source.display()))?;
        let path = entry.path();
        if entry.file_type().is_dir() {
            if path != source {
                directories.push(normalized_relative(source, path));
            }
        }
    }
    directories.sort_unstable();
    let files = collect_reference_files(source, false)?;
    let mut errors = Vec::new();
    let started = Utc::now().to_rfc3339();
    let progress_path = store_progress_path();
    let mut progress = CatalogProgress {
        source_label: STORE_LABEL.to_string(),
        source_root: source.to_string_lossy().to_string(),
        discovered: files.len(),
        started_at: started.clone(),
        updated_at: started,
        ..CatalogProgress::default()
    };
    write_progress(&progress_path, &progress);
    eprintln!(
        "{STORE_LABEL}: discovered {} packages; loading persistent cache",
        files.len()
    );
    let mut cache_index = cache::load_cache(source);
    eprintln!("{STORE_LABEL}: persistent cache loaded; processing packages");
    let mut packages = Vec::with_capacity(files.len());
    for (position, path) in files.iter().enumerate() {
        let (reference, cache_hit) = store_reference(path, source, &mut cache_index);
        if let Some(error) = reference.parse_error.as_ref() {
            errors.push(format!("{}: {}", reference.relative_path, error));
        }
        progress.completed = position + 1;
        progress.resources += reference.resource_count;
        progress.bytes_read += reference.file_size;
        if cache_hit {
            progress.cache_hits += 1;
        } else {
            progress.cache_misses += 1;
        }
        progress.current_relative_path = Some(reference.relative_path.clone());
        progress.updated_at = Utc::now().to_rfc3339();
        packages.push(reference);
        if progress.completed == 1
            || progress.completed % 25 == 0
            || progress.completed == files.len()
        {
            eprintln!(
                "{STORE_LABEL}: {}/{} packages, {} resources",
                progress.completed, progress.discovered, progress.resources
            );
            write_progress(&progress_path, &progress);
            if let Err(error) = cache::save_cache(source, &cache_index) {
                errors.push(format!("cache checkpoint: {error}"));
            }
        }
    }
    let resource_count = packages.iter().map(|value| value.resource_count).sum();
    if let Err(error) = cache::save_cache(source, &cache_index) {
        errors.push(format!("cache save: {error}"));
    }
    let _ = fs::remove_file(&progress_path);
    Ok((
        StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            source_label: STORE_LABEL.to_string(),
            source_root: LOCAL_SOURCE_MARKER.to_string(),
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
    let files = collect_reference_files(source, true)?;
    let mut errors = Vec::new();
    let started = Utc::now().to_rfc3339();
    let progress_path = official_progress_path();
    let mut progress = CatalogProgress {
        source_label: OFFICIAL_LABEL.to_string(),
        source_root: source.to_string_lossy().to_string(),
        discovered: files.len(),
        started_at: started.clone(),
        updated_at: started,
        ..CatalogProgress::default()
    };
    write_progress(&progress_path, &progress);
    eprintln!(
        "{OFFICIAL_LABEL}: discovered {} archives; loading persistent cache",
        files.len()
    );
    let mut cache_index = cache::load_cache(source);
    eprintln!("{OFFICIAL_LABEL}: persistent cache loaded; processing archives");
    let mut archives = Vec::with_capacity(files.len());
    for (position, path) in files.iter().enumerate() {
        let (reference, cache_hit) = official_reference(path, source, &mut cache_index);
        if let Some(error) = reference.parse_error.as_ref() {
            errors.push(format!("{}: {}", reference.relative_path, error));
        }
        progress.completed = position + 1;
        progress.resources += reference.resource_count;
        progress.bytes_read += reference.file_size;
        if cache_hit {
            progress.cache_hits += 1;
        } else {
            progress.cache_misses += 1;
        }
        progress.current_relative_path = Some(reference.relative_path.clone());
        progress.updated_at = Utc::now().to_rfc3339();
        archives.push(reference);
        if progress.completed == 1
            || progress.completed % 10 == 0
            || progress.completed == files.len()
        {
            eprintln!(
                "{OFFICIAL_LABEL}: {}/{} archives, {} resources",
                progress.completed, progress.discovered, progress.resources
            );
            write_progress(&progress_path, &progress);
            if let Err(error) = cache::save_cache(source, &cache_index) {
                errors.push(format!("cache checkpoint: {error}"));
            }
        }
    }
    let resource_count = archives.iter().map(|value| value.resource_count).sum();
    let products = archives
        .iter()
        .map(|value| value.product.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if let Err(error) = cache::save_cache(source, &cache_index) {
        errors.push(format!("cache save: {error}"));
    }
    let _ = fs::remove_file(&progress_path);
    Ok((
        OfficialCatalog {
            version: CATALOG_VERSION,
            generated_at: Utc::now().to_rfc3339(),
            source_label: OFFICIAL_LABEL.to_string(),
            source_root: LOCAL_SOURCE_MARKER.to_string(),
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
    let started = Instant::now();
    let store_path = store_catalog_path();
    let (store, mut errors) = if let Some(existing) = read_catalog::<StoreCatalog>(&store_path)
        .filter(|catalog| catalog.version == CATALOG_VERSION)
        .filter(|catalog| store_catalog_matches_source(store_root, catalog))
    {
        eprintln!(
            "{STORE_LABEL}: reusing persisted catalog {}",
            existing.package_count
        );
        (existing, Vec::new())
    } else {
        let (catalog, _, catalog_errors) = build_store_catalog(store_root)?;
        write_json_atomic(&store_path, &catalog)?;
        (catalog, catalog_errors)
    };
    materialize_store_hierarchy(&store)?;
    let official_path = official_catalog_path();
    let official = if let Some(existing) = read_catalog::<OfficialCatalog>(&official_path)
        .filter(|catalog| catalog.version == CATALOG_VERSION)
        .filter(|catalog| official_catalog_matches_source(official_root, catalog))
    {
        eprintln!(
            "{OFFICIAL_LABEL}: reusing persisted catalog {}",
            existing.archive_count
        );
        existing
    } else {
        let (catalog, _, official_errors) = build_official_catalog(official_root)?;
        errors.extend(official_errors);
        write_json_atomic(&official_path, &catalog)?;
        catalog
    };
    let official_metadata = OfficialInstallationMetadata {
        source_root: LOCAL_SOURCE_MARKER.to_string(),
        products: official.products.clone(),
        generated_at: Utc::now().to_rfc3339(),
    };
    write_json_atomic(&official_metadata_path(), &official_metadata)?;
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
        duration_ms: started.elapsed().as_millis(),
        errors,
    })
}

fn materialize_store_hierarchy(catalog: &StoreCatalog) -> Result<()> {
    let root = data_root().join(STORE_LABEL);
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    for relative in &catalog.directories {
        let path = relative
            .split(['\\', '/'])
            .filter(|part| !part.is_empty() && *part != "." && *part != "..")
            .fold(root.clone(), |base, part| base.join(part));
        fs::create_dir_all(path).with_context(|| "materialize Store directory")?;
    }
    Ok(())
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

fn compare_package(package: &Package, catalogs: &ReferenceCatalogSet) -> Option<ReferenceEvidence> {
    let resources = package_resources(package).ok()?;
    if resources.is_empty() {
        return None;
    }
    let technical_signature = signatures(&resources)?.1;
    let file_hash = sha256_path(&package.path).ok().map(|(hash, _)| hash);
    let mut result = ReferenceEvidence::default();

    if let Some(store) = catalogs.store.as_ref() {
        let mut candidate_indices = file_hash
            .as_ref()
            .and_then(|hash| catalogs.store_index.by_sha256.get(hash).cloned())
            .unwrap_or_default();
        let exact = !candidate_indices.is_empty();
        if candidate_indices.is_empty() {
            candidate_indices = catalogs
                .store_index
                .by_technical_signature
                .get(&technical_signature)
                .cloned()
                .unwrap_or_default();
        }
        if !candidate_indices.is_empty() {
            let matches = candidate_indices
                .iter()
                .filter_map(|index| store.packages.get(*index))
                .collect::<Vec<_>>();
            let categories = matches
                .iter()
                .map(|value| value.category_parts.clone())
                .collect::<BTreeSet<_>>();
            result.store_match_kind = Some(
                if exact {
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
            let mut shared_by_package: HashMap<usize, usize> = HashMap::new();
            for resource in &resources {
                for package_index in catalogs
                    .store_index
                    .by_tgi
                    .get(&resource.key())
                    .into_iter()
                    .flatten()
                {
                    *shared_by_package.entry(*package_index).or_default() += 1;
                }
            }
            if let Some(shared) = shared_by_package.values().max().copied() {
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
    }

    if let Some(official) = catalogs.official.as_ref() {
        let mut shared = 0;
        let mut identical = 0;
        let mut changed = 0;
        let mut unmatched = 0;
        let mut occurrences = BTreeSet::new();
        for resource in &resources {
            let candidates = catalogs.official_index.by_tgi.get(&resource.key());
            let Some(candidates) = candidates else {
                unmatched += 1;
                continue;
            };
            shared += 1;
            let mut same = false;
            for occurrence in candidates {
                let Some(archive) = official.archives.get(occurrence.archive_index) else {
                    continue;
                };
                let Some(candidate) = archive.resources.get(occurrence.resource_index) else {
                    continue;
                };
                occurrences.insert(format!("{}|{}", archive.relative_path, resource.key()));
                same |= candidate.payload_sha256 == resource.payload_sha256;
            }
            if same {
                identical += 1;
            } else {
                changed += 1;
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
        .then(|| compare_package(package, &catalogs))
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
        let (tgi_signature, technical_signature) = signatures(&resources).unwrap_or_default();
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

    fn catalog_set(
        store: Option<StoreCatalog>,
        official: Option<OfficialCatalog>,
    ) -> ReferenceCatalogSet {
        let mut value = ReferenceCatalogSet {
            store,
            official,
            ..ReferenceCatalogSet::default()
        };
        rebuild_query_indexes(&mut value);
        value
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
        let evidence = compare_package(&package, &catalog_set(Some(catalog), None)).unwrap();
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
        let evidence = compare_package(&package, &catalog_set(Some(catalog), None)).unwrap();
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
        let evidence = compare_package(&package, &catalog_set(None, Some(catalog))).unwrap();
        assert_eq!(
            evidence.official_overlap_kind.as_deref(),
            Some("replacement_candidate")
        );
        assert_eq!(evidence.changed_resource_count, 1);
    }

    #[test]
    fn empty_resource_sets_cannot_create_signatures_or_matches() {
        assert!(signatures(&[]).is_none());
        let catalog = StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: STORE_LABEL.to_string(),
            source_root: String::new(),
            directories: vec![],
            package_count: 1,
            packages: vec![store("empty.package", &["CAS"], vec![])],
            resource_count: 0,
        };
        assert!(build_store_query_index(Some(&catalog)).by_tgi.is_empty());
    }

    #[test]
    fn duplicate_tgi_occurrences_are_preserved_in_the_index() {
        let resources = vec![reference("10", "one"), reference("10", "two")];
        let catalog = StoreCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: STORE_LABEL.to_string(),
            source_root: String::new(),
            directories: vec![],
            package_count: 1,
            packages: vec![store("same-tgi.package", &["CAS"], resources)],
            resource_count: 2,
        };
        let occurrences = build_store_query_index(Some(&catalog))
            .by_tgi
            .get(&reference("10", "one").key())
            .cloned()
            .unwrap_or_default();
        assert_eq!(occurrences.len(), 2);
    }

    #[test]
    fn incomplete_official_archives_are_not_queryable() {
        let catalog = OfficialCatalog {
            version: CATALOG_VERSION,
            generated_at: String::new(),
            source_label: OFFICIAL_LABEL.to_string(),
            source_root: String::new(),
            archives: vec![OfficialArchiveReference {
                relative_path: "broken.package".to_string(),
                name: "broken.package".to_string(),
                product: "Unknown".to_string(),
                version: None,
                file_size: 0,
                sha256: String::new(),
                dbpf_major: None,
                dbpf_minor: None,
                resources: vec![reference("10", "one")],
                resource_count: 1,
                resource_type_counts: BTreeMap::from([(1, 1)]),
                parse_error: Some("invalid DBPF".to_string()),
            }],
            archive_count: 1,
            resource_count: 1,
            products: vec!["Unknown".to_string()],
        };
        assert!(build_official_query_index(Some(&catalog)).by_tgi.is_empty());
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
        let evidence = compare_package(&package, &catalog_set(Some(catalog), None)).unwrap();
        assert_eq!(
            evidence.store_match_kind.as_deref(),
            Some("partial_resource_overlap")
        );
        assert_eq!(evidence.store_confidence.as_deref(), Some("review"));
        assert!(evidence.store_category_parts.is_empty());
    }

    fn package_fixture(resources: Vec<ReferenceResource>) -> Package {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "s3cc-reference-{}-{}.package",
            std::process::id(),
            sequence
        ));
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
