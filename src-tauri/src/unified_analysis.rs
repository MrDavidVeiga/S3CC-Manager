//! One coordinated, read-only analysis for Organizer, Duplicates and
//! Conflicts.  The scanner remains the source of CASP/OBJD/FBLN
//! classification, while the persistent fingerprint cache is the shared
//! DBPF resource index consumed by duplicate and conflict comparison.

use crate::{
    conflicts::{analyze_conflicts_core, ConflictAnalysis},
    duplicates::{analyze_duplicates_core_from_shared_index, DuplicateAnalysis},
    i18n::AppLanguage,
    manifest::sha256_file,
    operation::{self, CANCELLED_ERROR},
    resource_cfg::find_resource_cfg,
    scanner::{current_package_signatures, scan_packages_core_quiet_progress, ScanResult},
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

    if let Some(mut cached) = current_cached(&root, language) {
        // Report the work performed by *this* request, not the historical
        // cost stored with the result.  A valid reuse still walks signatures
        // for invalidation, but it does not reopen DBPFs or rerun either
        // relationship analyzer.
        cached.stats = UnifiedAnalysisStats {
            packages: cached.scan.items.len(),
            total_ms: started.elapsed().as_millis(),
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
        let mut bytes = vec![0u8; 96];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8] = seed;
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        bytes
    }

    #[test]
    fn unified_analysis_reuses_one_current_result_for_all_consumers() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-unified-analysis-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.package"), minimal_dbpf(7)).unwrap();
        fs::write(root.join("b.package"), minimal_dbpf(7)).unwrap();

        let first = analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("first unified analysis");
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

        // A single changed package invalidates the unified result, but the
        // scanner keeps the unaffected item's classification and resource
        // index. The duplicate/conflict phases consume that mixed index
        // without reopening either DBPF.
        fs::write(root.join("b.package"), minimal_dbpf(9)).unwrap();
        let third = analyze_unified_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("incremental unified analysis");
        assert!(!third.stats.unified_cache_reused);
        assert_eq!(third.stats.package_reads, 1);
        assert_eq!(third.duplicates.stats.cache_misses, 0);
        assert_eq!(third.conflicts.stats.cache_misses, 0);

        let _ = fs::remove_dir_all(&root);
    }
}
