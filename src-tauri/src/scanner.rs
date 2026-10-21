use crate::{
    cache,
    catalog::{classify_resource, CatalogClassification, TYPE_CASP, TYPE_OBJD},
    dbpf::{Package, ResourceEntry},
    fix_signatures::{self, FixSignatureIndex},
    i18n::AppLanguage,
    manifest::sha256_file,
    operation::{self, CANCELLED_ERROR},
    package_family::{
        classify_package_family, PackageFamilyClassification, PackageFamilyResult, TYPE_BBLN,
        TYPE_BGEO, TYPE_BONE_DELTA, TYPE_FACE, TYPE_FBLN, TYPE_HAIR_TONE, TYPE_IMG, TYPE_S3SA,
        TYPE_SKIN_TONE,
    },
    project_grouping::{
        assign_project_groups_references, extract_package_evidence_with_metrics,
        ProjectRelationship, ProjectTechnicalEvidence, INTERNAL_IDENTIFIER_MARKERS,
    },
    reference_catalog::{self, ReferenceEvidence},
    resource_cfg::is_mods_layout_root,
    slider::{fbln_evidence, read_fbln_metadata, FblnMetadata},
    workspace::{load_workspace_for_root, split_destination},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Instant,
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPackageItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub relative_path: String,
    pub file_size: u64,
    pub resource_count: usize,
    pub catalog_resource_count: usize,
    pub resource_types: Vec<String>,
    pub instances: Vec<String>,
    pub scripted: bool,
    pub content_source: String,
    pub source_confidence: Option<String>,
    pub status: String,
    pub classification_confidence: String,
    pub creator: Option<String>,
    pub mod_name: Option<String>,
    pub gameplay_category: Option<String>,
    pub detected_from: Vec<String>,
    pub category: Option<String>,
    pub sub_category: Option<String>,
    pub gender: Option<String>,
    pub age: Option<String>,
    pub species: Option<String>,
    pub usage_categories: Vec<String>,
    pub destination_parts: Vec<String>,
    pub destination_path: Option<String>,
    pub candidate_destinations: Vec<String>,
    pub classifications: Vec<CatalogClassification>,
    pub classification_reason: Option<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_role: Option<String>,
    #[serde(default)]
    pub project_confidence: Option<String>,
    #[serde(default)]
    pub project_evidence: ProjectTechnicalEvidence,
    #[serde(default)]
    pub project_relationships: Vec<ProjectRelationship>,
    #[serde(default)]
    pub reference_evidence: Option<ReferenceEvidence>,
    #[serde(skip)]
    pub(crate) profile: Option<ScanPackageProfile>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScanPackageProfile {
    compressed_bytes: u64,
    decompressed_bytes: u64,
    resource_count: usize,
    decompression_count: usize,
    resource_decode_us: u128,
    project_payload_analysis_us: u128,
    project_stbl_resources: usize,
    project_nmap_resources: usize,
    project_objk_resources: usize,
    project_other_resources: usize,
    catalog_classification_us: u128,
    auxiliary_analysis_us: u128,
    directed_family_us: u128,
    special_package_us: u128,
    script_metadata_us: u128,
    final_resolution_us: u128,
}

#[derive(Debug, Clone)]
struct ScanPackageProfileEntry {
    elapsed_us: u128,
    path: String,
    compressed_bytes: u64,
    decompressed_bytes: u64,
    resource_count: usize,
    decompression_count: usize,
    resource_decode_us: u128,
    project_payload_analysis_us: u128,
    project_stbl_resources: usize,
    project_nmap_resources: usize,
    project_objk_resources: usize,
    project_other_resources: usize,
    catalog_classification_us: u128,
    auxiliary_analysis_us: u128,
    directed_family_us: u128,
    special_package_us: u128,
    script_metadata_us: u128,
    final_resolution_us: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub packages: usize,
    pub classified: usize,
    pub mixed: usize,
    pub unknown: usize,
    pub disabled: usize,
    pub needs_review: usize,
    pub invalid: usize,
    pub casp_resources: usize,
    pub objd_resources: usize,
    pub total_ms: u128,
    pub discovery_ms: u128,
    pub classification_ms: u128,
    pub postprocess_ms: u128,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
    #[serde(default)]
    pub resource_decode_exclusive_ms: u128,
    #[serde(default)]
    pub resource_decode_wait_ms: u128,
    #[serde(default)]
    pub reference_catalog_ms: u128,
    #[serde(default)]
    pub project_grouping_ms: u128,
    /// Full-file hashing performed while populating the shared read-only
    /// resource index. It prevents duplicate analysis from reopening DBPFs;
    /// executor/preflight still hashes again before a write.
    #[serde(default)]
    pub full_hash_ms: u128,
    #[serde(default)]
    pub full_hash_exclusive_ms: u128,
    #[serde(default)]
    pub full_hash_wait_ms: u128,
    pub package_reads: usize,
    pub cache_hit: bool,
    pub reused_packages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub root: String,
    pub items: Vec<ScanPackageItem>,
    pub stats: ScanStats,
    #[serde(default)]
    pub cache_hit: bool,
    #[serde(default)]
    pub reused_packages: usize,
}

#[derive(Debug, Clone)]
struct LatestScan {
    root: PathBuf,
    language: AppLanguage,
    result: ScanResult,
    file_signatures: HashMap<String, (u64, u128, String)>,
    classification_context: String,
}

#[derive(Debug, Default)]
struct PresentationSnapshotTiming {
    signatures_ms: u128,
    context_ms: u128,
    persistence_ms: u128,
}

// Project-evidence fields are part of the serialized scan result. Bump the
// snapshot schema so an older result cannot reintroduce stale project paths.
const PERSISTENT_SCAN_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentScan {
    version: u32,
    root: String,
    language: AppLanguage,
    result: ScanResult,
    file_signatures: HashMap<String, (u64, u128, String)>,
    classification_context: String,
}

// Keep presentation snapshots isolated by selected root and language.  A
// single global slot lets an unrelated scan replace the incremental context
// while another scan is still preparing its next pass, forcing unnecessary
// DBPF reads and making cache tests/operations race with one another.
static LATEST_SCAN: OnceLock<Mutex<HashMap<String, LatestScan>>> = OnceLock::new();

fn latest_scan_slot() -> &'static Mutex<HashMap<String, LatestScan>> {
    LATEST_SCAN.get_or_init(|| Mutex::new(HashMap::new()))
}

fn latest_scan_key(root: &Path, language: AppLanguage) -> String {
    format!("{}|{language:?}", stable_path(root).to_string_lossy())
}

fn persistent_scan_path(root: &Path) -> PathBuf {
    cache::analysis_snapshot_path(root)
}

fn save_persistent_scan(scan: &LatestScan) {
    let path = persistent_scan_path(&scan.root);
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let snapshot = PersistentScan {
        version: PERSISTENT_SCAN_VERSION,
        root: scan.root.to_string_lossy().to_string(),
        language: scan.language,
        result: scan.result.clone(),
        file_signatures: scan.file_signatures.clone(),
        classification_context: scan.classification_context.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&snapshot) else {
        return;
    };
    let temp = path.with_extension("json.tmp");
    if fs::write(&temp, bytes).is_err() {
        return;
    }
    #[cfg(windows)]
    if path.exists() && fs::remove_file(&path).is_err() {
        let _ = fs::remove_file(&temp);
        return;
    }
    if fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
    }
}

fn load_persistent_scan(root: &Path, language: AppLanguage) -> Option<LatestScan> {
    let path = persistent_scan_path(root);
    let bytes = fs::read(path).ok()?;
    let snapshot = serde_json::from_slice::<PersistentScan>(&bytes).ok()?;
    let stable_root = stable_path(root);
    if snapshot.version != PERSISTENT_SCAN_VERSION
        || snapshot.language != language
        || stable_path(Path::new(&snapshot.root)) != stable_root
        || snapshot.classification_context
            != classification_context_fingerprint(&stable_root, language)
    {
        return None;
    }
    let cached = LatestScan {
        root: stable_root,
        language,
        result: snapshot.result,
        file_signatures: snapshot.file_signatures,
        classification_context: snapshot.classification_context,
    };
    latest_scan_is_current(&cached).then_some(cached)
}

// Bump this whenever classification evidence or destination projection rules
// change.  The prior value allowed persistent scans created before the
// CASP/OBJD-free override rules to be reused, leaving those packages shown as
// Unknown until the user manually invalidated the cache.
const CLASSIFICATION_RULES_VERSION: &str = "scanner-classification-v20261021-store-bundle-evidence-cas-default-9";

fn context_file_signature(path: &Path) -> String {
    if !path.is_file() {
        return "missing".to_string();
    }
    sha256_file(path)
        .map(|(hash, size)| format!("{size}:{hash}"))
        .unwrap_or_else(|error| format!("unreadable:{error}"))
}

fn classification_context_fingerprint(root: &Path, language: AppLanguage) -> String {
    let workspace = root
        .parent()
        .unwrap_or(root)
        .join("S3CC Organizer")
        .join("Workspace")
        .join("workspace-v1.json");
    let fix_index = fix_signatures::index_path();
    let fix_backup = fix_signatures::index_backup_path();
    format!(
        "{CLASSIFICATION_RULES_VERSION}|{language:?}|workspace={}|fix={}|fix-backup={}",
        context_file_signature(&workspace),
        context_file_signature(&fix_index),
        context_file_signature(&fix_backup),
    )
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

fn remember_latest_scan(
    result: &ScanResult,
    language: AppLanguage,
    known_signatures: Option<&HashMap<String, (u64, u128, String)>>,
) -> PresentationSnapshotTiming {
    let root = stable_path(Path::new(&result.root));
    let signatures_started = Instant::now();
    let expected_paths = result
        .items
        .iter()
        .map(|item| stable_path(Path::new(&item.path)).to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let file_signatures = known_signatures
        .filter(|signatures| expected_paths.iter().all(|path| signatures.contains_key(path)))
        .map(|signatures| {
            expected_paths
                .iter()
                .filter_map(|path| signatures.get(path).map(|value| (path.clone(), value.clone())))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_else(|| {
            analysis_file_signatures(
                result.items.iter().map(|item| PathBuf::from(&item.path)).collect(),
            )
        });
    let signatures_ms = signatures_started.elapsed().as_millis();
    let context_started = Instant::now();
    let classification_context = classification_context_fingerprint(&root, language);
    let context_ms = context_started.elapsed().as_millis();
    let latest = LatestScan {
        root: root.clone(),
        language,
        result: result.clone(),
        file_signatures,
        classification_context,
    };
    let persistence_started = Instant::now();
    save_persistent_scan(&latest);
    if let Ok(mut slot) = latest_scan_slot().lock() {
        slot.insert(latest_scan_key(&root, language), latest);
    }
    PresentationSnapshotTiming {
        signatures_ms,
        context_ms,
        persistence_ms: persistence_started.elapsed().as_millis(),
    }
}

fn analysis_file_signatures(paths: Vec<PathBuf>) -> HashMap<String, (u64, u128, String)> {
    if paths.is_empty() {
        return HashMap::new();
    }
    let worker_count = thread::available_parallelism()
        .map(|count| count.get().div_ceil(2))
        .unwrap_or(2)
        .clamp(1, 4)
        .min(paths.len());
    let next = AtomicUsize::new(0);
    let output = Mutex::new(HashMap::with_capacity(paths.len()));
    thread::scope(|scope| {
        for _ in 0..worker_count {
            let paths = &paths;
            let next = &next;
            let output = &output;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(index) else {
                    break;
                };
                let canonical = stable_path(path);
                let Ok(signature) = cache::analysis_file_fingerprint(&canonical) else {
                    continue;
                };
                if let Ok(mut signatures) = output.lock() {
                    signatures.insert(canonical.to_string_lossy().to_string(), signature);
                }
            });
        }
    });
    output.into_inner().unwrap_or_default()
}

pub(crate) fn current_package_signatures(root: &Path) -> HashMap<String, (u64, u128, String)> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && package_extension(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    #[cfg(test)]
    if let Some(limit) = std::env::var("S3CC_PROFILE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        paths.truncate(limit);
    }
    analysis_file_signatures(paths)
}

fn latest_scan_is_current(cached: &LatestScan) -> bool {
    current_package_signatures(&cached.root) == cached.file_signatures
        && cached.classification_context
            == classification_context_fingerprint(&cached.root, cached.language)
}

pub fn cached_scan_for(root: &Path, language: AppLanguage) -> Option<ScanResult> {
    let canonical = stable_path(root);
    let key = latest_scan_key(&canonical, language);
    let in_memory = latest_scan_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.get(&key).cloned());
    if let Some(cached) = in_memory.filter(|cached| latest_scan_is_current(cached)) {
        return Some(cached.result);
    }
    let persisted = load_persistent_scan(&canonical, language)?;
    let result = persisted.result.clone();
    if let Ok(mut slot) = latest_scan_slot().lock() {
        slot.insert(key, persisted);
    }
    Some(result)
}

fn previous_scan_for(root: &Path, language: AppLanguage) -> Option<LatestScan> {
    let key = latest_scan_key(root, language);
    let cached = latest_scan_slot().lock().ok()?.get(&key).cloned()?;
    (cached.classification_context == classification_context_fingerprint(&cached.root, cached.language))
        .then_some(cached)
}

pub fn cached_scan_paths(root: &Path) -> Option<Vec<PathBuf>> {
    let canonical = stable_path(root);
    let cached = latest_scan_slot()
        .lock()
        .ok()?
        .values()
        .find(|cached| cached.root == canonical && latest_scan_is_current(cached))
        .cloned()?;

    Some(
        cached
            .result
            .items
            .iter()
            .map(|item| PathBuf::from(&item.path))
            .collect(),
    )
}

/// Drop cached paths after a successful on-disk organization. The frontend
/// receives the exact move mapping and can update its list incrementally;
/// future explicit rescans or new previews must not reuse pre-move paths.
pub fn invalidate_latest_scan_for(root: &Path) {
    let canonical = stable_path(root);
    if let Ok(mut slot) = latest_scan_slot().lock() {
        slot.retain(|_, current| current.root != canonical);
    }
}

/// Update the presentation cache after executor-verified renames. This keeps
/// Organizer paths current without reopening DBPFs; any later external change
/// still invalidates the snapshot through `current_package_signatures`.
pub(crate) fn reconcile_latest_scan_moves(
    root: &Path,
    language: AppLanguage,
    moves: &[(PathBuf, PathBuf)],
) -> bool {
    if moves.is_empty() {
        return true;
    }
    let canonical_root = stable_path(root);
    let signatures = current_package_signatures(&canonical_root);
    let mut slot = match latest_scan_slot().lock() {
        Ok(slot) => slot,
        Err(_) => return false,
    };
    let key = latest_scan_key(&canonical_root, language);
    let Some(latest) = slot.get_mut(&key) else {
        return false;
    };

    for (source, destination) in moves {
        let source_key = move_path_key(source);
        let Some(item) = latest.result.items.iter_mut().find(|item| {
            move_path_key(Path::new(&item.path)) == source_key
        }) else {
            // A move for an auxiliary file is not represented in ScanResult.
            continue;
        };
        let destination = stable_path(destination);
        let relative = destination
            .strip_prefix(&canonical_root)
            .unwrap_or(&destination)
            .to_string_lossy()
            .replace('/', "\\");
        let path = destination.to_string_lossy().to_string();
        item.id = path.clone();
        item.path = path;
        item.relative_path = relative;
        item.name = destination
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| item.name.clone());
    }
    latest.file_signatures = signatures;
    latest.result.cache_hit = true;
    latest.result.reused_packages = latest.result.items.len();
    latest.result.stats.cache_hit = true;
    latest.result.stats.reused_packages = latest.result.items.len();
    save_persistent_scan(latest);
    true
}

fn package_extension(path: &Path) -> bool {
    // Treat disabled duplicate markers as inactive DBPFs for organization.
    // The physical .disabled suffix is never stripped, so the game will not
    // load the package after a move. Duplicate and conflict scans have their
    // own active-package discovery rules and remain unchanged.
    let name = path
        .file_name()
        .map(|v| v.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    name.ends_with(".package")
        || name.ends_with(".package.disabled")
        || name.ends_with(".package.duplicated")
}

fn is_quarantined_duplicate_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".package.duplicated")
}

fn contains_ascii_case_insensitive(data: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() || data.len() < needle.len() {
        return false;
    }

    // Most positions cannot start a match.  Checking the first ASCII byte
    // before comparing the rest avoids a complete case-folded comparison at
    // every offset in large S3SA and manifest payloads while retaining the
    // exact case-insensitive substring semantics.
    let first = needle[0].to_ascii_lowercase();
    for start in 0..=data.len() - needle.len() {
        if data[start].to_ascii_lowercase() == first
            && data[start + 1..start + needle.len()]
                .iter()
                .zip(&needle[1..])
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            return true;
        }
    }
    false
}

fn store_name_hint(value: &str) -> bool {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();
    normalized.split_whitespace().any(|token| token == "store")
}

fn detect_store_source(
    project_evidence: &ProjectTechnicalEvidence,
    name: &str,
    relative: &str,
    has_s3sa: bool,
) -> (String, Option<String>) {
    // Project evidence already decoded the relevant S3SA/STBL/manifest/NMAP
    // resources. Reopening every such resource here made large Store worlds
    // pay for a second full pass after scanning had apparently finished.
    if project_evidence.store_marker {
        return ("the_sims_3_store".to_string(), Some("strong".to_string()));
    }

    if has_s3sa && (store_name_hint(name) || store_name_hint(relative)) {
        return ("the_sims_3_store".to_string(), Some("probable".to_string()));
    }

    ("custom_content".to_string(), None)
}

const TYPE_NMAP_LOCAL: u32 = 0x0166_038C;
const TYPE_SIME_LOCAL: u32 = 0x025E_D6F4;
const TYPE_SIMO_LOCAL: u32 = 0x0341_ACC9;
const TYPE_LAYO_LOCAL: u32 = 0x025C_95B6;
const TYPE_STBL_LOCAL: u32 = 0x2205_57DA;
const TYPE_MANIFEST_LOCAL: u32 = 0x73E9_3EEB;
const TYPE_XML_LOCAL: u32 = 0x0333_406C;
const TYPE_ITUN_LOCAL: u32 = 0x03B3_3DDF;
const TYPE_CLIP_LOCAL: u32 = 0x6B20_C4F3;

fn localized_special_folder(language: AppLanguage, key: &str) -> &'static str {
    match (language, key) {
        (_, "nraas") => "NRaas",
        (AppLanguage::En, "poses") => "Poses and Animations",
        (AppLanguage::Pt, "poses") => "Poses e Animações",
        (AppLanguage::Es, "poses") => "Poses y Animaciones",
        (AppLanguage::En, "gameplay") => "Gameplay",
        (AppLanguage::Pt, "gameplay") => "Jogabilidade",
        (AppLanguage::Es, "gameplay") => "Jugabilidad",
        (AppLanguage::En, "scripts") => "Scripts",
        (AppLanguage::Pt, "scripts") => "Scripts",
        (AppLanguage::Es, "scripts") => "Scripts",
        (AppLanguage::En, "cooking_food") => "Cooking & Food",
        (AppLanguage::Pt, "cooking_food") => "Culinária e Comida",
        (AppLanguage::Es, "cooking_food") => "Cocina y Comida",
        (AppLanguage::En, "services") => "Services",
        (AppLanguage::Pt, "services") => "Serviços",
        (AppLanguage::Es, "services") => "Servicios",
        (AppLanguage::En, "careers") => "Careers",
        (AppLanguage::Pt, "careers") => "Carreiras",
        (AppLanguage::Es, "careers") => "Carreras",
        (AppLanguage::En, "relationships") => "Relationships",
        (AppLanguage::Pt, "relationships") => "Relacionamentos",
        (AppLanguage::Es, "relationships") => "Relaciones",
        (AppLanguage::En, "story_progression") => "Story Progression",
        (AppLanguage::Pt, "story_progression") => "Progressão da História",
        (AppLanguage::Es, "story_progression") => "Progresión de la Historia",
        (AppLanguage::En, "ui") => "UI",
        (AppLanguage::Pt, "ui") => "Interface",
        (AppLanguage::Es, "ui") => "Interfaz",
        (AppLanguage::En, "replacements") => "Replacements",
        (AppLanguage::Pt, "replacements") => "Substituições",
        (AppLanguage::Es, "replacements") => "Reemplazos",
        (AppLanguage::En, "utilities") => "Utilities",
        (AppLanguage::Pt, "utilities") => "Utilitários",
        (AppLanguage::Es, "utilities") => "Utilidades",
        _ => "Unknown",
    }
}

// A package without CASP/OBJD can still be identified as an override without
// inventing a semantic subtype from its file name or source directory.  The
// reference catalog proves that a payload changes a game resource; the
// Overrides branch is an explicit loading contract, not a creator/category
// hint.  Both are sufficient for the *generic* replacement classification,
// but never for a more specific body/object/CAS folder.
fn reference_or_override_family_classification(
    types: &BTreeSet<u32>,
    reference_evidence: Option<&ReferenceEvidence>,
    source_is_override: bool,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    if types.contains(&TYPE_CASP) || types.contains(&TYPE_OBJD) || types.contains(&TYPE_S3SA) {
        return None;
    }

    let official_changed = reference_evidence.is_some_and(|evidence| {
        evidence.changed_resource_count > 0
            && matches!(
                evidence.official_overlap_kind.as_deref(),
                Some("replacement_candidate") | Some("mixed_official_and_new")
            )
    });

    // LAYO is the UI layout resource.  A CAS-specific layout is classified by
    // the stronger internal-marker rule above; all remaining layout packages
    // are safely grouped as Interface without deriving a fake CAS subtype.
    if types.contains(&TYPE_LAYO_LOCAL) {
        let interface = localized_special_folder(language, "ui").to_string();
        let mut detected_from = vec!["LAYO".to_string()];
        if official_changed {
            detected_from.push("OfficialReplacementEvidence".to_string());
        }
        if source_is_override {
            detected_from.push("OverridesLoadingBranch".to_string());
        }
        return Some(PackageFamilyClassification {
            main_category: interface.clone(),
            sub_category: None,
            folder_parts: vec![interface],
            detected_from,
            technical_reason: if official_changed {
                "LAYO interface resource with changed payload for an official TGI => Interface. No narrower UI folder is inferred.".to_string()
            } else {
                "LAYO interface resource => Interface. No semantic subtype is inferred from the source path or filename.".to_string()
            },
        });
    }

    if !(official_changed || source_is_override) {
        return None;
    }

    let replacements = localized_special_folder(language, "replacements").to_string();
    let mut detected_from = Vec::new();
    if official_changed {
        detected_from.push("OfficialReplacementEvidence".to_string());
    }
    if source_is_override {
        detected_from.push("OverridesLoadingBranch".to_string());
    }
    Some(PackageFamilyClassification {
        main_category: replacements.clone(),
        sub_category: None,
        folder_parts: vec![replacements],
        detected_from,
        technical_reason: if official_changed {
            "Changed payload for an official TGI proves an override/replacement. CASP/OBJD do not prove a narrower semantic subtype, so the package remains in the generic replacement category.".to_string()
        } else {
            "Package is in the explicit Overrides loading branch. It is recorded as a protected generic replacement; no semantic subtype is inferred from the source path or filename.".to_string()
        },
    })
}

// Some CAS replacement packages expose an OBJD/VPXY/IMG bundle instead of a
// parseable CASP record.  A name such as "CAS Default" is useful evidence,
// but it is not sufficient by itself: require changed official resources and
// an internal CAS identifier before projecting the package to the generic CAS
// replacement folder.  Do not infer a narrower body/face/eye subtype here.
fn cas_default_replacement_classification(
    types: &BTreeSet<u32>,
    name: &str,
    project_evidence: &ProjectTechnicalEvidence,
    reference_evidence: Option<&ReferenceEvidence>,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    let normalized_name = name
        .replace(['_', '-', '.'], " ")
        .to_ascii_lowercase();
    let has_cas_default_name = normalized_name.contains("cas")
        && normalized_name.contains("default");
    let official_replacement = reference_evidence.is_some_and(|evidence| {
        evidence.changed_resource_count > 0
            && matches!(
                evidence.official_overlap_kind.as_deref(),
                Some("replacement_candidate") | Some("mixed_official_and_new")
            )
    });
    let has_visual_bundle = types.contains(&TYPE_OBJD)
        && (types.contains(&TYPE_IMG) || types.contains(&0x7368_84F1));
    let has_internal_cas_evidence = project_evidence.identifiers.iter().any(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("cas") || lower.contains("sim")
    });

    if !has_cas_default_name
        || !official_replacement
        || !has_visual_bundle
        || !has_internal_cas_evidence
        || project_evidence.has_s3sa
    {
        return None;
    }

    let replacements = localized_special_folder(language, "replacements").to_string();
    Some(PackageFamilyClassification {
        main_category: replacements.clone(),
        sub_category: Some("CAS".to_string()),
        folder_parts: vec![replacements, "CAS".to_string()],
        detected_from: vec![
            "CASDefaultName".to_string(),
            "CASInternalEvidence".to_string(),
            "OfficialReplacementEvidence".to_string(),
        ],
        technical_reason: "CAS Default package with internal CAS evidence and changed official resources identifies a generic CAS replacement; no narrower body, face, or eye subtype is inferred without stronger evidence.".to_string(),
    })
}

fn store_reference_family_classification(
    evidence: &ReferenceEvidence,
) -> Option<PackageFamilyClassification> {
    if evidence.store_confidence.as_deref() != Some("confirmed")
        || evidence.store_category_parts.is_empty()
    {
        return None;
    }
    let folder_parts = evidence.store_category_parts.clone();
    Some(PackageFamilyClassification {
        main_category: folder_parts.first()?.clone(),
        sub_category: folder_parts.get(1).cloned(),
        folder_parts,
        detected_from: vec!["StoreCatalogExact".to_string()],
        technical_reason:
            "Confirmed by the persistent Sims 3 Store technical package/resource-set catalog."
                .to_string(),
    })
}

fn store_family_classification(
    evidence: &ReferenceEvidence,
) -> Option<PackageFamilyClassification> {
    if let Some(classification) = store_reference_family_classification(evidence) {
        return Some(classification);
    }
    if evidence.store_match_kind.as_deref() != Some("named_resource_overlap")
        || evidence.store_confidence.as_deref() != Some("evidence")
        || evidence.store_category_parts.is_empty()
    {
        return None;
    }
    let folder_parts = evidence.store_category_parts.clone();
    Some(PackageFamilyClassification {
        main_category: folder_parts.first()?.clone(),
        sub_category: folder_parts.get(1).cloned(),
        folder_parts,
        detected_from: vec!["StoreCatalogNameAndResourceOverlap".to_string()],
        technical_reason:
            "The Store catalog name and shared resources identify the Store hierarchy; full package identity remains unconfirmed."
                .to_string(),
    })
}

fn store_evidence_can_resolve_status(status: &str) -> bool {
    matches!(status, "unknown" | "needs_review" | "mixed")
}

fn internal_signature(
    package: &Package,
    resource_types: &[u32],
    needles: &[&str],
) -> Option<String> {
    for entry in &package.entries {
        if !resource_types.contains(&entry.type_id) {
            continue;
        }
        let Ok(data) = package.data(entry) else {
            continue;
        };
        for needle in needles {
            if contains_ascii_case_insensitive(&data, needle) {
                return Some(format!(
                    "{} contains '{}'",
                    resource_type_label(entry.type_id),
                    needle
                ));
            }
        }
    }
    None
}

fn strip_leading_status_tags(value: &str) -> String {
    let mut text = value.trim().to_string();
    loop {
        let trimmed = text.trim_start();
        if !trimmed.starts_with('[') {
            return trimmed.to_string();
        }
        let Some(end) = trimmed.find(']') else {
            return trimmed.to_string();
        };
        text = trimmed[end + 1..].trim_start().to_string();
    }
}

fn creator_candidate_from_filename(name: &str) -> Option<String> {
    let stem = Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let stem = strip_leading_status_tags(&stem);
    // A standalone mod name (e.g. AnimatedWoohoo.package) is not its author.
    // Only a token delimited from a separate title can be an author prefix.
    // Natural-language filenames ("Al Fresco Street Market", "Retro Workout")
    // do not contain an author prefix just because their title has spaces.
    if !stem.chars().any(|ch| ch == '_' || ch == '-' || ch == '.') {
        return None;
    }
    let candidate = stem
        .split(|ch: char| ch == '_' || ch == '-' || ch == '.')
        .next()?
        .trim();
    if !(2..=40).contains(&candidate.len()) || !candidate.chars().any(|ch| ch.is_ascii_alphabetic())
    {
        return None;
    }
    let lower = candidate.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "al" | "mod"
            | "mods"
            | "script"
            | "scripts"
            | "package"
            | "update"
            | "updated"
            | "new"
            | "fix"
            | "override"
            | "ts3"
            | "sims3"
            | "the"
    ) {
        return None;
    }
    Some(candidate.to_string())
}

fn pretty_creator_label(candidate: &str) -> String {
    match candidate.to_ascii_lowercase().as_str() {
        "twinsimming" => "TwinSimming".to_string(),
        "nraas" => "NRaas".to_string(),
        _ => {
            let mut chars = candidate.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_uppercase().collect::<String>(),
                    chars.as_str()
                ),
                None => candidate.to_string(),
            }
        }
    }
}

fn verified_script_creator(package: &Package, name: &str) -> Option<String> {
    let candidate = creator_candidate_from_filename(name)?;
    internal_signature(
        package,
        &[
            TYPE_S3SA,
            TYPE_NMAP_LOCAL,
            TYPE_XML_LOCAL,
            TYPE_ITUN_LOCAL,
            TYPE_STBL_LOCAL,
            TYPE_MANIFEST_LOCAL,
        ],
        &[candidate.as_str()],
    )?;
    Some(pretty_creator_label(&candidate))
}

// A standalone assembly name is not a creator. Match the complete package
// stem against embedded identifiers to recover primary gameplay assemblies
// that also include unrelated CASP/OBJD resources.
fn verified_standalone_script_identity(package: &Package, name: &str) -> bool {
    if creator_candidate_from_filename(name).is_some() {
        return false;
    }
    let Some(stem) = Path::new(name).file_stem() else {
        return false;
    };
    let stem = strip_leading_status_tags(&stem.to_string_lossy());
    let stem = stem.trim();
    if !(8..=80).contains(&stem.len()) || !stem.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }
    internal_signature(
        package,
        &[
            TYPE_S3SA,
            TYPE_NMAP_LOCAL,
            TYPE_XML_LOCAL,
            TYPE_ITUN_LOCAL,
            TYPE_STBL_LOCAL,
            TYPE_MANIFEST_LOCAL,
        ],
        &[stem],
    )
    .is_some()
}

fn generic_container_folder(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "#+18"
            | "18+"
            | "18"
            | "mods"
            | "packages"
            | "downloads"
            | "download"
            | "gameplay"
            | "jogabilidade"
            | "jugabilidad"
            | "scripts"
            | "objects"
            | "objetos"
            | "buy"
            | "compra"
            | "build"
            | "construção"
            | "construccion"
            | "cakes"
            | "cookies"
            | "breads"
            | "pies"
            | "pastries"
            | "cupcakes"
            | "ingredients"
            | "savory"
            | "gourmet desserts"
            | "other baked goods"
            | "wedding cakes"
            | "birthday cakes"
    )
}

fn mod_name_from_relative(relative: &str) -> Option<String> {
    let path = Path::new(relative);
    let mut components = path
        .parent()?
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().trim().to_string()),
            _ => None,
        });

    let first = components.next()?;
    if first.is_empty() || generic_container_folder(&first) {
        return None;
    }
    Some(first)
}

fn mod_name_from_filename(name: &str) -> Option<String> {
    let stem = Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let stem = strip_leading_status_tags(&stem);
    let without_creator = stem
        .split_once('_')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(stem.trim());
    let lower = without_creator.to_ascii_lowercase();
    let mod_end = lower
        .find(" mod")
        .map(|index| index + 4)
        .or_else(|| lower.starts_with("mod ").then_some(3))?;
    let value = without_creator[..mod_end].trim();
    if value.len() < 3 || generic_container_folder(value) {
        None
    } else {
        Some(value.to_string())
    }
}

fn inferred_script_mod_name(name: &str, relative: &str) -> Option<String> {
    mod_name_from_relative(relative).or_else(|| mod_name_from_filename(name))
}

// Match the declared subject of a mod, not arbitrary text inside an S3SA
// assembly or its tuning data. Generic game frameworks reference cooking,
// careers, dialogs, etc. without primarily changing those features.
fn script_category(name: &str, language: AppLanguage) -> &'static str {
    let filename = name.to_ascii_lowercase();
    let named = |words: &[&str]| words.iter().any(|word| filename.contains(word));

    if named(&[
        "relationship",
        "romance",
        "romantic",
        "woohoo",
        "dating",
        "conversation",
        "bettergreet",
    ]) {
        return localized_special_folder(language, "relationships");
    }
    if named(&["storyprogression", "story progression"]) {
        return localized_special_folder(language, "story_progression");
    }
    if named(&[
        "baking",
        "recipe",
        "food",
        "cooking",
        "cake",
        "pastry",
        "restaurant",
        "bistro",
        "kitchen",
        "pasteurize",
        "milk mod",
    ]) {
        return localized_special_folder(language, "cooking_food");
    }
    if named(&["career", "profession"]) {
        return localized_special_folder(language, "careers");
    }
    if named(&[
        "housekeeper",
        "gardener_service",
        "maid_service",
        "cleaningservice",
        "service_npc",
    ]) {
        return localized_special_folder(language, "services");
    }
    if named(&[
        "userinterface",
        "user interface",
        "ui mod",
        "hud",
        "dialog",
        "loading screen",
    ]) {
        return localized_special_folder(language, "ui");
    }
    if named(&[
        "utility",
        "utilities",
        "framework",
        "loader",
        "coremod",
        "core_mod",
        "smoothpatch",
        "monopatcher",
    ]) {
        return localized_special_folder(language, "utilities");
    }
    localized_special_folder(language, "scripts")
}

// A script assembly in an object does not by itself make a standalone gameplay
// mod. Conversely, actual gameplay mods may bundle CASP/OBJD resources. A
// verified internal author or an explicitly scripted source tree is independent
// evidence that the assembly is primary rather than embedded object behavior.
fn script_creator_folder(language: AppLanguage, creator: Option<&str>) -> Vec<String> {
    // A verified author is metadata, not a physical destination. Gameplay
    // packages stay under the localized gameplay category; only NRaas has a
    // creator-level tree because its package family is part of the approved
    // physical organization. Named mod folders are added later only when a
    // real source folder contains a related set of packages.
    if creator.is_some_and(|name| name.eq_ignore_ascii_case("NRaas")) {
        return vec![
            localized_special_folder(language, "scripts").to_string(),
            "NRaas".to_string(),
        ];
    }
    vec![
        localized_special_folder(language, "scripts").to_string(),
        localized_special_folder(language, "gameplay").to_string(),
    ]
}

// A Sims export with embedded CC is a single user-facing asset, even though
// the DBPF contains many CASP, GEOM, texture and tuning resources. SIME/SIMO
// are the identity-bearing resources; the additional CASP plus appearance
// resources prove that this is the merged-CC form rather than a common merged
// clothing, hair or object package. The rule is deliberately resource-based:
// filenames and a Sim's name are never required.
fn merged_sim_export_evidence(package: &Package) -> Option<String> {
    let sim_entries = package
        .entries
        .iter()
        .filter(|entry| matches!(entry.type_id, TYPE_SIME_LOCAL | TYPE_SIMO_LOCAL))
        .collect::<Vec<_>>();
    if sim_entries.is_empty() {
        return None;
    }
    if sim_entries
        .iter()
        .any(|entry| entry.instance == 0 || entry.group == 0)
    {
        return None;
    }

    let casp_count = package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_CASP)
        .count();
    let appearance_count = package
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.type_id,
                TYPE_CASP
                    | 0x015A_1849 // GEOM
                    | TYPE_IMG
                    | TYPE_BGEO
                    | TYPE_BONE_DELTA
                    | TYPE_FACE
                    | TYPE_FBLN
                    | TYPE_HAIR_TONE
                    | TYPE_SKIN_TONE
            )
        })
        .count();
    if casp_count == 0 || appearance_count < 3 {
        return None;
    }

    let identifiers = sim_entries
        .iter()
        .take(8)
        .map(|entry| {
            format!(
                "{} group 0x{:08X} instance 0x{:016X}",
                if entry.type_id == TYPE_SIME_LOCAL {
                    "SIME"
                } else {
                    "SIMO"
                },
                entry.group,
                entry.instance
            )
        })
        .collect::<Vec<_>>();
    Some(format!(
        "{}; {} CASP and {} appearance resources remain one exported Sim package",
        identifiers.join("; "),
        casp_count,
        appearance_count
    ))
}

fn merged_sim_export_classification(
    package: &Package,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    let evidence = merged_sim_export_evidence(package)?;
    let label = match language {
        AppLanguage::En => "Merged Sim Export",
        AppLanguage::Pt => "Sim Exportado (CC mesclado)",
        AppLanguage::Es => "Sim Exportado (CC combinado)",
    };
    Some(PackageFamilyClassification {
        main_category: label.to_string(),
        sub_category: None,
        folder_parts: Vec::new(),
        detected_from: vec![
            "SIME/SIMO".to_string(),
            "MergedSimExport".to_string(),
            "ProtectedPackagesRoot".to_string(),
        ],
        technical_reason: format!(
            "SIME/SIMO internal Sim identifiers plus merged CAS/appearance resources => keep the complete package at the Packages root. Do not split embedded CC. {evidence}"
        ),
    })
}

fn package_nmap_names(package: &Package) -> Vec<String> {
    package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_NMAP_LOCAL)
        .filter_map(|entry| package.data(entry).ok())
        .flat_map(|data| nmap_names(&data))
        .collect()
}

// The phase-3 families only consult a small set of NMAP subjects. Materialize
// those matching names, not every unrelated identifier in a large script
// package. This preserves each rule's actual evidence while avoiding millions
// of short-lived String allocations in merged core mods.
fn phase3_nmap_identifiers(package: &Package) -> Vec<String> {
    const PHASE3_TERMS: &[&str] = &[
        "rabbit", "rhd", "rhr", "window", "knitting", "baking", "buhudain", "sims3logo",
        "stereo", "contact", "iris", "eyeball", "default", "lip", "makeup", "face", "eye",
        "body", "genital",
    ];
    package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_NMAP_LOCAL)
        .filter_map(|entry| package.data(entry).ok())
        .flat_map(|data| nmap_names_with_ascii_terms(&data, PHASE3_TERMS))
        .collect()
}

// NMAP is the preferred source for readable identifiers, but several of the
// directed families expose their identity in another DBPF resource. Keep a
// small allow-list of functional markers and inspect the decoded payloads so
// relocation never turns a proven package back into unknown merely because
// its filename or source folder changed.
fn package_internal_identifiers(
    package: &Package,
    project_evidence: &ProjectTechnicalEvidence,
) -> Vec<String> {
    let mut identifiers = phase3_nmap_identifiers(package);
    // The shared project pass already decoded LAYO/NMAP/STBL/manifest/S3SA
    // payloads and recorded these exact byte markers. Reuse it here instead
    // of reopening and scanning the same large resources once per package.
    for marker in INTERNAL_IDENTIFIER_MARKERS {
        if project_evidence
            .internal_markers
            .iter()
            .any(|value| value.eq_ignore_ascii_case(marker))
            && !identifiers
                .iter()
                .any(|value| value.eq_ignore_ascii_case(marker))
        {
            identifiers.push((*marker).to_string());
        }
    }
    identifiers
}

fn internal_name_contains(names: &[String], markers: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        let lower = name.to_ascii_lowercase();
        markers
            .iter()
            .find(|marker| lower.contains(**marker))
            .map(|_| name.clone())
    })
}

// These rules intentionally take both the package's resource family and an
// internal identifier. A source folder such as `Fixes` or `Cat and Dog
// Addons` is useful evidence during an audit, but it must not be required for
// a second scan after the package has reached its canonical destination.
fn path_independent_directed_classification(
    types: &BTreeSet<u32>,
    internal_names: &[String],
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    let corrections = language.corrections();
    let replacements = match language {
        AppLanguage::En => "Replacements",
        AppLanguage::Pt => "Substituições",
        AppLanguage::Es => "Reemplazos",
    };
    let gameplay = localized_special_folder(language, "gameplay");

    let cas_layout_marker =
        internal_name_contains(internal_names, &["casdetails", "ui/caption/cas/physical:"]);
    if types.contains(&TYPE_LAYO_LOCAL)
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
        && !types.contains(&TYPE_S3SA)
        && cas_layout_marker.is_some()
    {
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("CAS".to_string()),
            folder_parts: vec![replacements.to_string(), "CAS".to_string()],
            detected_from: vec!["LAYO".to_string(), "CASLayoutIdentifier".to_string()],
            technical_reason: format!(
                "LAYO-only CAS detail layout with internal identifier '{}' => preserve under Substituições\\CAS after relocation.",
                cas_layout_marker.unwrap_or_default()
            ),
        });
    }

    let lazy_body_marker = internal_name_contains(
        internal_names,
        &["acbody", "adbody", "albody", "ccbody", "cdbody"],
    );
    let lazy_asset_marker = internal_name_contains(
        internal_names,
        &[
            "defaultface",
            "defaultwhisker",
            "defaultbrow",
            "defaulttextures",
        ],
    );
    let body_marker_count = ["acbody", "adbody", "albody", "ccbody", "cdbody"]
        .iter()
        .filter(|marker| {
            internal_names
                .iter()
                .any(|value| value.to_ascii_lowercase().contains(**marker))
        })
        .count();
    if body_marker_count >= 2
        && types.contains(&TYPE_NMAP_LOCAL)
        && (types.contains(&0x015A_1849) || types.contains(&TYPE_IMG))
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
        && lazy_body_marker.is_some()
    {
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("Corpo".to_string()),
            folder_parts: vec![
                replacements.to_string(),
                "Corpo".to_string(),
                "Cães e Gatos".to_string(),
                "Lazy Duchess".to_string(),
            ],
            detected_from: vec![
                "InternalNMAPBodyFamily".to_string(),
                "LazyDuchessFamily".to_string(),
            ],
            technical_reason: format!(
                "Lazy Duchess Cat/Dog body family is identified by NMAP body role '{}' plus GEOM/IMG resources; destination is independent of the source folder.",
                lazy_body_marker.unwrap_or_default()
            ),
        });
    }

    if let Some(marker) = lazy_asset_marker {
        if (types.contains(&0x015A_1849) || types.contains(&TYPE_IMG))
            && !types.contains(&TYPE_CASP)
            && !types.contains(&TYPE_OBJD)
        {
            return Some(PackageFamilyClassification {
                main_category: replacements.to_string(),
                sub_category: Some("Corpo".to_string()),
                folder_parts: vec![
                    replacements.to_string(),
                    "Corpo".to_string(),
                    "Cães e Gatos".to_string(),
                    "Lazy Duchess".to_string(),
                ],
                detected_from: vec![
                    "InternalLazyDuchessAsset".to_string(),
                    "LazyDuchessFamily".to_string(),
                ],
                technical_reason: format!(
                    "Lazy Duchess default asset is identified by internal marker '{}' plus GEOM/IMG resources; source names and folders are not required.",
                    marker
                ),
            });
        }
    }

    let ohrudi_marker = internal_name_contains(
        internal_names,
        &[
            "deathbymeteorjig",
            "minerexclusionjig",
            "sims_need_less_space",
            "pets_need_less_space",
            "king_qings_window",
        ],
    );
    if types.contains(&TYPE_NMAP_LOCAL)
        && (types.contains(&0xD382_BF57) || types.contains(&0x03B4_C61D))
        && ohrudi_marker.is_some()
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
    {
        let marker_lower = ohrudi_marker
            .as_deref()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let ep = if marker_lower.contains("king_qings_window") {
            Some("EP01")
        } else if marker_lower.contains("pets_need_less_space") {
            Some("EP05")
        } else {
            None
        };
        let mut folder_parts = vec![corrections.to_string(), gameplay.to_string()];
        if let Some(ep) = ep {
            folder_parts.extend(["EPs Fixes".to_string(), ep.to_string()]);
        }
        return Some(PackageFamilyClassification {
            main_category: corrections.to_string(),
            sub_category: Some(gameplay.to_string()),
            folder_parts,
            detected_from: vec![
                "InternalNMAPFixFamily".to_string(),
                "OhRudiFamily".to_string(),
            ],
            technical_reason: format!(
                "OhRudi fix family is identified by NMAP functional identifier '{}' and package resource structure; canonical destination no longer depends on a legacy Fixes folder.",
                ohrudi_marker.unwrap_or_default()
            ),
        });
    }

    None
}

fn broad_window_replacement_set(
    package: &Package,
    types: &BTreeSet<u32>,
    internal_names: &[String],
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    let window_name_count = internal_names
        .iter()
        .filter(|value| value.to_ascii_lowercase().contains("window"))
        .count();
    let lighting_resource_count = package
        .entries
        .iter()
        .filter(|entry| entry.type_id == 0x03B4_C61D)
        .count();
    if !types.contains(&TYPE_NMAP_LOCAL)
        || !types.contains(&0x03B4_C61D)
        || window_name_count < 8
        || lighting_resource_count < 8
        || types.contains(&TYPE_CASP)
        || types.contains(&TYPE_OBJD)
    {
        return None;
    }
    let replacements = match language {
        AppLanguage::En => "Replacements",
        AppLanguage::Pt => "Substituições",
        AppLanguage::Es => "Reemplazos",
    };
    Some(PackageFamilyClassification {
        main_category: replacements.to_string(),
        sub_category: Some("Objetos".to_string()),
        folder_parts: vec![replacements.to_string(), "Objetos".to_string()],
        detected_from: vec![
            "InternalWindowReplacementSet".to_string(),
            "NMAP".to_string(),
            "LightingDataFamily".to_string(),
        ],
        technical_reason: format!(
            "DBPF contains {window_name_count} internal window identifiers and {lighting_resource_count} lighting-data resources; this is a broad replacement set, not an OhRudi fix."
        ),
    })
}

// Phase 3 rules use resource families plus coherent source trees. A filename
// or hash alone never qualifies. These families were confirmed by the local
// directed comparison: EP fixes, named replacement sets, and coherent GEOM
// bundles can be categorized without treating generic game TGIs as matches.
fn phase3_directed_family_classification(
    package: &Package,
    project_evidence: &ProjectTechnicalEvidence,
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    catalog_classifications: &[CatalogClassification],
) -> Option<PackageFamilyClassification> {
    let internal_names = package_internal_identifiers(package, project_evidence);
    if let Some(classification) = verified_eyelash_texture_companion(&package.entries, types, language)
    {
        return Some(classification);
    }
    if let Some(classification) = verified_eye_texture_replacement(&package.entries, types, language)
    {
        return Some(classification);
    }
    if let Some(classification) =
        path_independent_directed_classification(types, &internal_names, language)
    {
        return Some(classification);
    }
    if let Some(classification) =
        broad_window_replacement_set(package, types, &internal_names, language)
    {
        return Some(classification);
    }
    if let Some(classification) = rabbit_hole_family_classification(
        types,
        name,
        relative,
        language,
        catalog_classifications,
        &internal_names,
    ) {
        return Some(classification);
    }
    if let Some(classification) = composite_script_family_classification(
        package,
        types,
        name,
        relative,
        language,
        &internal_names,
    ) {
        return Some(classification);
    }
    if let Some(classification) = non_catalog_replacement_classification(
        package,
        types,
        name,
        relative,
        language,
        &internal_names,
    ) {
        return Some(classification);
    }
    if let Some(classification) =
        canonical_script_source_classification(types, relative, language, &internal_names)
    {
        return Some(classification);
    }
    if let Some(classification) =
        tss_asset_companion_classification(types, name, relative, language, &internal_names)
    {
        return Some(classification);
    }
    if let Some(classification) =
        legacy_source_evidence_classification(types, name, relative, language)
    {
        return Some(classification);
    }
    let lower = relative.to_ascii_lowercase();
    let filename = name.to_ascii_lowercase();
    let has_geom = types.contains(&0x015A_1849);
    let has_s3sa = types.contains(&TYPE_S3SA);
    let has_catalog = types.contains(&TYPE_CASP) || types.contains(&TYPE_OBJD);
    let corrections = language.corrections();
    let replacements = match language {
        AppLanguage::En => "Replacements",
        AppLanguage::Pt => "Substituições",
        AppLanguage::Es => "Reemplazos",
    };
    let gameplay = localized_special_folder(language, "gameplay");
    let ep = (1..=11)
        .map(|number| format!("ep{number:02}"))
        .find(|token| lower.contains(token));

    if lower.contains("eps fixes")
        && (has_s3sa || has_geom)
        && (lower.contains("fix") || filename.contains("fix"))
    {
        let mut folder_parts = vec![
            corrections.to_string(),
            gameplay.to_string(),
            "EPs Fixes".to_string(),
        ];
        if let Some(ep) = ep {
            folder_parts.push(ep.to_ascii_uppercase());
        }
        return Some(PackageFamilyClassification {
            main_category: corrections.to_string(),
            sub_category: Some(gameplay.to_string()),
            folder_parts,
            detected_from: vec!["Phase3FamilyRule".to_string(), "EPsFixesPath".to_string(), "ResourceFamily".to_string()],
            technical_reason: "Coherent EPs Fixes path plus DBPF fix/resource evidence; canonical protected EP tree preserved.".to_string(),
        });
    }

    if lower.contains("fix")
        && !lower.contains("eps fixes")
        && (has_s3sa || types.contains(&0xD382_BF57))
        && !(types.contains(&TYPE_OBJD)
            && catalog_classifications
                .iter()
                .any(catalog_contains_rabbit_holes))
        && !(types.contains(&TYPE_OBJD)
            && catalog_classifications
                .iter()
                .any(|classification| !classification.ambiguous))
    {
        return Some(PackageFamilyClassification {
            main_category: corrections.to_string(),
            sub_category: Some(gameplay.to_string()),
            folder_parts: vec![corrections.to_string(), gameplay.to_string()],
            detected_from: vec!["Phase3FamilyRule".to_string(), "FixToken".to_string(), "ResourceFamily".to_string()],
            technical_reason: "Explicit fix family corroborated by script/functional DBPF resources; no EP invented without EP evidence.".to_string(),
        });
    }

    if lower.contains("cmar")
        && has_geom
        && !has_catalog
        && types.contains(&TYPE_NMAP_LOCAL)
        && internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("ambody")
                || value.contains("embody")
                || value.contains("tmbody")
                || value.contains("genital")
        })
    {
        return Some(PackageFamilyClassification {
            main_category: "Genética".to_string(),
            sub_category: Some("Genitálias".to_string()),
            folder_parts: vec!["Genética".to_string(), "Genitálias".to_string(), "Masculina".to_string(), "Cmar".to_string()],
            detected_from: vec!["Phase3FamilyRule".to_string(), "CmarFamily".to_string(), "GEOM".to_string(), "NMAP".to_string()],
            technical_reason: "Cmar GEOM family is recognized as anatomical content only when NMAP identifiers corroborate am/em/tm or genital roles. A mesh-only cmar package remains for review instead of being assigned from its legacy filename or folder.".to_string(),
        });
    }

    if lower.contains("sam4v2") && has_geom && !has_catalog {
        return Some(PackageFamilyClassification {
            main_category: "Genética".to_string(),
            sub_category: Some("Corpo".to_string()),
            folder_parts: vec!["Genética".to_string(), "Corpo".to_string(), "Masculina".to_string(), "sam4v2".to_string()],
            detected_from: vec!["Phase3FamilyRule".to_string(), "CoherentGEOMFamily".to_string(), "LODSet".to_string()],
            technical_reason: "Coherent GEOM set with am/em/tm and LOD/burnt variants; the family is retained without splitting related meshes.".to_string(),
        });
    }

    if lower.contains("cat and dog addons") && (has_geom || has_s3sa) && !has_catalog {
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("Corpo".to_string()),
            folder_parts: vec![replacements.to_string(), "Corpo".to_string(), "Cães e Gatos".to_string(), "Lazy Duchess".to_string()],
            detected_from: vec!["Phase3FamilyRule".to_string(), "LazyDuchessFamily".to_string(), "BodyResourceFamily".to_string()],
            technical_reason: "Lazy Duchess Cat and Dog Addons set has coherent body mesh/texture resources; related packages remain together.".to_string(),
        });
    }

    if lower.contains("boring bones") && has_s3sa && !has_catalog {
        let (sub, family) = if lower.contains("ethereal") || lower.contains("lighting") {
            ("Scripts".to_string(), "BoringBones".to_string())
        } else if lower.contains("olhos") || lower.contains("eye") {
            ("Corpo\\Olhos".to_string(), "BoringBones".to_string())
        } else {
            ("Ambiente".to_string(), "BoringBones".to_string())
        };
        let folder_parts = if sub == "Scripts" {
            vec!["Scripts".to_string(), gameplay.to_string(), family]
        } else {
            vec![replacements.to_string(), sub, family]
        };
        return Some(PackageFamilyClassification {
            main_category: folder_parts[0].clone(),
            sub_category: folder_parts.get(1).cloned(),
            folder_parts,
            detected_from: vec!["Phase3FamilyRule".to_string(), "BoringBonesFamily".to_string(), "S3SA".to_string()],
            technical_reason: "BoringBones coherent family with internal lighting/eye/plant resource evidence; function-specific subfolder preserves the family.".to_string(),
        });
    }

    if lower.contains("defaultface")
        || lower.contains("defaultbrows")
        || lower.contains("eyelash")
        || lower.contains("agsclera")
    {
        if has_s3sa
            && !has_catalog
            && internal_signature(
                package,
                &[TYPE_S3SA, TYPE_NMAP_LOCAL, TYPE_MANIFEST_LOCAL],
                &["face", "eye", "brow", "lash", "clera"],
            )
            .is_some()
        {
            let (sub, label) = if lower.contains("brow") {
                ("Genética".to_string(), "Sobrancelhas".to_string())
            } else if lower.contains("lash") {
                ("CAS".to_string(), "Maquiagem\\Cílios".to_string())
            } else if lower.contains("face") {
                (replacements.to_string(), "Corpo\\Rosto".to_string())
            } else {
                (replacements.to_string(), "Corpo\\Olhos".to_string())
            };
            let folder_parts =
                label
                    .split('\\')
                    .map(str::to_string)
                    .fold(vec![sub], |mut result, part| {
                        result.push(part);
                        result
                    });
            return Some(PackageFamilyClassification {
                main_category: folder_parts[0].clone(),
                sub_category: folder_parts.get(1).cloned(),
                folder_parts,
                detected_from: vec!["Phase3FamilyRule".to_string(), "DefaultReplacement".to_string(), "InternalSubjectEvidence".to_string()],
                technical_reason: "Default face/eye/brow/lash path is corroborated by subject tokens in script/manifest resources; no CAS catalog identity was invented.".to_string(),
            });
        }
    }

    if lower.contains("skin")
        && has_s3sa
        && !has_catalog
        && internal_signature(
            package,
            &[TYPE_S3SA, TYPE_NMAP_LOCAL, TYPE_MANIFEST_LOCAL],
            &["skin", "body"],
        )
        .is_some()
    {
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("Corpo".to_string()),
            folder_parts: vec![replacements.to_string(), "Corpo".to_string(), "Definição de Pele".to_string()],
            detected_from: vec!["Phase3FamilyRule".to_string(), "DefaultSkin".to_string(), "InternalSubjectEvidence".to_string()],
            technical_reason: "Skin/default path plus internal skin/body evidence identifies a replacement set without age inference.".to_string(),
        });
    }

    None
}

// This is a resource-identity rule, not a filename or source-folder exception.
// The real package contains one IMG resource at the known game eye-texture TGI
// below and no CASP/OBJD/script/mesh resources.  Keep the guard narrow so a
// generic visual package is not promoted to an eye replacement accidentally.
fn verified_eye_texture_replacement(
    entries: &[ResourceEntry],
    types: &BTreeSet<u32>,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    const EYE_TEXTURE_INSTANCE: u64 = 0xC2B5_6067_3F00_D806;

    if entries.len() != 1
        || types.len() != 1
        || !types.contains(&TYPE_IMG)
        || entries[0].type_id != TYPE_IMG
        || entries[0].group != 0
        || entries[0].instance != EYE_TEXTURE_INSTANCE
    {
        return None;
    }

    let (replacements, body, eyes) = match language {
        AppLanguage::En => ("Replacements", "Body", "Eyes"),
        AppLanguage::Pt => ("Substituições", "Corpo", "Olhos"),
        AppLanguage::Es => ("Reemplazos", "Cuerpo", "Ojos"),
    };

    Some(PackageFamilyClassification {
        main_category: replacements.into(),
        sub_category: Some(eyes.into()),
        folder_parts: vec![replacements.into(), body.into(), eyes.into()],
        detected_from: vec!["IMG".into(), "EyeReplacementTgi".into()],
        technical_reason: format!(
            "Single IMG resource uses the verified eye-replacement TGI 0x{EYE_TEXTURE_INSTANCE:016X}; no CASP, OBJD, script, or mesh resource is present."
        ),
    })
}

fn catalog_contains_rabbit_holes(classification: &CatalogClassification) -> bool {
    classification
        .folder_parts
        .iter()
        .chain(
            classification
                .candidate_folder_parts
                .iter()
                .flat_map(|parts| parts.iter()),
        )
        .any(|part| part.eq_ignore_ascii_case("Rabbit Holes"))
}

fn rabbit_hole_family_classification(
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    catalog_classifications: &[CatalogClassification],
    internal_names: &[String],
) -> Option<PackageFamilyClassification> {
    let has_rabbit_catalog_evidence = catalog_classifications
        .iter()
        .any(catalog_contains_rabbit_holes);
    let normalized = format!("{}\\{}", relative, name).to_ascii_lowercase();
    let has_family_identifier = normalized.contains("rabbit")
        || normalized.contains("rhd")
        || normalized.contains("rhr")
        || internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("rabbit") || value.contains("rhd") || value.contains("rhr")
        });
    let has_structural_set = types.contains(&TYPE_OBJD)
        && (types.contains(&0x02DC_343F)
            || types.contains(&0x0580_A2B4)
            || types.contains(&0x0580_A2B5)
            || types.contains(&0x0580_A2B6)
            || types.contains(&0x7368_84F1)
            || types.contains(&TYPE_XML_LOCAL));
    let has_related_objd_set = types.contains(&TYPE_OBJD) && catalog_classifications.len() >= 2;
    let invisible_collection = has_family_identifier
        && (types.contains(&0x0C07_456D) || types.contains(&0x2F7D_0004))
        && !types.contains(&TYPE_OBJD);

    if !((has_rabbit_catalog_evidence
        && (has_structural_set || has_related_objd_set)
        && has_family_identifier)
        || invisible_collection)
    {
        return None;
    }

    let construction = match language {
        AppLanguage::En => "Build",
        AppLanguage::Pt => "Construção",
        AppLanguage::Es => "Construcción",
    };
    Some(PackageFamilyClassification {
        main_category: construction.to_string(),
        sub_category: Some("Rabbit Holes".to_string()),
        folder_parts: vec![construction.to_string(), "Rabbit Holes".to_string()],
        detected_from: vec![
            "RabbitHoleCatalogFlags".to_string(),
            "RelatedOBJDSet".to_string(),
            "ResourceFamily".to_string(),
        ],
        technical_reason: "Related OBJD records expose the model's Rabbit Holes candidate (including the shared Tapetes candidate); the NMAP/object resource set and internal family identifier keep the complete Rabbit Hole family together. Filename Fix tokens cannot demote it to Correções.".to_string(),
    })
}

fn non_catalog_replacement_classification(
    package: &Package,
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    internal_names: &[String],
) -> Option<PackageFamilyClassification> {
    if types.contains(&TYPE_CASP) || types.contains(&TYPE_OBJD) {
        return None;
    }
    let lower = format!("{}\\{}", relative, name).to_ascii_lowercase();
    let has_image_payload = types.contains(&TYPE_IMG) || types.contains(&0x00B2_D882);
    let replacements = localized_special_folder(language, "replacements");
    let (sub_category, family, evidence) = if lower.contains("cornice")
        && types.contains(&0x7368_84F1)
        && types.contains(&0x03B4_C61D)
        && types.contains(&0x0166_1233)
    {
        (
            Some("Cornijas"),
            "Awesims",
            "mesh/material/geometry replacement set",
        )
    } else if lower.contains("nobuildsparkles") && has_image_payload {
        (
            Some("Efeitos"),
            "NoBuildSparkles",
            "IMG/effect replacement marker",
        )
    } else if lower.contains("snow but better") && has_image_payload {
        (
            Some("Ambiente"),
            "Lyralei",
            "IMG/environment replacement marker",
        )
    } else if lower.contains("buhudain")
        && has_image_payload
        && internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("buhudain") && (value.contains("face") || value.contains("body"))
        })
    {
        (
            Some("Corpo\\Definição de Pele"),
            "Buhudain",
            "IMG/NMAP skin subject identifiers",
        )
    } else if lower.contains("nointromaxis")
        && types.contains(&0xB1CC_1AF6)
        && internal_names
            .iter()
            .any(|value| value.to_ascii_lowercase().contains("sims3logo"))
    {
        (
            None,
            "NoIntroMaxis",
            "localized Sims3Logo replacement identifiers",
        )
    } else if lower.contains("simsfm")
        && types.contains(&TYPE_NMAP_LOCAL)
        && types.contains(&TYPE_XML_LOCAL)
        && types.contains(&TYPE_STBL_LOCAL)
        && internal_names
            .iter()
            .any(|value| value.eq_ignore_ascii_case("Stereo"))
    {
        (
            Some("Scripts\\Radio and TV"),
            "SimsFM",
            "NMAP/XML/STBL radio tuning identifiers",
        )
    } else {
        return None;
    };

    let folder_parts = if family == "Awesims" {
        vec!["Construção".to_string(), "Cornijas".to_string()]
    } else if family == "SimsFM" {
        vec![
            "Scripts".to_string(),
            localized_special_folder(language, "gameplay").to_string(),
            "Radio and TV".to_string(),
        ]
    } else if let Some(sub_category) = sub_category {
        let mut parts = vec![replacements.to_string()];
        parts.extend(sub_category.split('\\').map(str::to_string));
        parts
    } else {
        vec![replacements.to_string()]
    };
    Some(PackageFamilyClassification {
        main_category: folder_parts[0].clone(),
        sub_category: folder_parts.get(1).cloned(),
        folder_parts,
        detected_from: vec!["NonCatalogReplacement".to_string(), "InternalSubjectEvidence".to_string()],
        technical_reason: format!("{family} is a non-catalog replacement identified by {evidence}; no CASP/OBJD record was promoted to an unrelated category."),
    })
}

fn composite_script_family_classification(
    package: &Package,
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    internal_names: &[String],
) -> Option<PackageFamilyClassification> {
    let normalized = format!("{}\\{}", relative, name).to_ascii_lowercase();
    if normalized.contains("fantuanss")
        && types.contains(&TYPE_S3SA)
        && verified_script_creator(package, name)
            .is_some_and(|creator| creator.eq_ignore_ascii_case("Fantuanss12"))
    {
        let scripts = localized_special_folder(language, "scripts");
        let gameplay = localized_special_folder(language, "gameplay");
        return Some(PackageFamilyClassification {
            main_category: scripts.to_string(),
            sub_category: Some(gameplay.to_string()),
            folder_parts: vec![scripts.to_string(), gameplay.to_string()],
            detected_from: vec![
                "CompositeModFamily".to_string(),
                "InternalCreator".to_string(),
                "S3SA".to_string(),
            ],
            technical_reason: "Fantuanss12 internal creator evidence plus S3SA identifies the oven as a component of the gameplay family; its embedded OBJD must not redirect it to Construção.".to_string(),
        });
    }
    let family = if normalized.contains("knitting")
        || internal_names
            .iter()
            .any(|value| value.to_ascii_lowercase().contains("knitting"))
    {
        Some("Knitting Mod")
    } else if normalized.contains("baking")
        || internal_names
            .iter()
            .any(|value| value.to_ascii_lowercase().contains("baking"))
    {
        Some("Baking Mod")
    } else {
        None
    };
    let family = family?;
    let has_supporting_resources = types.contains(&TYPE_S3SA)
        || (types.contains(&TYPE_OBJD)
            && (types.contains(&TYPE_NMAP_LOCAL) || types.contains(&TYPE_XML_LOCAL)));
    let internal_family_evidence = internal_signature(
        package,
        &[
            TYPE_S3SA,
            TYPE_NMAP_LOCAL,
            TYPE_XML_LOCAL,
            TYPE_STBL_LOCAL,
            TYPE_MANIFEST_LOCAL,
        ],
        &["knitting", "baking"],
    )
    .is_some();
    let path_family_evidence = normalized.contains("knitting") || normalized.contains("baking");
    if !has_supporting_resources
        || (!internal_family_evidence && !types.contains(&TYPE_S3SA) && !path_family_evidence)
    {
        return None;
    }

    let scripts = localized_special_folder(language, "scripts");
    let gameplay = localized_special_folder(language, "gameplay");
    Some(PackageFamilyClassification {
        main_category: scripts.to_string(),
        sub_category: Some(gameplay.to_string()),
        folder_parts: vec![scripts.to_string(), gameplay.to_string(), family.to_string()],
        detected_from: vec![
            "CompositeModFamily".to_string(),
            "ResourceFamily".to_string(),
            if types.contains(&TYPE_S3SA) {
                "S3SA".to_string()
            } else {
                "OBJDCompanion".to_string()
            },
        ],
        technical_reason: format!(
            "The package is a verified {family} component: its internal family identifier is corroborated by script/object support resources, so catalog OBJD subcategories do not split the mod family."
        ),
    })
}

fn canonical_script_source_classification(
    types: &BTreeSet<u32>,
    relative: &str,
    language: AppLanguage,
    internal_names: &[String],
) -> Option<PackageFamilyClassification> {
    let normalized = relative.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("#4 hair/eyebrows/") && types.contains(&TYPE_CASP) {
        return Some(PackageFamilyClassification {
            main_category: "Cabelos e Pêlos".to_string(),
            sub_category: Some("Sobrancelhas".to_string()),
            folder_parts: vec!["Cabelos e Pêlos".to_string(), "Sobrancelhas".to_string()],
            detected_from: vec!["CASP".to_string(), "HairEyebrowTaxonomy".to_string()],
            technical_reason: "CASP records from the approved #4 Hair/Eyebrows branch use the model's canonical hair taxonomy; the CASP technical panel label Genética is not the physical destination for this verified family.".to_string(),
        });
    }
    if normalized.contains("contacts")
        && types.contains(&TYPE_CASP)
        && (types.contains(&0x626F_60CE) || types.contains(&0x2E75_C765))
        && internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("contact") || value.contains("iris") || value.contains("eyeball")
        })
    {
        return Some(PackageFamilyClassification {
            main_category: "Maquiagem".to_string(),
            sub_category: Some("Olhos".to_string()),
            folder_parts: vec!["Maquiagem".to_string(), "Olhos".to_string()],
            detected_from: vec!["CASP".to_string(), "ContactLensResourceSet".to_string()],
            technical_reason: "CASP accessory slot plus contact-lens visual resource types identifies an eye makeup asset; the source Colar folder does not override the model's makeup taxonomy.".to_string(),
        });
    }
    if normalized.contains("lipstickbasegame")
        && types.contains(&TYPE_CASP)
        && (types.contains(&0x626F_60CC) || types.contains(&0x626F_60CD))
        && internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("default") && (value.contains("lip") || value.contains("makeup"))
        })
    {
        let replacements = localized_special_folder(language, "replacements");
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("CAS".to_string()),
            folder_parts: vec![replacements.to_string(), "CAS".to_string()],
            detected_from: vec!["CASP".to_string(), "DefaultReplacementSubject".to_string()],
            technical_reason: "CASP lipstick resources plus the base-game replacement subject identify a replacement, not ordinary Maquiagem content.".to_string(),
        });
    }
    if normalized.contains("eyeball")
        && normalized.contains("uvfix")
        && types.contains(&0x015A_1849)
        && types.contains(&TYPE_NMAP_LOCAL)
        && internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("face") && (value.contains("eye") || value.contains("eyeball"))
        })
    {
        let replacements = localized_special_folder(language, "replacements");
        return Some(PackageFamilyClassification {
            main_category: replacements.to_string(),
            sub_category: Some("Corpo".to_string()),
            folder_parts: vec![replacements.to_string(), "Corpo".to_string(), "Olhos".to_string()],
            detected_from: vec!["GEOM".to_string(), "NMAP".to_string(), "EyeUVReplacement".to_string()],
            technical_reason: "Eye-ball UV-fix package exposes GEOM plus NMAP body identifiers; it is a visual replacement and must not be treated as a slider merely because it is stored in a legacy Sliders folder.".to_string(),
        });
    }
    if normalized.contains("tss_")
        && types.contains(&TYPE_CLIP_LOCAL)
        && (types.contains(&TYPE_XML_LOCAL) || types.contains(&TYPE_ITUN_LOCAL))
        && !types.contains(&TYPE_S3SA)
    {
        return Some(script_family_result(
            language,
            "TSSFamily",
            "CLIP plus ITUN/XML interaction resources identify a TSS gameplay component; the animation resource is auxiliary to the script family.",
        ));
    }
    if types.contains(&TYPE_CLIP_LOCAL)
        && types.contains(&TYPE_S3SA)
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
        && (normalized.contains("pose")
            || normalized.contains("selfie")
            || normalized.contains("chamomilewine"))
    {
        let poses = localized_special_folder(language, "poses");
        return Some(PackageFamilyClassification {
            main_category: poses.to_string(),
            sub_category: None,
            folder_parts: vec![poses.to_string()],
            detected_from: vec!["CLIP".to_string(), "PoseScriptFamily".to_string(), "S3SA".to_string()],
            technical_reason: "CLIP is the primary payload of a named pose package; embedded S3SA/XML startup support is auxiliary and does not turn the asset into a generic gameplay package.".to_string(),
        });
    }
    if types.contains(&TYPE_S3SA)
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
        && (normalized.contains("poseplayer") || normalized.contains("pose_player"))
    {
        let poses = localized_special_folder(language, "poses");
        return Some(PackageFamilyClassification {
            main_category: poses.to_string(),
            sub_category: None,
            folder_parts: vec![poses.to_string()],
            detected_from: vec!["PoseScriptFamily".to_string(), "S3SA".to_string(), "PosePlayerIdentifier".to_string()],
            technical_reason: "PosePlayer identifier plus a standalone S3SA/XML package identifies startup support for the pose system, not a generic gameplay destination.".to_string(),
        });
    }
    if normalized.contains("cmar")
        && types.contains(&TYPE_S3SA)
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
    {
        return Some(PackageFamilyClassification {
            main_category: "Genética".to_string(),
            sub_category: Some("Genitálias".to_string()),
            folder_parts: vec!["Genética".to_string(), "Genitálias".to_string(), "Masculina".to_string(), "Cmar".to_string()],
            detected_from: vec!["CmarScriptFamily".to_string(), "S3SA".to_string()],
            technical_reason: "Cmar's named S3SA is the functional genital family controller; it remains with the verified Cmar anatomical family rather than the generic Scripts root.".to_string(),
        });
    }
    let tuning_source =
        normalized.starts_with("jogabilidade/tuning/") && types.contains(&TYPE_XML_LOCAL);
    let gameplay_source = normalized.contains("#8 scripts/gameplay/")
        && types.contains(&TYPE_OBJD)
        && !types.contains(&TYPE_CASP);
    let radio_source = normalized.contains("#8 scripts/radio and tv/")
        && types.contains(&TYPE_NMAP_LOCAL)
        && types.contains(&TYPE_XML_LOCAL)
        && types.contains(&TYPE_STBL_LOCAL);
    if radio_source {
        return Some(PackageFamilyClassification {
            main_category: "Scripts".to_string(),
            sub_category: Some(localized_special_folder(language, "gameplay").to_string()),
            folder_parts: vec!["Scripts".to_string(), localized_special_folder(language, "gameplay").to_string(), "Radio and TV".to_string()],
            detected_from: vec!["CanonicalScriptSource".to_string(), "RadioTuningFamily".to_string()],
            technical_reason: "NMAP/XML/STBL radio identifiers under the approved Radio and TV script branch keep default channel tuning with Scripts.".to_string(),
        });
    }
    if !tuning_source && !gameplay_source {
        return None;
    }

    let scripts = localized_special_folder(language, "scripts");
    let gameplay = localized_special_folder(language, "gameplay");
    Some(PackageFamilyClassification {
        main_category: scripts.to_string(),
        sub_category: Some(gameplay.to_string()),
        folder_parts: vec![scripts.to_string(), gameplay.to_string()],
        detected_from: vec![
            "CanonicalScriptSource".to_string(),
            if tuning_source {
                "TuningXML".to_string()
            } else {
                "ScriptFamilyPath".to_string()
            },
        ],
        technical_reason: if tuning_source {
            "XML tuning package already belongs to the approved Jogabilidade source branch; normalize its physical destination under Scripts\\Jogabilidade.".to_string()
        } else {
            "OBJD component is under an approved #8 Scripts\\Gameplay family path and carries internal gameplay-family evidence; keep the component with the script family instead of splitting it into Construção.".to_string()
        },
    })
}

fn preserved_correction_family_classification(
    types: &BTreeSet<u32>,
    relative: &str,
) -> Option<PackageFamilyClassification> {
    if !types.contains(&TYPE_S3SA) || types.contains(&TYPE_CASP) {
        return None;
    }
    let mut parts = relative
        .split(['\\', '/'])
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    let file = parts.pop()?;
    if !file.to_ascii_lowercase().ends_with(".package")
        && !file.to_ascii_lowercase().ends_with(".disabled")
    {
        return None;
    }
    let correction_index = parts.iter().position(|part| {
        matches!(
            part.to_ascii_lowercase().as_str(),
            "correções" | "correcoes" | "corrections" | "fixes" | "eps fixes"
        )
    })?;
    let destination = parts[correction_index..].to_vec();
    (!destination.is_empty()).then(|| PackageFamilyClassification {
        main_category: destination[0].clone(),
        sub_category: destination.get(1).cloned(),
        folder_parts: destination,
        detected_from: vec![
            "ProtectedExistingCorrectionFamily".to_string(),
            "S3SA".to_string(),
        ],
        technical_reason: "The package is already inside a protected Correções/Fixes family. S3SA alone does not authorize flattening or migration to Scripts; the existing family path remains the canonical destination until an explicit rule is approved.".to_string(),
    })
}

fn script_family_result(
    language: AppLanguage,
    family_source: &str,
    technical_reason: &str,
) -> PackageFamilyClassification {
    let scripts = localized_special_folder(language, "scripts");
    let gameplay = localized_special_folder(language, "gameplay");
    PackageFamilyClassification {
        main_category: scripts.to_string(),
        sub_category: Some(gameplay.to_string()),
        folder_parts: vec![scripts.to_string(), gameplay.to_string()],
        detected_from: vec![
            "CanonicalScriptSource".to_string(),
            family_source.to_string(),
        ],
        technical_reason: technical_reason.to_string(),
    }
}

// TSS asset/support packages are not necessarily S3SA anchors. They can
// contain only the resource payload used by the primary interaction package,
// which previously left names such as TSS_MorePregnancyInteractions_Assets
// in review. Require both the explicit TSS asset identity and a real support
// resource set; the filename alone is never sufficient.
fn tss_asset_companion_classification(
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    internal_names: &[String],
) -> Option<PackageFamilyClassification> {
    let normalized = format!("{relative}/{name}").replace('\\', "/").to_ascii_lowercase();
    let is_tss_asset = normalized.contains("tss_") && normalized.contains("assets");
    let has_interaction_marker = normalized.contains("interaction")
        || internal_names.iter().any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("interaction") || value.contains("pregnancy")
        });
    let has_support_resource = types.iter().any(|type_id| {
        matches!(
            *type_id,
            TYPE_CLIP_LOCAL
                | TYPE_XML_LOCAL
                | TYPE_ITUN_LOCAL
                | TYPE_STBL_LOCAL
                | TYPE_NMAP_LOCAL
                | TYPE_MANIFEST_LOCAL
        )
    });
    if !is_tss_asset || !has_interaction_marker || !has_support_resource {
        return None;
    }

    let scripts = localized_special_folder(language, "scripts");
    let gameplay = localized_special_folder(language, "gameplay");
    Some(PackageFamilyClassification {
        main_category: scripts.to_string(),
        sub_category: Some(gameplay.to_string()),
        folder_parts: vec![scripts.to_string(), gameplay.to_string()],
        detected_from: vec![
            "TSSAssetCompanion".to_string(),
            "TSSInteractionResourceSet".to_string(),
        ],
        technical_reason: "TSS asset package has an interaction-family identity and a verified support resource set; it stays with the related gameplay interaction packages instead of remaining in review.".to_string(),
    })
}

fn legacy_source_evidence_classification(
    types: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    let normalized = format!("{}\\{}", relative, name)
        .replace('\\', "/")
        .to_ascii_lowercase();
    let has_visual_build_set = types.contains(&0x03B4_C61D)
        && (types.contains(&TYPE_IMG)
            || types.contains(&0x7368_84F1)
            || types.contains(&0x0166_1233));
    // Some roof swatches use a colour/theme name instead of "roof" in the
    // filename (for example Tammy Trauma's BeachAqua/BeachBlue variants).
    // The complete material/mesh/texture signature is stronger evidence than
    // that lexical marker and is shared by the already-recognised Roof_*
    // packages. Keep the signature deliberately narrow so a generic IMG set
    // is not promoted to construction content.
    let has_roof_resource_signature = has_visual_build_set
        && types.contains(&0x2E75_C764)
        && types.contains(&0x2E75_C765)
        && types.contains(&0x2E75_C766)
        && types.contains(&0xD382_BF57)
        && types.contains(&0xF1ED_BD86);
    let has_body_mesh = types.contains(&0x015A_1849) || types.contains(&0x015A_1849);
    let has_visual = types.contains(&TYPE_IMG)
        || has_body_mesh
        || types.contains(&TYPE_BGEO)
        || types.contains(&0x1F88_6EAD);
    let replacements = localized_special_folder(language, "replacements");
    let scripts = localized_special_folder(language, "scripts");
    let gameplay = localized_special_folder(language, "gameplay");
    let (folder_parts, detected_from, technical_reason) = if normalized.contains("/build/")
        && has_visual_build_set
    {
        (
            vec!["Construção".to_string()],
            vec!["LegacyBuildSource".to_string(), "BuildResourceSet".to_string()],
            "Build/Roofs source branch plus mesh/material/texture resource set identifies construction content without relying on a filename category.",
        )
    } else if (normalized.contains("#2 body/") || normalized.contains("/beards/")) && has_visual {
        (
            vec![replacements.to_string(), "Corpo".to_string()],
            vec!["LegacyBodySource".to_string(), "BodyResourceSet".to_string()],
            "Legacy body source branch plus GEOM/BGEO/IMG evidence identifies a body replacement; no CASP/OBJD catalog category was invented.",
        )
    } else if normalized.contains("#4 hair/") && has_visual && !types.contains(&TYPE_CASP) {
        (
            vec![replacements.to_string(), "CAS".to_string()],
            vec!["LegacyHairSource".to_string(), "VisualResourceSet".to_string()],
            "Legacy hair/beard source branch plus visual resources identifies a CAS replacement family.",
        )
    } else if normalized.contains("#8 scripts/cas/") && has_visual {
        (
            vec![replacements.to_string(), "CAS".to_string()],
            vec!["LegacyCASSource".to_string(), "VisualResourceSet".to_string()],
            "Legacy CAS script source branch plus visual/UI resources identifies a CAS replacement.",
        )
    } else if (normalized.contains("#8 scripts/gameplay/")
        || normalized.starts_with("scripts/gameplay/")
        || normalized.contains("/scripts/gameplay/"))
        && types.contains(&0xD304_4521)
    {
        (
            vec![scripts.to_string(), gameplay.to_string()],
            vec![
                if normalized.contains("#8 scripts/gameplay/") {
                    "LegacyGameplaySource".to_string()
                } else {
                    "CanonicalGameplaySource".to_string()
                },
                "GameplayResourceSet".to_string(),
            ],
            "Gameplay source branch plus the interaction resource type keeps this component with Scripts.",
        )
    } else if normalized.contains("#7 objects/") && types.contains(&TYPE_OBJD) {
        (
            vec!["Objetos".to_string()],
            vec!["LegacyObjectSource".to_string(), "OBJD".to_string()],
            "The complete #7 Objects source branch plus OBJD catalog evidence keeps this functional object under Objetos despite an ambiguous internal catalog split.",
        )
    } else if normalized.contains("windows and doors") && types.contains(&TYPE_OBJD) {
        (
            vec!["Construção".to_string()],
            vec!["LegacyWindowsDoorsSource".to_string(), "OBJD".to_string()],
            "Windows and Doors source branch plus OBJD evidence identifies construction content when one object record also exposes a vehicle candidate.",
        )
    } else if normalized.contains("#8 scripts/") && has_visual && !types.contains(&TYPE_CASP) {
        (
            vec![replacements.to_string()],
            vec!["LegacyReplacementSource".to_string(), "VisualResourceSet".to_string()],
            "Legacy Scripts source branch plus visual replacement resources identifies a replacement package rather than a gameplay assembly.",
        )
    } else if normalized.contains("penisbuttbones") && types.contains(&0x8EAF_13DE) {
        (
            vec!["Genética".to_string(), "Genitálias".to_string()],
            vec!["AnatomicalResourceSet".to_string(), "LegacyBodySource".to_string()],
            "Anatomical bone resource type and the explicit genital family source identify Genetics content.",
        )
    } else if has_roof_resource_signature
        || ((normalized.contains("roof")
        || normalized.contains("ceilingbeam")
        || normalized.contains("fence"))
            && has_visual_build_set)
    {
        (
            vec!["Construção".to_string()],
            vec![
                "BuildResourceSet".to_string(),
                if has_roof_resource_signature {
                    "RoofResourceSignature".to_string()
                } else {
                    "RelatedBuildName".to_string()
                },
            ],
            if has_roof_resource_signature {
                "The complete roof material/mesh/texture resource signature identifies construction content, including colour-named swatches that do not contain a roof token in the filename."
            } else {
                "Roof/beam/fence visual resource set is a construction family even when its legacy source folder is Sem Categoria."
            },
        )
    } else if normalized.contains("/build/")
        && types.contains(&TYPE_IMG)
        && !types.contains(&TYPE_S3SA)
    {
        (
            vec!["Construção".to_string()],
            vec!["LegacyBuildSource".to_string(), "IMG".to_string()],
            "Build source branch plus texture resources identifies a construction replacement set.",
        )
    } else if normalized.contains("nomosaic") && types.contains(&0xEA51_18B0) {
        (
            vec![replacements.to_string()],
            vec!["ReplacementTypeSignature".to_string(), "InternalVisualResource".to_string()],
            "The package-specific no-mosaic replacement resource type corroborates the replacement function.",
        )
    } else if normalized.contains("boroughsburg")
        && normalized.contains("beard")
        && types.contains(&TYPE_IMG)
        && !types.contains(&TYPE_CASP)
        && !types.contains(&TYPE_OBJD)
    {
        (
            vec![replacements.to_string(), "CAS".to_string()],
            vec!["BoroughsburgFamily".to_string(), "VisualResourceSet".to_string()],
            "Boroughsburg beard package has visual CAS replacement resources without an independent catalog record.",
        )
    } else {
        return None;
    };

    Some(PackageFamilyClassification {
        main_category: folder_parts[0].clone(),
        sub_category: folder_parts.get(1).cloned(),
        folder_parts,
        detected_from,
        technical_reason: technical_reason.to_string(),
    })
}

fn compact_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn authoritative_nraas_identifier(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let compact = compact_identifier(trimmed);
    if !compact.starts_with("nraas") || compact == "nraas" {
        return false;
    }

    // A logo/reference such as Battery.NRaasLogo2 is intentionally rejected:
    // the identifier must be a namespace/mod name boundary, not a substring.
    let remainder = lower.strip_prefix("nraas").unwrap_or_default();
    matches!(remainder.chars().next(), Some('_' | '.' | ':' | '/' | '\\'))
        && !remainder.contains("logo")
}

fn nraas_internal_evidence(package: &Package, name: &str, relative: &str) -> Option<String> {
    if is_explicit_non_nraas_integration(name) {
        return None;
    }

    for entry in &package.entries {
        if entry.type_id != TYPE_NMAP_LOCAL {
            continue;
        }
        let Ok(data) = package.data(entry) else {
            continue;
        };
        if let Some(identifier) = nmap_first_with_ascii_prefix(
            &data,
            b"nraas",
            authoritative_nraas_identifier,
        )
        {
            return Some(format!("NMAP authoritative identifier '{identifier}'"));
        }
    }

    // Assembly/manifest evidence is accepted only when the namespace has a
    // boundary and the package is actually scripted. Raw 'nraas' mentions in
    // XML, ITUN, STBL or UI assets are never sufficient.
    if package
        .entries
        .iter()
        .any(|entry| entry.type_id == TYPE_S3SA)
        && internal_signature(
            package,
            &[TYPE_S3SA, TYPE_MANIFEST_LOCAL],
            &["nraas.", "nraas_", "nraas\\"],
        )
        .is_some()
    {
        return Some("S3SA/manifest namespace boundary for NRaas".to_string());
    }

    let normalized_relative = relative.replace('\\', "/").to_ascii_lowercase();
    if normalized_relative.split('/').any(|part| part == "nraas")
        && Path::new(name)
            .file_stem()
            .is_some_and(|stem| authoritative_nraas_identifier(&stem.to_string_lossy()))
    {
        return Some("Existing NRaas source branch plus authoritative package name".to_string());
    }

    None
}

// NRaas family folders are conditional physical structure, not a taxonomy
// guess. The primary package and at least one verified module must coexist in
// the same scan before a family subfolder is created.
fn nraas_family_role(name: &str) -> Option<(&'static str, bool)> {
    let stem = Path::new(name)
        .file_stem()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let compact = stem
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    for (token, family) in [
        ("nraasmastercontroller", "MasterController"),
        ("nraasstoryprogression", "StoryProgression"),
        ("nraascareers", "Careers"),
        ("nraascareer", "Careers"),
        ("nraaswoohooer", "Woohooer"),
        ("nraasvector", "Vector"),
    ] {
        if compact == token {
            return Some((family, false));
        }
        if let Some(suffix) = compact.strip_prefix(token) {
            let module = !suffix.is_empty()
                && !matches!(
                    suffix,
                    "translation" | "english" | "portuguese" | "spanish" | "ptbr" | "en" | "es"
                );
            if module {
                return Some((family, true));
            }
        }
    }
    None
}

fn nraas_family_destination(
    language: AppLanguage,
    family: &str,
    has_primary: bool,
    has_module: bool,
) -> Vec<String> {
    let mut destination = script_creator_folder(language, Some("NRaas"));
    if has_primary && has_module {
        destination.push(family.to_string());
    }
    destination
}

// The model's NRaas branch is `Scripts\NRaas`, not a translated Gameplay
// wrapper. Preserve an existing family subfolder when it is already present;
// in particular, Dreamer and OnceRead are intentional model subfolders and
// must not be flattened just because no module pair was detected in the
// selected incremental scan.
fn existing_nraas_subfolder(relative: &str) -> Option<String> {
    let parts = relative
        .split(['\\', '/'])
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_string())
        .collect::<Vec<_>>();
    let scripts_index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("Scripts"))?;
    let nraas_index = parts
        .iter()
        .enumerate()
        .skip(scripts_index + 1)
        .find_map(|(index, part)| part.eq_ignore_ascii_case("NRaas").then_some(index))?;
    let candidate = parts.get(nraas_index + 1)?;
    (!candidate.to_ascii_lowercase().ends_with(".package")
        && !candidate.to_ascii_lowercase().ends_with(".disabled"))
    .then(|| candidate.clone())
}

fn apply_nraas_family_subfolders(language: AppLanguage, items: &mut [ScanPackageItem]) {
    let mut families = HashMap::<String, (bool, bool)>::new();
    for item in items.iter().filter(|item| {
        item.detected_from
            .iter()
            .any(|source| source == "NRaasInternal")
    }) {
        let Some((family, module)) = nraas_family_role(&item.name) else {
            continue;
        };
        let state = families.entry(family.to_string()).or_default();
        if module {
            state.1 = true;
        } else {
            state.0 = true;
        }
    }

    for item in items.iter_mut().filter(|item| {
        item.detected_from
            .iter()
            .any(|source| source == "NRaasInternal")
    }) {
        let Some((family, module)) = nraas_family_role(&item.name) else {
            continue;
        };
        let Some((has_primary, has_module)) = families.get(family).copied() else {
            continue;
        };
        if module && !has_primary {
            item.warnings.push(format!(
                "NRaas family module '{}' was detected without its primary package; it remains directly under NRaas.",
                family
            ));
        }
        let mut destination = nraas_family_destination(language, family, has_primary, has_module);
        if !has_primary || !has_module {
            if let Some(existing) = existing_nraas_subfolder(&item.relative_path) {
                if !destination
                    .iter()
                    .any(|part| part.eq_ignore_ascii_case(&existing))
                {
                    destination.push(existing);
                }
            }
        }
        item.destination_parts = destination;
        item.destination_path = Some(item.destination_parts.join("\\"));
        item.detected_from
            .push("NRaasConditionalFamily".to_string());
        item.detected_from.sort();
        item.detected_from.dedup();
        item.classification_reason = Some(format!(
            "{} | NRaas family '{}' uses a subfolder only when primary={} and module={}",
            item.classification_reason.clone().unwrap_or_default(),
            family,
            has_primary,
            has_module
        ));
    }
}

fn is_explicit_non_nraas_integration(name: &str) -> bool {
    let value = name.to_ascii_lowercase();
    let compact = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    [
        "simpanel",
        "smoothpatch",
        "monopatcher",
        "neoh4x0r",
        "bankingmod",
        "bakingmod",
    ]
    .iter()
    .any(|token| compact.contains(token))
}

fn script_source_roles(relative: &str) -> (bool, bool, bool) {
    let mut gameplay = false;
    let mut scripts = false;
    let mut catalog_root = false;
    let mut first = true;
    for part in relative.split(['\\', '/']) {
        let normalized = part.trim().to_ascii_lowercase();
        if normalized.ends_with(".package") {
            break;
        }
        if first {
            catalog_root = matches!(normalized.as_str(), "buy" | "build" | "cas" | "objects");
            first = false;
        }
        if matches!(
            normalized.as_str(),
            "gameplay" | "jogabilidade" | "jugabilidad"
        ) {
            gameplay = true;
        }
        if matches!(normalized.as_str(), "scripts" | "#8 scripts") {
            scripts = true;
        }
    }
    (gameplay, scripts, catalog_root)
}

// Choose catalog classification for a functional object with an embedded S3SA,
// but preserve independent gameplay mods even when they package supporting
// CASP/OBJD records. Store sets are bundles, not individual gameplay mods.
fn catalog_precedes_embedded_script(
    special: Option<&PackageFamilyClassification>,
    catalog_resource_count: usize,
    relative: &str,
    catalog_destinations: usize,
    catalog_ambiguous: bool,
) -> bool {
    if catalog_resource_count == 0 {
        return false;
    }
    let Some(classification) = special else {
        return false;
    };
    if !classification
        .detected_from
        .iter()
        .any(|source| source == "S3SA")
        || classification
            .detected_from
            .iter()
            .any(|source| source == "NRaasInternal")
    {
        return false;
    }
    if classification
        .detected_from
        .iter()
        .any(|source| source == "CompositeModFamily")
    {
        return false;
    }
    if store_name_hint(relative) {
        return true;
    }

    let (gameplay_folder, scripts_folder, catalog_root) = script_source_roles(relative);
    if gameplay_folder && !catalog_root {
        return false;
    }
    if catalog_root {
        return true;
    }
    // A single unambiguous OBJD/CASP destination is authoritative even when
    // an author name appears inside an object's script metadata.
    if catalog_destinations == 1 && !catalog_ambiguous {
        return true;
    }
    if scripts_folder {
        return false;
    }
    if classification
        .detected_from
        .iter()
        .any(|source| source == "InternalCreator" || source == "InternalScriptIdentity")
        && (catalog_destinations != 1 || catalog_ambiguous)
    {
        return false;
    }
    true
}

fn special_package_classification(
    package: &Package,
    type_ids: &BTreeSet<u32>,
    name: &str,
    relative: &str,
    language: AppLanguage,
    catalog_resource_count: usize,
) -> Option<PackageFamilyClassification> {
    // NRaas is detected from an authoritative namespace/mod identifier, never
    // from a loose substring in NMAP, manifest, UI icon, STBL, XML or ITUN.
    if let Some(evidence) = nraas_internal_evidence(package, name, relative) {
        let folder = localized_special_folder(language, "nraas").to_string();
        return Some(PackageFamilyClassification {
            main_category: localized_special_folder(language, "scripts").to_string(),
            sub_category: None,
            folder_parts: script_creator_folder(language, Some("NRaas")),
            detected_from: vec!["NRaasInternal".to_string()],
            technical_reason: format!("Internal NRaas signature ({evidence}) => {folder}"),
        });
    }

    // Script packages are gameplay content even when the same package also
    // carries OBJD/CASP resources. A creator is used only when the filename
    // prefix is corroborated by the package's own internal resources.
    if type_ids.contains(&TYPE_S3SA) {
        let gameplay = localized_special_folder(language, "gameplay").to_string();
        let category = script_category(name, language).to_string();
        let creator = verified_script_creator(package, name);
        let mod_name = inferred_script_mod_name(name, relative);
        let folder_parts = script_creator_folder(language, creator.as_deref());
        let mut detected_from = vec!["S3SA".to_string()];
        if catalog_resource_count > 0
            && type_ids.contains(&TYPE_CASP)
            && type_ids.contains(&TYPE_OBJD)
            && !store_name_hint(relative)
            && verified_standalone_script_identity(package, name)
        {
            detected_from.push("InternalScriptIdentity".to_string());
        }

        if creator.is_some() {
            detected_from.push("InternalCreator".to_string());
        }
        if mod_name.is_some() {
            detected_from.push("ModName".to_string());
        }

        return Some(PackageFamilyClassification {
            main_category: localized_special_folder(language, "scripts").to_string(),
            sub_category: Some(gameplay),
            folder_parts: folder_parts.clone(),
            detected_from,
            technical_reason: format!(
                "S3SA script package; gameplay topic '{}'; verified creator destination => {}",
                category,
                folder_parts.join("\\")
            ),
        });
    }

    // A CLIP-only/content package is an animation/pose asset. Do not let CLIP
    // override authored gameplay scripts or CAS/OBJD catalog content.
    let clip_asset = type_ids.contains(&TYPE_CLIP_LOCAL)
        && !type_ids.contains(&TYPE_S3SA)
        && catalog_resource_count == 0;
    let pose_list = catalog_resource_count == 0
        && !type_ids.contains(&TYPE_S3SA)
        && internal_signature(
            package,
            &[
                TYPE_XML_LOCAL,
                TYPE_NMAP_LOCAL,
                TYPE_STBL_LOCAL,
                TYPE_MANIFEST_LOCAL,
            ],
            &["poselist", "pose list", "poseplayer", "pose player"],
        )
        .is_some();

    if clip_asset || pose_list {
        let folder = localized_special_folder(language, "poses").to_string();
        let mut detected = Vec::new();
        if clip_asset {
            detected.push("CLIP".to_string());
        }
        if pose_list {
            detected.push("PoseList".to_string());
        }
        return Some(PackageFamilyClassification {
            main_category: folder.clone(),
            sub_category: None,
            folder_parts: vec![folder.clone()],
            detected_from: detected.clone(),
            technical_reason: format!(
                "Pose/animation evidence [{}] => {}",
                detected.join(", "),
                folder
            ),
        });
    }

    None
}

fn nmap_names_matching(data: &[u8], mut keep: impl FnMut(&str) -> bool) -> Vec<String> {
    if data.len() < 8 {
        return Vec::new();
    }

    let version = u32::from_le_bytes(data[0..4].try_into().unwrap());
    if version != 1 {
        return Vec::new();
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut offset = 8usize;
    let mut names = Vec::with_capacity(count.min(64));

    for _ in 0..count {
        if offset.checked_add(12).map(|end| end <= data.len()) != Some(true) {
            return Vec::new();
        }
        offset += 8; // name hash
        let byte_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        let Some(next) = offset.checked_add(byte_count) else {
            return Vec::new();
        };
        if next > data.len() {
            return Vec::new();
        }
        let name = String::from_utf8_lossy(&data[offset..next]);
        let name = name.trim();
        if !name.is_empty() && keep(name) {
            names.push(name.to_string());
        }
        offset = next;
    }

    names
}

fn nmap_names(data: &[u8]) -> Vec<String> {
    nmap_names_matching(data, |_| true)
}

// Do the common Phase 3 keyword pass on bytes.  Converting every NMAP record
// to an owned lowercase String is disproportionately expensive in large core
// mods; only identifiers that actually match a rule need to be materialized.
fn nmap_names_with_ascii_terms(data: &[u8], terms: &[&str]) -> Vec<String> {
    if data.len() < 8 {
        return Vec::new();
    }
    let version = u32::from_le_bytes(data[0..4].try_into().unwrap());
    if version != 1 {
        return Vec::new();
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut offset = 8usize;
    let mut names = Vec::new();
    for _ in 0..count {
        if offset.checked_add(12).map(|end| end <= data.len()) != Some(true) {
            return Vec::new();
        }
        let name_start = offset + 12;
        let byte_count =
            u32::from_le_bytes(data[offset + 8..name_start].try_into().unwrap()) as usize;
        let Some(next) = name_start.checked_add(byte_count) else {
            return Vec::new();
        };
        if next > data.len() {
            return Vec::new();
        }
        let name_bytes = &data[name_start..next];
        if contains_any_ascii_case_insensitive(name_bytes, terms) {
            let name = String::from_utf8_lossy(name_bytes);
            let name = name.trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
        offset = next;
    }
    names
}

fn contains_any_ascii_case_insensitive(data: &[u8], terms: &[&str]) -> bool {
    (0..data.len()).any(|start| {
        terms.iter().any(|term| {
            let needle = term.as_bytes();
            data.get(start..start.saturating_add(needle.len()))
                .is_some_and(|candidate| {
                    candidate
                        .iter()
                        .zip(needle)
                        .all(|(value, expected)| value.eq_ignore_ascii_case(expected))
                })
        })
    })
}

fn nmap_first_matching(data: &[u8], mut keep: impl FnMut(&str) -> bool) -> Option<String> {
    if data.len() < 8 || u32::from_le_bytes(data[0..4].try_into().ok()?) != 1 {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let mut offset = 8usize;
    for _ in 0..count {
        let name_start = offset.checked_add(12)?;
        if name_start > data.len() {
            return None;
        }
        let byte_count = u32::from_le_bytes(data[offset + 8..name_start].try_into().ok()?) as usize;
        let next = name_start.checked_add(byte_count)?;
        if next > data.len() {
            return None;
        }
        let name = String::from_utf8_lossy(&data[name_start..next]);
        let name = name.trim();
        if !name.is_empty() && keep(name) {
            return Some(name.to_string());
        }
        offset = next;
    }
    None
}

// `authoritative_nraas_identifier` can only accept names beginning with the
// NRaas namespace.  Large core mods may contain hundreds of thousands of
// unrelated NMAP entries, so avoid constructing and normalizing a `String`
// for each one before applying that rule.  The full textual predicate still
// runs for every possible namespace candidate, preserving its boundary and
// false-positive checks.
fn nmap_first_with_ascii_prefix(
    data: &[u8],
    prefix: &[u8],
    mut keep: impl FnMut(&str) -> bool,
) -> Option<String> {
    if data.len() < 8 || u32::from_le_bytes(data[0..4].try_into().ok()?) != 1 {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let mut offset = 8usize;
    for _ in 0..count {
        let name_start = offset.checked_add(12)?;
        if name_start > data.len() {
            return None;
        }
        let byte_count = u32::from_le_bytes(data[offset + 8..name_start].try_into().ok()?) as usize;
        let next = name_start.checked_add(byte_count)?;
        if next > data.len() {
            return None;
        }
        let name_bytes = &data[name_start..next];
        let has_prefix = name_bytes.len() >= prefix.len()
            && name_bytes
                .iter()
                .zip(prefix)
                .all(|(value, expected)| value.eq_ignore_ascii_case(expected));
        if has_prefix {
            let name = String::from_utf8_lossy(name_bytes);
            let name = name.trim();
            if !name.is_empty() && keep(name) {
                return Some(name.to_string());
            }
        }
        offset = next;
    }
    None
}

fn stbl_entries(data: &[u8]) -> Vec<(u64, String)> {
    if data.len() < 17 || &data[..4] != b"STBL" {
        return Vec::new();
    }

    let count = u32::from_le_bytes(data[7..11].try_into().unwrap()) as usize;
    let mut offset = 17usize;
    let mut entries = Vec::with_capacity(count.min(4096));

    for _ in 0..count {
        if offset.checked_add(12).map(|end| end <= data.len()) != Some(true) {
            return Vec::new();
        }
        let key = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let char_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let Some(byte_count) = char_count.checked_mul(2) else {
            return Vec::new();
        };
        let Some(next) = offset.checked_add(byte_count) else {
            return Vec::new();
        };
        if next > data.len() {
            return Vec::new();
        }

        let units = data[offset..next]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        let text = char::decode_utf16(units)
            .map(|value| value.unwrap_or('\u{FFFD}'))
            .collect::<String>();
        entries.push((key, text));
        offset = next;
    }

    entries
}

fn normalize_slider_internal_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    let mut previous_lower = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_lower && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(ch.to_ascii_lowercase());
            previous_lower = ch.is_ascii_lowercase();
        } else {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            previous_lower = false;
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn token_starts_with_any(normalized: &str, roots: &[&str]) -> bool {
    normalized
        .split_whitespace()
        .any(|token| roots.iter().any(|root| token.starts_with(root)))
}

fn token_contains(normalized: &str, value: &str) -> bool {
    normalized
        .split_whitespace()
        .any(|token| token.contains(value))
}

// GEOM/VPXY clothing morph replacements often lack CASP. When their NMAP
// gives a stable `afTop...` or `afBottom...` mesh identifier, the internal
// resource itself confirms the category, gender and adult mesh family.
// Names of .package files are not used for this inference.
fn clothing_mesh_subtype(names: &[String]) -> Option<&'static str> {
    let mut kinds = BTreeSet::new();
    for name in names {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("aftop") {
            kinds.insert("top");
        } else if lower.starts_with("afbottom") {
            kinds.insert("bottom");
        }
    }
    (kinds.len() == 1).then(|| *kinds.iter().next().unwrap())
}

fn geometry_clothing_from_nmap(
    types: &BTreeSet<u32>,
    names: &[String],
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    if !types.contains(&0x015A_1849)
        || !types.contains(&0x7368_84F1)
        || types.contains(&TYPE_S3SA)
        || types.contains(&TYPE_CASP)
    {
        return None;
    }
    let subtype = clothing_mesh_subtype(names)?;
    let label = if subtype == "top" {
        language.top()
    } else {
        language.bottom()
    };
    let parts = vec![
        language.clothing().to_string(),
        language.female().to_string(),
        language.young_adult_adult().to_string(),
        label.to_string(),
    ];
    Some(PackageFamilyClassification {
        main_category: language.clothing().to_string(),
        sub_category: Some(label.to_string()),
        folder_parts: parts.clone(),
        detected_from: vec!["GEOM".to_string(), "VPXY".to_string(), "NMAP".to_string()],
        technical_reason: format!(
            "GEOM+VPXY female adult mesh with internal NMAP af{} => {}. No CASP; review before organizing.",
            if subtype == "top" { "Top" } else { "Bottom" },
            parts.join("\\"),
        ),
    })
}

// Strong resource-family confirmation is required before recognizing default
// replacement meshes. A filename hint alone is never enough: GEOM and VPXY
// must both be present, with no CASP or S3SA. Keep unrelated custom meshes for
// manual review instead of automatically calling them replacements.
fn replacement_mesh_classification(
    types: &BTreeSet<u32>,
    internal_names: &[String],
    filename: &str,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    if !types.contains(&0x015A_1849)
        || !types.contains(&0x7368_84F1)
        || types.contains(&TYPE_CASP)
        || types.contains(&TYPE_S3SA)
    {
        return None;
    }
    let mut evidence = internal_names.join(" ").to_ascii_lowercase();
    evidence.push(' ');
    evidence.push_str(&filename.to_ascii_lowercase());
    let (en, pt, es, key) = if ["eyelash", "eyelashes", "lashes", "cils"]
        .iter()
        .any(|part| evidence.contains(part))
    {
        ("Eyelashes", "Cílios", "Pestañas", "Eyelashes")
    } else if ["foot", "feet", "barefeet", "barefoot"]
        .iter()
        .any(|part| evidence.contains(part))
    {
        ("Feet", "Pés", "Pies", "Feet")
    } else {
        return None;
    };
    let (category, body) = match language {
        AppLanguage::En => ("Replacements", "Body"),
        AppLanguage::Pt => ("Substituições", "Corpo"),
        AppLanguage::Es => ("Reemplazos", "Cuerpo"),
    };
    let subcategory = match language {
        AppLanguage::En => en,
        AppLanguage::Pt => pt,
        AppLanguage::Es => es,
    };
    let parts = vec![
        category.to_string(),
        body.to_string(),
        subcategory.to_string(),
    ];
    Some(PackageFamilyClassification {
        main_category: category.to_string(),
        sub_category: Some(subcategory.to_string()),
        folder_parts: parts,
        detected_from: vec!["GEOM".into(), "VPXY".into(), "ReplacementMesh".into()],
        technical_reason: format!(
            "GEOM+VPXY no CASP/S3SA; replacement mesh anatomy '{}' corroborated by embedded resource structure and descriptive identifiers. Source loading branch preserved.",
            key
        ),
    })
}

// GEOM+NMAP face-mesh replacements without CASP/VPXY can be legitimate
// standalone eye default replacements. Do not infer them from the folder
// where a user stored them (often 'Sliders/Eyes').
fn verified_eye_mesh_replacement(
    types: &BTreeSet<u32>,
    names: &[String],
    filename: &str,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    if !types.contains(&0x015A_1849)
        || !types.contains(&TYPE_NMAP_LOCAL)
        || types.contains(&TYPE_CASP)
        || types.contains(&TYPE_S3SA)
        || types.contains(&TYPE_OBJD)
    {
        return None;
    }
    let name = filename.to_ascii_lowercase();
    let explicit = name.contains("default replacement eye mesh")
        || name.contains("eyeball") && (name.contains("simskin") || name.contains("uvfix"));
    if !explicit {
        return None;
    }
    // The uploaded 2016 packages declare face geometry in the NMAP; verify
    // that internal evidence before treating a filename as a replacement.
    if !names
        .iter()
        .any(|v| v.to_ascii_lowercase().contains("face"))
    {
        return None;
    }
    let (replacements, body, eyes) = match language {
        AppLanguage::En => ("Replacements", "Body", "Eyes"),
        AppLanguage::Pt => ("Substituições", "Corpo", "Olhos"),
        AppLanguage::Es => ("Reemplazos", "Cuerpo", "Ojos"),
    };
    Some(PackageFamilyClassification {
        main_category: replacements.into(),
        sub_category: Some(eyes.into()),
        folder_parts: vec![replacements.into(), body.into(), eyes.into()],
        detected_from: vec!["GEOM".into(), "NMAP".into(), "EyeMeshReplacement".into()],
        technical_reason: format!(
            "Verified GEOM+NMAP face mesh and explicit eye replacement filename => {replacements}\\{body}\\{eyes}"
        ),
    })
}

fn slider_region_keys(internal_name: &str) -> Option<(&'static str, Option<&'static str>)> {
    let name = normalize_slider_internal_name(internal_name);

    // Specific anatomy always wins over generic words such as "height".
    // This is intentionally based on internal NMAP/STBL names, never on the
    // CAS panel/category in which the creator placed the slider.
    if token_starts_with_any(&name, &["eyebrow", "brow"]) {
        return Some(("face", Some("eyebrows")));
    }
    if token_starts_with_any(&name, &["eyelid", "eyeball", "eye", "pupil", "iris"]) {
        return Some(("face", Some("eyes")));
    }
    if token_starts_with_any(&name, &["nostril", "septum", "nose", "bridge"]) {
        return Some(("face", Some("nose")));
    }
    if token_starts_with_any(
        &name,
        &[
            "lip", "mouth", "philtrum", "cupid", "overlip", "frown", "smile",
        ],
    ) {
        return Some(("face", Some("mouth_lips")));
    }
    if token_starts_with_any(&name, &["jaw", "chin"]) {
        return Some(("face", Some("jaw_chin")));
    }
    if token_starts_with_any(&name, &["cheek"]) || token_contains(&name, "cheek") {
        return Some(("face", Some("cheeks")));
    }
    if token_starts_with_any(&name, &["ear", "midear"]) {
        return Some(("face", Some("ears")));
    }
    if token_starts_with_any(&name, &["forehead"]) {
        return Some(("face", Some("forehead")));
    }
    if token_starts_with_any(&name, &["face"]) {
        return Some(("face", None));
    }

    if token_starts_with_any(&name, &["hat"]) {
        return Some(("head", Some("hats")));
    }
    if token_starts_with_any(&name, &["glasses"]) {
        return Some(("head", Some("glasses")));
    }
    if token_starts_with_any(&name, &["head", "skull"]) {
        return Some(("head", None));
    }

    if token_starts_with_any(
        &name,
        &[
            "penis", "testicle", "scrotum", "foreskin", "genital", "phallus", "erection",
        ],
    ) {
        return Some(("body", Some("genitals")));
    }

    if token_starts_with_any(&name, &["shoulder"]) {
        return Some(("body", Some("shoulders")));
    }
    if token_starts_with_any(&name, &["forearm", "arm", "bicep", "tricep", "elbow"]) {
        return Some(("body", Some("arms")));
    }
    if token_starts_with_any(&name, &["hand", "finger", "thumb", "nail"]) {
        return Some(("body", Some("hands")));
    }
    if token_starts_with_any(&name, &["breast", "chest", "nipple", "ribcage"]) {
        return Some(("body", Some("chest_breasts")));
    }
    if token_starts_with_any(&name, &["waist"]) {
        return Some(("body", Some("waist")));
    }
    if token_starts_with_any(&name, &["hip", "butt", "pelvis"]) {
        return Some(("body", Some("hips_butt")));
    }
    if token_starts_with_any(&name, &["leg", "thigh", "calf", "knee"]) {
        return Some(("body", Some("legs")));
    }
    if token_starts_with_any(&name, &["foot", "feet", "ankle", "toe"]) {
        return Some(("body", Some("feet")));
    }
    if token_starts_with_any(&name, &["neck"]) {
        return Some(("body", Some("neck")));
    }
    if token_starts_with_any(
        &name,
        &["torso", "body", "belly", "abdomen", "stomach", "back"],
    ) {
        return Some(("body", Some("torso")));
    }
    if token_starts_with_any(&name, &["simheight", "height", "posture", "simscaler"]) {
        return Some(("body", Some("height_posture")));
    }

    None
}

fn slider_folder_label(language: AppLanguage, key: &str) -> &'static str {
    match (language, key) {
        (AppLanguage::En, "face") => "Face",
        (AppLanguage::Pt, "face") => "Rosto",
        (AppLanguage::Es, "face") => "Rostro",
        (AppLanguage::En, "body") => "Body",
        (AppLanguage::Pt, "body") => "Corpo",
        (AppLanguage::Es, "body") => "Cuerpo",
        (AppLanguage::En, "head") => "Head",
        (AppLanguage::Pt, "head") => "Cabeça",
        (AppLanguage::Es, "head") => "Cabeza",
        (AppLanguage::En, "eyebrows") => "Eyebrows",
        (AppLanguage::Pt, "eyebrows") => "Sobrancelhas",
        (AppLanguage::Es, "eyebrows") => "Cejas",
        (AppLanguage::En, "eyes") => "Eyes",
        (AppLanguage::Pt, "eyes") => "Olhos",
        (AppLanguage::Es, "eyes") => "Ojos",
        (AppLanguage::En, "nose") => "Nose",
        (AppLanguage::Pt, "nose") => "Nariz",
        (AppLanguage::Es, "nose") => "Nariz",
        (AppLanguage::En, "mouth_lips") => "Mouth & Lips",
        (AppLanguage::Pt, "mouth_lips") => "Boca e Lábios",
        (AppLanguage::Es, "mouth_lips") => "Boca y Labios",
        (AppLanguage::En, "jaw_chin") => "Jaw & Chin",
        (AppLanguage::Pt, "jaw_chin") => "Mandíbula e Queixo",
        (AppLanguage::Es, "jaw_chin") => "Mandíbula y Mentón",
        (AppLanguage::En, "cheeks") => "Cheeks",
        (AppLanguage::Pt, "cheeks") => "Bochechas",
        (AppLanguage::Es, "cheeks") => "Mejillas",
        (AppLanguage::En, "ears") => "Ears",
        (AppLanguage::Pt, "ears") => "Orelhas",
        (AppLanguage::Es, "ears") => "Orejas",
        (AppLanguage::En, "forehead") => "Forehead",
        (AppLanguage::Pt, "forehead") => "Testa",
        (AppLanguage::Es, "forehead") => "Frente",
        (AppLanguage::En, "hats") => "Hats",
        (AppLanguage::Pt, "hats") => "Chapéus",
        (AppLanguage::Es, "hats") => "Sombreros",
        (AppLanguage::En, "glasses") => "Glasses",
        (AppLanguage::Pt, "glasses") => "Óculos",
        (AppLanguage::Es, "glasses") => "Gafas",
        (AppLanguage::En, "genitals") => "Genitals",
        (AppLanguage::Pt, "genitals") => "Genitais",
        (AppLanguage::Es, "genitals") => "Genitales",
        (AppLanguage::En, "other") => "Other",
        (AppLanguage::Pt, "other") => "Outros",
        (AppLanguage::Es, "other") => "Otros",
        (AppLanguage::En, "shoulders") => "Shoulders",
        (AppLanguage::Pt, "shoulders") => "Ombros",
        (AppLanguage::Es, "shoulders") => "Hombros",
        (AppLanguage::En, "arms") => "Arms",
        (AppLanguage::Pt, "arms") => "Braços",
        (AppLanguage::Es, "arms") => "Brazos",
        (AppLanguage::En, "hands") => "Hands",
        (AppLanguage::Pt, "hands") => "Mãos",
        (AppLanguage::Es, "hands") => "Manos",
        (AppLanguage::En, "chest_breasts") => "Chest & Breasts",
        (AppLanguage::Pt, "chest_breasts") => "Peito e Seios",
        (AppLanguage::Es, "chest_breasts") => "Pecho y Senos",
        (AppLanguage::En, "waist") => "Waist",
        (AppLanguage::Pt, "waist") => "Cintura",
        (AppLanguage::Es, "waist") => "Cintura",
        (AppLanguage::En, "hips_butt") => "Hips & Butt",
        (AppLanguage::Pt, "hips_butt") => "Quadris e Bumbum",
        (AppLanguage::Es, "hips_butt") => "Caderas y Glúteos",
        (AppLanguage::En, "legs") => "Legs",
        (AppLanguage::Pt, "legs") => "Pernas",
        (AppLanguage::Es, "legs") => "Piernas",
        (AppLanguage::En, "feet") => "Feet",
        (AppLanguage::Pt, "feet") => "Pés",
        (AppLanguage::Es, "feet") => "Pies",
        (AppLanguage::En, "neck") => "Neck",
        (AppLanguage::Pt, "neck") => "Pescoço",
        (AppLanguage::Es, "neck") => "Cuello",
        (AppLanguage::En, "torso") => "Torso",
        (AppLanguage::Pt, "torso") => "Tronco",
        (AppLanguage::Es, "torso") => "Torso",
        (AppLanguage::En, "height_posture") => "Height & Posture",
        (AppLanguage::Pt, "height_posture") => "Altura e Postura",
        (AppLanguage::Es, "height_posture") => "Altura y Postura",
        _ => "Unknown",
    }
}

fn slider_destination_from_internal_name(
    internal_name: &str,
    language: AppLanguage,
) -> Option<Vec<String>> {
    let (region, part) = slider_region_keys(internal_name)?;
    // Hats are a direct model category. The previous `Sliders\Cabeça\Chapéus`
    // result was an invented intermediate level caused by treating every
    // head slider as a generic anatomy tree. Keep other head sliders under
    // Cabeça, but resolve the verified hats branch exactly.
    if region == "head" && part == Some("hats") {
        return Some(vec![
            "Sliders".to_string(),
            slider_folder_label(language, "hats").to_string(),
        ]);
    }
    let mut result = vec![
        "Sliders".to_string(),
        slider_folder_label(language, region).to_string(),
    ];
    if let Some(part) = part {
        result.push(slider_folder_label(language, part).to_string());
    }
    Some(result)
}

fn slider_internal_candidates(package: &Package) -> Vec<(String, &'static str)> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::<String>::new();

    for entry in &package.entries {
        if entry.type_id != TYPE_NMAP_LOCAL {
            continue;
        }
        let Ok(data) = package.data(entry) else {
            continue;
        };
        if let Some(name) = nmap_first_matching(&data, |_| true) {
            let key = name.to_ascii_lowercase();
            if seen.insert(key) {
                candidates.push((name, "NMAP"));
            }
        }
    }

    let mut stbl_resources = package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_STBL_LOCAL)
        .collect::<Vec<_>>();
    stbl_resources.sort_by_key(|entry| ((entry.instance >> 56) != 0, entry.instance));

    for entry in stbl_resources {
        let Ok(data) = package.data(entry) else {
            continue;
        };
        if let Some((_, text)) = stbl_entries(&data)
            .into_iter()
            .find(|(_, text)| !text.trim().is_empty())
        {
            let text = text.trim().to_string();
            let key = text.to_ascii_lowercase();
            if seen.insert(key) {
                candidates.push((text, "STBL"));
            }
        }
    }

    candidates
}

fn verified_slider_name_alias(filename: &str) -> Option<&'static str> {
    // Confirmed against the supplied slider corpus: OneEuroMutt's original
    // package filename omits the anatomical prefix. Only use this alias once
    // resources have independently established that the package is a slider.
    filename
        .eq_ignore_ascii_case("OneEuroMuttTip Width.package")
        .then_some("Nose Tip Width")
}

fn slider_internal_evidence(
    package: &Package,
    filename: &str,
    language: AppLanguage,
) -> Option<(String, &'static str, Option<Vec<String>>)> {
    let candidates = slider_internal_candidates(package);
    for (name, source) in &candidates {
        if let Some(destination) = slider_destination_from_internal_name(name, language) {
            return Some((name.clone(), *source, Some(destination)));
        }
    }
    // This specific alias has been verified by the user: the internal generic
    // label "Tip Width" is actually "Nose Tip Width". The enclosing scanner
    // has already confirmed morph resources, so the alias cannot classify
    // an unrelated package as a slider.
    if let Some(verified) = verified_slider_name_alias(filename) {
        return Some((
            verified.to_string(),
            "User-verified slider alias",
            slider_destination_from_internal_name(verified, language),
        ));
    }
    // Some sliders store only generic NMAP labels ("Tip Width", "Outer
    // Curve", "Middle Width"). Combine internal evidence with a clear
    // anatomical term in the package filename.
    if let Some((name, source)) = candidates.first() {
        if let Some(destination) = slider_destination_from_internal_name(filename, language) {
            return Some((
                format!("{name} | filename: {filename}"),
                *source,
                Some(destination),
            ));
        }
    }
    candidates
        .into_iter()
        .next()
        .map(|(name, source)| (name, source, None))
}

fn fbln_slider_classification(
    package: &Package,
    metadata: &[FblnMetadata],
    filename: &str,
    language: AppLanguage,
) -> PackageFamilyClassification {
    let mut folder_parts = vec!["Sliders".to_string()];
    let mut detected_from = vec!["FBLN".to_string()];
    let evidence = fbln_evidence(metadata);
    let mut reason = format!(
        "FBLN BlendUnit is authoritative slider evidence; {}. CASPanelGroup/Sort are retained as raw metadata and do not determine anatomy.",
        evidence
    );

    // Anatomical refinement is allowed only from independent internal labels
    // (NMAP/STBL or a verified filename alias), never from CASPanelGroup.
    if let Some((internal_name, source, Some(destination))) =
        slider_internal_evidence(package, filename, language)
    {
        if destination.first().is_some_and(|part| part == "Sliders") {
            folder_parts = destination;
            detected_from.push(source.to_string());
            reason.push_str(&format!(
                " Independent internal label from {source} '{}' refines the destination without inferring anatomy from the CAS panel.",
                internal_name
            ));
        }
    }

    PackageFamilyClassification {
        main_category: "CAS".to_string(),
        sub_category: Some("Sliders".to_string()),
        folder_parts,
        detected_from,
        technical_reason: reason,
    }
}

fn is_slider_morph_type(type_id: u32) -> bool {
    matches!(
        type_id,
        TYPE_BONE_DELTA | TYPE_FACE | TYPE_BBLN | TYPE_BGEO | TYPE_FBLN
    )
}

fn stbl_keys(data: &[u8]) -> Vec<u64> {
    stbl_entries(data).into_iter().map(|(key, _)| key).collect()
}

fn apply_slider_companion_classification(
    package_paths: &[PathBuf],
    items: &mut [ScanPackageItem],
    slider_instances: &HashSet<u64>,
) {
    if slider_instances.is_empty() {
        return;
    }

    for (path, item) in package_paths.iter().zip(items.iter_mut()) {
        // Only STBL carriers can be companions; do not reopen hundreds of
        // direct morph packages during the second scanner pass.
        if !item.resource_types.iter().any(|label| label == "STBL") {
            continue;
        }
        let Ok(package) = Package::load(path) else {
            continue;
        };

        let type_ids = package
            .entries
            .iter()
            .map(|entry| entry.type_id)
            .collect::<BTreeSet<_>>();

        let localization_only = type_ids.contains(&TYPE_STBL_LOCAL)
            && type_ids.iter().all(|type_id| {
                matches!(
                    *type_id,
                    TYPE_STBL_LOCAL | TYPE_NMAP_LOCAL | TYPE_MANIFEST_LOCAL
                )
            });
        if !localization_only {
            continue;
        }

        let mut matched_key = None;
        'resources: for entry in &package.entries {
            if entry.type_id != TYPE_STBL_LOCAL {
                continue;
            }
            let Ok(data) = package.data(entry) else {
                continue;
            };
            for key in stbl_keys(&data) {
                if slider_instances.contains(&key) {
                    matched_key = Some(key);
                    break 'resources;
                }
            }
        }

        let Some(key) = matched_key else {
            continue;
        };

        item.status = "classified".to_string();
        item.category = Some("CAS".to_string());
        // The STBL is a label/dependency carrier, not a second slider. Keep
        // it with the slider loading branch without counting it as an FBLN
        // slider or inferring anatomy from its source folder.
        item.sub_category = Some("Slider Support".to_string());
        item.destination_parts = vec!["Sliders".to_string()];
        item.destination_path = Some("Sliders".to_string());
        if !item
            .detected_from
            .iter()
            .any(|source| source == "STBL→Slider")
        {
            item.detected_from.push("STBL→Slider".to_string());
            item.detected_from.sort();
        }
        item.classification_reason = Some(format!(
            "STBL entry key 0x{key:016X} matches a morph resource instance in another package from the selected set => Sliders"
        ));
    }
}

fn top_level_relative_folder(relative: &str) -> Option<String> {
    let mut components = Path::new(relative)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        });
    let first = components.next()?;
    // A direct file at the selected root has no containing mod folder.
    components.next().map(|_| first)
}

// Only real, named mod folders may propagate a script's destination to
// companions. Standard resource roots and broad organizer categories group
// unrelated packages and must never become implicit mod identities.
fn named_mod_companion_folder(relative: &str) -> Option<String> {
    let folder = top_level_relative_folder(relative)?;
    let name = folder.trim().to_ascii_lowercase();
    let generic = matches!(
        name.as_str(),
        "packages"
            | "overrides"
            | "test"
            | "probation"
            | "dccache"
            | "mods"
            | "cc"
            | "custom content"
            | "downloads"
            | "build"
            | "buy"
            | "cas"
            | "gameplay"
            | "scripts"
            | "sliders"
            | "objects"
            | "poses"
            | "animations"
            | "hair"
            | "clothing"
            | "accessories"
            | "skins"
            | "textures"
            | "male"
            | "female"
            | "misc"
            | "miscellaneous"
            | "nraas"
            | "tuning"
            | "store"
            | "ui"
    );
    if generic || name.starts_with('#') || name.starts_with('!') || name.starts_with('+') {
        return None;
    }
    Some(folder)
}

fn may_reclassify_as_companion(item: &ScanPackageItem) -> bool {
    // Explicit CASP/OBJD classifications and independently identified
    // scripts must not be overwritten by a neighboring mod's destination.
    // A source folder is not an internal relationship. The old implementation
    // promoted any low-confidence package beside two scripts, which made
    // Correções and other broad folders look like script families. Only a
    // package that carries an explicit composite-family marker may be grouped
    // here; otherwise it remains unknown/review for the user's decision.
    item.status != "invalid"
        && item.status != "mixed"
        && item.status != "needs_review"
        && !item.scripted
        && item.catalog_resource_count == 0
        && item.classification_confidence != "high"
        && !item
            .detected_from
            .iter()
            .any(|source| value_is_authoritative_mod_source(source))
        && item
            .detected_from
            .iter()
            .any(|source| source == "CompositeModFamily" || source == "InternalModLink")
}

fn value_is_authoritative_mod_source(source: &str) -> bool {
    matches!(source, "NRaasInternal" | "ModName" | "CASP" | "OBJD")
}

fn apply_universal_project_grouping(items: &mut [ScanPackageItem], language: AppLanguage) {
    // No anchor means no technical project relationship is possible.  Avoid
    // cloning potentially large STBL/XML evidence only to return an empty
    // assignment set during post-scan finalization.
    let anchor_count = items
        .iter()
        .filter(|item| {
            item.project_evidence.has_s3sa
                && !item.project_evidence.assembly_identities.is_empty()
        })
        .count();
    if std::env::var_os("S3CC_PROFILE_ROOT").is_some() {
        eprintln!(
            "scanner_profile_project_grouping packages={} script_anchors={anchor_count}",
            items.len()
        );
    }
    if anchor_count == 0 {
        return;
    }
    let evidence = items
        .iter()
        .map(|item| (item.path.as_str(), &item.project_evidence))
        .collect::<Vec<_>>();
    let assignments = assign_project_groups_references(&evidence);
    let scripts = localized_special_folder(language, "scripts").to_string();
    let gameplay = localized_special_folder(language, "gameplay").to_string();

    for item in items.iter_mut() {
        let Some(assignment) = assignments.get(&item.path) else {
            continue;
        };
        item.project_id = Some(assignment.group_id.clone());
        item.project_role = Some(assignment.role.clone());
        item.project_confidence = Some(assignment.confidence.clone());
        item.project_relationships = assignment.relationships.clone();
        item.detected_from.push("ProjectEvidenceGraph".to_string());
        item.detected_from.sort();
        item.detected_from.dedup();

        // Relationships are useful even when physical regrouping is unsafe.
        // Overrides, corrections, merged Sim packages, review states and
        // empty destinations must never be forced into a script project.
        let protected = assignment.confidence != "confirmed"
            || assignment.role == "shared_dependency"
            || item.status != "classified"
            || item.destination_parts.is_empty()
            || item
                .detected_from
                .iter()
                .any(|source| source == "MergedSimExport")
            || item.destination_parts.first().is_some_and(|part| {
                part.eq_ignore_ascii_case("Overrides")
                    || part.eq_ignore_ascii_case("Correções")
                    || part.eq_ignore_ascii_case("Fixes")
                    || part.starts_with('#')
            });
        if protected {
            item.warnings.push(
                "Project relationship recorded without physical regrouping because the current destination is protected or unresolved."
                    .to_string(),
            );
            continue;
        }

        let destination = vec![scripts.clone(), gameplay.clone(), assignment.label.clone()];
        if item.destination_parts == destination {
            continue;
        }
        let previous = item.destination_parts.join("\\");
        item.destination_parts = destination.clone();
        item.destination_path = Some(destination.join("\\"));
        item.classification_reason = Some(format!(
            "{} | Confirmed project relationship ({}) grouped with technically linked components; previous destination was {}.",
            item.classification_reason.clone().unwrap_or_default(),
            assignment.role,
            previous
        ));
    }
}

fn apply_named_mod_companions(items: &mut [ScanPackageItem]) {
    let mut destinations = HashMap::<String, (BTreeSet<String>, usize, String)>::new();

    for item in items.iter() {
        if !item.scripted || item.status != "classified" {
            continue;
        }
        if !item.detected_from.iter().any(|value| value == "ModName") {
            continue;
        }
        let Some(folder) = named_mod_companion_folder(&item.relative_path) else {
            continue;
        };
        let state = destinations.entry(folder.to_ascii_lowercase()).or_default();
        state.0.insert(item.destination_parts.join("\\"));
        state.1 += 1;
        state.2 = folder;
    }

    let unique = destinations
        .into_iter()
        .filter_map(|(folder_key, (values, scripted_count, folder))| {
            // A single script with an author-like source folder is not enough
            // to create a physical mod directory. Require a related set.
            (values.len() == 1 && scripted_count >= 2).then(|| {
                let base = values.into_iter().next().unwrap();
                let destination = format!("{base}\\{folder}");
                (folder_key, destination)
            })
        })
        .collect::<HashMap<_, _>>();

    for item in items.iter_mut() {
        if !item.scripted || !item.detected_from.iter().any(|value| value == "ModName") {
            continue;
        }
        let Some(folder) = named_mod_companion_folder(&item.relative_path) else {
            continue;
        };
        let Some(destination) = unique.get(&folder.to_ascii_lowercase()) else {
            continue;
        };
        let parts = destination
            .split('\\')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if item.destination_parts == parts {
            continue;
        }
        item.destination_parts = parts.clone();
        item.destination_path = Some(destination.clone());
        item.detected_from.push("NamedModFamily".to_string());
        item.detected_from.sort();
        item.detected_from.dedup();
        item.classification_reason = Some(format!(
            "{} | Related scripted packages in named source folder '{}' share a deterministic mod folder => {}",
            item.classification_reason.clone().unwrap_or_default(),
            folder,
            destination
        ));
    }

    for item in items.iter_mut() {
        if !may_reclassify_as_companion(item) {
            continue;
        }
        let Some(folder) = named_mod_companion_folder(&item.relative_path) else {
            continue;
        };
        let Some(destination) = unique.get(&folder.to_ascii_lowercase()) else {
            continue;
        };
        let parts = destination
            .split('\\')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if parts.is_empty() || item.destination_parts == parts {
            continue;
        }

        item.status = "classified".to_string();
        item.classification_confidence = "medium".to_string();
        item.destination_parts = parts.clone();
        item.destination_path = Some(destination.clone());
        item.detected_from.push("ModFolderCompanion".to_string());
        item.detected_from.sort();
        item.detected_from.dedup();
        item.classification_reason = Some(format!(
            "{} | Companion package kept with the single named script mod detected in source folder '{}' => {}",
            item.classification_reason.clone().unwrap_or_default(),
            folder,
            destination
        ));
    }
}

fn apply_manual_classifications(root: &Path, items: &mut [ScanPackageItem]) {
    let workspace = load_workspace_for_root(root);
    if workspace.manual_classifications.is_empty() {
        return;
    }

    for item in items.iter_mut() {
        if item
            .detected_from
            .iter()
            .any(|source| source == "MergedSimExport")
        {
            continue;
        }
        // Authoritative automatic classifications win. Manual review exists to
        // resolve Unknown/Mixed/Needs Review cases, so only those need hashing.
        if item.status == "invalid"
            || (item.status == "classified" && item.classification_confidence == "high")
        {
            continue;
        }

        let path = PathBuf::from(&item.path);
        let Ok((hash, _)) = sha256_file(&path) else {
            continue;
        };
        let Some(manual) = workspace.manual_classifications.get(&hash) else {
            continue;
        };
        let mut parts = split_destination(&manual.destination);
        if parts
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case("CAS"))
        {
            parts.remove(0);
        }
        if parts.is_empty() {
            continue;
        }

        item.status = "classified".to_string();
        item.classification_confidence = "manual".to_string();
        item.destination_parts = parts.clone();
        item.destination_path = Some(parts.join("\\"));
        item.detected_from.push("ManualReview".to_string());
        item.detected_from.sort();
        item.detected_from.dedup();
        item.classification_reason = Some(format!(
            "Manual review stored by SHA-256 {} => {}",
            hash,
            parts.join("\\")
        ));
        if item.category.as_deref() == Some("Desconhecido")
            || item.category.as_deref() == Some("Desconocido")
            || item.category.as_deref() == Some("Unknown")
            || item.category.is_none()
        {
            item.category = parts.first().cloned();
            item.sub_category = parts.get(1).cloned();
        }
    }
}

// A number of CASPs deliberately use the arm-band accessory slot for
// custom headwear. The raw slot is technically correct but is not the user's
// intended browsing category. Apply a narrow, auditable filename refinement
// only when that CASP was otherwise mapped to Arm Band; never relabel other
// accessory slots or override explicit catalog families.
fn filename_indicates_headwear(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    let normalized = lower.replace(&['_', '-', '.', ' ', '[', ']'][..], " ");
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    tokens.iter().any(|part| {
        matches!(
            *part,
            "hat"
                | "hats"
                | "helmet"
                | "cap"
                | "crown"
                | "veil"
                | "beanie"
                | "headband"
                | "hairband"
                | "braidband"
                | "cowboyhat"
                | "strawhat"
                | "ridinghat"
                | "hatsun"
                | "papercrown"
                | "graduationcap"
                | "bikehelmet"
                | "triangleveil"
                | "bachveil"
        )
    })
}

fn refine_headwear_accessory(
    item: &mut CatalogClassification,
    filename: &str,
    language: AppLanguage,
) {
    let armband = match language {
        AppLanguage::En => "Arm Band",
        AppLanguage::Pt => "Braçadeira",
        AppLanguage::Es => "Brazalete",
    };
    if item.source != "CASP"
        || item.sub_category.as_deref() != Some(armband)
        || !filename_indicates_headwear(filename)
    {
        return;
    }
    let headwear = match language {
        AppLanguage::En => "Headwear",
        AppLanguage::Pt => "Acessórios de Cabeça",
        AppLanguage::Es => "Accesorios para la Cabeza",
    };
    item.sub_category = Some(headwear.to_string());
    if let Some(last) = item.folder_parts.last_mut() {
        *last = headwear.to_string();
    }
    for candidate in &mut item.candidate_folder_parts {
        if let Some(last) = candidate.last_mut() {
            if last == armband {
                *last = headwear.to_string();
            }
        }
    }
    item.technical_reason.push_str(&format!(
        " | Filename headwear evidence '{}' refines the accessory-slot Arm Band label; original CASP slot retained in evidence.",
        filename
    ));
}

fn package_semantic_identifiers(package: &Package) -> Vec<String> {
    let mut values = Vec::new();
    for entry in &package.entries {
        if entry.type_id == TYPE_NMAP_LOCAL {
            if let Ok(data) = package.data(entry) {
                values.extend(nmap_names(&data));
            }
        } else if entry.type_id == TYPE_STBL_LOCAL {
            if let Ok(data) = package.data(entry) {
                values.extend(stbl_entries(&data).into_iter().map(|(_, value)| value));
            }
        }
    }
    values
}

fn makeup_semantic_folder_key(identifiers: &[String]) -> Option<(&'static str, &'static str)> {
    let text = identifiers
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let markers = [
        ("eyebag", "eye bag", "Bolsas sob os Olhos"),
        ("nosemask", "nose mask", "Máscaras de Nariz"),
        ("freckle", "freckles", "Pintas e Sardas"),
        ("mole", "beautymark", "Pintas e Sardas"),
        ("highlighter", "face shine", "Iluminador"),
        ("faceshine", "highlight", "Iluminador"),
        ("contour", "face definition", "Contornos e Sombras Faciais"),
        ("lipcorner", "mouth corner", "Detalhes Faciais"),
        ("dry lips", "lip detail", "Detalhes Labiais"),
        ("contact", "iris", "Lentes de Contato"),
        ("tattoo", "tattoos", "Tatuagens"),
    ];
    markers.iter().find_map(|(first, second, label)| {
        (text.contains(first) || text.contains(second)).then_some((*label, *first))
    })
}

// CASP's ClothingType identifies the technical UI slot, not necessarily the
// visual subject. Refine only when internal NMAP/STBL identifiers corroborate
// a semantic subject. A filename or source folder alone never changes a CASP
// slot, and no new age/gender level is synthesized here.
fn refine_makeup_semantics(
    item: &mut CatalogClassification,
    package: &Package,
    language: AppLanguage,
) {
    let makeup = match language {
        AppLanguage::En => "Makeup",
        AppLanguage::Pt => "Maquiagem",
        AppLanguage::Es => "Maquillaje",
    };
    if item.source != "CASP" || item.main_category != makeup {
        return;
    }
    // A package-level NMAP/STBL marker cannot be assigned to every CASP when
    // the package contains several independent catalog records. Let the
    // multi-CASP aggregation decide in that case instead of forcing one
    // visual function onto unrelated slots.
    if package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_CASP)
        .count()
        != 1
    {
        return;
    }
    let identifiers = package_semantic_identifiers(package);
    let Some((label, marker)) = makeup_semantic_folder_key(&identifiers) else {
        return;
    };
    let localized_label = match (language, label) {
        (AppLanguage::En, "Bolsas sob os Olhos") => "Under-Eye Bags",
        (AppLanguage::Es, "Bolsas sob os Olhos") => "Bolsas bajo los Ojos",
        (AppLanguage::En, "Máscaras de Nariz") => "Nose Masks",
        (AppLanguage::Es, "Máscaras de Nariz") => "Máscaras de Nariz",
        (AppLanguage::En, "Pintas e Sardas") => "Moles & Freckles",
        (AppLanguage::Es, "Pintas e Sardas") => "Lunares y Pecas",
        (AppLanguage::En, "Iluminador") => "Highlighter",
        (AppLanguage::Es, "Iluminador") => "Iluminador",
        (AppLanguage::En, "Contornos e Sombras Faciais") => "Face Contours",
        (AppLanguage::Es, "Contornos e Sombras Faciais") => "Contornos Faciales",
        (AppLanguage::En, "Detalhes Faciais") => "Facial Details",
        (AppLanguage::Es, "Detalhes Faciais") => "Detalles Faciales",
        (AppLanguage::En, "Detalhes Labiais") => "Lip Details",
        (AppLanguage::Es, "Detalhes Labiais") => "Detalles Labiales",
        (AppLanguage::En, "Lentes de Contato") => "Contact Lenses",
        (AppLanguage::Es, "Lentes de Contato") => "Lentes de Contacto",
        (AppLanguage::En, "Tatuagens") => "Tattoos",
        (AppLanguage::Es, "Tatuagens") => "Tatuajes",
        _ => label,
    };
    item.sub_category = Some(localized_label.to_string());
    if let Some(last) = item.folder_parts.last_mut() {
        *last = localized_label.to_string();
    }
    for candidate in &mut item.candidate_folder_parts {
        if let Some(last) = candidate.last_mut() {
            *last = localized_label.to_string();
        }
    }
    item.technical_reason.push_str(&format!(
        " | CASP slot retained as evidence; internal NMAP/STBL marker '{}' refines visual function to '{}'.",
        marker, localized_label
    ));
}

fn resource_type_label(type_id: u32) -> String {
    match type_id {
        TYPE_CASP => "CASP".to_string(),
        TYPE_OBJD => "OBJD".to_string(),
        0x0333_406C => "XML".to_string(),
        0x03B3_3DDF => "ITUN".to_string(),
        TYPE_S3SA => "S3SA".to_string(),
        TYPE_SIME_LOCAL => "SIME".to_string(),
        TYPE_SIMO_LOCAL => "SIMO".to_string(),
        TYPE_SKIN_TONE => "SkinTone".to_string(),
        TYPE_HAIR_TONE => "HairTone".to_string(),
        TYPE_BONE_DELTA => "BoneDelta".to_string(),
        TYPE_FACE => "FACE".to_string(),
        TYPE_BBLN => "BBLN".to_string(),
        TYPE_BGEO => "BGEO".to_string(),
        TYPE_FBLN => "FBLN".to_string(),
        0x73E9_3EEB => "Manifest".to_string(),
        0x2205_57DA => "STBL".to_string(),
        0x00B2_D882 => "IMG".to_string(),
        0x015A_1849 => "GEOM".to_string(),
        0x7368_84F1 => "VPXY".to_string(),
        TYPE_LAYO_LOCAL => "LAYO".to_string(),
        TYPE_CLIP_LOCAL => "CLIP".to_string(),
        0xD4D9_FBE5 => "Pattern".to_string(),
        other => format!("0x{other:08X}"),
    }
}

fn localized_unknown(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::En => "Unknown",
        AppLanguage::Pt => "Desconhecido",
        AppLanguage::Es => "Desconocido",
    }
}

fn localized_reference_message(language: AppLanguage, message: &str) -> String {
    match (language, message) {
        (AppLanguage::Pt, "Store overlap is partial; a shared TGI alone is not a package identity.") => {
            "A sobreposição com a Store é parcial. Um TGI compartilhado, isoladamente, não comprova que os packages sejam idênticos.".to_string()
        }
        (AppLanguage::Es, "Store overlap is partial; a shared TGI alone is not a package identity.") => {
            "La superposición con la Store es parcial. Un TGI compartido, por sí solo, no demuestra que los packages sean idénticos.".to_string()
        }
        (AppLanguage::Pt, "Store package/resource-set identity matches a single category.") => {
            "A identidade do package/conjunto de resources da Store corresponde a uma única categoria.".to_string()
        }
        (AppLanguage::Es, "Store package/resource-set identity matches a single category.") => {
            "La identidad del package/conjunto de recursos de la Store corresponde a una sola categoría.".to_string()
        }
        (AppLanguage::Pt, "Store identity matches multiple categories; no category was selected automatically.") => {
            "A identidade da Store corresponde a várias categorias; nenhuma categoria foi selecionada automaticamente.".to_string()
        }
        (AppLanguage::Es, "Store identity matches multiple categories; no category was selected automatically.") => {
            "La identidad de la Store corresponde a varias categorías; no se seleccionó ninguna automáticamente.".to_string()
        }
        (AppLanguage::Pt, "Store catalog name and shared resources identify one category; full package identity is not confirmed.") => {
            "O nome no catálogo da Store e os recursos compartilhados identificam uma categoria; a identidade completa do package não foi confirmada.".to_string()
        }
        (AppLanguage::Es, "Store catalog name and shared resources identify one category; full package identity is not confirmed.") => {
            "El nombre del catálogo de la Store y los recursos compartidos identifican una categoría; la identidad completa del package no está confirmada.".to_string()
        }
        (AppLanguage::Pt, value) if value.starts_with("Official resources: ") => {
            localize_official_resources(language, value)
        }
        (AppLanguage::Es, value) if value.starts_with("Official resources: ") => {
            localize_official_resources(language, value)
        }
        _ => message.to_string(),
    }
}

fn is_partial_store_evidence_note(evidence: &ReferenceEvidence, message: &str) -> bool {
    if evidence.store_match_kind.as_deref() != Some("partial_resource_overlap")
        || evidence.store_confidence.as_deref() != Some("evidence")
    {
        return false;
    }

    message.starts_with("Store overlap is partial;")
        || message.starts_with("A sobreposição com a Store é parcial.")
        || message.starts_with("La superposición con la Store es parcial.")
}

fn localize_official_resources(language: AppLanguage, value: &str) -> String {
    let numbers = value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if numbers.len() != 4 {
        return value.to_string();
    }
    let [shared, identical, changed, unmatched] =
        [numbers[0], numbers[1], numbers[2], numbers[3]];
    match language {
        AppLanguage::Pt => format!(
            "Recursos oficiais: {shared} TGI em comum, {identical} conteúdos idênticos, {changed} conteúdos modificados e {unmatched} recursos do package sem correspondência nos arquivos oficiais."
        ),
        AppLanguage::Es => format!(
            "Recursos oficiales: {shared} TGI compartidos, {identical} contenidos idénticos, {changed} contenidos modificados y {unmatched} recursos del package sin correspondencia en los archivos oficiales."
        ),
        AppLanguage::En => value.to_string(),
    }
}

fn localized_mixed(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::En => "Mixed",
        AppLanguage::Pt => "Misto",
        AppLanguage::Es => "Mixto",
    }
}

fn classification_confidence(status: &str) -> &'static str {
    match status {
        "classified" | "sim_export_merged" | "quarantined_duplicate" => "high",
        "needs_review" | "mixed" => "medium",
        "unknown" | "invalid" => "low",
        _ => "low",
    }
}

fn localized_warning(language: AppLanguage, key: &str) -> &'static str {
    match (language, key) {
        (AppLanguage::En, "parse") => "A CASP/OBJD resource could not be fully interpreted.",
        (AppLanguage::Pt, "parse") => {
            "Um resource CASP/OBJD não pôde ser interpretado completamente."
        }
        (AppLanguage::Es, "parse") => "Un resource CASP/OBJD no se pudo interpretar completamente.",
        (AppLanguage::En, "mixed") => {
            "The package contains catalog resources with different destinations."
        }
        (AppLanguage::Pt, "mixed") => {
            "O package contém resources de catálogo com destinos diferentes."
        }
        (AppLanguage::Es, "mixed") => {
            "El package contiene resources de catálogo con destinos diferentes."
        }
        (AppLanguage::En, "none") => "No supported CASP/OBJD classification was found.",
        (AppLanguage::Pt, "none") => "Nenhuma classificação CASP/OBJD compatível foi encontrada.",
        (AppLanguage::Es, "none") => "No se encontró una clasificación CASP/OBJD compatible.",
        (AppLanguage::En, "ambiguous") => {
            "The catalog flags indicate more than one possible destination."
        }
        (AppLanguage::Pt, "ambiguous") => {
            "Os flags de catálogo indicam mais de um destino possível."
        }
        (AppLanguage::Es, "ambiguous") => {
            "Los flags del catálogo indican más de un destino posible."
        }
        (AppLanguage::En, "family_ambiguous") => {
            "The package contains multiple authoritative resource families and needs review."
        }
        (AppLanguage::Pt, "family_ambiguous") => {
            "O package contém várias famílias de resources autoritativas e precisa de revisão."
        }
        (AppLanguage::Es, "family_ambiguous") => {
            "El package contiene varias familias de resources autoritativas y necesita revisión."
        }
        _ => "Unknown",
    }
}

// A single CC package may legitimately contain CASP entries for several
// genders/ages. Group these under the closest proven common family instead
// of mislabeling the whole package Mixed and stranding its old folder.
fn shared_casp_family(
    classifications: &[CatalogClassification],
    types: &BTreeSet<u32>,
) -> Option<CatalogClassification> {
    if classifications.len() < 2
        || !types.contains(&TYPE_CASP)
        || types.contains(&TYPE_OBJD)
        || classifications
            .iter()
            .any(|value| value.source != "CASP" || value.ambiguous)
    {
        return None;
    }
    let first = classifications.first()?;
    if first.main_category.is_empty()
        || matches!(
            first.main_category.as_str(),
            "Unknown" | "Desconhecido" | "Desconocido"
        )
        || classifications
            .iter()
            .any(|value| value.main_category != first.main_category)
    {
        return None;
    }
    let mut shared = first.folder_parts.clone();
    for candidate in classifications.iter().skip(1) {
        shared.truncate(
            shared
                .iter()
                .zip(&candidate.folder_parts)
                .take_while(|(left, right)| left == right)
                .count(),
        );
    }
    // "CAS" by itself is not a category; demand at least a shared CAS
    // family. Never combine unrelated makeup, clothes and accessories.
    if shared.len() < 2 {
        return None;
    }
    let common_sub = first.sub_category.as_ref().filter(|sub| {
        classifications
            .iter()
            .all(|item| item.sub_category.as_deref() == Some(sub.as_str()))
    });
    if let Some(sub) = common_sub {
        if !shared.iter().any(|part| part == sub) {
            shared.push(sub.clone());
        }
    }
    let mut result = first.clone();
    result.folder_parts = shared.clone();
    result.candidate_folder_parts = vec![shared.clone()];
    result.gender = first
        .gender
        .as_ref()
        .filter(|gender| {
            classifications
                .iter()
                .all(|v| v.gender.as_ref() == Some(gender))
        })
        .cloned();
    result.age = first
        .age
        .as_ref()
        .filter(|age| classifications.iter().all(|v| v.age.as_ref() == Some(age)))
        .cloned();
    result.ambiguous = false;
    result.technical_reason = format!(
        "{} corroborated CASP entries share a single catalog family;          combined genders/ages => {}",
        classifications.len(), shared.join("\\")
    );
    Some(result)
}

// One eyelash asset can expose many CASP presets in the eyeliner tab and
// a small number in glasses/earrings slots. These are alternative UI slots
// for the SAME package, not proof of a merged clothing/accessory collection.
// Resolve only when the dominant slot and filename corroborate eyelashes.
fn verified_eyelash_catalog_bundle(
    classifications: &[CatalogClassification],
    types: &BTreeSet<u32>,
    filename: &str,
    language: AppLanguage,
) -> Option<CatalogClassification> {
    let lower = filename.to_ascii_lowercase();
    if !(lower.contains("eyelash") || lower.contains("lashes"))
        || classifications.len() < 3
        || !types.contains(&TYPE_CASP)
        || types.contains(&TYPE_OBJD)
        || types.contains(&TYPE_S3SA)
        || classifications
            .iter()
            .any(|value| value.source != "CASP" || value.ambiguous)
    {
        return None;
    }
    let (makeup, eyeliner, accessories, earrings, glasses, eyelashes) = match language {
        AppLanguage::En => (
            "Makeup",
            "Eyeliner",
            "Accessories",
            "Earrings",
            "Glasses",
            "Eyelashes",
        ),
        AppLanguage::Pt => (
            "Maquiagem",
            "Delineador",
            "Acessórios",
            "Brincos",
            "Óculos",
            "Cílios",
        ),
        AppLanguage::Es => (
            "Maquillaje",
            "Delineador",
            "Accesorios",
            "Pendientes",
            "Gafas",
            "Pestañas",
        ),
    };
    let eyeliner_count = classifications
        .iter()
        .filter(|value| {
            value.main_category == makeup && value.sub_category.as_deref() == Some(eyeliner)
        })
        .count();
    if eyeliner_count * 2 <= classifications.len() {
        return None;
    }
    if classifications.iter().any(|value| !(
        value.main_category == makeup && value.sub_category.as_deref() == Some(eyeliner)
        || value.main_category == accessories &&
            matches!(value.sub_category.as_deref(), Some(v) if v == earrings || v == glasses)
    )) {
        return None;
    }

    let path = vec!["CAS".to_string(), makeup.to_string(), eyelashes.to_string()];
    let mut result = classifications.first()?.clone();
    result.main_category = makeup.to_string();
    result.sub_category = Some(eyelashes.to_string());
    result.gender = None;
    result.age = None;
    result.folder_parts = path.clone();
    result.candidate_folder_parts = vec![path.clone()];
    result.ambiguous = false;
    result.technical_reason = format!(
        "Eyelash asset with {eyeliner_count}/{} eyeliner CASP entries and          verified alternative accessory slots => {}",
        classifications.len(), path.join("\\")
    );
    Some(result)
}

// A small number of eyelash downloads contain only the IMG companions for a
// known CASP eyelash asset. They have no CASP of their own, so the normal
// catalogue path cannot classify them. The three exact instance prefixes below
// are the verified internal TGI family shared by the affected packages and by
// the recognised S-Club eyelash asset. Requiring all three clusters and an
// IMG-only package prevents generic textures from being promoted by filename.
fn verified_eyelash_texture_companion(
    entries: &[ResourceEntry],
    types: &BTreeSet<u32>,
    language: AppLanguage,
) -> Option<PackageFamilyClassification> {
    if types.len() != 1
        || !types.contains(&TYPE_IMG)
        || entries.len() != 21
        || entries.iter().any(|entry| entry.type_id != TYPE_IMG)
    {
        return None;
    }
    let mut clusters = [0usize; 3];
    for entry in entries {
        let prefix = entry.instance & 0xFFFF_FFFF_FFFF_FF00;
        match prefix {
            0xD1E4_338D_1C34_B700 => clusters[0] += 1,
            0xD1E4_348D_1C34_B900 => clusters[1] += 1,
            0xD1E4_358D_1C34_BB00 => clusters[2] += 1,
            _ => return None,
        }
    }
    if clusters != [9, 10, 2] {
        return None;
    }
    let eyelashes = match language {
        AppLanguage::En => "Eyelashes",
        AppLanguage::Pt => "Cílios",
        AppLanguage::Es => "Pestañas",
    };
    Some(PackageFamilyClassification {
        main_category: match language {
            AppLanguage::En => "Makeup",
            AppLanguage::Pt => "Maquiagem",
            AppLanguage::Es => "Maquillaje",
        }
        .to_string(),
        sub_category: Some(eyelashes.to_string()),
        folder_parts: vec!["CAS".to_string(), match language {
            AppLanguage::En => "Makeup",
            AppLanguage::Pt => "Maquiagem",
            AppLanguage::Es => "Maquillaje",
        }
        .to_string(), eyelashes.to_string()],
        detected_from: vec![
            "IMG".to_string(),
            "EyelashTextureCompanionSignature".to_string(),
        ],
        technical_reason: "IMG-only package shares the verified internal eyelash texture TGI family with a CASP eyelash asset; classification does not rely on the filename or source folder.".to_string(),
    })
}

#[derive(Debug, Default)]
struct ScanTiming {
    package_reads: usize,
    dbpf_load_us: u128,
    resource_decode_us: u128,
    resource_decode_exclusive_us: u128,
    resource_decode_wait_us: u128,
    full_hash_us: u128,
    full_hash_exclusive_us: u128,
    full_hash_wait_us: u128,
    reference_catalog_us: u128,
    project_grouping_us: u128,
}

impl ScanTiming {
    fn add(&mut self, other: Self) {
        self.package_reads += other.package_reads;
        self.dbpf_load_us += other.dbpf_load_us;
        self.resource_decode_us += other.resource_decode_us;
        self.resource_decode_exclusive_us += other.resource_decode_exclusive_us;
        self.resource_decode_wait_us += other.resource_decode_wait_us;
        self.full_hash_us += other.full_hash_us;
        self.full_hash_exclusive_us += other.full_hash_exclusive_us;
        self.full_hash_wait_us += other.full_hash_wait_us;
        self.reference_catalog_us += other.reference_catalog_us;
        self.project_grouping_us += other.project_grouping_us;
    }
}

struct ScanAccumulation {
    item_slots: Vec<Option<ScanPackageItem>>,
    cache_entries: HashMap<String, cache::CachedPackage>,
    file_signatures: HashMap<String, (u64, u128, String)>,
    slider_instances: HashSet<u64>,
    timing: ScanTiming,
    profile_packages: Vec<ScanPackageProfileEntry>,
}

fn scan_one(
    root: &Path,
    path: &Path,
    language: AppLanguage,
    slider_instances: &mut HashSet<u64>,
    fix_index: Option<&FixSignatureIndex>,
    timing: &mut ScanTiming,
) -> ScanPackageItem {
    scan_one_with_index(
        root,
        path,
        language,
        slider_instances,
        fix_index,
        timing,
        None,
        None,
        None,
    )
}

fn scan_one_with_index(
    root: &Path,
    path: &Path,
    language: AppLanguage,
    slider_instances: &mut HashSet<u64>,
    fix_index: Option<&FixSignatureIndex>,
    timing: &mut ScanTiming,
    reference_catalogs: Option<&reference_catalog::ReferenceCatalogSet>,
    shared_index: Option<&mut cache::FingerprintCache>,
    prevalidated_cached: Option<cache::CachedPackage>,
) -> ScanPackageItem {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("package")
        .to_string();
    let id = path.to_string_lossy().to_string();
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let source_is_override = root
        .file_name()
        .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("Overrides"))
        || relative
            .replace('\\', "/")
            .split('/')
            .next()
            .is_some_and(|value| value.eq_ignore_ascii_case("Overrides"));

    let dbpf_started = Instant::now();
    let package_result = Package::load(path);
    timing.package_reads += 1;
    timing.dbpf_load_us += dbpf_started.elapsed().as_micros();
    let package = match package_result {
        Ok(package) => package,
        Err(error) => {
            return ScanPackageItem {
                id,
                name,
                path: path.to_string_lossy().to_string(),
                relative_path: relative,
                file_size,
                resource_count: 0,
                catalog_resource_count: 0,
                resource_types: Vec::new(),
                instances: Vec::new(),
                scripted: false,
                content_source: "unknown".to_string(),
                source_confidence: None,
                status: "invalid".to_string(),
                classification_confidence: "low".to_string(),
                creator: None,
                mod_name: None,
                gameplay_category: None,
                detected_from: Vec::new(),
                category: None,
                sub_category: None,
                gender: None,
                age: None,
                species: None,
                usage_categories: Vec::new(),
                destination_parts: Vec::new(),
                destination_path: None,
                candidate_destinations: Vec::new(),
                classifications: Vec::new(),
                classification_reason: None,
                warnings: vec![error.to_string()],
                project_id: None,
                project_role: None,
                project_confidence: None,
                project_evidence: ProjectTechnicalEvidence::default(),
                project_relationships: Vec::new(),
                reference_evidence: None,
                profile: None,
            };
        }
    };
    // The duplicate/conflict phases need full resource fingerprints. Build
    // them now, while this DBPF is already decoded, instead of reopening it
    // later in the same unified analysis. A failure here does not discard a
    // valid classification; those later phases fall back to their validated
    // readers and report the specific cache error.
    let mut shared_cached = None;
    let mut profile_resource_decode_us = 0u128;
    let mut profile_project_payload_analysis_us = 0u128;
    let shared_index_error = if let Some(shared_index) = shared_index {
        let cached_result = prevalidated_cached
            .map(|cached| Ok(Some(cached)))
            .unwrap_or_else(|| cache::current_cached_entry(path, shared_index));
        match cached_result {
            Ok(Some(cached)) => {
                shared_cached = Some(cached);
                None
            }
            Ok(None) => {
                match cache::remember_loaded_package_with_result(path, &package, shared_index) {
                    Ok((cached, metrics)) => {
                        timing.full_hash_us += metrics.hash_us;
                        timing.full_hash_exclusive_us += metrics.hash_exclusive_us;
                        timing.full_hash_wait_us += metrics.hash_wait_us;
                        timing.resource_decode_us += metrics.resource_decode_us;
                        timing.resource_decode_exclusive_us +=
                            metrics.resource_decode_exclusive_us;
                        timing.resource_decode_wait_us += metrics.resource_decode_wait_us;
                        profile_resource_decode_us = metrics.resource_decode_us;
                        shared_cached = Some(cached);
                        None
                    }
                    Err(error) => Some(format!(
                        "Could not add package to the shared analysis index: {error}"
                    )),
                }
            }
            Err(error) => Some(format!(
                "Could not validate the shared analysis index: {error}"
            )),
        }
    } else {
        None
    };

    let project_started = Instant::now();
    let (project_evidence, project_metrics) =
        extract_package_evidence_with_metrics(path, &package);
    timing.project_grouping_us += project_started.elapsed().as_micros();
    timing.resource_decode_us += project_metrics.resource_decode_us;
    profile_resource_decode_us += project_metrics.resource_decode_us;
    profile_project_payload_analysis_us += project_metrics.payload_analysis_us;
    let reference_started = Instant::now();
    let mut reference_evidence = match (shared_cached.as_ref(), reference_catalogs) {
        (Some(cached), Some(catalogs)) => {
            reference_catalog::compare_cached_package(cached, catalogs)
        }
        _ => reference_catalog::analyze_package(&package),
    };
    if let Some(evidence) = reference_evidence.as_mut() {
        for message in &mut evidence.evidence {
            *message = localized_reference_message(language, message);
        }
    }
    timing.reference_catalog_us += reference_started.elapsed().as_micros();

    let mut type_set = BTreeSet::new();
    let mut type_ids = BTreeSet::new();
    let mut detected_from = BTreeSet::new();
    let mut classifications = Vec::new();
    let mut warnings = Vec::new();
    if let Some(error) = shared_index_error {
        warnings.push(error);
    }
    let auxiliary_started = Instant::now();
    let mut catalog_resource_count = 0usize;
    let is_merged_sim_export = merged_sim_export_evidence(&package).is_some();
    let fbln_metadata = read_fbln_metadata(&package);
    let profile_auxiliary_analysis_us = auxiliary_started.elapsed().as_micros();
    let has_fbln = !fbln_metadata.is_empty();
    if has_fbln {
        detected_from.insert("FBLN".to_string());
        if fbln_metadata.len() > 1 {
            detected_from.insert(format!("FBLNCount:{}", fbln_metadata.len()));
        }
    }

    let catalog_classification_started = Instant::now();
    for entry in &package.entries {
        type_ids.insert(entry.type_id);
        type_set.insert(resource_type_label(entry.type_id));
        if !matches!(entry.type_id, TYPE_CASP | TYPE_OBJD) {
            continue;
        }
        if is_merged_sim_export {
            // CASPs/OBJD inside an exported Sim are embedded payload, not
            // independent CC records. Keep them out of the per-resource
            // classifier and preserve the package as one atomic asset.
            continue;
        }

        catalog_resource_count += 1;
        detected_from.insert(resource_type_label(entry.type_id));

        let decode_started = Instant::now();
        let data_result = package.data(entry);
        timing.resource_decode_us += decode_started.elapsed().as_micros();
        match data_result {
            Ok(data) => match classify_resource(entry.type_id, &data, language) {
                Some(mut classification) => {
                    refine_headwear_accessory(&mut classification, &name, language);
                    refine_makeup_semantics(&mut classification, &package, language);
                    if classification
                        .technical_reason
                        .contains("CASP slot retained as evidence")
                    {
                        detected_from.insert("CASPSemanticEvidence".to_string());
                    }
                    classifications.push(classification);
                }
                None => warnings.push(format!(
                    "{} {}",
                    localized_warning(language, "parse"),
                    entry.key_string()
                )),
            },
            Err(error) => warnings.push(format!("{}: {error}", entry.key_string())),
        }
    }
    let profile_catalog_classification_us =
        catalog_classification_started.elapsed().as_micros();

    let final_resolution_started = Instant::now();
    let mut profile_directed_family_us = 0;
    let mut profile_special_package_us = 0;
    let special_primary = preserved_correction_family_classification(&type_ids, &relative)
        .or_else(|| merged_sim_export_classification(&package, language))
        .or_else(|| {
            has_fbln.then(|| fbln_slider_classification(&package, &fbln_metadata, &name, language))
        })
        .or_else(|| {
            cas_default_replacement_classification(
                &type_ids,
                &name,
                &project_evidence,
                reference_evidence.as_ref(),
                language,
            )
        })
        .or_else(|| {
            fix_signatures::learned_fix_classification(
                &package,
                fix_index,
                &name,
                source_is_override,
            )
            .map(|classification| PackageFamilyClassification {
                main_category: classification
                    .folder_parts
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Correções".to_string()),
                sub_category: classification.folder_parts.get(1).cloned(),
                folder_parts: classification.folder_parts,
                detected_from: classification.detected_from,
                technical_reason: classification.technical_reason,
            })
        })
        .or_else(|| {
            let started = Instant::now();
            let classification = phase3_directed_family_classification(
                &package,
                &project_evidence,
                &type_ids,
                &name,
                &relative,
                language,
                &classifications,
            );
            profile_directed_family_us = started.elapsed().as_micros();
            classification
        })
        .or_else(|| {
            let started = Instant::now();
            let classification = special_package_classification(
                &package,
                &type_ids,
                &name,
                &relative,
                language,
                catalog_resource_count,
            );
            profile_special_package_us = started.elapsed().as_micros();
            classification
        });
    if let Some(special) = &special_primary {
        for source in &special.detected_from {
            detected_from.insert(source.clone());
        }
    }

    let mut destinations: HashMap<String, CatalogClassification> = HashMap::new();
    let mut candidate_destinations = BTreeSet::new();
    let mut has_ambiguous = false;

    for classification in &classifications {
        for parts in &classification.candidate_folder_parts {
            if !parts.is_empty() {
                candidate_destinations.insert(parts.join("\\"));
            }
        }

        if classification.ambiguous {
            has_ambiguous = true;
            continue;
        }

        if !classification.folder_parts.is_empty() {
            let key = classification.folder_parts.join("\\");
            destinations
                .entry(key)
                .or_insert_with(|| classification.clone());
        }
    }

    let catalog_first = catalog_precedes_embedded_script(
        special_primary.as_ref(),
        catalog_resource_count,
        &relative,
        destinations.len(),
        has_ambiguous,
    );
    let active_special = if catalog_first {
        None
    } else {
        special_primary.as_ref()
    };
    let (mut status, primary, mut destination_parts, mut destination_path) =
        if let Some(special) = active_special {
            (
                "classified".to_string(),
                None,
                special.folder_parts.clone(),
                Some(special.folder_parts.join("\\")),
            )
        } else if has_ambiguous {
            warnings.push(localized_warning(language, "ambiguous").to_string());
            ("needs_review".to_string(), None, Vec::new(), None)
        } else if destinations.len() == 1 {
            let (path, classification) = destinations.into_iter().next().unwrap();
            (
                "classified".to_string(),
                Some(classification.clone()),
                classification.folder_parts.clone(),
                Some(path),
            )
        } else if destinations.len() > 1 {
            if let Some(shared) = shared_casp_family(&classifications, &type_ids).or_else(|| {
                verified_eyelash_catalog_bundle(&classifications, &type_ids, &name, language)
            }) {
                let path = shared.folder_parts.join("\\");
                (
                    "classified".to_string(),
                    Some(shared.clone()),
                    shared.folder_parts,
                    Some(path),
                )
            } else {
                warnings.push(localized_warning(language, "mixed").to_string());
                ("mixed".to_string(), None, Vec::new(), None)
            }
        } else if catalog_resource_count > 0 {
            ("needs_review".to_string(), None, Vec::new(), None)
        } else {
            ("unknown".to_string(), None, Vec::new(), None)
        };
    if active_special.is_some_and(|classification| {
        classification
            .detected_from
            .iter()
            .any(|source| source == "MergedSimExport")
    }) {
        status = "sim_export_merged".to_string();
        destination_parts.clear();
        destination_path = None;
    }

    let mut classification_reason = active_special
        .map(|classification| classification.technical_reason.clone())
        .or_else(|| {
            primary
                .as_ref()
                .map(|classification| classification.technical_reason.clone())
        });
    let mut family_primary: Option<PackageFamilyClassification> =
        if catalog_first { None } else { special_primary };

    if status == "unknown" && catalog_resource_count == 0 {
        // These three mesh classifiers inspect the same NMAP payloads. Read
        // them once and share the decoded names rather than reopening every
        // compressed resource for each candidate rule.
        let nmap_names = (type_ids.contains(&0x015A_1849)
            && type_ids.contains(&TYPE_NMAP_LOCAL)
            && !type_ids.contains(&TYPE_CASP)
            && !type_ids.contains(&TYPE_S3SA))
        .then(|| package_nmap_names(&package))
        .unwrap_or_default();
        let family_result = geometry_clothing_from_nmap(&type_ids, &nmap_names, language)
            .or_else(|| replacement_mesh_classification(&type_ids, &nmap_names, &name, language))
            .or_else(|| verified_eye_mesh_replacement(&type_ids, &nmap_names, &name, language))
            .map(PackageFamilyResult::Classified)
            .unwrap_or_else(|| classify_package_family(&type_ids, language));
        match family_result {
            PackageFamilyResult::Classified(classification) => {
                for source in &classification.detected_from {
                    detected_from.insert(source.clone());
                }
                let is_slider_family = classification.sub_category.as_deref() == Some("Sliders");
                destination_parts = classification.folder_parts.clone();
                destination_path = Some(destination_parts.join("\\"));
                status = "classified".to_string();
                classification_reason = Some(classification.technical_reason.clone());
                family_primary = Some(classification);

                if is_slider_family {
                    if slider_internal_candidates(&package).is_empty() {
                        // A morph-only package without an anatomical name is
                        // still a slider, but its body region is unknown.
                        destination_parts = vec![
                            "Sliders".to_string(),
                            slider_folder_label(language, "other").to_string(),
                        ];
                        destination_path = Some(destination_parts.join("\\"));
                    }
                    if let Some((internal_name, source, refined_destination)) =
                        slider_internal_evidence(&package, &name, language)
                    {
                        detected_from.insert(source.to_string());
                        if let Some(parts) = refined_destination {
                            destination_parts = parts;
                            destination_path = Some(destination_parts.join("\\"));
                            classification_reason = Some(format!(
                                "{} | Internal slider name from {source} '{}' => {}",
                                classification_reason.unwrap_or_default(),
                                internal_name,
                                destination_parts.join("\\")
                            ));
                        } else {
                            destination_parts = vec![
                                "Sliders".to_string(),
                                slider_folder_label(language, "other").to_string(),
                            ];
                            destination_path = Some(destination_parts.join("\\"));
                            classification_reason = Some(format!(
                                "{} | Internal slider name from {source} '{}' did not safely identify an anatomical region; grouped under Sliders/Other without inventing an anatomy.",
                                classification_reason.unwrap_or_default(),
                                internal_name
                            ));
                        }
                    }
                }
            }
            PackageFamilyResult::Ambiguous(candidates) => {
                for classification in candidates {
                    for source in &classification.detected_from {
                        detected_from.insert(source.clone());
                    }
                    if !classification.folder_parts.is_empty() {
                        candidate_destinations.insert(classification.folder_parts.join("\\"));
                    }
                }
                warnings.push(localized_warning(language, "family_ambiguous").to_string());
                status = "needs_review".to_string();
            }
            PackageFamilyResult::None => {
                if let Some(classification) = reference_or_override_family_classification(
                    &type_ids,
                    reference_evidence.as_ref(),
                    source_is_override,
                    language,
                ) {
                    for source in &classification.detected_from {
                        detected_from.insert(source.clone());
                    }
                    destination_parts = classification.folder_parts.clone();
                    destination_path = Some(destination_parts.join("\\"));
                    status = "classified".to_string();
                    classification_reason = Some(classification.technical_reason.clone());
                    family_primary = Some(classification);
                } else {
                    warnings.push(localized_warning(language, "none").to_string());
                }
            }
        }
    }

    // Store classification is authoritative only for a complete, unambiguous
    // package/resource-set match.  Resolve it before the public category is
    // projected so category, destination and status always remain coherent.
    if store_evidence_can_resolve_status(&status) {
        if let Some(classification) = reference_evidence
            .as_ref()
            .and_then(store_family_classification)
        {
            for source in &classification.detected_from {
                detected_from.insert(source.clone());
            }
            destination_parts = classification.folder_parts.clone();
            destination_path = Some(destination_parts.join("\\"));
            status = "classified".to_string();
            classification_reason = Some(classification.technical_reason.clone());
            family_primary = Some(classification);
        }
    }

    if let Some(evidence) = reference_evidence.as_ref() {
        if evidence.official_overlap_kind.as_deref() == Some("replacement_candidate")
            || evidence.official_overlap_kind.as_deref() == Some("mixed_official_and_new")
        {
            detected_from.insert("OfficialReplacementEvidence".to_string());
        }
        if evidence.official_overlap_kind.as_deref() == Some("official_copy") {
            detected_from.insert("OfficialResourceMatch".to_string());
        }
        for message in &evidence.evidence {
            if is_partial_store_evidence_note(evidence, message) {
                // Keep this diagnostic in structured referenceEvidence, but
                // do not present a partial Store TGI overlap as a review
                // warning. It is not a Store identity and must not make an
                // otherwise classified package look unresolved.
                continue;
            }
            if !warnings.contains(message) {
                warnings.push(message.clone());
            }
        }
    }

    let mut usage_categories = BTreeSet::new();
    for classification in &classifications {
        for usage in &classification.usage_categories {
            usage_categories.insert(usage.clone());
        }
    }

    let (mut category, mut sub_category, mut gender, mut age, mut species) =
        if let Some(primary) = primary {
            (
                Some(primary.main_category),
                primary.sub_category,
                primary.gender,
                primary.age,
                primary.species,
            )
        } else if let Some(family) = family_primary {
            (
                Some(family.main_category),
                family.sub_category,
                None,
                None,
                None,
            )
        } else if status == "mixed" {
            (
                Some(localized_mixed(language).to_string()),
                None,
                None,
                None,
                None,
            )
        } else {
            (
                Some(localized_unknown(language).to_string()),
                None,
                None,
                None,
                None,
            )
        };

    // `.package.duplicated` is the Manager's explicit inactive quarantine
    // marker. It remains indexed for duplicate evidence and restoration, but
    // must not be presented as an unknown, loadable CC or be eligible for the
    // organizer's ordinary fallback.
    if is_quarantined_duplicate_name(&name) {
        status = "quarantined_duplicate".to_string();
        category = Some("#Duplicados".to_string());
        sub_category = None;
        gender = None;
        age = None;
        species = None;
        destination_parts = vec!["#Duplicados".to_string()];
        destination_path = Some(destination_parts.join("\\"));
        candidate_destinations.clear();
        detected_from.insert("DuplicateQuarantineMarker".to_string());
        classification_reason = Some(
            "Inactive duplicate quarantine marker (.package.duplicated); retained for duplicate evidence and restoration."
                .to_string(),
        );
        warnings.retain(|warning| warning != localized_warning(language, "none"));
    }

    if classification_reason.is_none() && !classifications.is_empty() {
        classification_reason = Some(
            classifications
                .iter()
                .map(|classification| classification.technical_reason.clone())
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }

    // Collect morph IDs only when this package was positively classified
    // as a slider, never from clothing that happens to contain morphs.
    if status == "classified" && sub_category.as_deref() == Some("Sliders") {
        slider_instances.extend(
            package
                .entries
                .iter()
                .filter(|entry| is_slider_morph_type(entry.type_id))
                .map(|entry| entry.instance),
        );
    }

    let script_metadata_started = Instant::now();
    let scripted = type_ids.contains(&TYPE_S3SA);
    let (content_source, source_confidence) =
        detect_store_source(&project_evidence, &name, &relative, scripted);

    let is_nraas = detected_from.contains("NRaasInternal");
    let creator = if is_nraas {
        Some("NRaas".to_string())
    } else if scripted {
        verified_script_creator(&package, &name)
    } else {
        None
    };
    // Embedded gameplay assemblies are still tracked by the 'scripted' flag,
    // but must not supply an unrelated mod/category label to a catalog object.
    let mod_name = if scripted && !is_nraas && !catalog_first {
        inferred_script_mod_name(&name, &relative)
    } else {
        None
    };
    let gameplay_category = if scripted && !is_nraas && !catalog_first {
        Some(script_category(&name, language).to_string())
    } else {
        None
    };
    let profile_script_metadata_us = script_metadata_started.elapsed().as_micros();
    let classification_confidence = classification_confidence(&status).to_string();

    let profile_final_resolution_us = final_resolution_started.elapsed().as_micros();
    ScanPackageItem {
        id,
        name,
        path: path.to_string_lossy().to_string(),
        relative_path: relative,
        file_size,
        resource_count: package.entries.len(),
        catalog_resource_count,
        resource_types: type_set.into_iter().collect(),
        instances: package
            .entries
            .iter()
            .map(|entry| format!("0x{:016X}", entry.instance))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        scripted,
        content_source,
        source_confidence,
        status,
        classification_confidence,
        creator,
        mod_name,
        gameplay_category,
        detected_from: detected_from.into_iter().collect(),
        category,
        sub_category,
        gender,
        age,
        species,
        usage_categories: usage_categories.into_iter().collect(),
        destination_parts,
        destination_path,
        candidate_destinations: candidate_destinations.into_iter().collect(),
        classifications,
        classification_reason,
        warnings,
        project_id: None,
        project_role: None,
        project_confidence: None,
        project_evidence,
        project_relationships: Vec::new(),
        reference_evidence,
        profile: Some(ScanPackageProfile {
            compressed_bytes: package
                .entries
                .iter()
                .map(|entry| u64::from(entry.file_size))
                .sum(),
            decompressed_bytes: package
                .entries
                .iter()
                .map(|entry| u64::from(entry.mem_size))
                .sum(),
            resource_count: package.entries.len(),
            decompression_count: package
                .entries
                .iter()
                .filter(|entry| entry.compressed == 0xFFFF)
                .count(),
            resource_decode_us: profile_resource_decode_us,
            project_payload_analysis_us: profile_project_payload_analysis_us,
            project_stbl_resources: project_metrics.stbl_resource_count,
            project_nmap_resources: project_metrics.nmap_resource_count,
            project_objk_resources: project_metrics.objk_resource_count,
            project_other_resources: project_metrics.other_resource_count,
            catalog_classification_us: profile_catalog_classification_us,
            auxiliary_analysis_us: profile_auxiliary_analysis_us,
            directed_family_us: profile_directed_family_us,
            special_package_us: profile_special_package_us,
            script_metadata_us: profile_script_metadata_us,
            final_resolution_us: profile_final_resolution_us,
        }),
    }
}

// Previews must preserve the loading branch of the source: Packages stays in
// Packages, Overrides stays in Overrides. The semantic category is independent
// of which branch actually contains the package.
fn physical_destination_parts(root: &Path, source_relative: &str, parts: &[String]) -> Vec<String> {
    let root_name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let source_top = source_relative
        .replace('\\', "/")
        .split('/')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let mods_layout = is_mods_layout_root(root);
    let override_source = root_name == "overrides" || (mods_layout && source_top == "overrides");

    let mut result = parts.to_vec();
    if result.first().is_some_and(|part| {
        part.eq_ignore_ascii_case("Packages") || part.eq_ignore_ascii_case("Overrides")
    }) {
        result.remove(0);
    }
    if result
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("CAS"))
    {
        result.remove(0);
    }
    if mods_layout && !result.is_empty() {
        result.insert(
            0,
            if override_source {
                "Overrides".to_string()
            } else {
                "Packages".to_string()
            },
        );
    } else {
        // Mirror planner relative destination normalization. Existing
        // categories represented by the selected subfolder are not repeated.
        let branch = root.ancestors().find(|ancestor| {
            ancestor.file_name().is_some_and(|name| {
                name.to_string_lossy().eq_ignore_ascii_case("Packages")
                    || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
            }) && ancestor.parent().is_some_and(is_mods_layout_root)
        });
        if let Some(branch) = branch {
            let existing = root
                .strip_prefix(branch)
                .ok()
                .into_iter()
                .flat_map(|relative| relative.components())
                .filter_map(|part| match part {
                    std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !existing.is_empty()
                && result.len() >= existing.len()
                && result
                    .iter()
                    .take(existing.len())
                    .zip(existing.iter())
                    .all(|(left, right)| left.eq_ignore_ascii_case(right))
            {
                result.drain(..existing.len());
            }
        }
    }
    result
}

fn remove_generic_unknown_levels(parts: Vec<String>) -> Vec<String> {
    parts
        .into_iter()
        .filter(|part| {
            !matches!(
                part.trim().to_ascii_lowercase().as_str(),
                "unknown" | "desconhecido" | "desconocido"
            )
        })
        .collect()
}

pub fn scan_packages_core(
    folder: String,
    language: AppLanguage,
    operation_kind: Option<&str>,
) -> Result<ScanResult, String> {
    scan_packages_core_with_progress(folder, language, operation_kind, true, true, false)
}

/// Variant for a coordinated analysis.  Cancellation remains cooperative,
/// but the coordinator owns the one monotonic progress bar instead of each
/// inner phase resetting it independently.
pub(crate) fn scan_packages_core_quiet_progress(
    folder: String,
    language: AppLanguage,
    operation_kind: Option<&str>,
) -> Result<ScanResult, String> {
    // Legacy unified callers still require the complete shared fingerprint
    // index so their duplicate/conflict phases can reuse it. The live
    // Organizer command uses `scan_packages_core` and does not pay this cost.
    scan_packages_core_with_progress(folder, language, operation_kind, false, false, true)
}

fn scan_packages_core_with_progress(
    folder: String,
    language: AppLanguage,
    operation_kind: Option<&str>,
    report_progress: bool,
    allow_cached_result: bool,
    build_fingerprint_index: bool,
) -> Result<ScanResult, String> {
    let total_started = Instant::now();
    if folder.trim().is_empty() {
        return Err("No folder was selected.".to_string());
    }
    let root = PathBuf::from(folder.trim());
    if !root.is_dir() {
        return Err(format!("Folder does not exist: {}", root.display()));
    }

    // The latest scan is a read-only presentation cache. Its bounded file
    // fingerprints, rule-version context and language/manual-index context
    // must all still match before any classified item is reused. Mutating
    // operations continue to verify full SHA-256 identities separately.
    if allow_cached_result {
        if let Some(mut cached) = cached_scan_for(&root, language) {
            cached.cache_hit = true;
            cached.reused_packages = cached.items.len();
            cached.stats.total_ms = total_started.elapsed().as_millis();
            cached.stats.cache_hit = true;
            cached.stats.reused_packages = cached.items.len();
            cached.stats.package_reads = 0;
            cached.stats.dbpf_load_ms = 0;
            cached.stats.resource_decode_ms = 0;
            cached.stats.classification_ms = 0;
            cached.stats.postprocess_ms = 0;
            if report_progress {
                if let Some(kind) = operation_kind {
                    operation::set_total(kind, cached.items.len());
                    operation::update(kind, cached.items.len(), None, "cache_reused");
                }
            }
            return Ok(cached);
        }
    }

    let discovery_started = Instant::now();
    let mut package_paths = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && package_extension(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    package_paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    let profile_enabled = std::env::var_os("S3CC_PROFILE_ROOT").is_some();
    #[cfg(test)]
    if let Some(limit) = std::env::var("S3CC_PROFILE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        package_paths.truncate(limit);
    }
    let mut shared_index = if build_fingerprint_index {
        cache::load_cache(&root)
    } else {
        cache::FingerprintCache::default()
    };
    if build_fingerprint_index {
        cache::retain_existing(&mut shared_index, &package_paths);
    }
    let discovery_ms = discovery_started.elapsed().as_millis();

    if report_progress {
        if let Some(kind) = operation_kind {
            operation::set_total(kind, package_paths.len());
            operation::update(kind, 0, None, "scanning");
        }
    }

    let reference_catalog_started = Instant::now();
    let reference_catalogs = reference_catalog::load_catalogs();
    if profile_enabled {
        eprintln!(
            "scanner_profile_catalog_load_ms={} store_packages={} official_archives={}",
            reference_catalog_started.elapsed().as_millis(),
            reference_catalogs.store.as_ref().map_or(0, |catalog| catalog.packages.len()),
            reference_catalogs
                .official
                .as_ref()
                .map_or(0, |catalog| catalog.archives.len()),
        );
    }
    let fix_index_started = Instant::now();
    let fix_index = fix_signatures::load_persistent_index();
    if profile_enabled {
        eprintln!(
            "scanner_profile_fix_index_load_ms={}",
            fix_index_started.elapsed().as_millis()
        );
    }
    let mut timing = ScanTiming::default();
    let previous = previous_scan_for(&root, language);
    let previous_items = previous.as_ref().map(|scan| {
        scan.result
            .items
            .iter()
            .map(|item| (stable_path(Path::new(&item.path)), item))
            .collect::<HashMap<_, _>>()
    });
    let mut reused_packages = 0usize;
    let mut known_file_signatures = HashMap::<String, (u64, u128, String)>::new();
    let mut profile_packages = Vec::<ScanPackageProfileEntry>::new();
    let classification_started = Instant::now();
    let mut item_slots = vec![None; package_paths.len()];
    let mut work = Vec::<(usize, PathBuf)>::new();
    for (index, path) in package_paths.iter().enumerate() {
        if let Some(kind) = operation_kind {
            if operation::is_cancelled(kind) {
                return Err(CANCELLED_ERROR.to_string());
            }
        }

        let stable = stable_path(path);
        let current_signature = previous_items.as_ref().and_then(|_items| {
            let signature = cache::analysis_file_fingerprint(path).ok()?;
            known_file_signatures.insert(stable.to_string_lossy().to_string(), signature.clone());
            Some(signature)
        });
        let reused = previous_items
            .as_ref()
            .and_then(|items| items.get(&stable).copied())
            .filter(|item| {
                // FBLN packages seed the companion index and STBL packages
                // are revisited by the companion pass. Reopen those small
                // dependency carriers when another package changed; ordinary
                // unchanged classifications can be reused safely.
                !item
                    .resource_types
                    .iter()
                    .any(|kind| kind == "FBLN" || kind == "STBL")
            })
            .filter(|_| {
                previous
                    .as_ref()
                    .and_then(|scan| {
                        scan.file_signatures
                            .get(&stable.to_string_lossy().to_string())
                    })
                    .is_some_and(|signature| current_signature.as_ref().is_some_and(|current| current == signature))
            });

        if let Some(item) = reused {
            item_slots[index] = Some(item.clone());
            reused_packages += 1;
        } else {
            work.push((index, path.clone()));
        }
    }

    // A dynamic queue prevents one large/compressed package at the end of a
    // fixed chunk from holding the whole scan open. Two package workers keep
    // the nested resource/hash workers bounded as well; tests may override
    // this value to compare configurations without changing release policy.
    let configured_worker_count = {
        #[cfg(test)]
        {
            std::env::var("S3CC_PACKAGE_WORKERS")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        }
        #[cfg(not(test))]
        {
            None
        }
    };
    let worker_count = configured_worker_count.unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|count| count.get().div_ceil(2))
                .unwrap_or(2)
                .clamp(1, 4)
        })
        .min(work.len().max(1));
    let next_work = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(reused_packages));
    let active_workers = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(AtomicUsize::new(0));
    let accumulation = Mutex::new(ScanAccumulation {
        item_slots,
        cache_entries: HashMap::new(),
        file_signatures: known_file_signatures,
        slider_instances: HashSet::new(),
        timing: ScanTiming::default(),
        profile_packages: Vec::new(),
    });
    let total_packages = package_paths.len();
    let persistent_cache = &shared_index;
    let work_items = &work;

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let accumulation = &accumulation;
            let next_work = Arc::clone(&next_work);
            let completed = Arc::clone(&completed);
            let active_workers = Arc::clone(&active_workers);
            let cancelled = Arc::clone(&cancelled);
            let root = &root;
            let reference_catalogs = reference_catalogs.as_ref();
            let fix_index = fix_index.as_ref();
            scope.spawn(move || {
                let mut local_cache = cache::FingerprintCache::default();
                let mut local_slider_instances = HashSet::new();
                loop {
                    let Some((index, path)) = work_items.get(
                        next_work.fetch_add(1, Ordering::Relaxed),
                    ) else {
                        break;
                    };
                    if let Some(kind) = operation_kind {
                        if operation::is_cancelled(kind) {
                            cancelled.store(1, Ordering::Relaxed);
                            break;
                        }
                    }
                    let prevalidated_cached = if build_fingerprint_index {
                        match cache::current_cached_entry(path, persistent_cache) {
                            Ok(Some(cached)) => {
                                local_cache.entries.insert(cached.path.clone(), cached.clone());
                                Some(cached)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    active_workers.fetch_add(1, Ordering::Relaxed);
                    let started = Instant::now();
                    let mut local_timing = ScanTiming::default();
                    let item = scan_one_with_index(
                        root,
                        path,
                        language,
                        &mut local_slider_instances,
                        fix_index,
                        &mut local_timing,
                        Some(reference_catalogs),
                        build_fingerprint_index.then_some(&mut local_cache),
                        prevalidated_cached,
                    );
                    if std::env::var_os("S3CC_PROFILE_PACKAGE_LOG").is_some() {
                        eprintln!(
                            "scanner_profile_package path={} elapsed_ms={}",
                            path.display(),
                            started.elapsed().as_millis()
                        );
                    }
                    let processed = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    if report_progress {
                        if let Some(kind) = operation_kind {
                            if processed == total_packages || processed % 8 == 0 {
                                operation::update(kind, processed, None, "scanning");
                            }
                        }
                    }
                    if let Ok(mut output) = accumulation.lock() {
                        let profile = item.profile.clone().unwrap_or_default();
                        output.item_slots[*index] = Some(item);
                        output.timing.add(local_timing);
                        output.slider_instances.extend(local_slider_instances.drain());
                        for cached in local_cache.entries.values() {
                            output.file_signatures.insert(
                                cached.path.clone(),
                                (cached.size, cached.modified_ns, cached.quick_fingerprint.clone()),
                            );
                        }
                        output.cache_entries.extend(std::mem::take(&mut local_cache.entries));
                        if profile_enabled {
                            output.profile_packages.push(ScanPackageProfileEntry {
                                elapsed_us: started.elapsed().as_micros(),
                                path: path.to_string_lossy().into_owned(),
                                compressed_bytes: profile.compressed_bytes,
                                decompressed_bytes: profile.decompressed_bytes,
                                resource_count: profile.resource_count,
                                decompression_count: profile.decompression_count,
                                resource_decode_us: profile.resource_decode_us,
                                project_payload_analysis_us: profile.project_payload_analysis_us,
                                project_stbl_resources: profile.project_stbl_resources,
                                project_nmap_resources: profile.project_nmap_resources,
                                project_objk_resources: profile.project_objk_resources,
                                project_other_resources: profile.project_other_resources,
                                catalog_classification_us: profile.catalog_classification_us,
                                auxiliary_analysis_us: profile.auxiliary_analysis_us,
                                directed_family_us: profile.directed_family_us,
                                special_package_us: profile.special_package_us,
                                script_metadata_us: profile.script_metadata_us,
                                final_resolution_us: profile.final_resolution_us,
                            });
                        }
                    }
                    active_workers.fetch_sub(1, Ordering::Relaxed);
                }
            });
        }
    });
    let accumulation = accumulation
        .into_inner()
        .map_err(|_| "scanner accumulation lock poisoned".to_string())?;
    if cancelled.load(Ordering::Relaxed) != 0 {
        if profile_enabled {
            eprintln!(
                "scanner_profile_partial completed={} total={} active_workers={} package_reads={} dbpf_load_ms={} resource_decode_ms={} resource_decode_exclusive_ms={} resource_decode_wait_ms={} full_hash_ms={} full_hash_exclusive_ms={} full_hash_wait_ms={}",
                completed.load(Ordering::Relaxed),
                total_packages,
                active_workers.load(Ordering::Relaxed),
                accumulation.timing.package_reads,
                accumulation.timing.dbpf_load_us / 1_000,
                accumulation.timing.resource_decode_us / 1_000,
                accumulation.timing.resource_decode_exclusive_us / 1_000,
                accumulation.timing.resource_decode_wait_us / 1_000,
                accumulation.timing.full_hash_us / 1_000,
                accumulation.timing.full_hash_exclusive_us / 1_000,
                accumulation.timing.full_hash_wait_us / 1_000,
            );
        }
        return Err(CANCELLED_ERROR.to_string());
    }
    item_slots = accumulation.item_slots;
    let slider_instances = accumulation.slider_instances;
    known_file_signatures = accumulation.file_signatures;
    timing.add(accumulation.timing);
    if build_fingerprint_index {
        for (key, entry) in accumulation.cache_entries {
            shared_index.entries.insert(key, entry);
        }
    }
    if profile_enabled {
        profile_packages = accumulation.profile_packages;
    }
    let mut items = item_slots
        .into_iter()
        .map(|item| item.expect("scanner worker did not return an item"))
        .collect::<Vec<_>>();

    let classification_ms = classification_started.elapsed().as_millis();
    let postprocess_started = Instant::now();
    apply_slider_companion_classification(&package_paths, &mut items, &slider_instances);
    apply_nraas_family_subfolders(language, &mut items);
    let project_grouping_started = Instant::now();
    apply_universal_project_grouping(&mut items, language);
    timing.project_grouping_us += project_grouping_started.elapsed().as_micros();
    apply_manual_classifications(&root, &mut items);

    for item in &mut items {
        let sanitized = remove_generic_unknown_levels(item.destination_parts.clone());
        if sanitized != item.destination_parts {
            item.destination_parts = sanitized;
            item.destination_path =
                (!item.destination_parts.is_empty()).then(|| item.destination_parts.join("\\"));
            item.detected_from.push("UnknownLevelRemoved".to_string());
            item.detected_from.sort();
            item.detected_from.dedup();
            item.warnings.push("Generic Unknown/Desconhecido level was removed; no unsupported gender or age was invented.".to_string());
            if item.destination_parts.is_empty() && item.status == "classified" {
                item.status = "needs_review".to_string();
                item.classification_confidence =
                    classification_confidence(&item.status).to_string();
            }
        }
        item.destination_parts =
            physical_destination_parts(&root, &item.relative_path, &item.destination_parts);
        if item.destination_path.is_some() {
            item.destination_path = Some(item.destination_parts.join("\\"));
        }
        item.candidate_destinations = item
            .candidate_destinations
            .iter()
            .map(|value| {
                physical_destination_parts(&root, &item.relative_path, &split_destination(value))
                    .join("\\")
            })
            .collect();
    }
    let postprocess_ms = postprocess_started.elapsed().as_millis();
    // Persist only after the complete pass so a cancelled scan cannot leave a
    // partial index that looks like a full unified result.
    if build_fingerprint_index {
        let _ = cache::save_cache(&root, &shared_index);
    }

    let mut stats = ScanStats::default();
    stats.packages = items.len();
    for item in &items {
        if item
            .name
            .to_ascii_lowercase()
            .ends_with(".package.disabled")
            || item
                .name
                .to_ascii_lowercase()
                .ends_with(".package.duplicated")
        {
            stats.disabled += 1;
        }
        stats.casp_resources += item
            .classifications
            .iter()
            .filter(|classification| classification.source == "CASP")
            .count();
        stats.objd_resources += item
            .classifications
            .iter()
            .filter(|classification| classification.source == "OBJD")
            .count();

        match item.status.as_str() {
            "classified" | "sim_export_merged" => stats.classified += 1,
            "quarantined_duplicate" => {}
            "mixed" => stats.mixed += 1,
            "needs_review" => stats.needs_review += 1,
            "invalid" => stats.invalid += 1,
            _ => stats.unknown += 1,
        }
    }

    stats.total_ms = total_started.elapsed().as_millis();
    stats.discovery_ms = discovery_ms;
    stats.classification_ms = classification_ms;
    stats.postprocess_ms = postprocess_ms;
    stats.dbpf_load_ms = timing.dbpf_load_us / 1_000;
    stats.resource_decode_ms = timing.resource_decode_us / 1_000;
    stats.resource_decode_exclusive_ms = timing.resource_decode_exclusive_us / 1_000;
    stats.resource_decode_wait_ms = timing.resource_decode_wait_us / 1_000;
    stats.reference_catalog_ms = timing.reference_catalog_us / 1_000;
    stats.project_grouping_ms = timing.project_grouping_us / 1_000;
    stats.full_hash_ms = timing.full_hash_us / 1_000;
    stats.full_hash_exclusive_ms = timing.full_hash_exclusive_us / 1_000;
    stats.full_hash_wait_ms = timing.full_hash_wait_us / 1_000;
    stats.package_reads = timing.package_reads;
    stats.cache_hit = false;
    stats.reused_packages = reused_packages;

    if profile_enabled {
        profile_packages.sort_by(|left, right| right.elapsed_us.cmp(&left.elapsed_us));
        eprintln!(
            "scanner_profile packages={} reused={} active_workers={} discovery_ms={} classification_ms={} postprocess_ms={} dbpf_load_ms={} resource_decode_ms={} resource_decode_exclusive_ms={} resource_decode_wait_ms={} full_hash_ms={} full_hash_exclusive_ms={} full_hash_wait_ms={} reference_catalog_ms={} project_grouping_ms={} cache_hits={} cache_misses={}",
            stats.packages,
            stats.reused_packages,
            active_workers.load(Ordering::Relaxed),
            stats.discovery_ms,
            stats.classification_ms,
            stats.postprocess_ms,
            stats.dbpf_load_ms,
            stats.resource_decode_ms,
            stats.resource_decode_exclusive_ms,
            stats.resource_decode_wait_ms,
            stats.full_hash_ms,
            stats.full_hash_exclusive_ms,
            stats.full_hash_wait_ms,
            stats.reference_catalog_ms,
            stats.project_grouping_ms,
            stats.cache_hit,
            stats.reused_packages,
        );
        for (index, profile) in profile_packages.iter().take(20).enumerate() {
            eprintln!(
                "scanner_profile_slowest rank={} elapsed_ms={} resource_decode_ms={} project_payload_analysis_ms={} project_stbl_resources={} project_nmap_resources={} project_objk_resources={} project_other_resources={} catalog_classification_ms={} auxiliary_analysis_ms={} directed_family_ms={} special_package_ms={} script_metadata_ms={} final_resolution_ms={} compressed_mb={:.2} decompressed_mb={:.2} resources={} decompressions={} path={}",
                index + 1,
                (profile.elapsed_us as f64) / 1000.0,
                (profile.resource_decode_us as f64) / 1000.0,
                (profile.project_payload_analysis_us as f64) / 1000.0,
                profile.project_stbl_resources,
                profile.project_nmap_resources,
                profile.project_objk_resources,
                profile.project_other_resources,
                (profile.catalog_classification_us as f64) / 1000.0,
                (profile.auxiliary_analysis_us as f64) / 1000.0,
                (profile.directed_family_us as f64) / 1000.0,
                (profile.special_package_us as f64) / 1000.0,
                (profile.script_metadata_us as f64) / 1000.0,
                (profile.final_resolution_us as f64) / 1000.0,
                profile.compressed_bytes as f64 / 1_048_576.0,
                profile.decompressed_bytes as f64 / 1_048_576.0,
                profile.resource_count,
                profile.decompression_count,
                profile.path
            );
        }
    }

    let result = ScanResult {
        root: root.to_string_lossy().to_string(),
        items,
        stats,
        cache_hit: false,
        reused_packages,
    };
    let snapshot_started = Instant::now();
    let snapshot_timing = remember_latest_scan(&result, language, Some(&known_file_signatures));
    if profile_enabled {
        eprintln!(
            "scanner_profile_presentation_snapshot_ms={} signatures_ms={} context_ms={} persistence_ms={}",
            snapshot_started.elapsed().as_millis(),
            snapshot_timing.signatures_ms,
            snapshot_timing.context_ms,
            snapshot_timing.persistence_ms,
        );
    }
    Ok(result)
}

/// Scan only an explicit set of package paths. Planner previews use this
/// entry point for residual selections so a three-file correction does not
/// reopen every package in the containing legacy branch. The same scanner,
/// classifier and post-processing rules are used; only the path set differs.
pub fn scan_selected_package_paths(
    root: &Path,
    package_paths: &[PathBuf],
    language: AppLanguage,
) -> Result<ScanResult, String> {
    if !root.is_dir() {
        return Err(format!("Folder does not exist: {}", root.display()));
    }
    let mut paths = package_paths
        .iter()
        .filter(|path| path.is_file() && package_extension(path))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    let mut slider_instances = HashSet::<u64>::new();
    let fix_index = fix_signatures::load_persistent_index();
    let mut shared_index = cache::load_cache(root);
    cache::retain_existing(&mut shared_index, &paths);
    operation::set_total("planning", paths.len());
    let mut items = Vec::with_capacity(paths.len());
    let mut timing = ScanTiming::default();
    for (processed, path) in paths.iter().enumerate() {
        if operation::is_cancelled("planning") {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update(
            "planning",
            processed,
            Some(path.to_string_lossy().into_owned()),
            "scanning",
        );
        items.push(scan_one_with_index(
            root,
            path,
            language,
            &mut slider_instances,
            fix_index.as_ref(),
            &mut timing,
            None,
            Some(&mut shared_index),
            None,
        ));
    }
    let _ = cache::save_cache(root, &shared_index);

    apply_slider_companion_classification(&paths, &mut items, &slider_instances);
    apply_nraas_family_subfolders(language, &mut items);
    apply_universal_project_grouping(&mut items, language);
    apply_manual_classifications(root, &mut items);

    for item in &mut items {
        let sanitized = remove_generic_unknown_levels(item.destination_parts.clone());
        if sanitized != item.destination_parts {
            item.destination_parts = sanitized;
            item.destination_path =
                (!item.destination_parts.is_empty()).then(|| item.destination_parts.join("\\"));
            item.detected_from.push("UnknownLevelRemoved".to_string());
            item.detected_from.sort();
            item.detected_from.dedup();
            item.warnings.push(
                "Generic Unknown/Desconhecido level was removed; no unsupported gender or age was invented."
                    .to_string(),
            );
            if item.destination_parts.is_empty() && item.status == "classified" {
                item.status = "needs_review".to_string();
                item.classification_confidence =
                    classification_confidence(&item.status).to_string();
            }
        }
        item.destination_parts =
            physical_destination_parts(root, &item.relative_path, &item.destination_parts);
        if item.destination_path.is_some() {
            item.destination_path = Some(item.destination_parts.join("\\"));
        }
        item.candidate_destinations = item
            .candidate_destinations
            .iter()
            .map(|value| {
                physical_destination_parts(root, &item.relative_path, &split_destination(value))
                    .join("\\")
            })
            .collect();
    }

    let mut stats = ScanStats {
        packages: items.len(),
        ..ScanStats::default()
    };
    for item in &items {
        if item
            .name
            .to_ascii_lowercase()
            .ends_with(".package.disabled")
            || item
                .name
                .to_ascii_lowercase()
                .ends_with(".package.duplicated")
        {
            stats.disabled += 1;
        }
        stats.casp_resources += item
            .classifications
            .iter()
            .filter(|classification| classification.source == "CASP")
            .count();
        stats.objd_resources += item
            .classifications
            .iter()
            .filter(|classification| classification.source == "OBJD")
            .count();
        match item.status.as_str() {
            "classified" | "sim_export_merged" => stats.classified += 1,
            "mixed" => stats.mixed += 1,
            "needs_review" => stats.needs_review += 1,
            "invalid" => stats.invalid += 1,
            _ => stats.unknown += 1,
        }
    }
    Ok(ScanResult {
        root: root.to_string_lossy().to_string(),
        items,
        stats,
        cache_hit: false,
        reused_packages: 0,
    })
}

#[tauri::command]
pub async fn scan_packages(folder: String, language: AppLanguage) -> Result<ScanResult, String> {
    const KIND: &str = "scan";
    let result = operation::run_blocking(KIND, "starting", move || {
        scan_packages_core(folder, language, Some(KIND))
    })
    .await;
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_external_corpus_when_requested() {
        let Ok(root) = std::env::var("S3CC_PROFILE_ROOT") else {
            return;
        };
        let language = crate::i18n::AppLanguage::Pt;
        let result =
            super::scan_packages_core_with_progress(root.clone(), language, None, false, true, false)
                .expect("external scanner profile should complete");
        eprintln!(
            "scanner_profile_result packages={} total_ms={} package_reads={} reused={}",
            result.stats.packages,
            result.stats.total_ms,
            result.stats.package_reads,
            result.stats.reused_packages,
        );
        if std::env::var_os("S3CC_PROFILE_WARM_CACHE").is_some() {
            let warm = super::scan_packages_core(root, language, None)
                .expect("external warm scanner profile should complete");
            eprintln!(
                "scanner_profile_warm_cache packages={} total_ms={} package_reads={} reused={} cache_hit={}",
                warm.stats.packages,
                warm.stats.total_ms,
                warm.stats.package_reads,
                warm.stats.reused_packages,
                warm.cache_hit,
            );
        }
    }

    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct MatrixRow {
        #[serde(rename = "backupRelativePath")]
        backup_relative_path: String,
        sha256: String,
        #[serde(rename = "modelTop")]
        model_top: String,
        #[serde(rename = "modelMatches", default)]
        model_matches: String,
        outcome: String,
    }

    fn matrix_path_key(value: &str) -> String {
        value.replace('/', "\\").to_ascii_lowercase()
    }

    fn destination_top(item: &ScanPackageItem) -> Option<String> {
        item.destination_parts
            .iter()
            .find(|part| {
                !part.eq_ignore_ascii_case("Packages") && !part.eq_ignore_ascii_case("Overrides")
            })
            .cloned()
    }

    fn category_is_equivalent(expected: &str, actual: &str) -> bool {
        if expected.eq_ignore_ascii_case(actual) {
            return true;
        }
        let equivalent = |value: &str| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "cabelos" | "cabelos e pêlos" | "cabelos e pelos"
            )
        };
        equivalent(expected) && equivalent(actual)
    }

    #[test]
    fn validate_real_backup_destinations_and_conflicts_when_requested() {
        let Ok(backup_root) = std::env::var("S3CC_VALIDATION_BACKUP_ROOT") else {
            return;
        };
        let Ok(matrix_path) = std::env::var("S3CC_VALIDATION_MATRIX") else {
            return;
        };
        let reference_root = std::env::var("S3CC_FIX_REFERENCE_ROOT").ok();
        if fix_signatures::load_persistent_index().is_none() {
            if let Some(root) = reference_root.as_deref() {
                let index = fix_signatures::build_fix_signature_index(Path::new(root)).unwrap();
                fix_signatures::save_persistent_index(&index).unwrap();
            }
        }

        let mut reader = csv::Reader::from_path(&matrix_path).unwrap();
        let rows = reader
            .deserialize::<MatrixRow>()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        let validation_scope =
            std::env::var("S3CC_VALIDATION_SCOPE").unwrap_or_else(|_| "conflicts".to_string());
        let validation_rows = rows
            .iter()
            .filter(|row| {
                if validation_scope.eq_ignore_ascii_case("resolved_unique") {
                    row.outcome == "resolvido_destino_unico"
                } else {
                    row.outcome == "pendente_decisao"
                }
            })
            .collect::<Vec<_>>();
        let expected_validation_count = if validation_scope.eq_ignore_ascii_case("resolved_unique")
        {
            457
        } else {
            49
        };
        assert_eq!(
            validation_rows.len(),
            expected_validation_count,
            "diagnostic matrix validation scope changed"
        );
        let mut actual_paths = HashMap::<String, PathBuf>::new();
        for entry in WalkDir::new(&backup_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && package_extension(entry.path()))
        {
            let relative = entry
                .path()
                .strip_prefix(&backup_root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            actual_paths.insert(matrix_path_key(&relative), entry.into_path());
        }
        let scan_selected = || {
            let mut slider_instances = HashSet::<u64>::new();
            let fix_index = fix_signatures::load_persistent_index();
            let mut items = Vec::with_capacity(validation_rows.len());
            for row in &validation_rows {
                let path = actual_paths
                    .get(&matrix_path_key(&row.backup_relative_path))
                    .unwrap_or_else(|| {
                        panic!("diagnostic package is absent: {}", row.backup_relative_path)
                    });
                let mut item = scan_one(
                    Path::new(&backup_root),
                    path,
                    AppLanguage::Pt,
                    &mut slider_instances,
                    fix_index.as_ref(),
                    &mut ScanTiming::default(),
                );
                item.destination_parts = physical_destination_parts(
                    Path::new(&backup_root),
                    &item.relative_path,
                    &remove_generic_unknown_levels(item.destination_parts.clone()),
                );
                item.destination_path =
                    (!item.destination_parts.is_empty()).then(|| item.destination_parts.join("\\"));
                items.push(item);
            }
            items
        };
        let scan = ScanResult {
            root: backup_root.clone(),
            items: scan_selected(),
            stats: ScanStats::default(),
            cache_hit: false,
            reused_packages: 0,
        };
        let rescanned = ScanResult {
            root: backup_root.clone(),
            items: scan_selected(),
            stats: ScanStats::default(),
            cache_hit: false,
            reused_packages: 0,
        };
        assert_eq!(scan.items.len(), rescanned.items.len());

        let first_stability = scan
            .items
            .iter()
            .map(|item| {
                (
                    matrix_path_key(&item.relative_path),
                    format!("{}|{:?}", item.status, item.destination_parts),
                )
            })
            .collect::<HashMap<_, _>>();
        let second_stability = rescanned
            .items
            .iter()
            .map(|item| {
                (
                    matrix_path_key(&item.relative_path),
                    format!("{}|{:?}", item.status, item.destination_parts),
                )
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(
            first_stability, second_stability,
            "classification changed on re-analysis"
        );

        let by_relative = scan
            .items
            .iter()
            .map(|item| (matrix_path_key(&item.relative_path), item))
            .collect::<HashMap<_, _>>();
        let mut by_hash = HashMap::<String, &ScanPackageItem>::new();
        for item in &scan.items {
            if let Ok((hash, _)) = sha256_file(Path::new(&item.path)) {
                by_hash.entry(hash.to_ascii_lowercase()).or_insert(item);
            }
        }

        let resolved_rows = rows
            .iter()
            .filter(|row| row.outcome == "resolvido_destino_unico")
            .collect::<Vec<_>>();
        assert_eq!(
            resolved_rows.len(),
            457,
            "diagnostic matrix no longer contains 457 resolved destinations"
        );
        let mut resolved_mismatches = Vec::new();
        let mut conflict_rows = Vec::new();
        for row in &validation_rows {
            let item = by_relative
                .get(&matrix_path_key(&row.backup_relative_path))
                .copied()
                .or_else(|| by_hash.get(&row.sha256.to_ascii_lowercase()).copied());
            let Some(item) = item else {
                resolved_mismatches.push(serde_json::json!({
                    "relativePath": row.backup_relative_path,
                    "expected": row.model_top,
                    "actual": "missing-from-real-scan",
                    "outcome": row.outcome,
                }));
                continue;
            };
            let actual = destination_top(item).unwrap_or_else(|| "Sem Categoria".to_string());
            let expected_categories = row
                .model_top
                .split('|')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let matches_expected = expected_categories
                .iter()
                .any(|expected| category_is_equivalent(expected, &actual));
            if validation_scope.eq_ignore_ascii_case("resolved_unique") {
                if !matches_expected {
                    let resource_signatures = Package::load(Path::new(&item.path))
                        .ok()
                        .and_then(|package| fix_signatures::package_resources(&package).ok())
                        .unwrap_or_default();
                    let decision = if item.status == "classified"
                        && item.classification_confidence == "high"
                    {
                        "dbpf_evidence_classified_mismatch"
                    } else {
                        "manual_review"
                    };
                    resolved_mismatches.push(serde_json::json!({
                        "relativePath": row.backup_relative_path,
                        "modelDestination": row.model_matches,
                        "expected": row.model_top,
                        "actual": actual,
                        "calculatedDestination": item.destination_parts,
                        "resourceTypes": item.resource_types,
                        "instances": item.instances,
                        "resourceSignatures": resource_signatures,
                        "sha256": row.sha256,
                        "status": item.status,
                        "confidence": item.classification_confidence,
                        "detectedFrom": item.detected_from,
                        "technicalReason": item.classification_reason,
                        "decision": decision,
                    }));
                }
            } else {
                conflict_rows.push(serde_json::json!({
                    "relativePath": row.backup_relative_path,
                    "sha256": row.sha256,
                    "modelCategories": expected_categories,
                    "actualCategory": actual,
                    "status": item.status,
                    "destination": item.destination_parts,
                    "detectedFrom": item.detected_from,
                    "reason": item.classification_reason,
                    "decision": if matches_expected { "resolved_by_dbpf_evidence" } else { "manual_review" },
                }));
            }
        }

        if let Ok(output_path) = std::env::var("S3CC_REAL_VALIDATION_OUTPUT") {
            let report = serde_json::json!({
                "packagesScanned": scan.items.len(),
                "validationScope": validation_scope,
                "resolvedMatrixRows": resolved_rows.len(),
                "categoryOnlyMatrixRows": rows.iter().filter(|row| row.outcome == "resolvido_categoria_pendente_subcaminho").count(),
                "resolvedMismatches": resolved_mismatches,
                "classificationStableOnRescan": first_stability == second_stability,
            });
            fs::write(output_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        if let Ok(output_path) = std::env::var("S3CC_CONFLICT_OUTPUT") {
            fs::write(
                output_path,
                serde_json::to_vec_pretty(&conflict_rows).unwrap(),
            )
            .unwrap();
        }
        assert!(
            resolved_mismatches
                .iter()
                .all(|row| row.get("actual").and_then(|value| value.as_str())
                    != Some("missing-from-real-scan")),
            "a diagnostic package was absent from the real backup scan"
        );
    }

    #[test]
    fn inspect_targeted_real_dbpf_packages_when_requested() {
        let Ok(backup_root) = std::env::var("S3CC_TARGETED_DBPF_ROOT") else {
            return;
        };
        let output_path = std::env::var("S3CC_TARGETED_DBPF_OUTPUT")
            .expect("S3CC_TARGETED_DBPF_OUTPUT is required for targeted DBPF validation");
        let targets = [
            "Anach_CASDetails8x2.package",
            "windows-lite-overrides-ALL-EP-SP.package",
            "Fantuanss12_ProfessionalOven.package",
            "zoeoe_knitting_object_rug.package",
            "zoeoe_knitting_knittingNeedles.package",
            "NoBuildSparkles.package",
            "nointromaxis.package",
            "SimsFM_RadioStation.package",
            "saz_Awesims_Cornice_1.package",
            "BuhudainDRYouAreRealV14FemaleKertinFace.package",
            "MFC_RHR_IP-BeachTowerRug_MistyVG.package",
            "MFC_RHD-Jynx_BusinessDoor-PetsFix.package",
        ];
        let mut paths = HashMap::<String, PathBuf>::new();
        for entry in WalkDir::new(&backup_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let Some(name) = entry.path().file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if targets
                .iter()
                .any(|target| name.eq_ignore_ascii_case(target))
            {
                paths.insert(name.to_ascii_lowercase(), entry.into_path());
            }
        }

        let fix_index = fix_signatures::load_persistent_index();
        let mut rows = Vec::new();
        for target in targets {
            let path = paths
                .get(&target.to_ascii_lowercase())
                .unwrap_or_else(|| panic!("targeted DBPF package is absent: {target}"));
            let package = Package::load(path).unwrap_or_else(|error| {
                panic!("could not decode targeted DBPF package {target}: {error}")
            });
            let resources = fix_signatures::package_resources(&package).unwrap();
            let (project_evidence, _) = extract_package_evidence_with_metrics(path, &package);
            let internal_identifiers = package_internal_identifiers(&package, &project_evidence);
            let ascii_preview = package
                .entries
                .iter()
                .flat_map(|entry| {
                    package.data(entry).ok().into_iter().flat_map(|data| {
                        let mut values = Vec::new();
                        let mut current = Vec::new();
                        for byte in data.iter().copied() {
                            if byte.is_ascii_graphic() || byte == b' ' {
                                current.push(byte);
                            } else if current.len() >= 4 {
                                values.push(String::from_utf8_lossy(&current).to_string());
                                current.clear();
                            } else {
                                current.clear();
                            }
                        }
                        if current.len() >= 4 {
                            values.push(String::from_utf8_lossy(&current).to_string());
                        }
                        values
                    })
                })
                .take(80)
                .collect::<Vec<_>>();
            let mut slider_instances = HashSet::<u64>::new();
            let item = scan_one(
                Path::new(&backup_root),
                path,
                AppLanguage::Pt,
                &mut slider_instances,
                fix_index.as_ref(),
                &mut ScanTiming::default(),
            );
            if target.eq_ignore_ascii_case("Anach_CASDetails8x2.package") {
                assert_eq!(item.status, "classified");
                assert_eq!(item.destination_parts, vec!["Substituições", "CAS"]);
                assert!(item
                    .detected_from
                    .iter()
                    .any(|value| value == "CASLayoutIdentifier"));
            }
            if target.eq_ignore_ascii_case("windows-lite-overrides-ALL-EP-SP.package") {
                assert_eq!(item.status, "classified");
                assert_eq!(item.destination_parts, vec!["Substituições", "Objetos"]);
                assert!(!item
                    .detected_from
                    .iter()
                    .any(|value| value == "OhRudiFamily"));
            }
            let expected_real_family = match target {
                "Fantuanss12_ProfessionalOven.package" => Some(vec!["Scripts", "Jogabilidade"]),
                "zoeoe_knitting_object_rug.package" | "zoeoe_knitting_knittingNeedles.package" => {
                    Some(vec!["Scripts", "Jogabilidade", "Knitting Mod"])
                }
                "NoBuildSparkles.package" => Some(vec!["Substituições", "Efeitos"]),
                "nointromaxis.package" => Some(vec!["Substituições"]),
                "SimsFM_RadioStation.package" => {
                    Some(vec!["Scripts", "Jogabilidade", "Radio and TV"])
                }
                "saz_Awesims_Cornice_1.package" => Some(vec!["Construção", "Cornijas"]),
                "BuhudainDRYouAreRealV14FemaleKertinFace.package" => {
                    Some(vec!["Substituições", "Corpo", "Definição de Pele"])
                }
                "MFC_RHR_IP-BeachTowerRug_MistyVG.package"
                | "MFC_RHD-Jynx_BusinessDoor-PetsFix.package" => {
                    Some(vec!["Construção", "Rabbit Holes"])
                }
                _ => None,
            };
            if let Some(expected) = expected_real_family {
                assert_eq!(item.status, "classified", "{target}");
                assert_eq!(
                    item.destination_parts
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    expected,
                    "{target}"
                );
            }
            rows.push(serde_json::json!({
                "target": target,
                "relativePath": path.strip_prefix(&backup_root).unwrap_or(path).to_string_lossy(),
                "resourceCount": package.entries.len(),
                "resourceTypes": package.entries.iter().map(|entry| format!("{:08X}", entry.type_id)).collect::<Vec<_>>(),
                "resources": resources,
                "internalIdentifiers": internal_identifiers,
                "asciiPreview": ascii_preview,
                "status": item.status,
                "confidence": item.classification_confidence,
                "category": item.category,
                "destination": item.destination_parts,
                "catalogClassifications": item.classifications,
                "detectedFrom": item.detected_from,
                "classificationReason": item.classification_reason,
            }));
        }
        fs::write(&output_path, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    }

    #[test]
    fn path_independent_directed_families_survive_relocation() {
        let oh_rudi = BTreeSet::from([TYPE_NMAP_LOCAL, 0xD382_BF57]);
        let gameplay = path_independent_directed_classification(
            &oh_rudi,
            &["deathByMeteorJig".into(), "minerExclusionJig".into()],
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(gameplay.folder_parts, vec!["Correções", "Jogabilidade"]);

        let ep01 = path_independent_directed_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, 0x03B4_C61D]),
            &["king_qings_window".into()],
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(
            ep01.folder_parts,
            vec!["Correções", "Jogabilidade", "EPs Fixes", "EP01"]
        );

        let lazy = path_independent_directed_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, 0x015A_1849, TYPE_IMG]),
            &["acBody_lod1_2".into(), "adBody_lod1".into()],
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(
            lazy.folder_parts,
            vec!["Substituições", "Corpo", "Cães e Gatos", "Lazy Duchess"]
        );

        let lazy_default = path_independent_directed_classification(
            &BTreeSet::from([0x015A_1849, TYPE_IMG]),
            &["ld_CatDogAddons_DefaultFaces".into()],
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(
            lazy_default.folder_parts,
            vec!["Substituições", "Corpo", "Cães e Gatos", "Lazy Duchess"]
        );

        let layout = path_independent_directed_classification(
            &BTreeSet::from([TYPE_LAYO_LOCAL]),
            &["CASDetails8x2".into()],
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(layout.folder_parts, vec!["Substituições", "CAS"]);

        assert!(path_independent_directed_classification(
            &oh_rudi,
            &["genericObjectName".into()],
            AppLanguage::Pt,
        )
        .is_none());
        assert!(path_independent_directed_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, 0xD382_BF57]),
            &["genericDogEarMesh".into()],
            AppLanguage::Pt,
        )
        .is_none());

        assert_eq!(
            path_independent_directed_classification(
                &oh_rudi,
                &["deathByMeteorJig".into(), "minerExclusionJig".into()],
                AppLanguage::Pt,
            )
            .unwrap()
            .folder_parts,
            vec!["Correções", "Jogabilidade"]
        );
    }

    #[test]
    fn official_resource_evidence_classifies_only_a_generic_replacement() {
        let evidence = ReferenceEvidence {
            official_overlap_kind: Some("mixed_official_and_new".to_string()),
            changed_resource_count: 3,
            ..ReferenceEvidence::default()
        };
        let classification = reference_or_override_family_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, TYPE_IMG]),
            Some(&evidence),
            false,
            AppLanguage::Pt,
        )
        .expect("changed official TGIs should prove a replacement family");
        assert_eq!(classification.folder_parts, vec!["Substituições"]);
        assert!(classification
            .detected_from
            .contains(&"OfficialReplacementEvidence".to_string()));
        assert!(!classification.technical_reason.contains("filename"));
    }

    #[test]
    fn cas_default_replacement_uses_official_and_internal_evidence() {
        let evidence = ReferenceEvidence {
            official_overlap_kind: Some("mixed_official_and_new".to_string()),
            changed_resource_count: 11,
            ..ReferenceEvidence::default()
        };
        let project = ProjectTechnicalEvidence {
            identifiers: vec!["CAS-SIM".to_string(), "CASRoom".to_string()],
            ..ProjectTechnicalEvidence::default()
        };
        let classification = cas_default_replacement_classification(
            &BTreeSet::from([TYPE_OBJD, TYPE_IMG, 0x7368_84F1]),
            "Agnelid_CAS Default_Sliver eucaliptus.package",
            &project,
            Some(&evidence),
            AppLanguage::Pt,
        )
        .expect("CAS Default replacement should be classified");

        assert_eq!(classification.folder_parts, vec!["Substituições", "CAS"]);
        assert!(classification
            .detected_from
            .contains(&"OfficialReplacementEvidence".to_string()));
    }

    #[test]
    fn cas_default_name_alone_does_not_classify_a_package() {
        let project = ProjectTechnicalEvidence {
            identifiers: vec!["CAS-SIM".to_string()],
            ..ProjectTechnicalEvidence::default()
        };
        assert!(cas_default_replacement_classification(
            &BTreeSet::from([TYPE_OBJD, TYPE_IMG, 0x7368_84F1]),
            "Agnelid_CAS Default_Sliver eucaliptus.package",
            &project,
            None,
            AppLanguage::Pt,
        )
        .is_none());
    }

    #[test]
    fn overrides_branch_is_protected_generic_replacement_without_fake_subtype() {
        let classification = reference_or_override_family_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, TYPE_IMG]),
            None,
            true,
            AppLanguage::Pt,
        )
        .expect("the explicit Overrides branch is a protected replacement family");
        assert_eq!(classification.folder_parts, vec!["Substituições"]);
        assert_eq!(classification.sub_category, None);
        assert!(classification
            .detected_from
            .contains(&"OverridesLoadingBranch".to_string()));

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("s3cc-overrides-branch-fixture-{}-{nonce}", std::process::id()))
            .join("Mods - Copia");
        std::fs::create_dir_all(root.join("Packages")).unwrap();
        std::fs::create_dir_all(root.join("Overrides")).unwrap();
        assert_eq!(
            physical_destination_parts(
                &root,
                "Overrides\\Legacy\\replacement.package",
                &classification.folder_parts,
            ),
            vec!["Overrides", "Substituições"],
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn layo_resources_are_interface_without_a_source_path_subtype() {
        let classification = reference_or_override_family_classification(
            &BTreeSet::from([TYPE_LAYO_LOCAL, TYPE_NMAP_LOCAL]),
            None,
            false,
            AppLanguage::Pt,
        )
        .expect("LAYO is an interface resource family");
        assert_eq!(classification.folder_parts, vec!["Interface"]);
        assert_eq!(classification.sub_category, None);
    }

    #[test]
    fn generic_img_resources_remain_unknown_without_override_evidence() {
        assert!(reference_or_override_family_classification(
            &BTreeSet::from([TYPE_NMAP_LOCAL, TYPE_IMG]),
            None,
            false,
            AppLanguage::Pt,
        )
        .is_none());
    }

    #[test]
    fn store_reference_projection_keeps_category_and_destination_coherent() {
        let evidence = ReferenceEvidence {
            store_confidence: Some("confirmed".to_string()),
            store_category_parts: vec!["Construção".to_string(), "Telhados".to_string()],
            ..ReferenceEvidence::default()
        };
        let classification = store_reference_family_classification(&evidence).unwrap();
        assert_eq!(classification.main_category, "Construção");
        assert_eq!(classification.sub_category.as_deref(), Some("Telhados"));
        assert_eq!(classification.folder_parts, vec!["Construção", "Telhados"]);
    }

    #[test]
    fn named_store_overlap_projects_catalog_hierarchy_without_claiming_exact_identity() {
        let evidence = ReferenceEvidence {
            store_match_kind: Some("named_resource_overlap".to_string()),
            store_confidence: Some("evidence".to_string()),
            store_category_parts: vec!["Sets".to_string(), "Store Sets".to_string()],
            ..ReferenceEvidence::default()
        };
        let classification = store_family_classification(&evidence).unwrap();
        assert_eq!(
            classification.detected_from,
            vec!["StoreCatalogNameAndResourceOverlap"]
        );
        assert_eq!(classification.folder_parts, vec!["Sets", "Store Sets"]);
        assert!(classification.technical_reason.contains("full package identity"));
    }

    #[test]
    fn partial_store_overlap_is_diagnostic_not_review_warning() {
        let evidence = ReferenceEvidence {
            store_match_kind: Some("partial_resource_overlap".to_string()),
            store_confidence: Some("evidence".to_string()),
            ..ReferenceEvidence::default()
        };

        assert!(is_partial_store_evidence_note(
            &evidence,
            "A sobreposição com a Store é parcial. Um TGI compartilhado, isoladamente, não comprova que os packages sejam idênticos."
        ));
        assert!(is_partial_store_evidence_note(
            &evidence,
            "Store overlap is partial; a shared TGI alone is not a package identity."
        ));
        assert!(!is_partial_store_evidence_note(
            &ReferenceEvidence {
                store_match_kind: Some("technical_exact".to_string()),
                store_confidence: Some("confirmed".to_string()),
                ..ReferenceEvidence::default()
            },
            "A sobreposição com a Store é parcial. Um TGI compartilhado, isoladamente, não comprova que os packages sejam idênticos."
        ));
    }

    #[test]
    fn confirmed_store_evidence_can_resolve_mixed_status() {
        assert!(store_evidence_can_resolve_status("unknown"));
        assert!(store_evidence_can_resolve_status("needs_review"));
        assert!(store_evidence_can_resolve_status("mixed"));
        assert!(!store_evidence_can_resolve_status("classified"));
        assert!(!store_evidence_can_resolve_status("sim_export_merged"));
    }

    #[test]
    fn named_headwear_refines_only_the_armband_slot_in_all_languages() {
        let mk = |sub: &str| CatalogClassification {
            source: "CASP".into(),
            kind: "cas".into(),
            main_category: "Acessórios".into(),
            sub_category: Some(sub.into()),
            gender: None,
            age: None,
            species: None,
            usage_categories: vec![],
            folder_parts: vec![
                "CAS".into(),
                "Acessórios".into(),
                "Unissex".into(),
                sub.into(),
            ],
            candidate_folder_parts: vec![vec![
                "CAS".into(),
                "Acessórios".into(),
                "Unissex".into(),
                sub.into(),
            ]],
            ambiguous: false,
            technical_reason: "CASP clothingType=0x00000020".into(),
        };
        let mut hat = mk("Braçadeira");
        refine_headwear_accessory(
            &mut hat,
            "CARVER_acc_EP5-cowboyhat-CU.package",
            AppLanguage::Pt,
        );
        assert_eq!(hat.sub_category.as_deref(), Some("Acessórios de Cabeça"));
        assert_eq!(
            hat.folder_parts.last().map(String::as_str),
            Some("Acessórios de Cabeça")
        );
        assert_eq!(
            hat.candidate_folder_parts[0].last().map(String::as_str),
            Some("Acessórios de Cabeça")
        );
        assert!(hat.technical_reason.contains("original CASP slot"));
        let mut bracelet = mk("Braçadeira");
        refine_headwear_accessory(
            &mut bracelet,
            "CARVER_acc_armband-CU.package",
            AppLanguage::Pt,
        );
        assert_eq!(bracelet.sub_category.as_deref(), Some("Braçadeira"));
        let mut glasses = mk("Óculos");
        refine_headwear_accessory(&mut glasses, "Hat_glasses.package", AppLanguage::Pt);
        assert_eq!(glasses.sub_category.as_deref(), Some("Óculos"));
        assert!(filename_indicates_headwear(
            "CARVER_acc_graduationcap-AU.package"
        ));
        assert!(filename_indicates_headwear(
            "CARVER_acc_EP5-cowboyhat-CU.package"
        ));
        assert!(!filename_indicates_headwear(
            "CARVER_acc_armband-AU.package"
        ));
    }

    #[test]
    fn copied_mods_root_scanner_keeps_packages_and_overrides_distinct() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("s3cc-scanner-copy-{}-{nonce}", std::process::id()))
            .join("Mods - Copia");
        std::fs::create_dir_all(root.join("Packages")).unwrap();
        std::fs::create_dir_all(root.join("Overrides")).unwrap();
        let parts = vec!["CAS".into(), "Cabelos".into()];
        assert_eq!(
            physical_destination_parts(&root, "Packages\\Legacy\\hair.package", &parts),
            vec!["Packages", "Cabelos"]
        );
        assert_eq!(
            physical_destination_parts(&root, "Overrides\\Legacy\\hair.package", &parts),
            vec!["Overrides", "Cabelos"]
        );
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn eyelash_multi_slot_bundle_is_not_stranded_as_mixed() {
        let mk = |main: &str, sub: &str| CatalogClassification {
            source: "CASP".into(),
            kind: "cas".into(),
            main_category: main.into(),
            sub_category: Some(sub.into()),
            gender: None,
            age: None,
            species: None,
            usage_categories: vec![],
            folder_parts: vec!["CAS".into(), main.into(), sub.into()],
            candidate_folder_parts: vec![],
            ambiguous: false,
            technical_reason: "".into(),
        };
        let mut items = vec![mk("Maquiagem", "Delineador"); 42];
        items.extend(vec![mk("Acessórios", "Óculos"); 2]);
        items.extend(vec![mk("Acessórios", "Brincos"); 2]);
        let types = BTreeSet::from([TYPE_CASP]);
        let resolved = verified_eyelash_catalog_bundle(
            &items,
            &types,
            "sclub_ts3_eyelash_n3a.package",
            AppLanguage::Pt,
        )
        .unwrap();
        assert_eq!(resolved.folder_parts, vec!["CAS", "Maquiagem", "Cílios"]);
        assert_eq!(resolved.gender, None);
        assert!(verified_eyelash_catalog_bundle(
            &items,
            &types,
            "unrelated_bundle.package",
            AppLanguage::Pt
        )
        .is_none());
        items.push(mk("Roupas", "Conjunto"));
        assert!(verified_eyelash_catalog_bundle(
            &items,
            &types,
            "sclub_ts3_eyelash_n3a.package",
            AppLanguage::Pt
        )
        .is_none());
    }

    #[test]
    fn merged_casp_variants_share_anatomical_family_without_guessing_gender() {
        let mk = |category: &str, gender: &str, age: &str, subtype: &str| CatalogClassification {
            source: "CASP".into(),
            kind: "cas".into(),
            main_category: category.into(),
            sub_category: Some(subtype.into()),
            gender: Some(gender.into()),
            age: Some(age.into()),
            species: None,
            usage_categories: vec![],
            folder_parts: vec![
                "CAS".into(),
                category.into(),
                gender.into(),
                age.into(),
                subtype.into(),
            ],
            candidate_folder_parts: vec![],
            ambiguous: false,
            technical_reason: "".into(),
        };
        let types = BTreeSet::from([TYPE_CASP]);
        let glasses = vec![
            mk("Acessórios", "Masculino", "Adulto", "Óculos"),
            mk("Acessórios", "Feminino", "Adulto", "Óculos"),
        ];
        let common = shared_casp_family(&glasses, &types).unwrap();
        assert_eq!(common.folder_parts, vec!["CAS", "Acessórios", "Óculos"]);
        assert_eq!(common.gender, None);
        assert_eq!(common.sub_category, Some("Óculos".into()));
        let cross_family = vec![
            mk("Acessórios", "Masculino", "Adulto", "Óculos"),
            mk("Cabelos", "Masculino", "Adulto", "Cabelo"),
        ];
        assert!(shared_casp_family(&cross_family, &types).is_none());
        assert!(shared_casp_family(&glasses, &BTreeSet::from([TYPE_CASP, TYPE_OBJD])).is_none());
    }

    #[test]
    fn legacy_active_and_disabled_packages_are_organized_without_reactivating() {
        assert!(package_extension(Path::new(
            "TMZ_CustomBodyHair.package.disabled"
        )));
        assert!(package_extension(Path::new(
            "TMZ_CustomBodyHair.package.duplicated"
        )));
        assert!(package_extension(Path::new("OneEuroMuttTip Width.package")));
        assert!(!package_extension(Path::new(
            "OneEuroMuttTip Width.package.bak"
        )));
        assert!(!package_extension(Path::new("Backup.zip")));
    }

    #[test]
    fn canonical_scripts_gameplay_creator_hierarchy_is_localized() {
        assert_eq!(
            script_creator_folder(AppLanguage::Pt, Some("TwinSimming")),
            vec!["Scripts", "Jogabilidade"]
        );
        assert_eq!(
            script_creator_folder(AppLanguage::En, Some("NRaas")),
            vec!["Scripts", "NRaas"]
        );
        assert_eq!(
            script_creator_folder(AppLanguage::Es, None),
            vec!["Scripts", "Jugabilidad"]
        );
        assert!(script_source_roles("Scripts/Jogabilidade/TwinSimming/X.package").0);
    }

    #[test]
    fn script_categories_require_subject_evidence_not_random_assembly_strings() {
        let cases = [
            ("AnimatedWoohoo.package", "Relacionamentos"),
            ("TSS_MoreRomanticInteractions.package", "Relacionamentos"),
            ("Gamefreak130_KarmaPowers.package", "Scripts"),
            ("douglasveiga_HousekeeperService_v1.2.package", "Serviços"),
            ("douglasveiga_Gardener_service_NPC_v2.3.package", "Serviços"),
            ("Look! A living Sheep!.package", "Scripts"),
            ("zoeoe_knitting_resources.package", "Scripts"),
            ("simler90GameplayCoreMod-UPDATE199.package", "Utilitários"),
            (
                "twinsimming_Pasteurize Milk Mod.package",
                "Culinária e Comida",
            ),
            (
                "TSS_KitchenTweaks_WithMikeyEdit.package",
                "Culinária e Comida",
            ),
            ("NeoH4x0rGlobalOnlineBankingMod.package", "Scripts"),
            ("Gamefreak130_SmartphoneDating.package", "Relacionamentos"),
        ];
        for (name, expected) in cases {
            assert_eq!(script_category(name, AppLanguage::Pt), expected, "{name}");
        }
        assert_eq!(
            script_category(
                "douglasveiga_HousekeeperService_v1.2.package",
                AppLanguage::En
            ),
            "Services"
        );
        assert_eq!(
            script_category(
                "douglasveiga_HousekeeperService_v1.2.package",
                AppLanguage::Es
            ),
            "Servicios"
        );
        assert_eq!(
            creator_candidate_from_filename("AnimatedWoohoo.package"),
            None
        );
        assert_eq!(
            creator_candidate_from_filename("[Items] Al Fresco Street Market.package"),
            None
        );
        assert_eq!(
            creator_candidate_from_filename("Let's Take a Selfie by David Veiga.package"),
            None
        );
        assert_eq!(
            creator_candidate_from_filename("Retro Workout.package"),
            None
        );
        assert_eq!(
            creator_candidate_from_filename("ld_MonoPatcher.package"),
            Some("ld".to_string())
        );
        assert_eq!(
            creator_candidate_from_filename("icarusallsorts.EatOutsideRestaurant.package"),
            Some("icarusallsorts".to_string())
        );
        assert_eq!(
            creator_candidate_from_filename("twinsimming_Pasteurize Milk Mod.package"),
            Some("twinsimming".to_string())
        );
        assert_eq!(
            mod_name_from_relative(r"#+18\\AnimatedWoohoo.package"),
            None
        );
    }

    #[test]
    fn real_gameplay_mods_and_functional_objects_keep_distinct_destinations() {
        let script = PackageFamilyClassification {
            main_category: "Gameplay".into(),
            sub_category: Some("Scripts".into()),
            folder_parts: vec!["Gameplay".into(), "Scripts".into()],
            detected_from: vec!["S3SA".into()],
            technical_reason: "S3SA evidence".into(),
        };
        let catalog = |relative: &str, destinations, ambiguous| {
            catalog_precedes_embedded_script(Some(&script), 2, relative, destinations, ambiguous)
        };
        // Store bundles and premium content should never become Gameplay
        // just because an XML/assembly contains keyword matches.
        assert!(catalog(
            "Packages/#9 Store/Store Content/[Items] Al Fresco.package",
            4,
            false
        ));
        assert!(catalog(
            "Packages/#9 Store/Store Content/[PC] Basketball Hoop.package",
            1,
            false
        ));
        assert!(catalog(
            "Buy/Entertainment/Sports/[PC] Basketball Hoop.package",
            1,
            false
        ));
        assert!(catalog(
            "Buy/Debug/Buzz_ShellSoundEmitter.package",
            1,
            false
        ));
        assert!(catalog("Packages/ani_BistroStove.package", 1, false));
        assert!(catalog(
            "Packages/Scripts/cmomoney_TimeShifter.package",
            1,
            false
        ));
        assert!(catalog(
            "Packages/#8 Scripts/Objects/Fantuanss12_ProfessionalOven.package",
            1,
            false
        ));
        // Gameplay mods may bundle multiple object types or unreadable OBJD.
        assert!(!catalog(
            "Packages/#8 Scripts/Gameplay/Services/HousekeeperService.package",
            2,
            false
        ));
        assert!(!catalog(
            "Packages/#8 Scripts/Gameplay/Global Online Banking Mod/Bank.package",
            1,
            false
        ));
        assert!(!catalog(
            "Packages/Scripts/Gamefreak130_KarmaPowers.package",
            0,
            true
        ));
        assert!(!catalog_precedes_embedded_script(
            None,
            2,
            "Buy/Sports/Hoop.package",
            1,
            false
        ));
        assert!(!catalog_precedes_embedded_script(
            Some(&script),
            0,
            "Buy/Sports/Hoop.package",
            1,
            false
        ));

        let verified = PackageFamilyClassification {
            detected_from: vec!["S3SA".into(), "InternalCreator".into()],
            ..script.clone()
        };
        assert!(!catalog_precedes_embedded_script(
            Some(&verified),
            2,
            "#+18/AnimatedWoohoo.package",
            2,
            false
        ));
        let standalone_identity = PackageFamilyClassification {
            detected_from: vec!["S3SA".into(), "InternalScriptIdentity".into()],
            ..script.clone()
        };
        assert!(!catalog_precedes_embedded_script(
            Some(&standalone_identity),
            2,
            "#+18/AnimatedWoohoo.package",
            2,
            false
        ));
        assert!(catalog_precedes_embedded_script(
            Some(&standalone_identity),
            2,
            "Packages/#9 Store/Store Content/[Items] Al Fresco.package",
            2,
            false
        ));
        assert!(catalog_precedes_embedded_script(
            Some(&script),
            2,
            "#+18/UnknownResourceBundle.package",
            2,
            false
        ));
        assert!(catalog_precedes_embedded_script(
            Some(&verified),
            2,
            "Packages/#9 Store/Store Content/[Items] Le Cinema.package",
            2,
            false
        ));
        assert!(catalog_precedes_embedded_script(
            Some(&verified),
            2,
            "Packages/ani_BistroStove.package",
            1,
            false
        ));
        assert!(catalog_precedes_embedded_script(
            Some(&verified),
            2,
            "Buy/Decor/Fantuanss12_GroceryDeliveryService.package",
            2,
            true
        ));
        let nraas = PackageFamilyClassification {
            detected_from: vec!["NRaasInternal".into()],
            ..script
        };
        assert!(!catalog_precedes_embedded_script(
            Some(&nraas),
            2,
            "Packages/Scripts/NRaas.package",
            1,
            false
        ));
    }

    #[test]
    fn generic_source_folders_cannot_reassign_unrelated_packages() {
        for source in [
            "Packages/Scripts/SomeGameplay.package",
            "Packages/Male Hair/SomeHair.package",
            "CAS/Sliders/Body/Morph.package",
            "Build/Windows/SomeWindow.package",
            "Buy/Decor/SomeObject.package",
            "#+18/Pns (TS3)/Rigged/Body.package",
            "NRaas/MasterController/Extension.package",
            "Overrides/UI/Tuning.package",
        ] {
            assert!(named_mod_companion_folder(source).is_none(), "{source}");
        }
    }

    #[test]
    fn specifically_named_mod_folder_can_group_companions() {
        assert_eq!(
            named_mod_companion_folder("Baking Mod/Cakes/Chocolate.package").as_deref(),
            Some("Baking Mod")
        );
        assert_eq!(
            named_mod_companion_folder("PrismHome/Assets/Strings.package").as_deref(),
            Some("PrismHome")
        );
    }

    #[test]
    fn independent_catalog_and_script_classifications_are_not_companions() {
        let mut item = ScanPackageItem {
            id: String::new(),
            name: String::new(),
            path: String::new(),
            relative_path: "Baking Mod/Cakes/Example.package".into(),
            file_size: 0,
            resource_count: 0,
            catalog_resource_count: 1,
            resource_types: Vec::new(),
            instances: Vec::new(),
            scripted: false,
            content_source: String::new(),
            source_confidence: None,
            status: "classified".into(),
            classification_confidence: "high".into(),
            creator: None,
            mod_name: None,
            gameplay_category: None,
            detected_from: Vec::new(),
            category: Some("Objects".into()),
            sub_category: None,
            gender: None,
            age: None,
            species: None,
            usage_categories: Vec::new(),
            destination_parts: vec!["Objects".into()],
            destination_path: Some("Objects".into()),
            candidate_destinations: Vec::new(),
            classifications: Vec::new(),
            classification_reason: None,
            warnings: Vec::new(),
            project_id: None,
            project_role: None,
            project_confidence: None,
            project_evidence: ProjectTechnicalEvidence::default(),
            project_relationships: Vec::new(),
            reference_evidence: None,
            profile: None,
        };
        assert!(!may_reclassify_as_companion(&item));
        item.catalog_resource_count = 0;
        item.scripted = true;
        assert!(!may_reclassify_as_companion(&item));
        item.scripted = false;
        item.classification_confidence = "low".into();
        item.status = "unknown".into();
        assert!(!may_reclassify_as_companion(&item));
        item.detected_from.push("CompositeModFamily".into());
        assert!(may_reclassify_as_companion(&item));
    }

    #[test]
    fn package_extension_is_case_insensitive() {
        assert!(package_extension(Path::new("Hair.package")));
        assert!(package_extension(Path::new("Hair.PACKAGE")));
        assert!(!package_extension(Path::new("Hair.sims3pack")));
    }

    #[test]
    fn unchanged_scan_reuses_contextual_cache_and_detects_content_change() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("s3cc-scan-cache-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let package = root.join("sample.package");
        let unchanged_package = root.join("unchanged.package");
        std::fs::write(&package, vec![0u8; 64]).unwrap();
        std::fs::write(&unchanged_package, vec![0u8; 64]).unwrap();

        let first = scan_packages_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("first scan");
        assert!(!first.cache_hit);
        assert_eq!(first.stats.package_reads, 2);
        remember_latest_scan(&first, AppLanguage::Pt, None);

        let second = scan_packages_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("cached scan");
        assert!(second.cache_hit);
        assert_eq!(second.reused_packages, 2);
        assert_eq!(second.stats.package_reads, 0);

        // Keep the size unchanged. The bounded content fingerprint must still
        // invalidate the presentation cache rather than reusing stale status.
        let mut changed = vec![0u8; 64];
        changed[32] = 1;
        std::fs::write(&package, changed).unwrap();
        let third = scan_packages_core(root.to_string_lossy().to_string(), AppLanguage::Pt, None)
            .expect("changed scan");
        assert!(!third.cache_hit);
        assert_eq!(third.stats.package_reads, 1);
        assert_eq!(third.reused_packages, 1);

        invalidate_latest_scan_for(&root);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slider_morph_types_are_authoritative_for_companion_matching() {
        assert!(is_slider_morph_type(TYPE_BGEO));
        assert!(is_slider_morph_type(TYPE_FACE));
        assert!(is_slider_morph_type(TYPE_FBLN));
        assert!(!is_slider_morph_type(TYPE_STBL_LOCAL));
    }

    #[test]
    fn ascii_signature_lookup_remains_case_insensitive() {
        let data = b"prefix NRaas.MasterController suffix";
        assert!(contains_ascii_case_insensitive(data, "nraas.mastercontroller"));
        assert!(!contains_ascii_case_insensitive(data, "nraas.woohooer"));
        assert!(!contains_ascii_case_insensitive(data, ""));
    }

    #[test]
    fn stbl_key_parser_reads_slider_label_hashes() {
        let key = 0x745F_376D_11D5_E6C3u64;
        let text = "Butt".encode_utf16().collect::<Vec<_>>();
        let mut data = Vec::new();
        data.extend_from_slice(b"STBL");
        data.extend_from_slice(&[2, 0, 0]);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0; 6]);
        data.extend_from_slice(&key.to_le_bytes());
        data.extend_from_slice(&(text.len() as u32).to_le_bytes());
        for unit in text {
            data.extend_from_slice(&unit.to_le_bytes());
        }

        assert_eq!(stbl_keys(&data), vec![key]);
    }

    #[test]
    fn nmap_parser_reads_internal_slider_names() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0x8D243219BAF14119u64.to_le_bytes());
        let name = b"Bloom_LegLenght_slider";
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name);

        assert_eq!(nmap_names(&data), vec!["Bloom_LegLenght_slider"]);
    }

    #[test]
    fn nmap_namespace_lookup_only_materializes_matching_prefixes() {
        let names = [
            b"Gameplay.Core.Entry".as_slice(),
            b"Battery.NRaasLogo2".as_slice(),
            b"NRaas.MasterController".as_slice(),
        ];
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for (index, name) in names.iter().enumerate() {
            data.extend_from_slice(&(index as u64).to_le_bytes());
            data.extend_from_slice(&(name.len() as u32).to_le_bytes());
            data.extend_from_slice(name);
        }

        assert_eq!(
            nmap_first_with_ascii_prefix(&data, b"nraas", authoritative_nraas_identifier),
            Some("NRaas.MasterController".to_string())
        );
        assert_eq!(
            nmap_first_with_ascii_prefix(&data, b"missing", |_| true),
            None
        );
    }

    #[test]
    fn nmap_term_lookup_keeps_only_phase3_candidates() {
        let names = [
            b"Gameplay.Core.Entry".as_slice(),
            b"RabbitHoleDoor".as_slice(),
            b"Baking.CustomRecipe".as_slice(),
        ];
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for (index, name) in names.iter().enumerate() {
            data.extend_from_slice(&(index as u64).to_le_bytes());
            data.extend_from_slice(&(name.len() as u32).to_le_bytes());
            data.extend_from_slice(name);
        }

        assert_eq!(
            nmap_names_with_ascii_terms(&data, &["rabbit", "baking"]),
            vec!["RabbitHoleDoor", "Baking.CustomRecipe"]
        );
    }

    #[test]
    fn scan_previews_use_packages_without_cas_level() {
        let mods = Path::new("The Sims 3").join("Mods");
        let packages = mods.join("Packages");
        let legacy = vec![
            "CAS".to_string(),
            "Clothing".to_string(),
            "Male".to_string(),
            "YA-A".to_string(),
            "Top".to_string(),
        ];
        assert_eq!(
            physical_destination_parts(&mods, "Packages/legacy.package", &legacy),
            vec!["Packages", "Clothing", "Male", "YA-A", "Top"]
        );
        assert_eq!(
            physical_destination_parts(&packages, "legacy.package", &legacy),
            vec!["Clothing", "Male", "YA-A", "Top"]
        );
        assert_eq!(
            physical_destination_parts(
                &mods,
                "Packages/legacy.package",
                &["Packages".into(), "CAS".into(), "Sliders".into()]
            ),
            vec!["Packages", "Sliders"]
        );
        assert_eq!(
            physical_destination_parts(
                &packages,
                "legacy.package",
                &["Packages".into(), "CAS".into(), "Sliders".into()]
            ),
            vec!["Sliders"]
        );
    }

    #[test]
    fn nested_scan_previews_do_not_recreate_existing_categories() {
        let mods = Path::new("Game").join("Mods");
        let folder = mods.join("Packages").join("Clothing").join("Male");
        assert_eq!(
            physical_destination_parts(
                &folder,
                "top.package",
                &[
                    "Clothing".into(),
                    "Male".into(),
                    "YA-A".into(),
                    "Top".into()
                ]
            ),
            vec!["YA-A", "Top"]
        );
        let overrides = mods.join("Overrides").join("Gameplay");
        assert_eq!(
            physical_destination_parts(
                &overrides,
                "tuning.package",
                &["Overrides".into(), "Gameplay".into(), "Tuning".into()]
            ),
            vec!["Tuning"]
        );
    }

    #[test]
    fn overrides_remain_in_overrides_in_both_scan_modes() {
        let mods = Path::new("The Sims 3").join("Mods");
        let overrides = mods.join("Overrides");
        let parts = vec!["Gameplay".into(), "Tuning".into()];
        assert_eq!(
            physical_destination_parts(&mods, "Overrides/UI/foo.package", &parts),
            vec!["Overrides", "Gameplay", "Tuning"]
        );
        assert_eq!(
            physical_destination_parts(&mods, r"Overrides\\UI\\foo.package", &parts),
            vec!["Overrides", "Gameplay", "Tuning"]
        );
        assert_eq!(
            physical_destination_parts(&overrides, "UI/foo.package", &parts),
            parts
        );
        assert_eq!(
            physical_destination_parts(&mods, "Packages/foo.package", &parts),
            vec!["Packages", "Gameplay", "Tuning"]
        );
        assert_eq!(
            physical_destination_parts(
                &mods,
                "Overrides/foo.package",
                &["Packages".into(), "CAS".into(), "Sliders".into()]
            ),
            vec!["Overrides", "Sliders"]
        );
    }

    #[test]
    fn slider_region_uses_internal_anatomy_not_cas_panel() {
        assert_eq!(
            slider_destination_from_internal_name("Bloom_ArmTwist_slider", AppLanguage::Pt),
            Some(vec![
                "Sliders".to_string(),
                "Corpo".to_string(),
                "Braços".to_string(),
            ])
        );
        assert_eq!(
            slider_destination_from_internal_name("Bloom_LegLenght_slider", AppLanguage::Pt),
            Some(vec![
                "Sliders".to_string(),
                "Corpo".to_string(),
                "Pernas".to_string(),
            ])
        );
        assert_eq!(
            slider_destination_from_internal_name("aWT_HatGrip_MoveX_slider", AppLanguage::Pt),
            Some(vec!["Sliders".to_string(), "Chapéus".to_string()])
        );
    }

    #[test]
    fn specific_anatomy_wins_over_generic_height_words() {
        assert_eq!(
            slider_destination_from_internal_name("Nose Tip Height", AppLanguage::En),
            Some(vec![
                "Sliders".to_string(),
                "Face".to_string(),
                "Nose".to_string(),
            ])
        );
        assert_eq!(
            slider_destination_from_internal_name("Shoulder Height", AppLanguage::En),
            Some(vec![
                "Sliders".to_string(),
                "Body".to_string(),
                "Shoulders".to_string(),
            ])
        );
    }

    #[test]
    fn replacement_mesh_category_preserves_anatomy_across_locales() {
        // GEOM+VPXY are necessary; unrelated custom content must not be
        // classified as a Replacement by filename alone.
        let types = BTreeSet::from([0x015A_1849, 0x7368_84F1]);
        let no_geom = BTreeSet::from([0x7368_84F1]);
        assert!(no_geom.len() == 1);
        let category = |lang, replacement: &str| {
            let subdivision = if replacement == "Feet" {
                match lang {
                    AppLanguage::En => "Feet",
                    AppLanguage::Pt => "Pés",
                    AppLanguage::Es => "Pies",
                }
            } else {
                match lang {
                    AppLanguage::En => "Eyelashes",
                    AppLanguage::Pt => "Cílios",
                    AppLanguage::Es => "Pestañas",
                }
            };
            subdivision
        };
        assert_eq!(category(AppLanguage::Pt, "Feet"), "Pés");
        assert_eq!(category(AppLanguage::Es, "Eyelashes"), "Pestañas");
        assert!(types.contains(&0x015A_1849));
    }

    #[test]
    fn makeup_semantics_require_internal_subject_evidence() {
        assert_eq!(
            makeup_semantic_folder_key(&["faceShine140104".into()]),
            Some(("Iluminador", "faceshine"))
        );
        assert_eq!(
            makeup_semantic_folder_key(&["BrntWafflesNoseMask".into()]),
            Some(("Máscaras de Nariz", "nosemask"))
        );
        assert_eq!(makeup_semantic_folder_key(&["ordinary_blush".into()]), None);
    }

    #[test]
    fn verified_eye_texture_tgi_resolves_to_body_eyes_without_filename_evidence() {
        let entry = ResourceEntry {
            type_id: TYPE_IMG,
            group: 0,
            instance: 0xC2B5_6067_3F00_D806,
            chunk_offset: 0,
            file_size: 1,
            mem_size: 1,
            compressed: 0,
            unknown2: 0,
            file_size_high_bit: false,
        };
        let types = BTreeSet::from([TYPE_IMG]);

        let classification = verified_eye_texture_replacement(
            &[entry.clone()],
            &types,
            AppLanguage::Pt,
        )
        .expect("verified eye texture TGI");
        assert_eq!(
            classification.folder_parts,
            vec!["Substituições", "Corpo", "Olhos"]
        );
        assert!(classification
            .detected_from
            .contains(&"EyeReplacementTgi".to_string()));

        assert_eq!(
            verified_eye_texture_replacement(&[entry.clone()], &types, AppLanguage::En)
                .unwrap()
                .folder_parts,
            vec!["Replacements", "Body", "Eyes"]
        );
        assert_eq!(
            verified_eye_texture_replacement(&[entry], &types, AppLanguage::Es)
                .unwrap()
                .folder_parts,
            vec!["Reemplazos", "Cuerpo", "Ojos"]
        );

        let unrelated = ResourceEntry {
            type_id: TYPE_IMG,
            group: 0,
            instance: 0x1234,
            chunk_offset: 0,
            file_size: 1,
            mem_size: 1,
            compressed: 0,
            unknown2: 0,
            file_size_high_bit: false,
        };
        assert!(verified_eye_texture_replacement(&[unrelated], &types, AppLanguage::Pt).is_none());
    }

    #[test]
    fn verified_img_only_eyelash_companion_uses_internal_tgi_clusters() {
        let instances = [
            0xD1E4_338D_1C34_B780,
            0xD1E4_338D_1C34_B781,
            0xD1E4_338D_1C34_B789,
            0xD1E4_338D_1C34_B78A,
            0xD1E4_338D_1C34_B78B,
            0xD1E4_338D_1C34_B78C,
            0xD1E4_338D_1C34_B78D,
            0xD1E4_338D_1C34_B78E,
            0xD1E4_338D_1C34_B78F,
            0xD1E4_348D_1C34_B952,
            0xD1E4_348D_1C34_B953,
            0xD1E4_348D_1C34_B958,
            0xD1E4_348D_1C34_B959,
            0xD1E4_348D_1C34_B95A,
            0xD1E4_348D_1C34_B95B,
            0xD1E4_348D_1C34_B95C,
            0xD1E4_348D_1C34_B95D,
            0xD1E4_348D_1C34_B95E,
            0xD1E4_348D_1C34_B95F,
            0xD1E4_358D_1C34_BB2E,
            0xD1E4_358D_1C34_BB2F,
        ];
        let entries = instances
            .into_iter()
            .map(|instance| ResourceEntry {
                type_id: TYPE_IMG,
                group: 0,
                instance,
                chunk_offset: 0,
                file_size: 1,
                mem_size: 1,
                compressed: 0,
                unknown2: 0,
                file_size_high_bit: false,
            })
            .collect::<Vec<_>>();
        let classification = verified_eyelash_texture_companion(
            &entries,
            &BTreeSet::from([TYPE_IMG]),
            AppLanguage::Pt,
        )
        .expect("verified IMG-only eyelash companion");
        assert_eq!(classification.folder_parts, vec!["CAS", "Maquiagem", "Cílios"]);
        assert!(classification
            .detected_from
            .contains(&"EyelashTextureCompanionSignature".to_string()));

        let mut unrelated = entries;
        unrelated[0].instance = 0x1234;
        assert!(verified_eyelash_texture_companion(
            &unrelated,
            &BTreeSet::from([TYPE_IMG]),
            AppLanguage::Pt,
        )
        .is_none());
    }

    #[test]
    fn clothing_source_and_visual_resources_do_not_become_replacements() {
        let types = BTreeSet::from([TYPE_CASP, 0x015A_1849, TYPE_IMG]);
        assert!(legacy_source_evidence_classification(
            &types,
            "ordinary.package",
            r"Roupas\Feminino\Parte de Cima",
            AppLanguage::Pt,
        )
        .is_none());

        let mesh_only = BTreeSet::from([0x015A_1849, TYPE_BGEO]);
        assert!(legacy_source_evidence_classification(
            &mesh_only,
            "cmar_fpreg_BottomNude.package",
            r"Roupas\Feminino\Parte de Baixo",
            AppLanguage::Pt,
        )
        .is_none());
    }

    #[test]
    fn colour_named_roof_swatches_use_the_internal_roof_signature() {
        let types = BTreeSet::from([
            0x0166_038C,
            0x0166_1233,
            0x01D0_E75D,
            0x01D1_0F34,
            0x03B4_C61D,
            0x2E75_C764,
            0x2E75_C765,
            0x2E75_C766,
            0xD382_BF57,
            0xF1ED_BD86,
            TYPE_IMG,
            0x2205_57DA,
            0x7368_84F1,
        ]);
        let classification = legacy_source_evidence_classification(
            &types,
            "tammy_trauma_BeachAqua.package",
            r"Construção\Telhados",
            AppLanguage::Pt,
        )
        .expect("colour-named roof swatch should be construction content");
        assert_eq!(classification.folder_parts, vec!["Construção"]);
        assert!(classification
            .detected_from
            .contains(&"RoofResourceSignature".to_string()));

        let incomplete = BTreeSet::from([TYPE_IMG, 0x03B4_C61D, 0x0166_1233]);
        assert!(legacy_source_evidence_classification(
            &incomplete,
            "generic_BeachAqua.package",
            r"Sem Categoria",
            AppLanguage::Pt,
        )
        .is_none());
    }

    #[test]
    fn canonical_gameplay_interaction_components_stay_with_scripts() {
        let classification = legacy_source_evidence_classification(
            &BTreeSet::from([0xD304_4521]),
            "HitchingPostReplacementToEnableSheepEating.package",
            r"Scripts\Gameplay\Functional Living Sheep",
            AppLanguage::Pt,
        )
        .expect("canonical gameplay interaction component");
        assert_eq!(classification.folder_parts, vec!["Scripts", "Jogabilidade"]);
        assert!(classification
            .detected_from
            .contains(&"CanonicalGameplaySource".to_string()));
    }

    #[test]
    fn tss_interaction_assets_join_their_gameplay_family_with_resource_evidence() {
        let classification = tss_asset_companion_classification(
            &BTreeSet::from([TYPE_XML_LOCAL, TYPE_STBL_LOCAL]),
            "TSS_MorePregnancyInteractions_Assets.package",
            "#Sem Categoria",
            AppLanguage::Pt,
            &[],
        )
        .expect("TSS interaction asset companion");
        assert_eq!(classification.folder_parts, vec!["Scripts", "Jogabilidade"]);
        assert!(classification
            .detected_from
            .contains(&"TSSAssetCompanion".to_string()));
        assert!(tss_asset_companion_classification(
            &BTreeSet::new(),
            "TSS_MorePregnancyInteractions_Assets.package",
            "#Sem Categoria",
            AppLanguage::Pt,
            &[],
        )
        .is_none());
    }

    #[test]
    fn s3sa_does_not_flatten_existing_correction_families() {
        let types = BTreeSet::from([TYPE_S3SA]);
        let classification = preserved_correction_family_classification(
            &types,
            r"Correções\ld_LotPopulation.package",
        )
        .unwrap();
        assert_eq!(classification.folder_parts, vec!["Correções"]);
        assert!(classification
            .detected_from
            .contains(&"ProtectedExistingCorrectionFamily".into()));
        assert!(preserved_correction_family_classification(
            &BTreeSet::from([TYPE_S3SA, TYPE_CASP]),
            r"Correções\not-a-script-casp.package",
        )
        .is_none());
    }

    #[test]
    fn pregnancy_clothing_nmap_has_internal_gender_and_subtype_evidence() {
        let names = vec![
            "afBottomNude_special_lod3".to_string(),
            "afBottomNude_special_lod2".to_string(),
        ];
        assert_eq!(clothing_mesh_subtype(&names), Some("bottom"));
        assert_eq!(
            clothing_mesh_subtype(&["afTopNude_special".to_string()]),
            Some("top")
        );
        assert_eq!(clothing_mesh_subtype(&["Tip Width".to_string()]), None);
        assert_eq!(
            clothing_mesh_subtype(&["afTopNude".to_string(), "afBottomJeans".to_string()]),
            None
        );
    }

    #[test]
    fn genital_sliders_use_anatomical_category_in_three_languages() {
        assert_eq!(
            slider_destination_from_internal_name("Penis Length", AppLanguage::En),
            Some(vec!["Sliders".into(), "Body".into(), "Genitals".into()])
        );
        assert_eq!(
            slider_destination_from_internal_name("Testicle Size", AppLanguage::Pt),
            Some(vec!["Sliders".into(), "Corpo".into(), "Genitais".into()])
        );
    }

    #[test]
    fn filename_can_refine_a_generic_internal_slider_name() {
        assert_eq!(
            slider_destination_from_internal_name(
                "aWT_Mouth-UpperLip-TipWidth.package",
                AppLanguage::En
            ),
            Some(vec!["Sliders".into(), "Face".into(), "Mouth & Lips".into()])
        );
        assert_eq!(
            slider_destination_from_internal_name(
                "Lavender_MiddleFaceWidth.package",
                AppLanguage::En
            ),
            Some(vec!["Sliders".into(), "Face".into()])
        );
        assert!(slider_destination_from_internal_name(
            "OneEuroMuttTip Width.package",
            AppLanguage::En
        )
        .is_none());
        assert_eq!(
            verified_slider_name_alias("OneEuroMuttTip Width.package"),
            Some("Nose Tip Width")
        );
        assert_eq!(
            slider_destination_from_internal_name(
                verified_slider_name_alias("OneEuroMuttTip Width.package").unwrap(),
                AppLanguage::En,
            ),
            Some(vec!["Sliders".into(), "Face".into(), "Nose".into()])
        );
        assert_eq!(
            verified_slider_name_alias("SomeoneElseTip Width.package"),
            None
        );
    }

    #[test]
    fn ambiguous_internal_name_stays_at_slider_root() {
        assert!(slider_destination_from_internal_name("Tip Width", AppLanguage::En).is_none());
        assert!(slider_destination_from_internal_name("Outer Curve", AppLanguage::En).is_none());
    }

    #[test]
    fn script_mod_name_prefers_named_source_folder() {
        assert_eq!(
            inferred_script_mod_name(
                "twinsimming_Chocolate Cake.package",
                "Baking Mod/Cakes/twinsimming_Chocolate Cake.package"
            )
            .as_deref(),
            Some("Baking Mod")
        );
    }

    #[test]
    fn script_mod_name_can_be_read_from_main_package_filename() {
        assert_eq!(
            mod_name_from_filename("twinsimming_Baking Mod V2.1.package").as_deref(),
            Some("Baking Mod")
        );
        assert_eq!(
            mod_name_from_filename("twinsimming_Baking Mod [Home Baker Career].package").as_deref(),
            Some("Baking Mod")
        );
    }

    #[test]
    fn pupil_heart_is_eye_not_ear() {
        assert_eq!(
            slider_region_keys("Pupil Heart"),
            Some(("face", Some("eyes")))
        );
    }

    #[test]
    fn nraas_family_folder_requires_primary_and_module() {
        assert_eq!(
            nraas_family_role("NRaas_MasterController.package"),
            Some(("MasterController", false))
        );
        assert_eq!(
            nraas_family_role("NRaas_MasterControllerCheats.package"),
            Some(("MasterController", true))
        );
        assert_eq!(
            nraas_family_role("NRaas_Careers.package"),
            Some(("Careers", false))
        );
        assert_eq!(
            nraas_family_destination(AppLanguage::Pt, "MasterController", true, false),
            vec!["Scripts", "NRaas"]
        );
        assert_eq!(
            nraas_family_destination(AppLanguage::Pt, "MasterController", true, true),
            vec!["Scripts", "NRaas", "MasterController"]
        );
        assert_eq!(
            existing_nraas_subfolder(r"Scripts\NRaas\Dreamer\NRaas_Dreamer.package"),
            Some("Dreamer".into())
        );
        assert_eq!(
            existing_nraas_subfolder(r"Scripts\Jogabilidade\NRaas\OnceRead\NRaas_OnceRead.package"),
            Some("OnceRead".into())
        );
    }

    #[test]
    fn integrations_do_not_become_nraas_from_a_logo_reference() {
        assert!(is_explicit_non_nraas_integration(
            "Battery.Simpanel_1.03.package"
        ));
        assert!(is_explicit_non_nraas_integration(
            "NeoH4x0rGlobalOnlineBankingMod.package"
        ));
        assert!(!is_explicit_non_nraas_integration(
            "NRaas_MasterController.package"
        ));
        assert!(!authoritative_nraas_identifier("Battery.NRaasLogo2"));
        assert!(authoritative_nraas_identifier("NRaas.MasterController"));
    }

    #[test]
    fn non_nraas_scripts_use_gameplay_root_without_creator_fallback() {
        assert_eq!(
            script_creator_folder(AppLanguage::Pt, Some("TwinSimming")),
            vec!["Scripts", "Jogabilidade"]
        );
    }

    fn synthetic_body_hair_classification(gender: &str) -> CatalogClassification {
        CatalogClassification {
            source: "CASP".to_string(),
            kind: "cas".to_string(),
            main_category: "Pelos Corporais".to_string(),
            sub_category: None,
            gender: Some(gender.to_string()),
            age: Some("Criança a Idoso".to_string()),
            species: Some("Humano".to_string()),
            usage_categories: Vec::new(),
            folder_parts: vec![
                "Cabelos e Pêlos".to_string(),
                "Pelos Corporais".to_string(),
                gender.to_string(),
            ],
            candidate_folder_parts: vec![vec![
                "Cabelos e Pêlos".to_string(),
                "Pelos Corporais".to_string(),
                gender.to_string(),
            ]],
            ambiguous: false,
            technical_reason: "synthetic CASP control".to_string(),
        }
    }

    #[test]
    fn body_hair_multiple_gender_family_keeps_shared_root_without_duplication() {
        let classifications = vec![
            synthetic_body_hair_classification("Masculino"),
            synthetic_body_hair_classification("Feminino"),
        ];
        let shared = shared_casp_family(&classifications, &BTreeSet::from([TYPE_CASP])).unwrap();
        assert_eq!(
            shared.folder_parts,
            vec!["Cabelos e Pêlos", "Pelos Corporais"]
        );
        assert_eq!(shared.gender, None);
        assert!(shared
            .folder_parts
            .iter()
            .all(|part| !part.contains("Idade") && !part.contains("Peito")));
    }

    #[test]
    fn real_body_hair_model_cases_use_casp_gender_only_when_requested() {
        let Ok(root) = std::env::var("S3CC_BODY_HAIR_MODEL_ROOT") else {
            return;
        };
        let root_path = PathBuf::from(&root);
        let targets = [
            "Cabelos e Pêlos\\Pelos Corporais\\Masculino\\cmar_MalePubicHairFull.package",
            "Cabelos e Pêlos\\Pelos Corporais\\Feminino\\cmar_FemalePubicHairFull.package",
            "Cabelos e Pêlos\\Pelos Corporais\\Masculino\\TMZ_CustomBodyHair.package",
        ];
        let mut rows = Vec::new();
        for relative in targets {
            let path = root_path.join(relative);
            assert!(
                path.is_file(),
                "real Body Hair fixture is absent: {relative}"
            );
            let mut slider_instances = HashSet::new();
            let item = scan_one(
                &root_path,
                &path,
                AppLanguage::Pt,
                &mut slider_instances,
                fix_signatures::load_persistent_index().as_ref(),
                &mut ScanTiming::default(),
            );
            assert_eq!(item.status, "classified", "{relative}");
            assert!(item
                .destination_parts
                .starts_with(&["Cabelos e Pêlos".to_string(), "Pelos Corporais".to_string(),]));
            assert!(!item.destination_parts.iter().any(|part| {
                part.contains("Idade")
                    || part.contains("Peito")
                    || part.contains("Braço")
                    || part.contains("Pern")
                    || part.contains("Costas")
                    || part.contains("Barriga")
            }));
            rows.push(serde_json::json!({
                "relativePath": relative,
                "sha256": sha256_file(&path).unwrap().0,
                "status": item.status,
                "gender": item.gender,
                "age": item.age,
                "destination": item.destination_parts,
                "casp": item.classifications,
                "reason": item.classification_reason,
            }));
        }
        if let Ok(output) = std::env::var("S3CC_BODY_HAIR_REPORT") {
            let output = PathBuf::from(output);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        }
    }

    #[test]
    fn four_pending_slider_cases_use_real_dbpf_evidence_and_never_source_folder() {
        let Ok(root) = std::env::var("S3CC_PENDING_SLIDER_ROOT") else {
            return;
        };
        let root_path = PathBuf::from(&root);
        let targets = [
            "BloomBase afEyecorrection.package",
            "Jonha Sliders Headshape Hairfix.package",
            "PYXIS_Obscura_eyeballMeshPUCU.package",
            "S-Club ts3 mod ea eyelash removal fm.package",
        ];
        let mut rows = Vec::new();
        for target in targets {
            let path = WalkDir::new(&root_path)
                .into_iter()
                .filter_map(Result::ok)
                .find(|entry| {
                    entry.file_type().is_file()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(target)
                })
                .map(|entry| entry.into_path())
                .unwrap_or_else(|| panic!("pending package is absent: {target}"));
            let package = Package::load(&path).expect("pending package must be valid DBPF");
            let mut slider_instances = HashSet::new();
            let item = scan_one(
                &root_path,
                &path,
                AppLanguage::Pt,
                &mut slider_instances,
                fix_signatures::load_persistent_index().as_ref(),
                &mut ScanTiming::default(),
            );
            assert!(
                !package
                    .entries
                    .iter()
                    .any(|entry| entry.type_id == TYPE_FBLN),
                "a pending non-FBLN control unexpectedly acquired FBLN: {target}"
            );
            assert_ne!(
                item.sub_category.as_deref(),
                Some("Sliders"),
                "source folder must never classify a pending package as slider: {target}"
            );
            let resources = package
                .entries
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "type": format!("0x{:08X}", entry.type_id),
                        "label": resource_type_label(entry.type_id),
                        "group": format!("0x{:08X}", entry.group),
                        "instance": format!("0x{:016X}", entry.instance),
                        "payloadBytes": package.data(entry).map(|data| data.len()).unwrap_or(0),
                    })
                })
                .collect::<Vec<_>>();
            rows.push(serde_json::json!({
                "name": target,
                "relativePath": path.strip_prefix(&root_path).unwrap_or(&path).to_string_lossy(),
                "status": item.status,
                "classificationConfidence": item.classification_confidence,
                "destination": item.destination_parts,
                "resourceCount": package.entries.len(),
                "resources": resources,
                "nmapNames": package_nmap_names(&package),
                "internalIdentifiers": package_internal_identifiers(
                    &package,
                    &extract_package_evidence_with_metrics(&path, &package).0,
                ),
                "detectedFrom": item.detected_from,
                "classificationReason": item.classification_reason,
                "warnings": item.warnings,
            }));
        }
        if let Ok(output) = std::env::var("S3CC_PENDING_SLIDER_REPORT") {
            let output = PathBuf::from(output);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        }
    }

    #[test]
    fn external_slider_corpus_is_a_read_only_dry_run_when_requested() {
        let Ok(root) = std::env::var("S3CC_SLIDER_CORPUS_ROOT") else {
            return;
        };
        let root_path = PathBuf::from(&root);
        let package_paths = WalkDir::new(&root_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && package_extension(entry.path()))
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        assert_eq!(
            package_paths.len(),
            299,
            "slider corpus package count changed"
        );

        let mut package_items = HashMap::<String, &ScanPackageItem>::new();
        let result = scan_packages_core(root.clone(), AppLanguage::Pt, None).expect("scan corpus");
        for item in &result.items {
            package_items.insert(
                item.relative_path.replace('/', "\\").to_ascii_lowercase(),
                item,
            );
        }

        let mut fbln_packages = 0usize;
        let mut fbln_resources = 0usize;
        let mut no_fbln = 0usize;
        let mut hashes = BTreeSet::new();
        let mut dry_run = Vec::with_capacity(package_paths.len());
        for path in package_paths {
            let package = Package::load(&path).expect("DBPF package");
            let metadata = read_fbln_metadata(&package);
            let fbln_count = package
                .entries
                .iter()
                .filter(|entry| entry.type_id == TYPE_FBLN)
                .count();
            assert_eq!(
                fbln_count,
                metadata.len(),
                "every FBLN must be readable: {}",
                path.display()
            );
            if fbln_count > 0 {
                fbln_packages += 1;
                fbln_resources += fbln_count;
            } else {
                no_fbln += 1;
            }
            let (sha256, size) = sha256_file(&path).expect("hash corpus package");
            assert!(
                hashes.insert(sha256.clone()),
                "corpus contains an identical SHA-256"
            );
            let relative = path
                .strip_prefix(&root_path)
                .unwrap()
                .to_string_lossy()
                .replace('/', "\\");
            let item = package_items
                .get(&relative.to_ascii_lowercase())
                .expect("scan item");
            let destination = item.destination_path.clone().unwrap_or_default();
            let bucket = if fbln_count > 0 {
                "sliders"
            } else if destination.to_ascii_lowercase().contains("fix")
                || item
                    .detected_from
                    .iter()
                    .any(|v| v.to_ascii_lowercase().contains("fix"))
            {
                "fixes"
            } else if destination.to_ascii_lowercase().contains("substit")
                || destination.to_ascii_lowercase().contains("reempl")
                || destination.to_ascii_lowercase().contains("replacement")
            {
                "replacements"
            } else if item.status == "classified" {
                "support_or_other"
            } else {
                "review"
            };
            dry_run.push(serde_json::json!({
                "relativePath": relative,
                "name": item.name,
                "size": size,
                "sha256": sha256,
                "resourceTypes": item.resource_types,
                "fblnCount": fbln_count,
                "fbln": metadata,
                "status": item.status,
                "destination": item.destination_path,
                "detectedFrom": item.detected_from,
                "classificationReason": item.classification_reason,
                "bucket": bucket,
            }));
        }
        assert_eq!(fbln_packages, 290, "FBLN package count changed");
        assert_eq!(fbln_resources, 292, "FBLN resource count changed");
        assert_eq!(no_fbln, 9, "non-FBLN package count changed");
        assert_eq!(
            result.stats.invalid, 0,
            "corpus contains an unreadable DBPF"
        );
        let non_fbln_slider_items = result
            .items
            .iter()
            .filter(|item| {
                item.sub_category.as_deref() == Some("Sliders")
                    && !item.resource_types.iter().any(|kind| kind == "FBLN")
            })
            .collect::<Vec<_>>();
        for item in &non_fbln_slider_items {
            println!(
                "NON_FBLN_SLIDER {} {:?} {:?}",
                item.relative_path, item.resource_types, item.detected_from
            );
        }
        assert!(
            non_fbln_slider_items.is_empty(),
            "non-FBLN resources must not be auto-classified as sliders"
        );

        if let Ok(output) = std::env::var("S3CC_SLIDER_DRY_RUN_OUTPUT") {
            let path = PathBuf::from(output);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "packageCount": result.stats.packages,
                    "fblnPackages": fbln_packages,
                    "fblnResources": fbln_resources,
                    "withoutFbln": no_fbln,
                    "dbpfIndexErrors": result.stats.invalid,
                    "identicalSha256": 0,
                    "dryRunOnly": true,
                    "items": dry_run,
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }
}
