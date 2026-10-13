use crate::{dbpf::Package, manifest::sha256_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Instant, UNIX_EPOCH},
};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheBuildMetrics {
    pub cache_hit: bool,
    pub total_ms: u128,
    pub hash_ms: u128,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResource {
    pub type_id: u32,
    pub group: u32,
    pub instance: u64,
    pub payload_sha256: String,
    pub payload_size: usize,
    pub file_size: u32,
    pub mem_size: u32,
    pub compressed: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPackage {
    pub path: String,
    pub size: u64,
    pub modified_ns: u128,
    pub file_sha256: String,
    pub dbpf_major: Option<u32>,
    pub dbpf_minor: Option<u32>,
    pub resources: Vec<CachedResource>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintCache {
    pub version: u32,
    pub entries: HashMap<String, CachedPackage>,
}

impl Default for FingerprintCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

fn metadata_signature(path: &Path) -> Result<(u64, u128), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not stat {}: {error}", path.display()))?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("Could not read modified time {}: {error}", path.display()))?;
    let modified_ns = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|_| format!("Modified time is before UNIX epoch: {}", path.display()))?
        .as_nanos();
    Ok((metadata.len(), modified_ns))
}

fn canonical_key(path: &Path) -> Result<String, String> {
    Ok(path
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", path.display()))?
        .to_string_lossy()
        .to_string())
}

fn legacy_cache_path(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("S3CC Organizer")
        .join("Cache")
        .join("fingerprints-v1.json")
}

pub fn cache_path(root: &Path) -> PathBuf {
    let base = dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| root.parent().unwrap_or(root).to_path_buf());

    base.join("Veiga's S3CC Manager")
        .join("Cache")
        .join("fingerprints-v1.json")
}

fn read_cache_file(path: &Path) -> Option<FingerprintCache> {
    let text = fs::read_to_string(path).ok()?;
    let cache = serde_json::from_str::<FingerprintCache>(&text).ok()?;
    (cache.version == CACHE_VERSION).then_some(cache)
}

pub fn load_cache(root: &Path) -> FingerprintCache {
    let path = cache_path(root);
    if let Some(cache) = read_cache_file(&path) {
        return cache;
    }

    let legacy = legacy_cache_path(root);
    let Some(cache) = read_cache_file(&legacy) else {
        return FingerprintCache::default();
    };

    // Migrate a valid legacy cache outside the game/Mods tree when possible.
    if save_cache(root, &cache).is_ok() {
        let _ = fs::remove_file(&legacy);
    }

    cache
}

pub fn save_cache(root: &Path, cache: &FingerprintCache) -> Result<(), String> {
    let path = cache_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| format!("Cache path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create cache directory {}: {error}", parent.display()))?;

    let temp = parent.join(".fingerprints-v1.json.tmp");
    let data = serde_json::to_vec(cache)
        .map_err(|error| format!("Could not serialize fingerprint cache: {error}"))?;
    fs::write(&temp, data)
        .map_err(|error| format!("Could not write cache temp {}: {error}", temp.display()))?;

    #[cfg(windows)]
    {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("Could not replace cache {}: {error}", path.display()))?;
        }
    }

    fs::rename(&temp, &path)
        .map_err(|error| format!("Could not commit cache {}: {error}", path.display()))
}

pub fn get_or_build_with_metrics(
    path: &Path,
    cache: &mut FingerprintCache,
) -> Result<(CachedPackage, CacheBuildMetrics), String> {
    let total_started = Instant::now();
    let key = canonical_key(path)?;
    let (size, modified_ns) = metadata_signature(path)?;

    // Metadata is a useful fast identity hint, but it is not sufficient to
    // prevent stale analysis when an external process restores bytes while
    // preserving size and mtime. Validate the persisted fingerprint before
    // reusing any structural result. This still avoids Package::load and
    // resource decompression on a valid cache hit.
    let hash_started = Instant::now();
    let (file_sha256, _) =
        sha256_file(path).map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    let hash_ms = hash_started.elapsed().as_millis();

    if let Some(existing) = cache.entries.get(&key) {
        if existing.size == size
            && existing.modified_ns == modified_ns
            && existing.file_sha256.eq_ignore_ascii_case(&file_sha256)
        {
            return Ok((
                existing.clone(),
                CacheBuildMetrics {
                    cache_hit: true,
                    total_ms: total_started.elapsed().as_millis(),
                    hash_ms,
                    ..CacheBuildMetrics::default()
                },
            ));
        }
    }

    let load_started = Instant::now();
    let loaded = Package::load(path);
    let dbpf_load_ms = load_started.elapsed().as_millis();

    let decode_started = Instant::now();
    let built = match loaded {
        Ok(package) => {
            let mut resources = Vec::with_capacity(package.entries.len());
            let mut parse_error = None;

            for entry in &package.entries {
                match package.data(entry) {
                    Ok(data) => resources.push(CachedResource {
                        type_id: entry.type_id,
                        group: entry.group,
                        instance: entry.instance,
                        payload_sha256: format!("{:X}", Sha256::digest(&data)),
                        payload_size: data.len(),
                        file_size: entry.file_size,
                        mem_size: entry.mem_size,
                        compressed: entry.compressed,
                    }),
                    Err(error) => {
                        parse_error = Some(format!("{}: {error}", entry.key_string()));
                        break;
                    }
                }
            }

            CachedPackage {
                path: key.clone(),
                size,
                modified_ns,
                file_sha256,
                dbpf_major: Some(package.major),
                dbpf_minor: Some(package.minor),
                resources,
                parse_error,
            }
        }
        Err(error) => CachedPackage {
            path: key.clone(),
            size,
            modified_ns,
            file_sha256,
            dbpf_major: None,
            dbpf_minor: None,
            resources: Vec::new(),
            parse_error: Some(error.to_string()),
        },
    };
    let resource_decode_ms = decode_started.elapsed().as_millis();

    cache.entries.insert(key, built.clone());
    Ok((
        built,
        CacheBuildMetrics {
            cache_hit: false,
            total_ms: total_started.elapsed().as_millis(),
            hash_ms,
            dbpf_load_ms,
            resource_decode_ms,
        },
    ))
}

pub fn get_or_build(
    path: &Path,
    cache: &mut FingerprintCache,
) -> Result<(CachedPackage, bool), String> {
    let (package, metrics) = get_or_build_with_metrics(path, cache)?;
    Ok((package, metrics.cache_hit))
}

pub fn retain_existing(cache: &mut FingerprintCache, existing_paths: &[PathBuf]) {
    let existing = existing_paths
        .iter()
        .filter_map(|path| canonical_key(path).ok())
        .collect::<HashSet<_>>();
    cache.entries.retain(|path, _| existing.contains(path));
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub path: String,
    pub entries: usize,
    pub bytes: u64,
}

#[tauri::command]
pub fn get_cache_info(folder: String) -> Result<CacheInfo, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve cache root: {error}"))?;
    let path = cache_path(&root);
    let bytes = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
    let entries = load_cache(&root).entries.len();
    Ok(CacheInfo {
        path: path.to_string_lossy().to_string(),
        entries,
        bytes,
    })
}

#[tauri::command]
pub fn clear_cache(folder: String) -> Result<bool, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve cache root: {error}"))?;

    // The learned Fix/Override model is a separate persistent evidence store.
    // Clearing transient DBPF fingerprints must never remove or invalidate it;
    // it is independently rebuilt through fix_signatures commands.
    let mut removed = false;
    for path in [cache_path(&root), legacy_cache_path(&root)] {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("Could not remove cache {}: {error}", path.display()))?;
            removed = true;
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_empty_dbpf(path: &Path, minor: u32) {
        let mut bytes = vec![0u8; 96];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&minor.to_le_bytes());
        bytes[36..40].copy_from_slice(&0u32.to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn cache_uses_manager_user_cache_location() {
        let root = Path::new(r"C:\Mods\Packages");
        let path = cache_path(root).to_string_lossy().to_string();
        assert!(path.contains("Veiga's S3CC Manager"));
        assert!(path.ends_with("fingerprints-v1.json"));
    }

    #[test]
    fn cache_hit_requires_matching_file_fingerprint() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-health-cache-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let package = root.join("empty.package");
        write_empty_dbpf(&package, 0);
        let mut cache = FingerprintCache::default();

        let (_, first) = get_or_build_with_metrics(&package, &mut cache).unwrap();
        assert!(!first.cache_hit);
        let (_, second) = get_or_build_with_metrics(&package, &mut cache).unwrap();
        assert!(second.cache_hit);
        assert_eq!(second.dbpf_load_ms, 0);
        assert_eq!(second.resource_decode_ms, 0);

        write_empty_dbpf(&package, 1);
        let (changed, third) = get_or_build_with_metrics(&package, &mut cache).unwrap();
        assert!(!third.cache_hit);
        assert_eq!(changed.dbpf_minor, Some(1));
        println!(
            "cache_benchmark first_ms={} hit_ms={} invalidated_ms={} hit_dbpf_load_ms={} hit_decode_ms={}",
            first.total_ms, second.total_ms, third.total_ms, second.dbpf_load_ms, second.resource_decode_ms
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
