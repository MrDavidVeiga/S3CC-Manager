use crate::{dbpf::Package, manifest::sha256_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
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

fn move_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace("\\\\?\\", "")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheBuildMetrics {
    pub cache_hit: bool,
    pub total_ms: u128,
    pub total_us: u128,
    pub hash_ms: u128,
    pub hash_us: u128,
    pub hash_exclusive_us: u128,
    pub hash_wait_us: u128,
    pub quick_fingerprint_ms: u128,
    pub quick_fingerprint_us: u128,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
    pub resource_decode_us: u128,
    pub resource_decode_exclusive_us: u128,
    pub resource_decode_wait_us: u128,
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

/// Return a complete read-only index entry when its bounded on-disk
/// fingerprint still matches. This is intentionally not an integrity proof;
/// write paths continue to recalculate the full SHA-256 before acting.
pub fn current_cached_entry(
    path: &Path,
    cache: &FingerprintCache,
) -> Result<Option<CachedPackage>, String> {
    let key = canonical_key(path)?;
    let Some(existing) = cache.entries.get(&key) else {
        return Ok(None);
    };
    let (size, modified_ns, quick) = analysis_file_fingerprint(path)?;
    if existing.size == size
        && existing.modified_ns == modified_ns
        && existing.quick_fingerprint.eq_ignore_ascii_case(&quick)
    {
        Ok(Some(existing.clone()))
    } else {
        Ok(None)
    }
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
    let base = std::env::var_os("S3CC_MANAGER_CACHE_ROOT")
        .map(PathBuf::from)
        .or_else(|| dirs::cache_dir())
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

    match fs::rename(&temp, &path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // A Windows security provider can deny the rename even though
            // the cache bytes were fully written and all handles are closed.
            // Preserve the same destination semantics with a flushed copy;
            // callers still treat the full cache as an optimization only.
            fs::copy(&temp, &path).map_err(|error| {
                format!(
                    "Could not commit cache {} after rename failed ({rename_error}): {error}",
                    path.display()
                )
            })?;
            fs::remove_file(&temp).map_err(|error| {
                format!(
                    "Could not remove cache temp {} after commit: {error}",
                    temp.display()
                )
            })
        }
    }
}

/// Rename cache keys after a verified same-volume file move. The resource
/// index remains valid because the executor has already verified the source
/// SHA-256 and `rename` does not alter package bytes. This is deliberately not
/// an integrity shortcut for future writes: executor preflight still hashes
/// every source in a new transaction.
pub fn relocate_cached_entries(root: &Path, moves: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    if moves.is_empty() {
        return Ok(());
    }
    let mut cache = load_cache(root);
    for (source, destination) in moves {
        let source_key = move_path_key(source);
        let matching_key = cache
            .entries
            .keys()
            .find(|key| move_path_key(Path::new(key)) == source_key)
            .cloned();
        let Some(key) = matching_key else {
            // Auxiliary files and pre-existing cache misses have no DBPF
            // entry to relocate. They do not invalidate other packages.
            continue;
        };
        let mut cached = cache.entries.remove(&key).expect("cache key exists");
        let destination_key = canonical_key(destination)?;
        if cache.entries.contains_key(&destination_key) {
            return Err(format!(
                "Cache destination collision after verified move: {}",
                destination.display()
            ));
        }
        let (size, modified_ns) = metadata_signature(destination)?;
        cached.path = destination_key.clone();
        cached.size = size;
        cached.modified_ns = modified_ns;
        // The bounded probe is refreshed from the moved file. It is not a
        // replacement for SHA-256 when a later transaction is authorized.
        cached.quick_fingerprint = quick_fingerprint(destination, size)?;
        cache.entries.insert(destination_key, cached);
    }
    // The in-memory root cache is updated before the persistent write. A
    // locked cache file must not invalidate an otherwise verified organizer
    // transaction or force a DBPF rescan; the next successful cache write
    // will persist the same entries.
    let _ = save_cache(root, &cache);
    Ok(())
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
    remember_loaded_package_with_result(path, package, cache).map(|(_, metrics)| metrics)
}

/// Index a DBPF that the scanner has already opened and return the resulting
/// entry so later unified-analysis phases can query the same fingerprints
/// without reopening or rehashing the package.
pub fn remember_loaded_package_with_result(
    path: &Path,
    package: &Package,
    cache: &mut FingerprintCache,
) -> Result<(CachedPackage, CacheBuildMetrics), String> {
    let total_started = Instant::now();
    let key = canonical_key(path)?;
    let (size, modified_ns) = metadata_signature(path)?;

    if let Some(existing) = cache.entries.get(&key) {
        let quick_started = Instant::now();
        let quick = quick_fingerprint(path, size)?;
        let quick_us = quick_started.elapsed().as_micros();
        if existing.size == size
            && existing.modified_ns == modified_ns
            && existing.quick_fingerprint.eq_ignore_ascii_case(&quick)
        {
            return Ok((
                existing.clone(),
                CacheBuildMetrics {
                    cache_hit: true,
                    total_ms: total_started.elapsed().as_millis(),
                    total_us: total_started.elapsed().as_micros(),
                    quick_fingerprint_ms: quick_us / 1_000,
                    quick_fingerprint_us: quick_us,
                    integrity_verified: false,
                    ..CacheBuildMetrics::default()
                },
            ));
        }
    }

    let hash_started = Instant::now();
    let hash_path = path.to_path_buf();
    let hash_worker = thread::spawn(move || {
        let exclusive_started = Instant::now();
        let result = sha256_file(&hash_path);
        (result, exclusive_started.elapsed().as_micros())
    });

    let quick_started = Instant::now();
    let quick_fingerprint = quick_fingerprint(path, size)?;
    let quick_fingerprint_ms = quick_started.elapsed().as_millis();
    let quick_fingerprint_us = quick_started.elapsed().as_micros();

    let decode_started = Instant::now();
    let decode_result = decode_resource_fingerprints(package);
    let (hash_result, hash_exclusive_us) = hash_worker
        .join()
        .map_err(|_| "file hash worker panicked".to_string())?;
    let (file_sha256, _) = hash_result
        .map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    let (resources, parse_error, resource_decode_exclusive_us) = decode_result?;
    let hash_ms = hash_started.elapsed().as_millis();
    let hash_us = hash_started.elapsed().as_micros();
    let resource_decode_ms = decode_started.elapsed().as_millis();
    let resource_decode_us = decode_started.elapsed().as_micros();

    let cached = CachedPackage {
        path: key,
        size,
        modified_ns,
        file_sha256,
        quick_fingerprint,
        dbpf_major: Some(package.major),
        dbpf_minor: Some(package.minor),
        resources,
        parse_error,
    };
    cache.entries.insert(cached.path.clone(), cached.clone());

    Ok((
        cached,
        CacheBuildMetrics {
            total_ms: total_started.elapsed().as_millis(),
            total_us: total_started.elapsed().as_micros(),
            hash_ms,
            hash_us,
            hash_exclusive_us,
            hash_wait_us: hash_us.saturating_sub(hash_exclusive_us),
            quick_fingerprint_ms,
            quick_fingerprint_us,
            resource_decode_ms,
            resource_decode_us,
            resource_decode_exclusive_us,
            resource_decode_wait_us: resource_decode_us
                .saturating_sub(resource_decode_exclusive_us),
            integrity_verified: true,
            ..CacheBuildMetrics::default()
        },
    ))
}

/// Decode and fingerprint the resources of a package with bounded parallelism.
///
/// Resource payload hashing is required by the duplicate/conflict index, but
/// the old implementation decoded every resource sequentially. Large merged
/// packages can contain hundreds of compressed resources, making that loop the
/// dominant cold-scan cost. Workers use independent read handles and return
/// results by original index, so the serialized cache and all consumers retain
/// exactly the same ordering and hashes. A small bound avoids turning a large
/// scan into an unbounded thread or file-handle fan-out.
fn decode_resource_fingerprints(
    package: &Package,
) -> Result<(Vec<CachedResource>, Option<String>, u128), String> {
    if package.entries.is_empty() {
        return Ok((Vec::new(), None, 0));
    }

    let worker_limit = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2)
        .clamp(1, 2);
    let worker_count = worker_limit.min(package.entries.len());
    let chunk_size = package.entries.len().div_ceil(worker_count);
    let mut slots = vec![None; package.entries.len()];
    let mut first_error = None;
    let mut exclusive_us = 0u128;

    thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for (chunk_index, entries) in package.entries.chunks(chunk_size).enumerate() {
            workers.push(
                scope.spawn(move || -> Result<(Vec<(usize, CachedResource)>, u128), String> {
                    let worker_started = Instant::now();
                    let mut data_file = File::open(&package.path).map_err(|error| {
                        format!("Could not reopen {}: {error}", package.path.display())
                    })?;
                    let start = chunk_index * chunk_size;
                    let mut results = Vec::with_capacity(entries.len());
                    for (offset, entry) in entries.iter().enumerate() {
                        let data = package
                            .data_from_file(&mut data_file, entry)
                            .map_err(|error| format!("{}: {error}", entry.key_string()))?;
                        results.push((
                            start + offset,
                            CachedResource {
                                type_id: entry.type_id,
                                group: entry.group,
                                instance: entry.instance,
                                payload_sha256: format!("{:X}", Sha256::digest(&data)),
                                payload_size: data.len(),
                                file_size: entry.file_size,
                                mem_size: entry.mem_size,
                                compressed: entry.compressed,
                            },
                        ));
                    }
                    Ok((results, worker_started.elapsed().as_micros()))
                }),
            );
        }

        for worker in workers {
            match worker.join() {
                Ok(Ok((results, worker_us))) => {
                    exclusive_us += worker_us;
                    for (index, resource) in results {
                        slots[index] = Some(resource);
                    }
                }
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(_) => {
                    if first_error.is_none() {
                        first_error = Some("resource fingerprint worker panicked".to_string());
                    }
                }
            }
        }
    });

    if let Some(error) = first_error {
        return Ok((Vec::new(), Some(error), exclusive_us));
    }
    Ok((
        slots.into_iter().flatten().collect::<Vec<CachedResource>>(),
        None,
        exclusive_us,
    ))
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
            let mut data_file = File::open(&package.path).ok();

            for (entry_index, entry) in package.entries.iter().enumerate() {
                if entry_index % 1000 == 0 && package.entries.len() >= 1000 {
                    eprintln!(
                        "reference cache: {} resource {}/{}",
                        package.path.display(),
                        entry_index + 1,
                        package.entries.len()
                    );
                }
                let result = data_file
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("could not reopen package data"))
                    .and_then(|file| package.data_from_file(file, entry));
                match result {
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
            hash_exclusive_us: hash_us,
            hash_wait_us: 0,
            quick_fingerprint_ms,
            quick_fingerprint_us,
            dbpf_load_ms,
            resource_decode_ms,
            resource_decode_us,
            resource_decode_exclusive_us: resource_decode_us,
            resource_decode_wait_us: 0,
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
