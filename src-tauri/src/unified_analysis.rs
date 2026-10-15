//! One coordinated, read-only analysis for Organizer, Duplicates and
//! Conflicts.  The scanner remains the source of CASP/OBJD/FBLN
//! classification, while the persistent fingerprint cache is the shared
//! DBPF resource index consumed by duplicate and conflict comparison.

use crate::{
    cache,
    conflicts::{analyze_conflicts_core, ConflictAnalysis},
    duplicates::{analyze_duplicates_core_from_shared_index, DuplicateAnalysis},
    i18n::AppLanguage,
    manifest::sha256_file,
    operation::{self, CANCELLED_ERROR},
    resource_cfg::find_resource_cfg,
    scanner::{
        current_package_signatures, reconcile_latest_scan_moves, scan_packages_core_quiet_progress,
        ScanResult,
    },
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Instant,
};

const KIND: &str = "unified_analysis";
const ANALYSIS_VERSION: &str = "unified-analysis-v20261014-1";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAnalysisStats {
    pub packages: usize,
    pub total_ms: u128,
    pub scanner_ms: u128,
    pub discovery_ms: u128,
    pub classification_ms: u128,
    pub postprocess_ms: u128,
    pub reference_catalog_ms: u128,
    pub project_grouping_ms: u128,
    pub cache_validation_ms: u128,
    pub duplicates_ms: u128,
    pub conflicts_ms: u128,
    pub package_reads: usize,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
    pub full_hash_ms: u128,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub scan_cache_reused: bool,
    pub unified_cache_reused: bool,
    pub duplicate_passes: usize,
    pub conflict_passes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAnalysis {
    pub root: String,
    pub scan: ScanResult,
    pub duplicates: DuplicateAnalysis,
    pub conflicts: ConflictAnalysis,
    pub stats: UnifiedAnalysisStats,
}

#[derive(Debug, Clone)]
struct LatestUnifiedAnalysis {
    root: PathBuf,
    language: AppLanguage,
    context: String,
    file_signatures: HashMap<String, (u64, u128, String)>,
    result: UnifiedAnalysis,
}

fn latest_slot() -> &'static Mutex<Option<LatestUnifiedAnalysis>> {
    static LATEST: OnceLock<Mutex<Option<LatestUnifiedAnalysis>>> = OnceLock::new();
    LATEST.get_or_init(|| Mutex::new(None))
}

fn stable_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn move_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace("\\\\?\\", "")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn optional_file_hash(path: Option<PathBuf>) -> String {
    match path {
        Some(path) if path.is_file() => sha256_file(&path)
            .map(|(hash, size)| format!("{size}:{hash}"))
            .unwrap_or_else(|error| format!("unreadable:{error}")),
        _ => "missing".to_string(),
    }
}

fn analysis_context(root: &Path, language: AppLanguage) -> String {
    let cfg = find_resource_cfg(root);
    format!(
        "{ANALYSIS_VERSION}|{language:?}|resource_cfg={}",
        optional_file_hash(cfg),
    )
}

fn current_cached(root: &Path, language: AppLanguage) -> Option<UnifiedAnalysis> {
    let canonical = stable_path(root);
    let cached = latest_slot().lock().ok()?.clone()?;
    if cached.root != canonical
        || cached.language != language
        || cached.context != analysis_context(&canonical, language)
        || cached.file_signatures != current_package_signatures(&canonical)
    {
        return None;
    }
    Some(cached.result)
}

pub fn invalidate_for(root: &Path) {
    let canonical = stable_path(root);
    if let Ok(mut slot) = latest_slot().lock() {
        if slot.as_ref().is_some_and(|value| value.root == canonical) {
            *slot = None;
        }
    }
}

fn reconcile_scan_paths(scan: &mut ScanResult, root: &Path, moves: &[(PathBuf, PathBuf)]) {
    let changed = moves
        .iter()
        .map(|(source, destination)| {
            (
                move_path_key(source),
                destination.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    for item in &mut scan.items {
        if let Some(destination) = changed.get(&move_path_key(Path::new(&item.path))) {
            let destination = stable_path(destination);
            let path = destination.to_string_lossy().to_string();
            item.id = path.clone();
            item.path = path;
            item.relative_path = destination
                .strip_prefix(root)
                .unwrap_or(&destination)
                .to_string_lossy()
                .replace('/', "\\");
            item.name = destination
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| item.name.clone());
        }
        for relationship in &mut item.project_relationships {
            if let Some(related_destination) = changed.get(&move_path_key(Path::new(
                &relationship.related_path,
            ))) {
                relationship.related_path = stable_path(related_destination)
                    .to_string_lossy()
                    .to_string();
            }
        }
    }
    scan.cache_hit = true;
    scan.reused_packages = scan.items.len();
    scan.stats.cache_hit = true;
    scan.stats.reused_packages = scan.items.len();
}

/// Apply executor-verified renames to the shared analysis. This path performs
/// no DBPF parsing or duplicate/conflict rediscovery: classification evidence,
/// SHA-256 values and resource relationships are unchanged by a same-volume
/// rename. It updates only presentation paths and Resource.cfg precedence for
/// existing conflict findings.
pub(crate) fn reconcile_organization_moves(
    root: &Path,
    language: AppLanguage,
    moves: &[(PathBuf, PathBuf)],
) -> Result<Option<UnifiedAnalysis>, String> {
    if moves.is_empty() {
        return Ok(None);
    }
    let root = stable_path(root);
    cache::relocate_cached_entries(&root, moves)?;
    // Keep the standalone scanner cache aligned too. The full reconciled
    // analysis below remains authoritative for the frontend response.
    let _ = reconcile_latest_scan_moves(&root, language, moves);

    let mut cached = {
        let mut slot = latest_slot()
            .lock()
            .map_err(|_| "Unified analysis cache lock is poisoned.")?;
        slot.take()
    };
    let Some(mut cached_value) = cached.take() else {
        return Ok(None);
    };
    if cached_value.root != root || cached_value.language != language {
        let mut slot = latest_slot()
            .lock()
            .map_err(|_| "Unified analysis cache lock is poisoned.")?;
        *slot = Some(cached_value);
        return Ok(None);
    }

    let reconcile_result = (|| -> Result<(), String> {
        reconcile_scan_paths(&mut cached_value.result.scan, &root, moves);
        crate::duplicates::reconcile_moved_paths(&mut cached_value.result.duplicates, &root, moves);
        crate::conflicts::reconcile_moved_paths(&mut cached_value.result.conflicts, &root, moves)?;
        cached_value.file_signatures = current_package_signatures(&root);
        cached_value.result.stats.package_reads = 0;
        cached_value.result.stats.dbpf_load_ms = 0;
        cached_value.result.stats.resource_decode_ms = 0;
        cached_value.result.stats.full_hash_ms = 0;
        cached_value.result.stats.cache_hits = cached_value.result.scan.items.len();
        cached_value.result.stats.cache_misses = 0;
        cached_value.result.stats.scan_cache_reused = true;
        cached_value.result.stats.unified_cache_reused = true;
        cached_value.result.stats.duplicate_passes = 0;
        cached_value.result.stats.conflict_passes = 0;
        Ok(())
    })();
    let mut slot = latest_slot()
        .lock()
        .map_err(|_| "Unified analysis cache lock is poisoned.")?;
    let response = cached_value.result.clone();
    *slot = Some(cached_value);
    reconcile_result?;
    Ok(Some(response))
}

pub(crate) fn analyze_unified_core(
    folder: String,
    language: AppLanguage,
    operation_kind: Option<&str>,
) -> Result<UnifiedAnalysis, String> {
    let started = Instant::now();
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve root folder: {error}"))?;
    if !root.is_dir() {
        return Err(format!("Folder does not exist: {}", root.display()));
    }

    let cache_validation_started = Instant::now();
    let current = current_cached(&root, language);
    let cache_validation_ms = cache_validation_started.elapsed().as_millis();
    if let Some(mut cached) = current {
        // Report the work performed by *this* request, not the historical
        // cost stored with the result.  A valid reuse still walks signatures
        // for invalidation, but it does not reopen DBPFs or rerun either
        // relationship analyzer.
        cached.stats = UnifiedAnalysisStats {
            packages: cached.scan.items.len(),
            total_ms: started.elapsed().as_millis(),
            cache_validation_ms,
            scan_cache_reused: true,
            unified_cache_reused: true,
            ..UnifiedAnalysisStats::default()
        };
        if let Some(kind) = operation_kind {
            operation::clear_progress_segment(kind);
            operation::set_total(kind, 1);
            operation::update(kind, 1, None, "cache_reused");
        }
        return Ok(cached);
    }

    if let Some(kind) = operation_kind {
        operation::clear_progress_segment(kind);
        operation::set_total(kind, 0);
        operation::update(kind, 0, None, "reading_packages");
    }
    // The quiet scanner retains cancellation checks, but leaves the unified
    // operation in charge of progress so later phases do not reset the bar.
    let scanner_started = Instant::now();
    let scan = scan_packages_core_quiet_progress(folder.clone(), language, operation_kind)?;
    let scanner_ms = scanner_started.elapsed().as_millis();
    if let Some(kind) = operation_kind {
        if operation::is_cancelled(kind) {
            return Err(CANCELLED_ERROR.to_string());
        }
    }

    let span = scan.items.len().max(1);
    let total = span.saturating_mul(3);
    if let Some(kind) = operation_kind {
        operation::set_total(kind, total);
        operation::update(kind, span, None, "identifying_contents");
        operation::set_progress_segment(kind, span, span, total);
    }

    let duplicates_started = Instant::now();
    let duplicates =
        analyze_duplicates_core_from_shared_index(folder.clone(), None, operation_kind)?;
    let duplicates_ms = duplicates_started.elapsed().as_millis();
    if let Some(kind) = operation_kind {
        operation::clear_progress_segment(kind);
        operation::update(kind, span.saturating_mul(2), None, "comparing_duplicates");
        if operation::is_cancelled(kind) {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::set_progress_segment(kind, span.saturating_mul(2), span, total);
    }

    let conflicts_started = Instant::now();
    let conflicts = analyze_conflicts_core(folder, None, operation_kind)?;
    let conflicts_ms = conflicts_started.elapsed().as_millis();
    if let Some(kind) = operation_kind {
        operation::clear_progress_segment(kind);
        operation::update(kind, total, None, "finalizing_results");
    }

    let stats = UnifiedAnalysisStats {
        packages: scan.items.len(),
        total_ms: started.elapsed().as_millis(),
        scanner_ms,
        discovery_ms: scan.stats.discovery_ms,
        classification_ms: scan.stats.classification_ms,
        postprocess_ms: scan.stats.postprocess_ms,
        reference_catalog_ms: scan.stats.reference_catalog_ms,
        project_grouping_ms: scan.stats.project_grouping_ms,
        cache_validation_ms,
        duplicates_ms,
        conflicts_ms,
        package_reads: scan.stats.package_reads,
        dbpf_load_ms: scan.stats.dbpf_load_ms
            + duplicates.stats.dbpf_load_ms
            + conflicts.stats.dbpf_load_ms,
        resource_decode_ms: scan.stats.resource_decode_ms
            + duplicates.stats.resource_decode_ms
            + conflicts.stats.resource_decode_ms,
        full_hash_ms: scan.stats.full_hash_ms
            + duplicates.stats.hashing_ms
            + conflicts.stats.hashing_ms,
        cache_hits: duplicates.stats.cache_hits + conflicts.stats.cache_hits,
        cache_misses: duplicates.stats.cache_misses + conflicts.stats.cache_misses,
        scan_cache_reused: scan.cache_hit,
        unified_cache_reused: false,
        duplicate_passes: 1,
        conflict_passes: 1,
    };
    let result = UnifiedAnalysis {
        root: root.to_string_lossy().to_string(),
        scan,
        duplicates,
        conflicts,
        stats,
    };
    if let Ok(mut slot) = latest_slot().lock() {
        *slot = Some(LatestUnifiedAnalysis {
            root: root.clone(),
            language,
            context: analysis_context(&root, language),
            file_signatures: current_package_signatures(&root),
            result: result.clone(),
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn analyze_unified(
    folder: String,
    language: AppLanguage,
) -> Result<UnifiedAnalysis, String> {
    operation::run_blocking(KIND, "reading_packages", move || {
        analyze_unified_core(folder, language, Some(KIND))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn minimal_dbpf(seed: u8) -> Vec<u8> {
        // One real S3SA entry keeps the fixture small while exercising the
        // shared resource index used by duplicates and conflicts.
        const DATA_OFFSET: u32 = 96;
        const INDEX_OFFSET: u32 = DATA_OFFSET + 1;
        let mut bytes = vec![0u8; DATA_OFFSET as usize];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&36u32.to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        bytes[64..68].copy_from_slice(&INDEX_OFFSET.to_le_bytes());
        bytes.push(seed);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for value in [0x073F_AA07u32, 0, 0, 0, DATA_OFFSET, 1, 1, 0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn unified_analysis_reuses_one_current_result_for_all_consumers() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("s3cc-unified-analysis-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("Resource.cfg"),
            "Priority 500\nPackedFile *.package\nPriority 600\nPackedFile Sliders/*.package\n",
        )
        .unwrap();
        fs::write(root.join("a.package"), minimal_dbpf(7)).unwrap();
        fs::write(root.join("b.package"), minimal_dbpf(7)).unwrap();

        let first = analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("first unified analysis");
        eprintln!(
            "unified_benchmark first_ms={} scanner_ms={} dbpf_ms={} hash_ms={} reads={} duplicates_ms={} conflicts_ms={}",
            first.stats.total_ms,
            first.stats.scanner_ms,
            first.stats.dbpf_load_ms,
            first.stats.full_hash_ms,
            first.stats.package_reads,
            first.stats.duplicates_ms,
            first.stats.conflicts_ms,
        );
        assert_eq!(first.scan.stats.packages, 2);
        assert_eq!(first.stats.package_reads, 2);
        assert_eq!(first.stats.duplicate_passes, 1);
        assert_eq!(first.stats.conflict_passes, 1);
        assert_eq!(first.duplicates.stats.cache_misses, 0);
        assert_eq!(first.conflicts.stats.cache_misses, 0);
        assert_eq!(first.duplicates.stats.exact_groups, 1);

        let second =
            analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
                .expect("reused unified analysis");
        eprintln!(
            "unified_benchmark second_ms={} cache_validation_ms={} reads={} unified_cache_reused={}",
            second.stats.total_ms,
            second.stats.cache_validation_ms,
            second.stats.package_reads,
            second.stats.unified_cache_reused,
        );
        assert!(second.stats.unified_cache_reused);
        assert_eq!(second.stats.duplicate_passes, 0);
        assert_eq!(second.stats.conflict_passes, 0);
        assert_eq!(second.stats.package_reads, 0);
        assert_eq!(
            second.duplicates.groups.len(),
            first.duplicates.groups.len()
        );
        assert_eq!(
            second.conflicts.findings.len(),
            first.conflicts.findings.len()
        );

        // A verified same-volume rename updates every shared view in place:
        // no DBPF is reopened, duplicate members keep their identity, and a
        // subsequent Analyze request is a cache reuse rather than a rescan.
        let moved = root.join("Sliders").join("a.package");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::rename(root.join("a.package"), &moved).unwrap();
        let reconciled = reconcile_organization_moves(
            &root,
            AppLanguage::Pt,
            &[(root.join("a.package"), moved.clone())],
        )
        .expect("reconcile verified rename")
        .expect("reconciled unified result");
        assert!(reconciled
            .scan
            .items
            .iter()
            .any(|item| item.relative_path == "Sliders\\a.package"));
        assert!(reconciled.duplicates.groups.iter().any(|group| {
            group
                .members
                .iter()
                .any(|member| member.relative_path == "Sliders\\a.package")
        }));
        assert!(!reconciled.conflicts.findings.is_empty());
        assert!(reconciled.conflicts.findings.iter().all(|finding| {
            // Every derived package reference in the returned result is the
            // post-rename reference the frontend must render.
            !finding.left.relative_path.eq_ignore_ascii_case("a.package")
                && !finding.right.relative_path.eq_ignore_ascii_case("a.package")
        }));
        assert!(reconciled.conflicts.findings.iter().any(|finding| {
            [&finding.left, &finding.right].into_iter().any(|package| {
                package.relative_path == "Sliders\\a.package"
                    && package.load_priority == Some(600)
            })
        }));
        let after_move =
            analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
                .expect("reused unified analysis after move");
        assert!(after_move.stats.unified_cache_reused);
        assert_eq!(after_move.stats.package_reads, 0);
        assert!(after_move
            .scan
            .items
            .iter()
            .any(|item| item.relative_path == "Sliders\\a.package"));
        assert!(after_move.duplicates.groups.iter().any(|group| {
            group
                .members
                .iter()
                .any(|member| member.relative_path == "Sliders\\a.package")
        }));

        // A single changed package invalidates the unified result, but the
        // scanner keeps the unaffected item's classification and resource
        // index. The duplicate/conflict phases consume that mixed index
        // without reopening either DBPF.
        fs::write(root.join("b.package"), minimal_dbpf(9)).unwrap();
        let third = analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("incremental unified analysis");
        eprintln!(
            "unified_benchmark changed_ms={} scanner_ms={} reads={} reused_packages={}",
            third.stats.total_ms,
            third.stats.scanner_ms,
            third.stats.package_reads,
            third.scan.stats.reused_packages,
        );
        assert!(!third.stats.unified_cache_reused);
        assert_eq!(third.stats.package_reads, 1);
        assert_eq!(third.duplicates.stats.cache_misses, 0);
        assert_eq!(third.conflicts.stats.cache_misses, 0);

        let _ = fs::remove_dir_all(&root);
    }
}
