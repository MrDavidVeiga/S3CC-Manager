use crate::{
    cache::{
        get_or_build_analysis_with_metrics, load_cache, retain_existing, save_cache, CachedResource,
    },
    catalog::{TYPE_CASP, TYPE_OBJD},
    operation::{self, CANCELLED_ERROR},
    resource_cfg::{
        find_resource_cfg, package_priority, parse_resource_cfg, relative_package_path,
        ResourceCfgInfo,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

const TYPE_IMG: u32 = 0x00B2_D882;
const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_FACE: u32 = 0x0358_B08A;
const TYPE_CBLN: u32 = 0x051D_F2DD;
const TYPE_BGEO: u32 = 0x067C_AA11;
const TYPE_BOND: u32 = 0x0355_E0A6;
const TYPE_BUNIT: u32 = 0xB52F_5055;
const TYPE_BUFF: u32 = 0xDD32_23A7;
const TYPE_GEOM: u32 = 0x015A_1849;
const TYPE_NMAP: u32 = 0x0166_038C;
const TYPE_LAYO: u32 = 0x025C_95B6;
const TYPE_MODL: u32 = 0x0166_1233;
const TYPE_MATD: u32 = 0x01D0_E75D;
const TYPE_MLOD: u32 = 0x01D1_0F34;
const TYPE_OBJK: u32 = 0x02DC_343F;
const TYPE_XML: u32 = 0x0333_406C;
const TYPE_TXTC: u32 = 0x033A_1435;
const TYPE_TXTF: u32 = 0x0341_ACC9;
const TYPE_ITUN: u32 = 0x03B3_3DDF;
const TYPE_STBL: u32 = 0x2205_57DA;
const TYPE_VPXY: u32 = 0x7368_84F1;
const TYPE_S3SA: u32 = 0x073F_AA07;
const TYPE_MANIFEST: u32 = 0x73E9_3EEB;

const TYPE_CATALOG_FENCE: u32 = 0x0418_FE2A;
const TYPE_CATALOG_STAIRS: u32 = 0x049C_A4CD;
const TYPE_CATALOG_PROXY: u32 = 0x04AC_5D93;
const TYPE_CATALOG_TERRAIN_GEOMETRY: u32 = 0x04B3_0669;
const TYPE_CATALOG_RAILING: u32 = 0x04C5_8103;
const TYPE_CATALOG_TERRAIN_PAINT: u32 = 0x04ED_4BB2;
const TYPE_CATALOG_FIREPLACE: u32 = 0x04F3_CC01;
const TYPE_CATALOG_WATER: u32 = 0x060B_390C;
const TYPE_CATALOG_POOL: u32 = 0x0A36_F07A;
const TYPE_CATALOG_FOUNDATION: u32 = 0x316C_78F2;

const MAX_FINDINGS: usize = 20_000;
const MAX_EVIDENCE_PER_FINDING: usize = 64;

#[derive(Debug, Clone)]
struct ResourceOccurrence {
    package_index: usize,
    type_id: u32,
    group: u32,
    instance: u64,
    payload_hash: String,
    payload_size: usize,
}

#[derive(Debug, Clone)]
struct PackageInfo {
    name: String,
    path: PathBuf,
    relative_path: PathBuf,
    file_sha256: String,
    readable: bool,
    resource_count: usize,
    resource_set_sha256: Option<String>,
    load_priority: Option<i32>,
    load_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPackage {
    pub name: String,
    pub path: String,
    pub relative_path: String,
    pub file_sha256: String,
    pub load_priority: Option<i32>,
    pub load_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEvidence {
    pub resource_type: u32,
    pub resource_type_hex: String,
    pub resource_label: String,
    pub resource_class: String,
    pub group: u32,
    pub group_hex: String,
    pub instance: u64,
    pub instance_hex: String,
    pub same_payload: bool,
    pub left_payload_sha256: String,
    pub right_payload_sha256: String,
    pub left_payload_size: usize,
    pub right_payload_size: usize,
    pub impact_kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFinding {
    pub id: String,
    pub decision_key: String,
    pub kind: String,
    pub impact_kinds: Vec<String>,
    pub severity: String,
    pub left: ConflictPackage,
    pub right: ConflictPackage,
    pub shared_resource_count: usize,
    pub identical_payload_count: usize,
    pub different_payload_count: usize,
    pub full_resource_set_comparable: bool,
    pub full_resource_set_same: bool,
    pub left_resource_count: usize,
    pub right_resource_count: usize,
    pub left_resource_set_sha256: Option<String>,
    pub right_resource_set_sha256: Option<String>,
    pub evidence: Vec<ConflictEvidence>,
    pub evidence_truncated: bool,
    pub explanation_key: String,
    pub load_order_status: String,
    pub higher_priority_path: Option<String>,
    pub load_order_explanation_key: String,
}

fn resource_set_sha256(resources: &[CachedResource]) -> String {
    // Compare the complete logical resource set: TGI plus decoded payload.
    // Container compression metadata is intentionally excluded because it
    // does not change the resource exposed to the game.
    let mut entries = resources
        .iter()
        .map(|resource| {
            format!(
                "{:08X}|{:08X}|{:016X}|{}|{}",
                resource.type_id,
                resource.group,
                resource.instance,
                resource.payload_sha256.to_ascii_uppercase(),
                resource.payload_size
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    format!("{:X}", Sha256::digest(entries.join("\n").as_bytes()))
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConflictStats {
    pub packages_scanned: usize,
    pub readable_packages: usize,
    pub unreadable_packages: usize,
    pub package_pairs: usize,
    pub shared_identical: usize,
    // Overlaps where one or both packages are outside the reliable
    // Resource.cfg PackedFile rules are not live in-game conflicts.
    pub inactive_pairs: usize,
    pub visual_overrides: usize,
    pub cas_morph_overlaps: usize,
    pub catalog_overrides: usize,
    pub gameplay_overrides: usize,
    pub script_conflicts: usize,
    pub text_overrides: usize,
    pub potential_conflicts: usize,
    pub mixed_overrides: usize,
    /// Same filename with different package bytes; not evidence of a TGI collision.
    pub filename_collisions: usize,
    pub analysis_truncated: bool,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub hashing_ms: u128,
    pub dbpf_load_ms: u128,
    pub resource_decode_ms: u128,
    pub comparison_ms: u128,
    pub total_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
    pub root: String,
    pub resource_cfg: Option<ResourceCfgInfo>,
    pub findings: Vec<ConflictFinding>,
    pub stats: ConflictStats,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
struct PairAccumulator {
    shared_resource_count: usize,
    identical_payload_count: usize,
    different_payload_count: usize,
    impact_kinds: BTreeSet<String>,
    evidence: Vec<ConflictEvidence>,
    evidence_truncated: bool,
    filename_collision: bool,
}

fn mark_filename_collisions(
    packages: &[PackageInfo],
    pairs: &mut HashMap<(usize, usize), PairAccumulator>,
) {
    let mut by_name = BTreeMap::<String, Vec<usize>>::new();
    for (index, package) in packages.iter().enumerate() {
        if package.file_sha256.is_empty() {
            continue;
        }
        by_name
            .entry(package.name.to_ascii_lowercase())
            .or_default()
            .push(index);
    }
    for indices in by_name.values() {
        for left_pos in 0..indices.len() {
            for right_pos in (left_pos + 1)..indices.len() {
                let left = indices[left_pos];
                let right = indices[right_pos];
                if packages[left]
                    .file_sha256
                    .eq_ignore_ascii_case(&packages[right].file_sha256)
                {
                    continue;
                }
                let key = (left.min(right), left.max(right));
                pairs.entry(key).or_default().filename_collision = true;
            }
        }
    }
}

fn is_package(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("package"))
        .unwrap_or(false)
}

fn is_package_local_metadata(type_id: u32) -> bool {
    matches!(type_id, TYPE_NMAP | TYPE_MANIFEST)
}

fn resource_label(type_id: u32) -> String {
    match type_id {
        TYPE_IMG => "_IMG".into(),
        TYPE_PNG => "PNG".into(),
        TYPE_FACE => "FACE".into(),
        TYPE_CBLN => "CBLN".into(),
        TYPE_BGEO => "BGEO".into(),
        TYPE_BOND => "BOND".into(),
        TYPE_BUNIT => "BUNIT".into(),
        TYPE_BUFF => "BUFF".into(),
        TYPE_GEOM => "GEOM".into(),
        TYPE_NMAP => "NMAP".into(),
        TYPE_LAYO => "LAYO".into(),
        TYPE_MODL => "MODL".into(),
        TYPE_MATD => "MATD".into(),
        TYPE_MLOD => "MLOD".into(),
        TYPE_OBJK => "OBJK".into(),
        TYPE_XML => "XML".into(),
        TYPE_TXTC => "TXTC".into(),
        TYPE_TXTF => "TXTF".into(),
        TYPE_ITUN => "ITUN".into(),
        TYPE_CASP => "CASP".into(),
        TYPE_OBJD => "OBJD".into(),
        TYPE_STBL => "STBL".into(),
        TYPE_VPXY => "VPXY".into(),
        TYPE_S3SA => "S3SA".into(),
        TYPE_MANIFEST => "Manifest".into(),
        other => format!("0x{other:08X}"),
    }
}

fn is_visual(type_id: u32) -> bool {
    matches!(
        type_id,
        TYPE_IMG
            | TYPE_PNG
            | TYPE_GEOM
            | TYPE_MODL
            | TYPE_MLOD
            | TYPE_VPXY
            | TYPE_MATD
            | TYPE_TXTC
            | TYPE_TXTF
            | TYPE_OBJK
            | TYPE_LAYO
    )
}

fn is_catalog(type_id: u32) -> bool {
    matches!(
        type_id,
        TYPE_CASP
            | TYPE_OBJD
            | TYPE_CATALOG_FENCE
            | TYPE_CATALOG_STAIRS
            | TYPE_CATALOG_PROXY
            | TYPE_CATALOG_TERRAIN_GEOMETRY
            | TYPE_CATALOG_RAILING
            | TYPE_CATALOG_TERRAIN_PAINT
            | TYPE_CATALOG_FIREPLACE
            | TYPE_CATALOG_WATER
            | TYPE_CATALOG_POOL
            | TYPE_CATALOG_FOUNDATION
    )
}

fn is_gameplay(type_id: u32) -> bool {
    matches!(type_id, TYPE_XML | TYPE_ITUN | TYPE_BUFF)
}

fn is_cas_morph(type_id: u32) -> bool {
    matches!(
        type_id,
        TYPE_FACE | TYPE_CBLN | TYPE_BGEO | TYPE_BOND | TYPE_BUNIT
    )
}

fn resource_class(type_id: u32) -> &'static str {
    if type_id == TYPE_S3SA {
        "script"
    } else if is_cas_morph(type_id) {
        "cas_morph"
    } else if is_gameplay(type_id) {
        "gameplay"
    } else if is_catalog(type_id) {
        "catalog"
    } else if is_visual(type_id) {
        "visual"
    } else if type_id == TYPE_STBL {
        "text"
    } else if matches!(type_id, TYPE_NMAP | TYPE_MANIFEST) {
        "metadata"
    } else {
        "unknown"
    }
}

fn impact_for(type_id: u32, same_payload: bool) -> &'static str {
    if same_payload {
        return "shared_identical";
    }

    match resource_class(type_id) {
        "script" => "script_conflict",
        "gameplay" => "gameplay_override",
        "catalog" => "catalog_override",
        "visual" => "visual_override",
        "cas_morph" => "cas_morph_overlap",
        "text" => "text_override",
        // Metadata and unknown payload differences are not promoted to a
        // specific conflict class without deeper semantics.
        _ => "potential_conflict",
    }
}

fn impact_priority(kind: &str) -> u8 {
    match kind {
        "script_conflict" => 70,
        "gameplay_override" => 60,
        "catalog_override" => 50,
        "visual_override" => 40,
        "cas_morph_overlap" => 35,
        "text_override" => 30,
        "potential_conflict" => 20,
        "shared_identical" => 0,
        _ => 10,
    }
}

fn primary_kind(kinds: &BTreeSet<String>, different_payload_count: usize) -> String {
    if different_payload_count == 0 {
        return "shared_identical".to_string();
    }

    let non_shared = kinds
        .iter()
        .filter(|kind| kind.as_str() != "shared_identical")
        .collect::<Vec<_>>();

    if non_shared.len() > 1 {
        return "mixed_override".to_string();
    }

    non_shared
        .into_iter()
        .max_by_key(|kind| impact_priority(kind))
        .map(|kind| kind.to_string())
        .unwrap_or_else(|| "potential_conflict".to_string())
}

fn severity_for(kind: &str) -> &'static str {
    match kind {
        "script_conflict" => "high",
        "gameplay_override" => "high",
        "mixed_override" => "high",
        "catalog_override" => "warning",
        "visual_override" => "warning",
        "cas_morph_overlap" => "review",
        "text_override" => "review",
        "potential_conflict" => "review",
        "shared_identical" => "info",
        _ => "review",
    }
}

fn finding_severity(
    kind: &str,
    impacts: &BTreeSet<String>,
    load_order_status: &str,
    reliable_resource_cfg: bool,
) -> &'static str {
    if kind == "filename_collision_review" {
        // Filename collisions stay visible for manual review even when the
        // packages are outside the current load rules or share identical
        // internal payloads.
        return "review";
    }
    // Resource conflicts require both packages to be loaded. Keep evidence
    // for diagnostics, but do not warn about a pair not loaded by these rules.
    if reliable_resource_cfg && matches!(load_order_status, "unmatched" | "partially_matched") {
        return "info";
    }

    // Mixed text/unknown resource differences alone are not evidence of a
    // high-severity gameplay or script conflict.
    if kind == "mixed_override" {
        if impacts.contains("script_conflict") || impacts.contains("gameplay_override") {
            return "high";
        }
        if impacts.contains("catalog_override") || impacts.contains("visual_override") {
            return "warning";
        }
        return "review";
    }
    severity_for(kind)
}

fn explanation_key(kind: &str) -> &'static str {
    match kind {
        "shared_identical" => "same_tgi_same_payload",
        "visual_override" => "same_tgi_different_visual_payload",
        "cas_morph_overlap" => "same_tgi_different_cas_morph_payload",
        "catalog_override" => "same_tgi_different_catalog_payload",
        "gameplay_override" => "same_tgi_different_gameplay_payload",
        "script_conflict" => "same_tgi_different_script_payload",
        "text_override" => "same_tgi_different_text_payload",
        "filename_collision_review" => "same_filename_different_package_bytes",
        "mixed_override" => "multiple_override_classes",
        _ => "same_tgi_different_unknown_payload",
    }
}

fn package_ref(package: &PackageInfo) -> ConflictPackage {
    ConflictPackage {
        name: package.name.clone(),
        path: package.path.to_string_lossy().to_string(),
        relative_path: package.relative_path.to_string_lossy().to_string(),
        file_sha256: package.file_sha256.clone(),
        load_priority: package.load_priority,
        load_rule: package.load_rule.clone(),
    }
}

fn package_info_from_reference(reference: &ConflictPackage) -> PackageInfo {
    PackageInfo {
        name: reference.name.clone(),
        path: PathBuf::from(&reference.path),
        relative_path: PathBuf::from(&reference.relative_path),
        file_sha256: reference.file_sha256.clone(),
        readable: true,
        resource_count: 0,
        resource_set_sha256: None,
        load_priority: reference.load_priority,
        load_rule: reference.load_rule.clone(),
    }
}

fn move_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace("\\\\?\\", "")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

/// Reconcile findings after executor-verified renames without rebuilding the
/// resource index. A rename cannot create or remove shared TGIs, so only the
/// moved package references and their Resource.cfg load precedence change.
pub(crate) fn reconcile_moved_paths(
    analysis: &mut ConflictAnalysis,
    root: &Path,
    moves: &[(PathBuf, PathBuf)],
) -> Result<(), String> {
    if moves.is_empty() {
        return Ok(());
    }
    let changed = moves
        .iter()
        .map(|(source, destination)| {
            (
                move_path_key(source),
                destination.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let resource_cfg = find_resource_cfg(root)
        .map(|path| parse_resource_cfg(&path))
        .transpose()?;
    let cfg_directory = resource_cfg
        .as_ref()
        .and_then(|cfg| PathBuf::from(&cfg.path).parent().map(Path::to_path_buf));
    let reliable = resource_cfg
        .as_ref()
        .map(|cfg| cfg.precedence_reliable)
        .unwrap_or(false);

    let mut inactive_pairs = 0usize;
    for finding in &mut analysis.findings {
        let mut finding_changed = false;
        for reference in [&mut finding.left, &mut finding.right] {
            let key = move_path_key(Path::new(&reference.path));
            let Some(destination) = changed.get(&key) else {
                continue;
            };
            let destination = destination
                .canonicalize()
                .unwrap_or_else(|_| destination.to_path_buf());
            reference.path = destination.to_string_lossy().to_string();
            reference.relative_path = destination
                .strip_prefix(root)
                .unwrap_or(&destination)
                .to_string_lossy()
                .replace('/', "\\");
            reference.name = destination
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| reference.name.clone());
            let priority = match (&resource_cfg, &cfg_directory) {
                (Some(cfg), Some(directory)) => package_priority(cfg, directory, &destination),
                _ => None,
            };
            reference.load_priority = priority.as_ref().map(|value| value.priority);
            reference.load_rule = priority.map(|value| value.rule);
            finding_changed = true;
        }
        if finding_changed {
            let left = package_info_from_reference(&finding.left);
            let right = package_info_from_reference(&finding.right);
            let (status, higher, explanation) =
                load_order_info(&left, &right, resource_cfg.is_some(), reliable);
            let impacts = finding.impact_kinds.iter().cloned().collect::<BTreeSet<_>>();
            finding.id = pair_id(&left, &right);
            finding.load_order_status = status.clone();
            finding.higher_priority_path = higher;
            finding.load_order_explanation_key = explanation;
            finding.severity = finding_severity(&finding.kind, &impacts, &status, reliable).to_string();
        }
        if reliable
            && matches!(finding.load_order_status.as_str(), "unmatched" | "partially_matched")
        {
            inactive_pairs += 1;
        }
    }
    analysis.resource_cfg = resource_cfg;
    analysis.stats.inactive_pairs = inactive_pairs;
    Ok(())
}

fn load_order_info(
    left: &PackageInfo,
    right: &PackageInfo,
    resource_cfg_present: bool,
    precedence_reliable: bool,
) -> (String, Option<String>, String) {
    if resource_cfg_present && !precedence_reliable {
        return (
            "advanced_cfg_unresolved".to_string(),
            None,
            "resource_cfg_advanced_unresolved".to_string(),
        );
    }

    match (left.load_priority, right.load_priority) {
        (Some(left_priority), Some(right_priority)) if left_priority > right_priority => (
            "resolved_by_priority".to_string(),
            Some(left.relative_path.to_string_lossy().to_string()),
            "higher_resource_cfg_priority".to_string(),
        ),
        (Some(left_priority), Some(right_priority)) if right_priority > left_priority => (
            "resolved_by_priority".to_string(),
            Some(right.relative_path.to_string_lossy().to_string()),
            "higher_resource_cfg_priority".to_string(),
        ),
        (Some(_), Some(_)) => (
            "same_priority".to_string(),
            None,
            "same_resource_cfg_priority".to_string(),
        ),
        (Some(_), None) | (None, Some(_)) => (
            "partially_matched".to_string(),
            None,
            "resource_cfg_only_one_match".to_string(),
        ),
        (None, None) if resource_cfg_present => (
            "unmatched".to_string(),
            None,
            "resource_cfg_no_matching_rule".to_string(),
        ),
        (None, None) => (
            "resource_cfg_missing".to_string(),
            None,
            "resource_cfg_missing".to_string(),
        ),
    }
}

fn pair_id(left: &PackageInfo, right: &PackageInfo) -> String {
    let mut paths = [
        left.relative_path.to_string_lossy().to_string(),
        right.relative_path.to_string_lossy().to_string(),
    ];
    paths.sort();
    let digest = format!(
        "{:X}",
        Sha256::digest(format!("{}|{}", paths[0], paths[1]).as_bytes())
    );
    format!("conflict:{}", &digest[..16])
}

fn decision_key(left: &PackageInfo, right: &PackageInfo) -> String {
    let mut hashes = [left.file_sha256.clone(), right.file_sha256.clone()];
    hashes.sort();
    let digest = format!(
        "{:X}",
        Sha256::digest(format!("{}|{}", hashes[0], hashes[1]).as_bytes())
    );
    format!("decision:{}", &digest[..24])
}

pub fn analyze_conflicts_core(
    folder: String,
    selected_paths: Option<Vec<String>>,
    operation_kind: Option<&str>,
) -> Result<ConflictAnalysis, String> {
    let total_started = Instant::now();
    if folder.trim().is_empty() {
        return Err("No folder was selected.".to_string());
    }

    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve root folder: {error}"))?;

    if !root.is_dir() {
        return Err(format!("Folder does not exist: {}", root.display()));
    }

    let resource_cfg = find_resource_cfg(&root)
        .map(|path| parse_resource_cfg(&path))
        .transpose()?;
    let resource_cfg_directory = resource_cfg
        .as_ref()
        .and_then(|cfg| PathBuf::from(&cfg.path).parent().map(Path::to_path_buf));

    let mut all_paths = crate::scanner::cached_scan_paths(&root).unwrap_or_else(|| {
        WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && is_package(entry.path()))
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>()
    });
    all_paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());

    let mut paths = if let Some(selected_paths) = selected_paths {
        if selected_paths.is_empty() {
            return Err("No packages were selected.".to_string());
        }

        let mut selected = std::collections::HashSet::<PathBuf>::new();
        for raw in selected_paths {
            let canonical = PathBuf::from(&raw)
                .canonicalize()
                .map_err(|error| format!("Could not resolve selected package {raw}: {error}"))?;
            if !canonical.starts_with(&root) || !is_package(&canonical) {
                return Err(format!(
                    "Selected package is outside the current library: {raw}"
                ));
            }
            selected.insert(canonical);
        }

        // An explicit selection is authoritative. Do not intersect it with
        // the cache-backed all_paths list: a package moved by a previous
        // execution may be valid on disk but absent from a stale cache.
        selected.into_iter().collect::<Vec<_>>()
    } else {
        all_paths.clone()
    };
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());

    if let Some(kind) = operation_kind {
        operation::set_total(kind, paths.len());
        operation::update(kind, 0, None, "indexing");
    }

    let mut cache = load_cache(&root);
    retain_existing(&mut cache, &all_paths);

    let mut packages = Vec::<PackageInfo>::with_capacity(paths.len());
    let mut resource_index = BTreeMap::<(u32, u32, u64), Vec<ResourceOccurrence>>::new();
    let mut errors = Vec::new();
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;
    let mut hashing_ms = 0u128;
    let mut dbpf_load_ms = 0u128;
    let mut resource_decode_ms = 0u128;

    for (index, path) in paths.iter().enumerate() {
        if let Some(kind) = operation_kind {
            if operation::is_cancelled(kind) {
                let _ = save_cache(&root, &cache);
                return Err(CANCELLED_ERROR.to_string());
            }
        }
        // The previous strict strip_prefix failed on Windows when paths came
        // from the scanner cache (C:\\...) and root was verbatim (\\\\?\\C:\\...).
        // That turned every relativePath into an absolute path and made all
        // Resource.cfg load priorities appear unmatched.
        let relative_path = match relative_package_path(&root, path) {
            Some(relative) => relative,
            None => {
                errors.push(format!(
                    "Skipping package outside or missing from the selected root: {}",
                    path.display()
                ));
                continue;
            }
        };
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("package")
            .to_string();

        let package_index = packages.len();
        let priority = match (&resource_cfg, &resource_cfg_directory) {
            (Some(cfg), Some(directory)) => package_priority(cfg, directory, &path),
            _ => None,
        };
        let load_priority = priority.as_ref().map(|value| value.priority);
        let load_rule = priority.map(|value| value.rule);

        // Conflict presentation can reuse a bounded-valid cached resource
        // index. Exact SHA-256 identities are still recomputed by duplicate
        // and mutating flows; a read-only conflict view must not hash the
        // full library a second time after the unified duplicate phase.
        let (cached, metrics) = match get_or_build_analysis_with_metrics(path, &mut cache) {
            Ok(value) => value,
            Err(error) => {
                packages.push(PackageInfo {
                    name,
                    path: path.clone(),
                    relative_path,
                    file_sha256: String::new(),
                    readable: false,
                    resource_count: 0,
                    resource_set_sha256: None,
                    load_priority,
                    load_rule,
                });
                errors.push(error);
                continue;
            }
        };

        if metrics.cache_hit {
            cache_hits += 1;
        } else {
            cache_misses += 1;
        }
        hashing_ms += metrics.hash_ms;
        dbpf_load_ms += metrics.dbpf_load_ms;
        resource_decode_ms += metrics.resource_decode_ms;

        let readable = cached.parse_error.is_none();
        if let Some(error) = &cached.parse_error {
            errors.push(format!("{}: {error}", path.display()));
        }

        packages.push(PackageInfo {
            name,
            path: path.clone(),
            relative_path,
            file_sha256: cached.file_sha256.clone(),
            readable,
            resource_count: cached.resources.len(),
            resource_set_sha256: readable.then(|| resource_set_sha256(&cached.resources)),
            load_priority,
            load_rule,
        });

        if readable {
            for resource in &cached.resources {
                // NMAP and Organizer merge manifests are package-local metadata.
                // They can intentionally reuse the same TGI across unrelated packages
                // (NMAP commonly uses instance 0), so indexing them as cross-package
                // conflicts creates large numbers of false positives.
                if is_package_local_metadata(resource.type_id) {
                    continue;
                }

                resource_index
                    .entry((resource.type_id, resource.group, resource.instance))
                    .or_default()
                    .push(ResourceOccurrence {
                        package_index,
                        type_id: resource.type_id,
                        group: resource.group,
                        instance: resource.instance,
                        payload_hash: resource.payload_sha256.clone(),
                        payload_size: resource.payload_size,
                    });
            }
        }

        if let Some(kind) = operation_kind {
            let processed = index + 1;
            if processed == paths.len() || processed % 8 == 0 {
                operation::update(kind, processed, None, "indexing");
            }
        }
    }

    if let Err(error) = save_cache(&root, &cache) {
        errors.push(error);
    }

    if let Some(kind) = operation_kind {
        if operation::is_cancelled(kind) {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update(kind, paths.len(), None, "comparing");
    }

    let comparison_started = Instant::now();
    let mut pairs = HashMap::<(usize, usize), PairAccumulator>::new();
    // Same-name collisions are review evidence only; resource-level evidence
    // is still collected independently below.
    mark_filename_collisions(&packages, &mut pairs);
    let mut truncated = false;
    let mut comparisons = 0usize;

    'resources: for occurrences in resource_index.values() {
        if let Some(kind) = operation_kind {
            if operation::is_cancelled(kind) {
                return Err(CANCELLED_ERROR.to_string());
            }
        }
        let mut per_package = BTreeMap::<usize, &ResourceOccurrence>::new();
        for occurrence in occurrences {
            per_package
                .entry(occurrence.package_index)
                .or_insert(occurrence);
        }

        let unique = per_package.values().copied().collect::<Vec<_>>();
        if unique.len() < 2 {
            continue;
        }

        for left_pos in 0..unique.len() {
            for right_pos in (left_pos + 1)..unique.len() {
                comparisons += 1;
                if comparisons % 512 == 0 {
                    if let Some(kind) = operation_kind {
                        if operation::is_cancelled(kind) {
                            return Err(CANCELLED_ERROR.to_string());
                        }
                        operation::set_message(
                            kind,
                            Some(format!("Compared {comparisons} shared-resource pair(s)")),
                        );
                    }
                }

                let left = unique[left_pos];
                let right = unique[right_pos];
                let key = (
                    left.package_index.min(right.package_index),
                    left.package_index.max(right.package_index),
                );

                if !pairs.contains_key(&key) && pairs.len() >= MAX_FINDINGS {
                    truncated = true;
                    break 'resources;
                }

                let same_payload = left.payload_hash == right.payload_hash
                    && left.payload_size == right.payload_size;
                let impact = impact_for(left.type_id, same_payload).to_string();

                let accumulator = pairs.entry(key).or_default();
                accumulator.shared_resource_count += 1;
                if same_payload {
                    accumulator.identical_payload_count += 1;
                } else {
                    accumulator.different_payload_count += 1;
                }
                accumulator.impact_kinds.insert(impact.clone());

                let evidence = ConflictEvidence {
                    resource_type: left.type_id,
                    resource_type_hex: format!("0x{:08X}", left.type_id),
                    resource_label: resource_label(left.type_id),
                    resource_class: resource_class(left.type_id).to_string(),
                    group: left.group,
                    group_hex: format!("0x{:08X}", left.group),
                    instance: left.instance,
                    instance_hex: format!("0x{:016X}", left.instance),
                    same_payload,
                    left_payload_sha256: left.payload_hash.clone(),
                    right_payload_sha256: right.payload_hash.clone(),
                    left_payload_size: left.payload_size,
                    right_payload_size: right.payload_size,
                    impact_kind: impact,
                };
                // Keep divergent evidence visible even when a heavily shared
                // package pair exceeds the preview limit. The old first-64
                // policy could show only identical resources despite a real
                // differing-payload count.
                if accumulator.evidence.len() < MAX_EVIDENCE_PER_FINDING {
                    accumulator.evidence.push(evidence);
                } else {
                    accumulator.evidence_truncated = true;
                    if !evidence.same_payload {
                        if let Some(index) = accumulator
                            .evidence
                            .iter()
                            .position(|item| item.same_payload)
                        {
                            accumulator.evidence[index] = evidence;
                        }
                    }
                }
            }
        }
    }

    let mut findings = Vec::with_capacity(pairs.len());
    let mut stats = ConflictStats {
        packages_scanned: packages.len(),
        readable_packages: packages.iter().filter(|package| package.readable).count(),
        unreadable_packages: packages.iter().filter(|package| !package.readable).count(),
        analysis_truncated: truncated,
        cache_hits,
        cache_misses,
        hashing_ms,
        dbpf_load_ms,
        resource_decode_ms,
        comparison_ms: comparison_started.elapsed().as_millis(),
        ..ConflictStats::default()
    };

    let mut ordered_pairs = pairs.into_iter().collect::<Vec<_>>();
    ordered_pairs.sort_by_key(|((left, right), _)| {
        (
            packages[*left]
                .relative_path
                .to_string_lossy()
                .to_ascii_lowercase(),
            packages[*right]
                .relative_path
                .to_string_lossy()
                .to_ascii_lowercase(),
        )
    });

    for ((left_index, right_index), accumulator) in ordered_pairs {
        let kind = if accumulator.filename_collision && accumulator.different_payload_count == 0 {
            "filename_collision_review".to_string()
        } else {
            primary_kind(
                &accumulator.impact_kinds,
                accumulator.different_payload_count,
            )
        };

        match kind.as_str() {
            "shared_identical" => stats.shared_identical += 1,
            "visual_override" => stats.visual_overrides += 1,
            "cas_morph_overlap" => stats.cas_morph_overlaps += 1,
            "catalog_override" => stats.catalog_overrides += 1,
            "gameplay_override" => stats.gameplay_overrides += 1,
            "script_conflict" => stats.script_conflicts += 1,
            "text_override" => stats.text_overrides += 1,
            "potential_conflict" => stats.potential_conflicts += 1,
            "mixed_override" => stats.mixed_overrides += 1,
            "filename_collision_review" => stats.filename_collisions += 1,
            _ => stats.potential_conflicts += 1,
        }

        let left = &packages[left_index];
        let right = &packages[right_index];
        let mut impact_kinds = accumulator.impact_kinds.iter().cloned().collect::<Vec<_>>();
        if accumulator.filename_collision {
            impact_kinds.push("filename_collision_review".to_string());
        }

        let (load_order_status, higher_priority_path, load_order_explanation_key) = load_order_info(
            left,
            right,
            resource_cfg.is_some(),
            resource_cfg
                .as_ref()
                .map(|cfg| cfg.precedence_reliable)
                .unwrap_or(false),
        );

        let reliable_resource_cfg = resource_cfg
            .as_ref()
            .map(|cfg| cfg.precedence_reliable)
            .unwrap_or(false);
        if reliable_resource_cfg
            && matches!(
                load_order_status.as_str(),
                "unmatched" | "partially_matched"
            )
        {
            stats.inactive_pairs += 1;
        }

        let mut evidence = accumulator.evidence;
        evidence.sort_by_key(|item| {
            (
                item.same_payload,
                item.resource_type,
                item.group,
                item.instance,
            )
        });

        findings.push(ConflictFinding {
            id: pair_id(left, right),
            decision_key: decision_key(left, right),
            severity: finding_severity(
                &kind,
                &accumulator.impact_kinds,
                &load_order_status,
                reliable_resource_cfg,
            )
            .to_string(),
            explanation_key: explanation_key(&kind).to_string(),
            kind,
            impact_kinds,
            left: package_ref(left),
            right: package_ref(right),
            shared_resource_count: accumulator.shared_resource_count,
            identical_payload_count: accumulator.identical_payload_count,
            different_payload_count: accumulator.different_payload_count,
            full_resource_set_comparable: left.resource_set_sha256.is_some()
                && right.resource_set_sha256.is_some(),
            full_resource_set_same: left.resource_set_sha256.is_some()
                && left.resource_set_sha256 == right.resource_set_sha256,
            left_resource_count: left.resource_count,
            right_resource_count: right.resource_count,
            left_resource_set_sha256: left.resource_set_sha256.clone(),
            right_resource_set_sha256: right.resource_set_sha256.clone(),
            evidence,
            evidence_truncated: accumulator.evidence_truncated,
            load_order_status,
            higher_priority_path,
            load_order_explanation_key,
        });
    }

    stats.package_pairs = findings.len();
    stats.total_ms = total_started.elapsed().as_millis();

    Ok(ConflictAnalysis {
        root: root.to_string_lossy().to_string(),
        resource_cfg,
        findings,
        stats,
        errors,
    })
}

#[tauri::command]
pub async fn analyze_conflicts(
    folder: String,
    selected_paths: Option<Vec<String>>,
) -> Result<ConflictAnalysis, String> {
    const KIND: &str = "conflicts";
    operation::run_blocking(KIND, "starting", move || {
        analyze_conflicts_core(folder, selected_paths, Some(KIND))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_external_conflict_analysis_when_requested() {
        let Ok(root) = std::env::var("S3CC_CONFLICTS_PROFILE_ROOT") else {
            return;
        };
        let limit = std::env::var("S3CC_ANALYSIS_PROFILE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(200);
        let mut paths = WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file() && is_package(entry.path()))
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        paths.sort_by_key(|path| path.to_ascii_lowercase());
        paths.truncate(limit);
        let started = Instant::now();
        let result = analyze_conflicts_core(root, Some(paths), None)
            .expect("external conflict profile should complete");
        eprintln!(
            "conflicts_profile packages={} pairs={} total_ms={} hashing_ms={} dbpf_load_ms={} resource_decode_ms={} cache_hits={} cache_misses={}",
            result.stats.packages_scanned,
            result.stats.package_pairs,
            started.elapsed().as_millis(),
            result.stats.hashing_ms,
            result.stats.dbpf_load_ms,
            result.stats.resource_decode_ms,
            result.stats.cache_hits,
            result.stats.cache_misses,
        );
    }

    #[test]
    fn mixed_text_and_unknown_overlap_requires_review_not_high_warning() {
        let impacts = [
            "potential_conflict".to_string(),
            "text_override".to_string(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(
            finding_severity("mixed_override", &impacts, "same_priority", true),
            "review"
        );
    }

    #[test]
    fn resource_cfg_excludes_unloaded_pair_from_severity_warning() {
        let impacts = ["gameplay_override".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            finding_severity("gameplay_override", &impacts, "unmatched", true),
            "info"
        );
        assert_eq!(
            finding_severity("gameplay_override", &impacts, "partially_matched", true),
            "info"
        );
        assert_eq!(
            finding_severity("gameplay_override", &impacts, "same_priority", true),
            "high"
        );
        assert_eq!(
            finding_severity("gameplay_override", &impacts, "unmatched", false),
            "high"
        );
    }

    #[test]
    fn advanced_resource_cfg_does_not_claim_a_winner() {
        let package = |name: &str, priority: i32| PackageInfo {
            name: name.to_string(),
            path: PathBuf::from(name),
            relative_path: PathBuf::from(name),
            file_sha256: format!("test-{name}"),
            readable: true,
            resource_count: 0,
            resource_set_sha256: None,
            load_priority: Some(priority),
            load_rule: Some("Packages/*.package".to_string()),
        };

        let (status, winner, _) = load_order_info(
            &package("a.package", 1000),
            &package("b.package", 500),
            true,
            false,
        );

        assert_eq!(status, "advanced_cfg_unresolved");
        assert!(winner.is_none());
    }

    #[test]
    fn package_local_metadata_is_not_cross_package_conflict_evidence() {
        assert!(is_package_local_metadata(TYPE_NMAP));
        assert!(is_package_local_metadata(TYPE_MANIFEST));
        assert!(!is_package_local_metadata(TYPE_STBL));
        assert!(!is_package_local_metadata(TYPE_GEOM));
    }

    #[test]
    fn same_name_different_bytes_is_review_only_until_resources_prove_overlap() {
        let package = |name: &str, sha: &str| PackageInfo {
            name: name.to_string(),
            path: PathBuf::from(name),
            relative_path: PathBuf::from(name),
            file_sha256: sha.to_string(),
            readable: true,
            resource_count: 0,
            resource_set_sha256: None,
            load_priority: None,
            load_rule: None,
        };
        let packages = vec![
            package("cmar_MalePubicHairFull.package", "A"),
            package("cmar_MalePubicHairFull.package", "B"),
            package("cmar_MalePubicHairFull.package", "A"),
        ];
        let mut pairs = HashMap::new();
        mark_filename_collisions(&packages, &mut pairs);
        assert!(pairs
            .get(&(0, 1))
            .is_some_and(|pair| pair.filename_collision && pair.shared_resource_count == 0));
        assert!(!pairs.contains_key(&(0, 2)));
        assert!(pairs.contains_key(&(1, 2)));
    }

    #[test]
    fn directed_selected_conflict_analysis_can_export_review_findings() {
        let Ok(root) = std::env::var("S3CC_CONFLICT_ROOT") else {
            return;
        };
        let Ok(raw_selected) = std::env::var("S3CC_CONFLICT_SELECTED") else {
            return;
        };
        let selected = raw_selected
            .split('|')
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let analysis =
            analyze_conflicts_core(root, Some(selected), None).expect("directed conflict analysis");
        if let Ok(output) = std::env::var("S3CC_CONFLICT_OUTPUT") {
            let output = PathBuf::from(output);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&analysis).unwrap()).unwrap();
        }
        assert!(analysis
            .findings
            .iter()
            .any(|finding| finding.kind == "filename_collision_review"));
    }

    #[test]
    fn same_payload_is_shared_not_conflict() {
        assert_eq!(impact_for(TYPE_ITUN, true), "shared_identical");
    }

    #[test]
    fn gameplay_payload_difference_is_gameplay_override() {
        assert_eq!(impact_for(TYPE_ITUN, false), "gameplay_override");
        assert_eq!(impact_for(TYPE_XML, false), "gameplay_override");
    }

    #[test]
    fn script_payload_difference_is_script_conflict() {
        assert_eq!(impact_for(TYPE_S3SA, false), "script_conflict");
    }

    #[test]
    fn visual_and_catalog_payload_differences_are_not_generic_conflicts() {
        assert_eq!(impact_for(TYPE_IMG, false), "visual_override");
        assert_eq!(impact_for(TYPE_LAYO, false), "visual_override");
        assert_eq!(resource_label(TYPE_LAYO), "LAYO");
        assert_eq!(impact_for(TYPE_CASP, false), "catalog_override");
        assert_eq!(impact_for(TYPE_OBJD, false), "catalog_override");
    }

    #[test]
    fn multiple_different_override_classes_are_mixed() {
        let kinds = [
            "visual_override".to_string(),
            "gameplay_override".to_string(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(primary_kind(&kinds, 2), "mixed_override");
    }

    #[test]
    fn known_sims3_resource_families_are_not_generic_unknowns() {
        assert_eq!(resource_label(TYPE_PNG), "PNG");
        assert_eq!(impact_for(TYPE_PNG, false), "visual_override");
        assert_eq!(resource_label(TYPE_BUFF), "BUFF");
        assert_eq!(impact_for(TYPE_BUFF, false), "gameplay_override");
        assert_eq!(resource_label(TYPE_CBLN), "CBLN");
        assert_eq!(resource_label(TYPE_FACE), "FACE");
        assert_eq!(resource_label(TYPE_BGEO), "BGEO");
        assert_eq!(resource_label(TYPE_BOND), "BOND");
        assert_eq!(resource_label(TYPE_BUNIT), "BUNIT");
        for type_id in [TYPE_CBLN, TYPE_FACE, TYPE_BGEO, TYPE_BOND, TYPE_BUNIT] {
            assert_eq!(impact_for(type_id, false), "cas_morph_overlap");
            assert_eq!(impact_for(type_id, true), "shared_identical");
        }
        assert_eq!(severity_for("cas_morph_overlap"), "review");
    }

    #[test]
    fn identical_only_pair_is_informational() {
        let kinds = ["shared_identical".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(primary_kind(&kinds, 0), "shared_identical");
        assert_eq!(severity_for("shared_identical"), "info");
    }
}
