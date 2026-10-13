use crate::{
    cache::{get_or_build_with_metrics, load_cache, retain_existing, save_cache, CacheBuildMetrics, FingerprintCache},
    catalog::{TYPE_CASP, TYPE_OBJD},
    operation::{self, CANCELLED_ERROR},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

const TYPE_IMG: u32 = 0x00B2_D882;
const TYPE_GEOM: u32 = 0x015A_1849;
const TYPE_NMAP: u32 = 0x0166_038C;
const TYPE_MODL: u32 = 0x0166_1233;
const TYPE_MATD: u32 = 0x01D0_E75D;
const TYPE_MLOD: u32 = 0x01D1_0F34;
const TYPE_OBJK: u32 = 0x02DC_343F;
const TYPE_XML: u32 = 0x0333_406C;
const TYPE_TXTC: u32 = 0x033A_1435;
const TYPE_TXTF: u32 = 0x0341_ACC9;
const TYPE_STBL: u32 = 0x2205_57DA;
const TYPE_VPXY: u32 = 0x7368_84F1;
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
const MAX_VARIANT_RELATIONS: usize = 10_000;

#[derive(Debug, Clone)]
struct ResourceFingerprint {
    type_id: u32,
    group: u32,
    instance: u64,
    payload_hash: String,
    payload_size: usize,
}

#[derive(Debug, Clone)]
struct PackageFingerprint {
    path: PathBuf,
    relative_path: PathBuf,
    name: String,
    size: u64,
    file_hash: String,
    content_fingerprint: Option<String>,
    structural_signature: Option<String>,
    texture_signature: Option<String>,
    catalog_signature: Option<String>,
    substantive_without_catalog_signature: Option<String>,
    resources: Vec<ResourceFingerprint>,
    parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    pub name: String,
    pub path: String,
    pub relative_path: String,
    pub size: u64,
    pub file_sha256: String,
    pub content_fingerprint: Option<String>,
    pub resource_count: usize,
    pub instances: Vec<String>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub kind: String,
    pub members: Vec<DuplicateMember>,
    pub explanation_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantRelation {
    pub id: String,
    pub kind: String,
    pub left: DuplicateMember,
    pub right: DuplicateMember,
    pub shared_resource_count: usize,
    pub identical_tgi_payload_count: usize,
    pub changed_same_tgi_count: usize,
    pub shared_structural_count: usize,
    pub changed_texture_count: usize,
    pub changed_catalog_count: usize,
    pub explanation_key: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateStats {
    pub packages_scanned: usize,
    pub readable_packages: usize,
    pub unreadable_packages: usize,
    pub exact_groups: usize,
    pub exact_files: usize,
    pub content_groups: usize,
    pub content_files: usize,
    pub retexture_relations: usize,
    pub recategorized_relations: usize,
    pub related_variant_relations: usize,
    pub variant_analysis_truncated: bool,
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
pub struct DuplicateAnalysis {
    pub root: String,
    pub groups: Vec<DuplicateGroup>,
    pub relations: Vec<VariantRelation>,
    pub stats: DuplicateStats,
    pub errors: Vec<String>,
}

fn is_package(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("package"))
        .unwrap_or(false)
}

fn resource_label(type_id: u32) -> String {
    match type_id {
        TYPE_IMG => "_IMG".to_string(),
        TYPE_GEOM => "GEOM".to_string(),
        TYPE_NMAP => "NMAP".to_string(),
        TYPE_MODL => "MODL".to_string(),
        TYPE_MATD => "MATD".to_string(),
        TYPE_MLOD => "MLOD".to_string(),
        TYPE_OBJK => "OBJK".to_string(),
        TYPE_XML => "XML".to_string(),
        TYPE_TXTC => "TXTC".to_string(),
        TYPE_TXTF => "TXTF".to_string(),
        TYPE_CASP => "CASP".to_string(),
        TYPE_OBJD => "OBJD".to_string(),
        TYPE_STBL => "STBL".to_string(),
        TYPE_VPXY => "VPXY".to_string(),
        TYPE_MANIFEST => "Manifest".to_string(),
        other => format!("0x{other:08X}"),
    }
}

fn is_structural(type_id: u32) -> bool {
    matches!(
        type_id,
        TYPE_GEOM | TYPE_MODL | TYPE_MLOD | TYPE_VPXY | TYPE_OBJK
    )
}

fn is_texture_or_material(type_id: u32) -> bool {
    matches!(type_id, TYPE_IMG | TYPE_TXTC | TYPE_TXTF | TYPE_MATD)
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

fn is_descriptive_metadata(type_id: u32) -> bool {
    matches!(type_id, TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST)
}

fn hash_parts(parts: impl IntoIterator<Item = String>) -> Option<String> {
    let mut parts = parts.into_iter().collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }
    parts.sort();

    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    Some(format!("{:X}", hasher.finalize()))
}

fn full_resource_key(resource: &ResourceFingerprint) -> String {
    format!(
        "{:08X}:{:08X}:{:016X}:{}:{}",
        resource.type_id,
        resource.group,
        resource.instance,
        resource.payload_hash,
        resource.payload_size
    )
}

fn semantic_resource_key(resource: &ResourceFingerprint) -> String {
    format!(
        "{:08X}:{}:{}",
        resource.type_id, resource.payload_hash, resource.payload_size
    )
}

fn package_member(package: &PackageFingerprint) -> DuplicateMember {
    DuplicateMember {
        name: package.name.clone(),
        path: package.path.to_string_lossy().to_string(),
        relative_path: package.relative_path.to_string_lossy().to_string(),
        size: package.size,
        file_sha256: package.file_hash.clone(),
        content_fingerprint: package.content_fingerprint.clone(),
        resource_count: package.resources.len(),
        instances: package.resources.iter()
            .map(|resource| format!("0x{:016X}", resource.instance))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        parse_error: package.parse_error.clone(),
    }
}

fn build_package_fingerprint(
    root: &Path,
    path: &Path,
    cache: &mut FingerprintCache,
) -> Result<(PackageFingerprint, CacheBuildMetrics), String> {
    let (cached, metrics) = get_or_build_with_metrics(path, cache)?;

    let relative_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_path_buf();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("package")
        .to_string();

    let resources = cached
        .resources
        .iter()
        .map(|resource| ResourceFingerprint {
            type_id: resource.type_id,
            group: resource.group,
            instance: resource.instance,
            payload_hash: resource.payload_sha256.clone(),
            payload_size: resource.payload_size,
        })
        .collect::<Vec<_>>();

    let readable = cached.parse_error.is_none();
    let content_fingerprint = readable
        .then(|| hash_parts(resources.iter().map(full_resource_key)))
        .flatten();
    let structural_signature = readable
        .then(|| {
            hash_parts(
                resources
                    .iter()
                    .filter(|resource| is_structural(resource.type_id))
                    .map(semantic_resource_key),
            )
        })
        .flatten();
    let texture_signature = readable
        .then(|| {
            hash_parts(
                resources
                    .iter()
                    .filter(|resource| is_texture_or_material(resource.type_id))
                    .map(semantic_resource_key),
            )
        })
        .flatten();
    let catalog_signature = readable
        .then(|| {
            hash_parts(
                resources
                    .iter()
                    .filter(|resource| is_catalog(resource.type_id))
                    .map(semantic_resource_key),
            )
        })
        .flatten();
    let substantive_without_catalog_signature = readable
        .then(|| {
            hash_parts(
                resources
                    .iter()
                    .filter(|resource| {
                        !is_catalog(resource.type_id) && !is_descriptive_metadata(resource.type_id)
                    })
                    .map(semantic_resource_key),
            )
        })
        .flatten();

    Ok((
        PackageFingerprint {
            path: path.to_path_buf(),
            relative_path,
            name,
            size: cached.size,
            file_hash: cached.file_sha256,
            content_fingerprint,
            structural_signature,
            texture_signature,
            catalog_signature,
            substantive_without_catalog_signature,
            resources,
            parse_error: cached.parse_error,
        },
        metrics,
    ))
}

fn group_id(kind: &str, value: &str) -> String {
    let short = value.chars().take(16).collect::<String>();
    format!("{kind}:{short}")
}

fn relation_id(kind: &str, left: &PackageFingerprint, right: &PackageFingerprint) -> String {
    let mut paths = [
        left.relative_path.to_string_lossy().to_string(),
        right.relative_path.to_string_lossy().to_string(),
    ];
    paths.sort();
    let joined = format!("{kind}|{}|{}", paths[0], paths[1]);
    let digest = format!("{:X}", Sha256::digest(joined.as_bytes()));
    format!("{kind}:{}", &digest[..16])
}

fn pair_metrics(
    left: &PackageFingerprint,
    right: &PackageFingerprint,
) -> (usize, usize, usize, usize, usize, usize, Vec<String>) {
    let mut left_by_tgi = HashMap::<(u32, u32, u64), &ResourceFingerprint>::new();
    let mut right_by_tgi = HashMap::<(u32, u32, u64), &ResourceFingerprint>::new();

    for resource in &left.resources {
        left_by_tgi.insert(
            (resource.type_id, resource.group, resource.instance),
            resource,
        );
    }
    for resource in &right.resources {
        right_by_tgi.insert(
            (resource.type_id, resource.group, resource.instance),
            resource,
        );
    }

    let mut shared_resource_count = 0usize;
    let mut identical_tgi_payload_count = 0usize;
    let mut changed_same_tgi_count = 0usize;
    let mut changed_texture_count = 0usize;
    let mut changed_catalog_count = 0usize;
    let mut evidence = Vec::new();

    for (tgi, left_resource) in &left_by_tgi {
        let Some(right_resource) = right_by_tgi.get(tgi) else {
            continue;
        };

        shared_resource_count += 1;
        if left_resource.payload_hash == right_resource.payload_hash {
            identical_tgi_payload_count += 1;
        } else {
            changed_same_tgi_count += 1;
            if is_texture_or_material(left_resource.type_id) {
                changed_texture_count += 1;
            }
            if is_catalog(left_resource.type_id) {
                changed_catalog_count += 1;
            }
            if evidence.len() < 12 {
                evidence.push(format!(
                    "{} 0x{:08X}-0x{:08X}-0x{:016X}: payload differs",
                    resource_label(left_resource.type_id),
                    left_resource.type_id,
                    left_resource.group,
                    left_resource.instance
                ));
            }
        }
    }

    let left_structural = left
        .resources
        .iter()
        .filter(|resource| is_structural(resource.type_id))
        .map(semantic_resource_key)
        .collect::<BTreeSet<_>>();
    let right_structural = right
        .resources
        .iter()
        .filter(|resource| is_structural(resource.type_id))
        .map(semantic_resource_key)
        .collect::<BTreeSet<_>>();

    let shared_structural_count = left_structural
        .intersection(&right_structural)
        .count();

    (
        shared_resource_count,
        identical_tgi_payload_count,
        changed_same_tgi_count,
        shared_structural_count,
        changed_texture_count,
        changed_catalog_count,
        evidence,
    )
}

fn classify_variant_pair(
    left: &PackageFingerprint,
    right: &PackageFingerprint,
) -> Option<VariantRelation> {
    if left.parse_error.is_some() || right.parse_error.is_some() {
        return None;
    }

    if left.file_hash == right.file_hash {
        return None;
    }

    if left.content_fingerprint.is_some()
        && left.content_fingerprint == right.content_fingerprint
    {
        return None;
    }

    let same_substantive_without_catalog =
        left.substantive_without_catalog_signature.is_some()
            && left.substantive_without_catalog_signature
                == right.substantive_without_catalog_signature;
    let catalog_differs = left.catalog_signature.is_some()
        && right.catalog_signature.is_some()
        && left.catalog_signature != right.catalog_signature;

    let same_structure = left.structural_signature.is_some()
        && left.structural_signature == right.structural_signature;
    let textures_present = left.texture_signature.is_some() && right.texture_signature.is_some();
    let textures_differ = textures_present && left.texture_signature != right.texture_signature;

    let kind = if same_substantive_without_catalog && catalog_differs {
        "recategorized_variant"
    } else if same_structure && textures_differ {
        "retexture"
    } else if same_structure {
        "related_variant"
    } else {
        return None;
    };

    let (
        shared_resource_count,
        identical_tgi_payload_count,
        changed_same_tgi_count,
        shared_structural_count,
        changed_texture_count,
        changed_catalog_count,
        mut evidence,
    ) = pair_metrics(left, right);

    if kind == "retexture" && changed_texture_count == 0 {
        evidence.push(
            "Structural payloads match, while texture/material signatures differ under different resource keys."
                .to_string(),
        );
    }
    if kind == "recategorized_variant" && changed_catalog_count == 0 {
        evidence.push(
            "All substantive non-catalog resources match, while catalog signatures differ."
                .to_string(),
        );
    }
    if kind == "related_variant" {
        evidence.push(
            "Structural resources match, but the remaining package content is not identical."
                .to_string(),
        );
    }

    Some(VariantRelation {
        id: relation_id(kind, left, right),
        kind: kind.to_string(),
        left: package_member(left),
        right: package_member(right),
        shared_resource_count,
        identical_tgi_payload_count,
        changed_same_tgi_count,
        shared_structural_count,
        changed_texture_count,
        changed_catalog_count,
        explanation_key: match kind {
            "retexture" => "same_structure_different_visuals",
            "recategorized_variant" => "same_content_different_catalog",
            _ => "same_structure_related_content",
        }
        .to_string(),
        evidence,
    })
}

fn add_variant_candidates(
    packages: &[PackageFingerprint],
    relations: &mut Vec<VariantRelation>,
    operation_kind: Option<&str>,
) -> Result<bool, String> {
    let mut by_structure = BTreeMap::<String, Vec<usize>>::new();

    for (index, package) in packages.iter().enumerate() {
        if let Some(signature) = &package.structural_signature {
            by_structure
                .entry(signature.clone())
                .or_default()
                .push(index);
        }
    }

    let mut seen_pairs = HashSet::<(usize, usize)>::new();
    let mut comparisons = 0usize;

    for indices in by_structure.values().filter(|indices| indices.len() > 1) {
        for left_pos in 0..indices.len() {
            for right_pos in (left_pos + 1)..indices.len() {
                comparisons += 1;
                if comparisons % 256 == 0 {
                    if let Some(kind) = operation_kind {
                        if operation::is_cancelled(kind) {
                            return Err(CANCELLED_ERROR.to_string());
                        }
                        operation::set_message(
                            kind,
                            Some(format!("Compared {comparisons} variant pair(s)")),
                        );
                    }
                }

                if relations.len() >= MAX_VARIANT_RELATIONS {
                    return Ok(true);
                }

                let left_index = indices[left_pos];
                let right_index = indices[right_pos];
                if !seen_pairs.insert((left_index.min(right_index), left_index.max(right_index))) {
                    continue;
                }

                if let Some(relation) =
                    classify_variant_pair(&packages[left_index], &packages[right_index])
                {
                    relations.push(relation);
                }
            }
        }
    }

    Ok(false)
}

pub fn analyze_duplicates_core(
    folder: String,
    selected_paths: Option<Vec<String>>,
    operation_kind: Option<&str>,
) -> Result<DuplicateAnalysis, String> {
    let total_started = Instant::now();
    let root_input = PathBuf::from(folder.trim());
    if folder.trim().is_empty() {
        return Err("No folder was selected.".to_string());
    }

    let root = root_input
        .canonicalize()
        .map_err(|error| format!("Could not resolve root folder: {error}"))?;
    if !root.is_dir() {
        return Err(format!("Folder does not exist: {}", root.display()));
    }

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
                return Err(format!("Selected package is outside the current library: {raw}"));
            }
            selected.insert(canonical);
        }

        all_paths
            .iter()
            .filter(|path| path.canonicalize().ok().map(|value| selected.contains(&value)).unwrap_or(false))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        all_paths.clone()
    };
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());

    if let Some(kind) = operation_kind {
        operation::set_total(kind, paths.len());
        operation::update(kind, 0, None, "fingerprinting");
    }

    let mut cache = load_cache(&root);
    retain_existing(&mut cache, &all_paths);

    let mut packages = Vec::with_capacity(paths.len());
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

        match build_package_fingerprint(&root, path, &mut cache) {
            Ok((package, metrics)) => {
                if metrics.cache_hit {
                    cache_hits += 1;
                } else {
                    cache_misses += 1;
                }
                hashing_ms += metrics.hash_ms;
                dbpf_load_ms += metrics.dbpf_load_ms;
                resource_decode_ms += metrics.resource_decode_ms;

                if let Some(error) = &package.parse_error {
                    errors.push(format!(
                        "{}: {}",
                        package.relative_path.display(),
                        error
                    ));
                }
                packages.push(package);
            }
            Err(error) => errors.push(error),
        }

        if let Some(kind) = operation_kind {
            let processed = index + 1;
            if processed == paths.len() || processed % 8 == 0 {
                operation::update(kind, processed, None, "fingerprinting");
            }
        }
    }

    if let Err(error) = save_cache(&root, &cache) {
        errors.push(error);
    }

    let mut stats = DuplicateStats {
        packages_scanned: packages.len(),
        readable_packages: packages
            .iter()
            .filter(|package| package.parse_error.is_none())
            .count(),
        unreadable_packages: packages
            .iter()
            .filter(|package| package.parse_error.is_some())
            .count(),
        cache_hits,
        cache_misses,
        hashing_ms,
        dbpf_load_ms,
        resource_decode_ms,
        ..DuplicateStats::default()
    };

    let mut groups = Vec::new();

    let mut by_file_hash = BTreeMap::<String, Vec<usize>>::new();
    for (index, package) in packages.iter().enumerate() {
        by_file_hash
            .entry(package.file_hash.clone())
            .or_default()
            .push(index);
    }

    for (hash, indices) in by_file_hash.iter().filter(|(_, indices)| indices.len() > 1) {
        let members = indices
            .iter()
            .map(|index| package_member(&packages[*index]))
            .collect::<Vec<_>>();

        stats.exact_groups += 1;
        stats.exact_files += members.len();
        groups.push(DuplicateGroup {
            id: group_id("exact", hash),
            kind: "exact_duplicate".to_string(),
            members,
            explanation_key: "same_file_sha256".to_string(),
        });
    }

    let mut by_content = BTreeMap::<String, Vec<usize>>::new();
    for (index, package) in packages.iter().enumerate() {
        if let Some(fingerprint) = &package.content_fingerprint {
            by_content
                .entry(fingerprint.clone())
                .or_default()
                .push(index);
        }
    }

    for (fingerprint, indices) in by_content
        .iter()
        .filter(|(_, indices)| indices.len() > 1)
    {
        let distinct_file_hashes = indices
            .iter()
            .map(|index| packages[*index].file_hash.as_str())
            .collect::<HashSet<_>>();

        if distinct_file_hashes.len() < 2 {
            continue;
        }

        let members = indices
            .iter()
            .map(|index| package_member(&packages[*index]))
            .collect::<Vec<_>>();

        stats.content_groups += 1;
        stats.content_files += members.len();
        groups.push(DuplicateGroup {
            id: group_id("content", fingerprint),
            kind: "content_duplicate".to_string(),
            members,
            explanation_key: "same_normalized_resources".to_string(),
        });
    }

    if let Some(kind) = operation_kind {
        if operation::is_cancelled(kind) {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update(kind, packages.len(), None, "comparing");
    }

    let comparison_started = Instant::now();
    let mut relations = Vec::new();
    let variant_analysis_truncated =
        add_variant_candidates(&packages, &mut relations, operation_kind)?;
    stats.comparison_ms = comparison_started.elapsed().as_millis();
    stats.variant_analysis_truncated = variant_analysis_truncated;
    relations.sort_by_key(|relation| {
        (
            relation.kind.clone(),
            relation.left.relative_path.to_ascii_lowercase(),
            relation.right.relative_path.to_ascii_lowercase(),
        )
    });

    for relation in &relations {
        match relation.kind.as_str() {
            "retexture" => stats.retexture_relations += 1,
            "recategorized_variant" => stats.recategorized_relations += 1,
            "related_variant" => stats.related_variant_relations += 1,
            _ => {}
        }
    }

    groups.sort_by_key(|group| (group.kind.clone(), group.id.clone()));

    stats.total_ms = total_started.elapsed().as_millis();

    Ok(DuplicateAnalysis {
        root: root.to_string_lossy().to_string(),
        groups,
        relations,
        stats,
        errors,
    })
}

#[tauri::command]
pub async fn analyze_duplicates(folder: String, selected_paths: Option<Vec<String>>) -> Result<DuplicateAnalysis, String> {
    const KIND: &str = "duplicates";
    operation::run_blocking(KIND, "starting", move || {
        analyze_duplicates_core(folder, selected_paths, Some(KIND))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(type_id: u32, instance: u64, hash: &str) -> ResourceFingerprint {
        ResourceFingerprint {
            type_id,
            group: 0,
            instance,
            payload_hash: hash.to_string(),
            payload_size: 10,
        }
    }

    fn package(
        name: &str,
        file_hash: &str,
        resources: Vec<ResourceFingerprint>,
    ) -> PackageFingerprint {
        let content_fingerprint = hash_parts(resources.iter().map(full_resource_key));
        let structural_signature = hash_parts(
            resources
                .iter()
                .filter(|resource| is_structural(resource.type_id))
                .map(semantic_resource_key),
        );
        let texture_signature = hash_parts(
            resources
                .iter()
                .filter(|resource| is_texture_or_material(resource.type_id))
                .map(semantic_resource_key),
        );
        let catalog_signature = hash_parts(
            resources
                .iter()
                .filter(|resource| is_catalog(resource.type_id))
                .map(semantic_resource_key),
        );
        let substantive_without_catalog_signature = hash_parts(
            resources
                .iter()
                .filter(|resource| {
                    !is_catalog(resource.type_id)
                        && !is_descriptive_metadata(resource.type_id)
                })
                .map(semantic_resource_key),
        );

        PackageFingerprint {
            path: PathBuf::from(name),
            relative_path: PathBuf::from(name),
            name: name.to_string(),
            size: 100,
            file_hash: file_hash.to_string(),
            content_fingerprint,
            structural_signature,
            texture_signature,
            catalog_signature,
            substantive_without_catalog_signature,
            resources,
            parse_error: None,
        }
    }

    #[test]
    fn same_normalized_resources_can_differ_as_container_files() {
        let resources = vec![
            resource(TYPE_GEOM, 1, "GEOM"),
            resource(TYPE_IMG, 2, "IMAGE"),
        ];
        let left = package("a.package", "FILE-A", resources.clone());
        let right = package("b.package", "FILE-B", resources);

        assert_ne!(left.file_hash, right.file_hash);
        assert_eq!(left.content_fingerprint, right.content_fingerprint);
    }

    #[test]
    fn same_structure_with_different_textures_is_retexture() {
        let left = package(
            "a.package",
            "FILE-A",
            vec![
                resource(TYPE_GEOM, 1, "GEOM"),
                resource(TYPE_IMG, 2, "RED"),
            ],
        );
        let right = package(
            "b.package",
            "FILE-B",
            vec![
                resource(TYPE_GEOM, 9, "GEOM"),
                resource(TYPE_IMG, 8, "BLUE"),
            ],
        );

        let relation = classify_variant_pair(&left, &right).unwrap();
        assert_eq!(relation.kind, "retexture");
    }

    #[test]
    fn same_non_catalog_content_with_changed_casp_is_recategorized() {
        let left = package(
            "a.package",
            "FILE-A",
            vec![
                resource(TYPE_GEOM, 1, "GEOM"),
                resource(TYPE_IMG, 2, "IMAGE"),
                resource(TYPE_CASP, 3, "CASP-A"),
            ],
        );
        let right = package(
            "b.package",
            "FILE-B",
            vec![
                resource(TYPE_GEOM, 1, "GEOM"),
                resource(TYPE_IMG, 2, "IMAGE"),
                resource(TYPE_CASP, 3, "CASP-B"),
            ],
        );

        let relation = classify_variant_pair(&left, &right).unwrap();
        assert_eq!(relation.kind, "recategorized_variant");
    }

    #[test]
    fn same_tgi_is_not_required_for_retexture_relationship() {
        let left = package(
            "a.package",
            "FILE-A",
            vec![resource(TYPE_GEOM, 1, "GEOM"), resource(TYPE_IMG, 2, "RED")],
        );
        let right = package(
            "b.package",
            "FILE-B",
            vec![resource(TYPE_GEOM, 99, "GEOM"), resource(TYPE_IMG, 88, "BLUE")],
        );

        let relation = classify_variant_pair(&left, &right).unwrap();
        assert_eq!(relation.kind, "retexture");
        assert_eq!(relation.changed_same_tgi_count, 0);
        assert!(relation.shared_structural_count > 0);
    }
}
