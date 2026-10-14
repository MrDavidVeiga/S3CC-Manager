use crate::{dbpf::Package, manifest::sha256_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Instant, UNIX_EPOCH},
};

const CACHE_VERSION: u32 = 1;

fn cache_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// The disk cache is an optimization across application sessions.  During an
/// active unified analysis we also retain the just-built index in memory,
/// scoped to the canonical root. This keeps scanner -> duplicates -> conflicts
/// truly single-pass even when a sandbox, antivirus or transient permission
/// issue prevents writing the cache file.
fn transient_root_caches() -> &'static Mutex<HashMap<String, FingerprintCache>> {
    static CACHES: OnceLock<Mutex<HashMap<String, FingerprintCache>>> = OnceLock::new();
    CACHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn root_cache_key(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheBuildMetrics {
    pub cache_hit: bool,
    pub total_ms: u128,
    pub total_us: u128,
    pub hash_ms: u128,
    pub hash_us: u128,
    pub quick_fingerprint_ms: u128,
    pub quick_fingerprint_us: u128,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
    pub resource_decode_us: u128,
    pub integrity_verified: bool,
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
    #[serde(default)]
    pub quick_fingerprint: String,
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

fn quick_fingerprint(path: &Path, size: u64) -> Result<String, String> {
    const SAMPLE_BYTES: u64 = 16 * 1024;
    let mut file = File::open(path).map_err(|error| {
        format!(
            "Could not open {} for cache validation: {error}",
            path.display()
        )
    })?;
    let mut offsets = vec![0u64];
    if size > SAMPLE_BYTES {
        offsets.extend([size / 4, size / 2, size.saturating_sub(SAMPLE_BYTES)]);
    }
    offsets.sort_unstable();
    offsets.dedup();

    let mut digest = Sha256::new();
    digest.update(size.to_le_bytes());
    let mut buffer = vec![0u8; SAMPLE_BYTES as usize];
    for offset in offsets {
        file.seek(SeekFrom::Start(offset)).map_err(|error| {
            format!(
                "Could not seek {} for cache validation: {error}",
                path.display()
            )
        })?;
        let requested = (size.saturating_sub(offset)).min(SAMPLE_BYTES) as usize;
        let mut read = 0usize;
        while read < requested {
            let count = file.read(&mut buffer[read..requested]).map_err(|error| {
                format!(
                    "Could not read {} for cache validation: {error}",
                    path.display()
                )
            })?;
            if count == 0 {
                break;
            }
            read += count;
        }
        digest.update(offset.to_le_bytes());
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:X}", digest.finalize()))
}

/// Bounded fingerprint used by read-only analysis caches. This is deliberately
/// not an integrity proof: mutating operations must still use the full
/// SHA-256 path. It prevents an unchanged scan from reopening and decoding
/// every DBPF while still detecting ordinary external edits, including edits
/// made without changing the file size.
pub fn analysis_file_fingerprint(path: &Path) -> Result<(u64, u128, String), String> {
    let (size, modified_ns) = metadata_signature(path)?;
    let fingerprint = quick_fingerprint(path, size)?;
    Ok((size, modified_ns, fingerprint))
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

/// Sidecar for the scanner's serialized classification snapshot.  It lives
/// beside the DBPF fingerprint index, outside the user's Mods tree, so an
/// application restart can reuse a current read-only classification without
/// treating that cache as proof for a later mutation.
pub fn analysis_snapshot_path(root: &Path) -> PathBuf {
    cache_path(root).with_file_name("analysis-snapshot-v1.json")
}

fn read_cache_file(path: &Path) -> Option<FingerprintCache> {
    let text = fs::read_to_string(path).ok()?;
    let cache = serde_json::from_str::<FingerprintCache>(&text).ok()?;
    (cache.version == CACHE_VERSION).then_some(cache)
}

pub fn load_cache(root: &Path) -> FingerprintCache {
    if let Ok(caches) = transient_root_caches().lock() {
        if let Some(cache) = caches.get(&root_cache_key(root)) {
            return cache.clone();
        }
    }
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
    if let Ok(mut caches) = transient_root_caches().lock() {
        caches.insert(root_cache_key(root), cache.clone());
    }
    let _guard = cache_write_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = cache_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| format!("Cache path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Could not create cache directory {}: {error}",
            parent.display()
        )
    })?;

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
    get_or_build_with_validation(path, cache, true)
}

/// Read-only UI/health consumers can use a bounded content probe. It avoids
/// reopening and decoding an unchanged DBPF, but it is deliberately not an
/// integrity proof; protected operations continue through the full SHA-256
/// path above.
pub fn get_or_build_analysis_with_metrics(
    path: &Path,
    cache: &mut FingerprintCache,
) -> Result<(CachedPackage, CacheBuildMetrics), String> {
    get_or_build_with_validation(path, cache, false)
}

/// Stores a complete package resource index while the caller already owns a
/// decoded DBPF.  The unified scanner uses this to avoid reopening the same
/// package in the duplicate/conflict phases.  It deliberately computes the
/// full file SHA-256: this is the identity later used for exact-duplicate
/// presentation, while mutating operations still hash the source again during
/// preflight and execution.
pub fn remember_loaded_package(
    path: &Path,
    package: &Package,
    cache: &mut FingerprintCache,
) -> Result<CacheBuildMetrics, String> {
    let total_started = Instant::now();
    let key = canonical_key(path)?;
    let (size, modified_ns) = metadata_signature(path)?;

    let hash_started = Instant::now();
    let (file_sha256, _) =
        sha256_file(path).map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    let hash_ms = hash_started.elapsed().as_millis();
    let hash_us = hash_started.elapsed().as_micros();

    let quick_started = Instant::now();
    let quick_fingerprint = quick_fingerprint(path, size)?;
    let quick_fingerprint_ms = quick_started.elapsed().as_millis();
    let quick_fingerprint_us = quick_started.elapsed().as_micros();

    let decode_started = Instant::now();
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
    let resource_decode_ms = decode_started.elapsed().as_millis();
    let resource_decode_us = decode_started.elapsed().as_micros();

    cache.entries.insert(
        key.clone(),
        CachedPackage {
            path: key,
            size,
            modified_ns,
            file_sha256,
            quick_fingerprint,
            dbpf_major: Some(package.major),
            dbpf_minor: Some(package.minor),
            resources,
            parse_error,
        },
    );

    Ok(CacheBuildMetrics {
        total_ms: total_started.elapsed().as_millis(),
        total_us: total_started.elapsed().as_micros(),
        hash_ms,
        hash_us,
        quick_fingerprint_ms,
        quick_fingerprint_us,
        resource_decode_ms,
        resource_decode_us,
        integrity_verified: true,
        ..CacheBuildMetrics::default()
    })
}

fn get_or_build_with_validation(
    path: &Path,
    cache: &mut FingerprintCache,
    require_integrity: bool,
) -> Result<(CachedPackage, CacheBuildMetrics), String> {
    let total_started = Instant::now();
    let key = canonical_key(path)?;
    let (size, modified_ns) = metadata_signature(path)?;

    if let Some(existing) = cache.entries.get(&key) {
        if existing.size == size && existing.modified_ns == modified_ns {
            if require_integrity {
                let hash_started = Instant::now();
                let (file_sha256, _) = sha256_file(path)
                    .map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
                let hash_ms = hash_started.elapsed().as_millis();
                let hash_us = hash_started.elapsed().as_micros();
                if existing.file_sha256.eq_ignore_ascii_case(&file_sha256) {
                    return Ok((
                        existing.clone(),
                        CacheBuildMetrics {
                            cache_hit: true,
                            total_ms: total_started.elapsed().as_millis(),
                            total_us: total_started.elapsed().as_micros(),
                            hash_ms,
                            hash_us,
                            integrity_verified: true,
                            ..CacheBuildMetrics::default()
                        },
                    ));
                }
            } else if !existing.quick_fingerprint.is_empty() {
                let quick_started = Instant::now();
                let fingerprint = quick_fingerprint(path, size)?;
                let quick_fingerprint_ms = quick_started.elapsed().as_millis();
                let quick_fingerprint_us = quick_started.elapsed().as_micros();
                if existing.quick_fingerprint == fingerprint {
                    return Ok((
                        existing.clone(),
                        CacheBuildMetrics {
                            cache_hit: true,
                            total_ms: total_started.elapsed().as_millis(),
                            total_us: total_started.elapsed().as_micros(),
                            quick_fingerprint_ms,
                            quick_fingerprint_us,
                            integrity_verified: false,
                            ..CacheBuildMetrics::default()
                        },
                    ));
                }
            }
        }
    }

    let hash_started = Instant::now();
    let (file_sha256, _) =
        sha256_file(path).map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    let hash_ms = hash_started.elapsed().as_millis();
    let hash_us = hash_started.elapsed().as_micros();
    if let Some(existing) = cache.entries.get(&key) {
        if existing.size == size
            && existing.modified_ns == modified_ns
            && existing.file_sha256.eq_ignore_ascii_case(&file_sha256)
        {
            let mut refreshed = existing.clone();
            refreshed.quick_fingerprint = quick_fingerprint(path, size)?;
            cache.entries.insert(key, refreshed.clone());
            return Ok((
                refreshed,
                CacheBuildMetrics {
                    cache_hit: true,
                    total_ms: total_started.elapsed().as_millis(),
                    total_us: total_started.elapsed().as_micros(),
                    hash_ms,
                    hash_us,
                    integrity_verified: true,
                    ..CacheBuildMetrics::default()
                },
            ));
        }
    }

    let quick_started = Instant::now();
    let quick_fingerprint_value = quick_fingerprint(path, size)?;
    let quick_fingerprint_ms = quick_started.elapsed().as_millis();
    let quick_fingerprint_us = quick_started.elapsed().as_micros();
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
                quick_fingerprint: quick_fingerprint_value.clone(),
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
            quick_fingerprint: quick_fingerprint_value,
            dbpf_major: None,
            dbpf_minor: None,
            resources: Vec::new(),
            parse_error: Some(error.to_string()),
        },
    };
    let resource_decode_ms = decode_started.elapsed().as_millis();
    let resource_decode_us = decode_started.elapsed().as_micros();

    cache.entries.insert(key, built.clone());
    Ok((
        built,
        CacheBuildMetrics {
            cache_hit: false,
            total_ms: total_started.elapsed().as_millis(),
            total_us: total_started.elapsed().as_micros(),
            hash_ms,
            hash_us,
            quick_fingerprint_ms,
            quick_fingerprint_us,
            dbpf_load_ms,
            resource_decode_ms,
            resource_decode_us,
            integrity_verified: true,
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

pub fn get_or_build_analysis(
    path: &Path,
    cache: &mut FingerprintCache,
) -> Result<(CachedPackage, bool), String> {
    let (package, metrics) = get_or_build_analysis_with_metrics(path, cache)?;
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
    let bytes = fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
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

    let scope = cache_path(&root).to_string_lossy().to_string();
    crate::operation::run_mutating_sync("cache_write", "clearing", &[scope.as_str()], move || {
        if let Ok(mut caches) = transient_root_caches().lock() {
            caches.remove(&root_cache_key(&root));
        }
        // The learned Fix/Override model is a separate persistent evidence store.
        // Clearing transient DBPF fingerprints must never remove or invalidate it;
        // it is independently rebuilt through fix_signatures commands.
        let mut removed = false;
        for path in [cache_path(&root), legacy_cache_path(&root)] {
            if path.exists() {
                fs::remove_file(&path).map_err(|error| {
                    format!("Could not remove cache {}: {error}", path.display())
                })?;
                removed = true;
            }
        }
        Ok(removed)
    })
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
        let root =
            std::env::temp_dir().join(format!("s3cc-health-cache-{}-{unique}", std::process::id()));
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
            "cache_benchmark first_ms={} hit_ms={} invalidated_ms={} hit_hash_ms={} hit_hash_us={} hit_dbpf_load_ms={} hit_decode_ms={}",
            first.total_ms, second.total_ms, third.total_ms, second.hash_ms, second.hash_us, second.dbpf_load_ms, second.resource_decode_ms
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn analysis_probe_invalidates_same_size_change_even_when_mtime_is_reused() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "s3cc-analysis-cache-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let package = root.join("same-size.package");
        write_empty_dbpf(&package, 0);
        let mut cache = FingerprintCache::default();
        let (_, first) = get_or_build_analysis_with_metrics(&package, &mut cache).unwrap();
        assert!(!first.cache_hit);
        let (_, hit) = get_or_build_analysis_with_metrics(&package, &mut cache).unwrap();
        assert!(hit.cache_hit);
        println!(
            "analysis_cache_hit quick_fingerprint_ms={} quick_fingerprint_us={} total_us={}",
            hit.quick_fingerprint_ms, hit.quick_fingerprint_us, hit.total_us
        );

        write_empty_dbpf(&package, 1);
        let key = canonical_key(&package).unwrap();
        let current_metadata = metadata_signature(&package).unwrap();
        let entry = cache.entries.get_mut(&key).unwrap();
        entry.size = current_metadata.0;
        entry.modified_ns = current_metadata.1;

        let (changed, second) = get_or_build_analysis_with_metrics(&package, &mut cache).unwrap();
        assert!(!second.cache_hit);
        assert_eq!(changed.dbpf_minor, Some(1));
        assert!(second.integrity_verified);
        println!(
            "analysis_cache_changed hash_ms={} hash_us={} quick_fingerprint_ms={} quick_fingerprint_us={} total_ms={} total_us={}",
            second.hash_ms, second.hash_us, second.quick_fingerprint_ms, second.quick_fingerprint_us, second.total_ms, second.total_us
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
