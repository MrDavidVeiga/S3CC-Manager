use crate::{
    cache::{get_or_build_analysis_with_metrics, load_cache, save_cache},
    i18n::AppLanguage,
    manifest::sha256_file,
    resource_cfg::{find_resource_cfg, package_priority, parse_resource_cfg, ResourceCfgInfo},
    resource_cfg_update::{preview_resource_cfg_update, ResourceCfgUpdate},
    scanner::{
        cached_scan_for, scan_packages_core, scan_selected_package_paths, ScanPackageItem,
        ScanResult, ScanStats,
    },
    workspace::{
        active_profile, is_protected, load_workspace_for_root, matching_rule, split_destination,
    },
};
use chrono::{Local, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub source_relative_path: String,
    pub destination_path: Option<String>,
    pub destination_relative_path: Option<String>,
    pub classification_status: String,
    pub classification_reason: Option<String>,
    pub plan_status: String,
    pub sha256: Option<String>,
    pub size: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlanStats {
    pub selected: usize,
    pub kept_uncategorized: usize,
    pub ready: usize,
    pub already_organized: usize,
    pub duplicate_skipped: usize,
    /// Exact duplicates that have a verified, executable destination in the
    /// flat #Duplicados quarantine. These are separate from ordinary CCs so
    /// the plan can show what will be disabled before execution.
    #[serde(default)]
    pub duplicate_quarantine_ready: usize,
    pub collision_same_content: usize,
    pub collision_different_content: usize,
    pub blocked: usize,
    pub directories_to_create: usize,
    pub empty_folders_to_clean: usize,
    /// Files that will still be physically present below the selected legacy
    /// roots after all `ready*` items are applied.
    #[serde(default)]
    pub remaining_legacy_files: usize,
    /// Legacy folders that will still contain protected/unresolved content
    /// after the ready operations are applied.
    #[serde(default)]
    pub legacy_folders_retained: usize,
    #[serde(default)]
    pub planning_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationAnalysisSnapshot {
    pub phase: String,
    pub packages: usize,
    pub classified: usize,
    pub mixed: usize,
    pub unknown: usize,
    pub needs_review: usize,
    pub invalid: usize,
    pub ready: usize,
    pub already_organized: usize,
    pub duplicate_skipped: usize,
    #[serde(default)]
    pub duplicate_quarantine_ready: usize,
    pub collision_same_content: usize,
    pub collision_different_content: usize,
    pub blocked: usize,
    pub remaining_legacy_files: usize,
    #[serde(default)]
    pub legacy_folders_retained: usize,
}

impl OrganizationAnalysisSnapshot {
    pub fn from_scan(phase: &str, stats: &ScanStats) -> Self {
        Self {
            phase: phase.to_string(),
            packages: stats.packages,
            classified: stats.classified,
            mixed: stats.mixed,
            unknown: stats.unknown,
            needs_review: stats.needs_review,
            invalid: stats.invalid,
            ready: 0,
            already_organized: 0,
            duplicate_skipped: 0,
            duplicate_quarantine_ready: 0,
            collision_same_content: 0,
            collision_different_content: 0,
            blocked: 0,
            remaining_legacy_files: 0,
            legacy_folders_retained: 0,
        }
    }

    pub fn with_plan(mut self, stats: &PlanStats, remaining_legacy_files: usize) -> Self {
        self.ready = stats.ready;
        self.already_organized = stats.already_organized;
        self.duplicate_skipped = stats.duplicate_skipped;
        self.duplicate_quarantine_ready = stats.duplicate_quarantine_ready;
        self.collision_same_content = stats.collision_same_content;
        self.collision_different_content = stats.collision_different_content;
        self.blocked = stats.blocked;
        self.remaining_legacy_files = remaining_legacy_files;
        self.legacy_folders_retained = stats.legacy_folders_retained;
        self
    }
}

#[derive(Debug, Clone)]
struct ResourceCfgContext {
    info: ResourceCfgInfo,
    directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualModelFile {
    relative_path: String,
    directory_parts: Vec<String>,
    file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualModelIndex {
    directories: BTreeMap<String, Vec<String>>,
    files_by_sha256: HashMap<String, Vec<ManualModelFile>>,
}

#[derive(Debug, Clone)]
struct ModelDestination {
    parts: Vec<String>,
    evidence: String,
}

fn model_key(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn model_relative_components(path: &Path) -> Result<Vec<String>, String> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_string_lossy().to_string()),
            _ => Err(format!("Unsafe model path: {}", path.display())),
        })
        .collect()
}

fn is_package_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let name = name.to_lowercase();
            name.ends_with(".package")
                || name.ends_with(".package.disabled")
                || name.ends_with(".package.duplicated")
        })
        .unwrap_or(false)
}

fn is_disabled_package_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let name = name.to_ascii_lowercase();
            name.ends_with(".package.disabled") || name.ends_with(".package.duplicated")
        })
        .unwrap_or(false)
}

fn build_manual_model_index(root: &Path) -> Result<ManualModelIndex, String> {
    if !root.is_dir() {
        return Err(format!(
            "The official manual model Packages directory is unavailable: {}",
            root.display()
        ));
    }
    let mut directories = BTreeMap::new();
    let mut files_by_sha256 = HashMap::<String, Vec<ManualModelFile>>::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false).into_iter() {
        let entry =
            entry.map_err(|error| format!("Failed to read complete manual model tree: {error}"))?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| format!("Could not index model path: {error}"))?;
        let components = model_relative_components(relative)?;
        if entry.file_type().is_dir() {
            directories.insert(model_key(&components), components);
            continue;
        }
        if !entry.file_type().is_file() || !is_package_file(entry.path()) {
            continue;
        }
        let file_name = components
            .last()
            .cloned()
            .ok_or_else(|| format!("Model package has no filename: {}", entry.path().display()))?;
        let directory_parts = components[..components.len().saturating_sub(1)].to_vec();
        let (sha256, _) = sha256_file(entry.path()).map_err(|error| {
            format!(
                "Could not hash model package {}: {error}",
                entry.path().display()
            )
        })?;
        files_by_sha256
            .entry(sha256.to_ascii_uppercase())
            .or_default()
            .push(ManualModelFile {
                relative_path: relative_key(relative),
                directory_parts,
                file_name,
            });
    }
    Ok(ManualModelIndex {
        directories,
        files_by_sha256,
    })
}

fn manual_model_packages_root() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("S3CC_MANUAL_MODEL_PACKAGES") {
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }
    let downloads = dirs::download_dir()
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|path| path.join("Downloads"))
        })
        .ok_or_else(|| {
            "Could not locate the current user's Downloads directory for the official manual model."
                .to_string()
        })?;
    Ok(downloads.join("Mods - ESTRUTURA MODELO").join("Packages"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentManualModelIndex {
    version: u32,
    generated_at: String,
    reference_root: String,
    index: ManualModelIndex,
}

const PERSISTENT_MANUAL_MODEL_INDEX_VERSION: u32 = 1;

fn manual_model_index_path() -> PathBuf {
    if let Ok(value) = std::env::var("S3CC_MANUAL_MODEL_INDEX") {
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("Veiga's S3CC Manager")
        .join("Data")
        .join("manual-model-index-v1.json")
}

fn manual_model_index_backup_path() -> PathBuf {
    let path = manual_model_index_path();
    PathBuf::from(format!("{}.bak", path.to_string_lossy()))
}

fn manual_model_root_matches(reference_root: &str, canonical_root: &Path) -> bool {
    normalized_path_key(Path::new(""), Path::new(reference_root))
        == normalized_path_key(Path::new(""), canonical_root)
}

fn load_persistent_manual_model_index(root: &Path) -> Option<ManualModelIndex> {
    let canonical_root = root.canonicalize().ok()?;
    let text = fs::read_to_string(manual_model_index_path()).ok()?;
    let envelope = serde_json::from_str::<PersistentManualModelIndex>(&text).ok()?;
    if envelope.version != PERSISTENT_MANUAL_MODEL_INDEX_VERSION
        || !manual_model_root_matches(&envelope.reference_root, &canonical_root)
    {
        return None;
    }
    Some(envelope.index)
}

fn save_persistent_manual_model_index(root: &Path, index: &ManualModelIndex) -> Result<(), String> {
    let path = manual_model_index_path();
    let parent = path
        .parent()
        .ok_or_else(|| format!("Manual model index has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create manual model index directory: {error}"))?;
    if path.exists() {
        fs::copy(&path, manual_model_index_backup_path())
            .map_err(|error| format!("Could not back up manual model index: {error}"))?;
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("Could not resolve manual model root: {error}"))?;
    let envelope = PersistentManualModelIndex {
        version: PERSISTENT_MANUAL_MODEL_INDEX_VERSION,
        generated_at: Local::now().to_rfc3339(),
        reference_root: root.to_string_lossy().to_string(),
        index: index.clone(),
    };
    let temp = parent.join(".manual-model-index-v1.json.tmp");
    let data = serde_json::to_vec_pretty(&envelope)
        .map_err(|error| format!("Could not serialize manual model index: {error}"))?;
    fs::write(&temp, data)
        .map_err(|error| format!("Could not write manual model index: {error}"))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not replace manual model index: {error}"))?;
    }
    fs::rename(&temp, &path)
        .map_err(|error| format!("Could not commit manual model index: {error}"))
}

fn load_or_build_manual_model_index(root: &Path) -> Result<ManualModelIndex, String> {
    if std::env::var("S3CC_REFRESH_MANUAL_MODEL").ok().as_deref() != Some("1") {
        if let Some(index) = load_persistent_manual_model_index(root) {
            return Ok(index);
        }
    }
    let index = build_manual_model_index(root)?;
    save_persistent_manual_model_index(root, &index)?;
    Ok(index)
}

fn model_parts_for_selected_root(
    root: &Path,
    source: &Path,
    model_parts: &[String],
) -> Result<Vec<String>, String> {
    if is_mods_root(root) {
        let mut parts = vec!["Packages".to_string()];
        parts.extend(model_parts.iter().cloned());
        return Ok(parts);
    }
    if let Some(branch) = loading_branch_ancestor(root, is_packages_root) {
        let target = branch.join(model_parts.iter().collect::<PathBuf>());
        if !path_is_within_root(root, &target) {
            return Err(format!(
                "The exact model destination for '{}' is outside the selected Packages subtree.",
                source.display()
            ));
        }
        return target
            .strip_prefix(root)
            .map(|relative| {
                relative_key(relative)
                    .split('\\')
                    .map(str::to_string)
                    .collect()
            })
            .map_err(|error| format!("Could not derive model destination: {error}"));
    }
    if is_packages_root(root) {
        return Ok(model_parts.to_vec());
    }
    Ok(model_parts.to_vec())
}

fn strip_packages_prefix(parts: &[String]) -> Vec<String> {
    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("Packages"))
    {
        parts[1..].to_vec()
    } else {
        parts.to_vec()
    }
}

fn semantic_path_component(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| character.to_lowercase())
        .map(|character| match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .filter(|character| !character.is_whitespace() && !matches!(character, '-' | '_' | '+'))
        .collect()
}

fn age_set(value: &str) -> Option<BTreeSet<String>> {
    let normalized = value.to_lowercase();
    let ages = [
        "recém-nascido",
        "bebê",
        "criança",
        "adolescente",
        "jovem adulto",
        "adulto",
        "idoso",
    ];
    if ["múltiplas idades", "multiple ages", "múltiples edades"]
        .iter()
        .any(|label| normalized.contains(label))
    {
        return Some(ages.iter().map(|age| age.to_string()).collect());
    }
    let mut positions = Vec::new();
    for (index, age) in ages.iter().enumerate() {
        let search = if *age == "adulto" && normalized.contains("jovem adulto") {
            normalized.replace("jovem adulto", "")
        } else {
            normalized.clone()
        };
        if search.contains(age) {
            positions.push(index);
        }
    }
    let mut result = BTreeSet::new();
    if normalized.contains(" a ") && positions.len() >= 2 {
        for index in *positions.first().unwrap()..=*positions.last().unwrap() {
            result.insert(ages[index].replace('-', " "));
        }
    } else {
        for index in positions {
            result.insert(ages[index].replace('-', " "));
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn model_components_match(candidate: &str, canonical: &str) -> bool {
    if candidate.eq_ignore_ascii_case(canonical) {
        return true;
    }
    let candidate_key = semantic_path_component(candidate);
    let canonical_key = semantic_path_component(canonical);
    if candidate_key == canonical_key {
        return true;
    }
    if let (Some(left), Some(right)) = (age_set(candidate), age_set(canonical)) {
        return left == right;
    }
    matches!(
        (candidate_key.as_str(), canonical_key.as_str()),
        ("jogabilidade", "gameplay")
            | ("gameplay", "jogabilidade")
            | ("makeup", "maquiagem")
            | ("maquiagem", "makeup")
            | ("pelocorporal", "peloscorporais")
            | ("peloscorporais", "pelocorporal")
            | ("pelofacial", "pelosfaciais")
            | ("pelosfaciais", "pelofacial")
            | ("fixes", "corrections")
            | ("corrections", "fixes")
            | ("fixes", "correcoes")
            | ("correcoes", "fixes")
            | ("fixes", "correções")
            | ("correções", "fixes")
            | ("fixes", "correcciones")
            | ("correcciones", "fixes")
            | ("corrections", "correcoes")
            | ("correcoes", "corrections")
            | ("corrections", "correcciones")
            | ("correcciones", "corrections")
            | ("substituicoes", "replacements")
            | ("replacements", "substituicoes")
            | ("substituições", "replacements")
            | ("replacements", "substituições")
            | ("reemplazos", "replacements")
            | ("replacements", "reemplazos")
            | ("substituicoes", "reemplazos")
            | ("reemplazos", "substituicoes")
            | ("substituições", "reemplazos")
            | ("reemplazos", "substituições")
            | ("iluminador", "iluminadores")
            | ("iluminadores", "iluminador")
            | ("batom", "batons")
            | ("batons", "batom")
            | ("delineador", "delineadores")
            | ("delineadores", "delineador")
            | ("tatuagem", "tatuagens")
            | ("tatuagens", "tatuagem")
    )
}

fn semantic_model_directory(index: &ManualModelIndex, candidate: &[String]) -> Option<Vec<String>> {
    let mut matches = index
        .directories
        .values()
        .filter(|canonical| {
            canonical.len() == candidate.len()
                && candidate
                    .iter()
                    .zip(canonical.iter())
                    .all(|(left, right)| model_components_match(left, right))
        })
        .cloned()
        .collect::<Vec<_>>();

    // A classifier may omit an age bucket for content whose CASP age range is
    // already part of the evidence. Permit exactly one omitted age component
    // only when the model has a single unambiguous canonical directory.
    if matches.is_empty() && candidate.len() >= 3 {
        matches = index
            .directories
            .values()
            .filter(|canonical| {
                if canonical.len() != candidate.len() + 1 {
                    return false;
                }
                let mut candidate_index = 0usize;
                let mut skipped_age = false;
                for canonical_component in canonical.iter() {
                    if candidate_index < candidate.len()
                        && model_components_match(&candidate[candidate_index], canonical_component)
                    {
                        candidate_index += 1;
                    } else if !skipped_age && age_set(canonical_component).is_some() {
                        skipped_age = true;
                    } else {
                        return false;
                    }
                }
                candidate_index == candidate.len() && skipped_age
            })
            .cloned()
            .collect::<Vec<_>>();
    }

    let unique = matches
        .iter()
        .map(|parts| model_key(parts))
        .collect::<BTreeSet<_>>();
    if unique.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

// Some localized/manual taxonomies use a coarser age bucket than the CASP
// classifier. Allow exactly one age component to be canonicalized only when
// every other component matches and the model has one unique destination.
// This never accepts a prefix, a lookalike name, or an invented directory.
fn unique_model_directory_ignoring_age_bucket(
    index: &ManualModelIndex,
    candidate: &[String],
) -> Option<Vec<String>> {
    let matches = index
        .directories
        .values()
        .filter(|canonical| canonical.len() == candidate.len())
        .filter(|canonical| {
            let mut age_mismatch = false;
            for (left, right) in candidate.iter().zip(canonical.iter()) {
                if model_components_match(left, right) {
                    continue;
                }
                if age_set(left).is_some() && age_set(right).is_some() && !age_mismatch {
                    age_mismatch = true;
                    continue;
                }
                return false;
            }
            age_mismatch
        })
        .cloned()
        .collect::<Vec<_>>();

    let unique = matches
        .iter()
        .map(|parts| model_key(parts))
        .collect::<BTreeSet<_>>();
    (unique.len() == 1).then(|| matches.into_iter().next().unwrap())
}

// A byte-identical exemplar is evidence of identity, not permission to reuse
// the exemplar's historical directory.  The physical destination is always
// resolved through the active taxonomy.  In particular, old model branches
// must not keep a package "organized" merely because the same SHA-256 still
// exists below that branch.
fn canonicalize_model_directory(
    index: &ManualModelIndex,
    model_parts: &[String],
) -> Result<Vec<String>, String> {
    let model_parts = strip_packages_prefix(model_parts);
    let Some(first) = model_parts.first() else {
        return Err("The manual model hash match has no destination directory.".to_string());
    };

    let first_key = semantic_path_component(first);
    let (canonical_parts, reason) = match first_key.as_str() {
        // The review bucket is a reserved Manager destination, not a model
        // subtree.  Never recreate the historical context below it.
        "semcategoria" => (vec!["#Sem Categoria".to_string()], "legacy review root"),
        "nraasmods" => {
            let mut parts = vec!["Scripts".to_string(), "NRaas".to_string()];
            parts.extend(model_parts.iter().skip(1).cloned());
            (parts, "legacy NRaas root")
        }
        "fixes" => {
            let mut parts = vec!["Correções".to_string()];
            parts.extend(model_parts.iter().skip(1).cloned());
            (parts, "legacy fixes root")
        }
        "cabelos" => {
            let mut parts = vec!["Cabelos e Pêlos".to_string(), "Cabelos".to_string()];
            parts.extend(model_parts.iter().skip(1).cloned());
            (parts, "legacy hair root")
        }
        _ => return Ok(model_parts.to_vec()),
    };

    let resolved = index
        .directories
        .get(&model_key(&canonical_parts))
        .cloned()
        .or_else(|| semantic_model_directory(index, &canonical_parts))
        .or_else(|| unique_model_directory_ignoring_age_bucket(index, &canonical_parts));

    resolved.ok_or_else(|| {
        format!(
            "The SHA-256 exemplar is below a {reason}, but no exact authorized directory exists in the active taxonomy: {}.",
            canonical_parts.join("\\")
        )
    })
}

fn resolve_model_destination(
    index: &ManualModelIndex,
    root: &Path,
    source: &Path,
    source_hash: &str,
    candidate_parts: &[String],
) -> Result<ModelDestination, String> {
    let name = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("Package has no valid filename: {}", source.display()))?;
    if let Some(matches) = index.files_by_sha256.get(&source_hash.to_ascii_uppercase()) {
        // Compare all files with these bytes, including differently named
        // ones. A filename match must not conceal a second model folder.
        let directories = matches
            .iter()
            .map(|entry| model_key(&entry.directory_parts))
            .collect::<BTreeSet<_>>();
        if directories.len() > 1 {
            return Err(format!(
                "The SHA-256 matches multiple different canonical model destinations: {}",
                matches
                    .iter()
                    .map(|entry| entry.relative_path.clone())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        let named = matches
            .iter()
            .filter(|entry| entry.file_name.eq_ignore_ascii_case(name))
            .collect::<Vec<_>>();
        let considered = if named.is_empty() {
            matches.iter().collect::<Vec<_>>()
        } else {
            named
        };
        let directory_key = directories
            .into_iter()
            .next()
            .ok_or_else(|| "The model hash match has no canonical directory.".to_string())?;
        let model_parts = index
            .directories
            .get(&directory_key)
            .cloned()
            .unwrap_or_else(|| considered[0].directory_parts.clone());
        let canonical_parts = canonicalize_model_directory(index, &model_parts)?;
        let parts = model_parts_for_selected_root(root, source, &canonical_parts)?;
        return Ok(ModelDestination {
            parts,
            evidence: format!(
                "Exact SHA-256 identity in the manual model, reconciled to the active taxonomy: {}",
                considered
                    .iter()
                    .map(|entry| entry.relative_path.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ),
        });
    }
    let candidate = strip_packages_prefix(candidate_parts);
    // The model index contains the selected root as an empty directory so
    // canonical empty roots can be preserved. That directory is not a
    // semantic destination for an unknown package, however: without an
    // exact SHA-256 identity, resolving [] would incorrectly mark every loose
    // root package as already organized. Route those packages through the
    // explicit review fallback instead.
    if candidate.is_empty() {
        return Err(
            "The manual model root is not a classification destination without an exact package identity."
                .to_string(),
        );
    }
    let canonical_parts = index.directories.get(&model_key(&candidate)).cloned()
        .or_else(|| semantic_model_directory(index, &candidate))
        .or_else(|| unique_model_directory_ignoring_age_bucket(index, &candidate))
        .ok_or_else(|| format!("No authorized taxonomy directory in the manual model matches the classified destination '{}'.", candidate_parts.join("\\")))?;
    let parts = model_parts_for_selected_root(root, source, &canonical_parts)?;
    let evidence = if model_key(&canonical_parts) == model_key(&candidate) {
        format!(
            "The classified destination matched the model directory exactly: {}",
            canonical_parts.join("\\")
        )
    } else {
        format!(
            "The classified destination matched the authorized model taxonomy alias '{}': {}",
            candidate.join("\\"),
            canonical_parts.join("\\")
        )
    };
    Ok(ModelDestination { parts, evidence })
}

fn resource_cfg_context(root: &Path) -> Option<ResourceCfgContext> {
    let path = find_resource_cfg(root)?;
    let info = parse_resource_cfg(&path).ok()?;
    if !info.precedence_reliable || info.rules.is_empty() {
        return None;
    }
    let directory = path.parent()?.to_path_buf();
    Some(ResourceCfgContext { info, directory })
}

// All modules must use the same loading-root detection, including backup copies.
fn is_mods_root(root: &Path) -> bool {
    crate::resource_cfg::is_mods_layout_root(root)
}

fn is_packages_root(root: &Path) -> bool {
    root.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("packages"))
}

fn is_overrides_root(root: &Path) -> bool {
    root.file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("overrides"))
}

// Users may select a subfolder of Packages or Overrides as their scan root.
// Resolve that real in-game loading branch from ancestors, not only from
// the name of the final directory. The resulting moves stay within root.
fn loading_branch_ancestor<'a>(root: &'a Path, is_branch: fn(&Path) -> bool) -> Option<&'a Path> {
    root.ancestors()
        .find(|ancestor| is_branch(ancestor) && ancestor.parent().is_some_and(is_mods_root))
}

fn is_within_packages(root: &Path) -> bool {
    loading_branch_ancestor(root, is_packages_root).is_some()
}

fn is_within_overrides(root: &Path) -> bool {
    loading_branch_ancestor(root, is_overrides_root).is_some()
}

// A user scanning Packages/Clothing/Male must not receive the redundant
// destination Packages/Clothing/Male/Clothing/Male/YA-A/Top. Only trim when
// the entire already-selected subpath is a matching prefix.
fn trim_selected_branch_prefix(root: &Path, branch: &Path, parts: &mut Vec<String>) {
    let Ok(relative) = root.strip_prefix(branch) else {
        return;
    };
    let existing = relative
        .components()
        .filter_map(|part| match part {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !existing.is_empty()
        && parts.len() >= existing.len()
        && parts
            .iter()
            .take(existing.len())
            .zip(existing.iter())
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    {
        parts.drain(..existing.len());
    }
}

// The origin decides the loading branch, not the CAS/gameplay category.
// Normal organization must never move any Override into Packages.
fn source_uses_overrides(root: &Path, source: &Path) -> bool {
    if is_within_overrides(root) {
        return true;
    }
    if !is_mods_root(root) {
        return false;
    }
    source
        .strip_prefix(root)
        .ok()
        .and_then(|relative| relative.components().next())
        .is_some_and(|part| match part {
            Component::Normal(name) => name.to_string_lossy().eq_ignore_ascii_case("Overrides"),
            _ => false,
        })
}

fn ensure_source_loading_branch(root: &Path, source: &Path, parts: &[String]) -> Vec<String> {
    if !source_uses_overrides(root, source) {
        return ensure_packages_destination(root, parts);
    }

    let mut parts = parts.to_vec();
    if parts.first().is_some_and(|part| {
        part.eq_ignore_ascii_case("Packages") || part.eq_ignore_ascii_case("Overrides")
    }) {
        parts.remove(0);
    }
    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("CAS"))
    {
        parts.remove(0);
    }
    if is_mods_root(root) {
        parts.insert(0, "Overrides".to_string());
    } else if let Some(branch) = loading_branch_ancestor(root, is_overrides_root) {
        trim_selected_branch_prefix(root, branch, &mut parts);
    }
    parts
}

// A package explicitly selected in a copied Mods tree may belong to an
// arbitrary old source folder (e.g. "#+18" or "Careers"). Do not strand it
// just because that folder is not a Manager-generated category. However,
// never reorganize the game's cache, downloads, backups or recovery trees.
fn is_mods_system_source(root: &Path, source: &Path) -> bool {
    if !is_mods_root(root) {
        return false;
    }
    let Some(Component::Normal(top)) = source
        .strip_prefix(root)
        .ok()
        .and_then(|relative| relative.components().next())
    else {
        return false;
    };
    let top = top.to_string_lossy().to_ascii_lowercase();
    matches!(
        top.as_str(),
        "dccache"
            | "dcbackup"
            | "downloads"
            | "savedsims"
            | "library"
            | "saves"
            | "installedworlds"
            | "exports"
            | "screenshots"
            | "collections"
            | "thumbnails"
            | "cache"
            | "caches"
            | "quarantine"
            | "restore manifests"
            | "s3cc manager"
            | "s3cc organizer"
            | "backups"
            | "backup"
    )
}

// Older versions created their category trees beside Packages inside Mods.
// Migrate only those recognizable legacy category roots. Never treat
// Overrides, DCCache or unrelated custom folders as organizer-owned.
fn legacy_manager_source(root: &Path, source: &Path) -> bool {
    if !is_mods_root(root) {
        return false;
    }
    let Ok(relative) = source.strip_prefix(root) else {
        return false;
    };
    if relative.components().count() < 2 {
        return false;
    }
    let Some(name) = relative
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
    else {
        return false;
    };
    [
        "cas",
        "sliders",
        "clothing",
        "roupas",
        "ropa",
        "hair",
        "cabelos",
        "cabello",
        "accessories",
        "acessórios",
        "accesorios",
        "makeup",
        "maquiagem",
        "maquillaje",
        "genetics",
        "genética",
        "genetica",
        "pets",
        "animais",
        "mascotas",
        "patterns",
        "padrões",
        "patrones",
        "buy",
        "compra",
        "build",
        "construção",
        "construcción",
        "objects",
        "objetos",
        "gameplay",
        "jogabilidade",
        "jugabilidad",
        "scripts",
        "store",
        "nraas",
        "localization",
        "localização",
        "localización",
        "poses and animations",
        "poses e animações",
        "poses y animaciones",
    ]
    .contains(&name.as_str())
}

// When the user selected Mods rather than Mods/Packages, the organizer's
// destination folders must still stay inside Packages, even if the selected
// package has no reliable Resource.cfg rule.
fn ensure_packages_destination(root: &Path, parts: &[String]) -> Vec<String> {
    let mut parts = parts.to_vec();
    let has_packages_prefix = parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("Packages"));
    // Explicit legacy rules may contain CAS or Packages/CAS. CAS must not
    // consume a Resource.cfg nesting level. Never produce Packages/Packages.
    let cas_index = usize::from(has_packages_prefix);
    if parts
        .get(cas_index)
        .is_some_and(|part| part.eq_ignore_ascii_case("CAS"))
    {
        parts.remove(cas_index);
    }
    if is_within_packages(root) && has_packages_prefix {
        parts.remove(0);
    } else if is_mods_root(root) && !has_packages_prefix {
        parts.insert(0, "Packages".to_string());
    }
    if let Some(branch) = loading_branch_ancestor(root, is_packages_root) {
        trim_selected_branch_prefix(root, branch, &mut parts);
    }
    parts
}

fn destination_path(root: &Path, parts: &[String], file_name: &std::ffi::OsStr) -> PathBuf {
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path.push(file_name);
    path
}

fn fit_destination_to_resource_cfg(
    root: &Path,
    source: &Path,
    file_name: &std::ffi::OsStr,
    parts: &[String],
    context: Option<&ResourceCfgContext>,
) -> Result<(Vec<String>, Option<String>), String> {
    let override_source = source_uses_overrides(root, source);
    let packages_parts = ensure_source_loading_branch(root, source, parts);
    let Some(context) = context else {
        if override_source {
            return Err("Overrides cannot be moved without a readable Resource.cfg. The category remains available for manual review.".to_string());
        }
        return Ok((packages_parts, None));
    };

    // Even when the original package sits in a legacy Mods/CAS directory
    // that Resource.cfg never loaded, the NEW destination must match
    // Resource.cfg or be compacted to a matching depth.
    let source_priority = package_priority(&context.info, &context.directory, source);
    let proposed = if let Some(source_priority) = source_priority {
        // Preserve literal prefixes for covered packages, without
        // accidentally repeating Mods/Packages.
        let rule_prefix = source_priority
            .rule
            .replace('\\', "/")
            .split('/')
            .take_while(|part| !part.contains('*'))
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let root_relative = root
            .strip_prefix(&context.directory)
            .ok()
            .map(|value| {
                value
                    .components()
                    .filter_map(|part| match part {
                        Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut prefix = Vec::<String>::new();
        if root_relative.len() < rule_prefix.len()
            && rule_prefix
                .iter()
                .take(root_relative.len())
                .zip(root_relative.iter())
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            prefix.extend(rule_prefix.iter().skip(root_relative.len()).cloned());
        }
        if packages_parts.len() >= prefix.len()
            && prefix
                .iter()
                .zip(packages_parts.iter())
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            packages_parts.clone()
        } else {
            prefix.extend(packages_parts.iter().cloned());
            prefix
        }
    } else {
        packages_parts.clone()
    };

    let direct = destination_path(root, &proposed, file_name);
    if package_priority(&context.info, &context.directory, &direct).is_some() {
        let note = (proposed != parts).then(|| {
            format!(
                "Resource.cfg loading prefix preserved: '{}' => '{}'.",
                parts.join("\\"),
                proposed.join("\\")
            )
        });
        return Ok((proposed, note));
    }

    // A depth fallback used to merge folder names such as
    // 'Scripts - Jogabilidade - Creator'. That destroys the explicitly
    // requested hierarchy and makes re-organization inconsistent. A
    // Resource.cfg update must be previewed and confirmed instead.
    let is_canonical_script = parts.first().is_some_and(|part| part == "Scripts")
        || (parts
            .first()
            .is_some_and(|part| part == "Packages" || part == "Overrides")
            && parts.get(1).is_some_and(|part| part == "Scripts"));
    if is_canonical_script && !override_source {
        return Err(format!(
            "Resource.cfg does not load the Scripts > Gameplay > Creator hierarchy '{}'. Review the planned Resource.cfg update before organizing. No script was moved.",
            proposed.join("\\")
        ));
    }

    let original = proposed.clone();
    let mut compacted = proposed;
    // Protect the loading branch itself. If Overrides/*.package is the
    // deepest supported rule, only a flat Overrides directory is safe:
    // preserve the slider/category as metadata instead of creating unloaded
    // subfolders. Never rename or merge "Overrides" with the category.
    let minimum_depth = if override_source {
        if is_mods_root(root) {
            1
        } else {
            0
        }
    } else if is_mods_root(root) {
        2
    } else {
        1
    };
    while compacted.len() > minimum_depth {
        if override_source && compacted.len() == minimum_depth + 1 {
            compacted.pop(); // Overrides root itself, not Overrides - Category.
        } else {
            let last = compacted.pop().unwrap();
            let previous = compacted.pop().unwrap();
            compacted.push(format!("{previous} - {last}"));
        }

        let candidate = destination_path(root, &compacted, file_name);
        if package_priority(&context.info, &context.directory, &candidate).is_some() {
            let note = if override_source && compacted.len() == minimum_depth {
                "Resource.cfg permits only the Overrides root; the semantic category is retained in Manager without creating unloaded subfolders."
                    .to_string()
            } else {
                format!(
                    "Resource.cfg depth adaptation: '{}' was compacted to '{}' so the game can load the organized package.",
                    original.join("\\"),
                    compacted.join("\\")
                )
            };
            return Ok((compacted.clone(), Some(note)));
        }
    }

    Err(format!(
        "Resource.cfg does not load the proposed destination '{}' and it could not be compacted into a covered depth.",
        original.join("\\")
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlan {
    pub root: String,
    pub items: Vec<PlanItem>,
    pub stats: PlanStats,
    pub directories_to_create: Vec<String>,
    /// Existing canonical taxonomy directories are protected from empty-tree
    /// cleanup. They may be intentionally empty in the manual model.
    #[serde(default)]
    pub protected_empty_directories: Vec<String>,
    /// Explicit directory selections are carried into execution so a run
    /// with no package moves can still remove empty legacy descendants. These
    /// are relative to the selected root and never authorize deletion of the
    /// selected root itself.
    #[serde(default)]
    pub cleanup_roots: Vec<String>,
    pub manifest_preview: String,
    pub resource_cfg_update: Option<ResourceCfgUpdate>,
    pub can_execute: bool,
    pub analysis_before: OrganizationAnalysisSnapshot,
}

fn language_code(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::En => "en",
        AppLanguage::Pt => "pt",
        AppLanguage::Es => "es",
    }
}

fn invalid_windows_component(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." {
        return true;
    }
    if value.ends_with(' ') || value.ends_with('.') {
        return true;
    }
    if value.chars().any(|ch| {
        ch < '\u{20}' || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
    }) {
        return true;
    }

    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_uppercase();

    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

fn validate_destination_parts(parts: &[String]) -> Result<(), String> {
    if parts.is_empty() {
        return Err("destination has no category parts".to_string());
    }
    for part in parts {
        if invalid_windows_component(part) {
            return Err(format!("unsafe destination folder component: {part}"));
        }
    }
    Ok(())
}

fn path_is_within_root(root: &Path, candidate: &Path) -> bool {
    candidate
        .components()
        .zip(root.components())
        .all(|(candidate_component, root_component)| candidate_component == root_component)
        && candidate.components().count() >= root.components().count()
}

fn relative_key(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\\")
}

/// Normalize a planned filesystem path without requiring the destination to
/// exist. Windows may represent the same path with `/`, a `\\?\` prefix,
/// different casing, or `.`/`..` components. Both absolute and relative plan
/// paths are resolved against the selected root before comparison.
pub(crate) fn normalized_path_key(root: &Path, path: &Path) -> String {
    fn strip_verbatim_prefix(value: String) -> String {
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        value.strip_prefix("\\\\?\\").unwrap_or(&value).to_string()
    }

    let raw = strip_verbatim_prefix(path.to_string_lossy().replace('/', "\\"));
    let root_raw = strip_verbatim_prefix(root.to_string_lossy().replace('/', "\\"));
    let is_absolute = Path::new(&raw).is_absolute()
        || raw.starts_with('\\')
        || (raw.as_bytes().get(1) == Some(&b':'));
    let combined = if is_absolute || root_raw.is_empty() {
        raw
    } else {
        format!("{}\\{}", root_raw.trim_end_matches('\\'), raw)
    };

    let mut components = Vec::<String>::new();
    for component in combined.split('\\') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            if let Some(previous) = components.last() {
                if previous != ":" {
                    components.pop();
                }
            }
            continue;
        }
        components.push(component.to_string());
    }
    components.join("\\").to_ascii_lowercase()
}

fn canonical_model_directories_for_root(
    root: &Path,
    manual_model: &ManualModelIndex,
) -> Vec<String> {
    if !(is_mods_root(root) || is_packages_root(root) || is_within_packages(root)) {
        return Vec::new();
    }

    manual_model
        .directories
        .values()
        .filter_map(|parts| {
            let mapped = model_parts_for_selected_root(root, root, parts).ok()?;
            let mut relative = PathBuf::new();
            for part in mapped {
                relative.push(part);
            }
            let text = relative_key(&relative);
            (!text.is_empty()).then_some(text)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn same_relative_path(left: &str, right: &str) -> bool {
    left.replace('/', "\\")
        .trim_matches('\\')
        .eq_ignore_ascii_case(right.replace('/', "\\").trim_matches('\\'))
}

fn exact_model_relative_path_matches(current: &str, canonical: &str) -> bool {
    // This is deliberately an exact, component-by-component comparison. A
    // folder such as "#6 Slider" or "Creator\\Sliders" is not equivalent to
    // the model's "Sliders", even when the final component looks familiar.
    relative_key(Path::new(current)) == relative_key(Path::new(canonical))
}

fn add_missing_directories(root: &Path, parts: &[String], output: &mut BTreeSet<String>) {
    let mut relative = PathBuf::new();
    for part in parts {
        relative.push(part);
        if !root.join(&relative).exists() {
            output.insert(relative_key(&relative));
        }
    }
}

fn fallback_relative_path(
    root: &Path,
    language: AppLanguage,
    source_relative: &str,
    source_hash: &str,
) -> Result<(PathBuf, Vec<String>), String> {
    let source_relative = Path::new(source_relative);
    if source_uses_overrides(root, &root.join(source_relative)) {
        // An uncategorized/colliding Override must never be redirected into
        // Packages or an unsupported Overrides subfolder.
        return Ok((source_relative.to_path_buf(), Vec::new()));
    }
    let components = source_relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_string_lossy().to_string()),
            _ => Err(format!(
                "Source relative path is not safe for Not Categorized: {}",
                source_relative.display()
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let file_name = components
        .last()
        .cloned()
        .ok_or_else(|| "Source relative path has no filename.".to_string())?;

    // Review fallbacks are intentionally flat. The original relative path is
    // already persisted in the manifest/history, so reproducing creator,
    // legacy category, or intermediate folder names below #Sem Categoria
    // would only recreate the obsolete hierarchy the planner is supposed to
    // retire.
    let mut destination_parts = vec![language.uncategorized_fallback_folder().to_string()];
    destination_parts = ensure_packages_destination(root, &destination_parts);

    let mut destination_relative = PathBuf::new();
    for part in &destination_parts {
        destination_relative.push(part);
    }
    destination_relative.push(&file_name);

    let source_absolute = root.join(source_relative);
    let mut destination_absolute = root.join(&destination_relative);

    if destination_absolute.exists()
        && !same_path_case_insensitive(&source_absolute, &destination_absolute)
    {
        let file = Path::new(&file_name);
        let stem = file
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("package");
        let extension = file
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("package");
        let short_hash = &source_hash[..source_hash.len().min(8)];

        let mut attempt = 0usize;
        loop {
            let suffix = if attempt == 0 {
                format!(" [{short_hash}]")
            } else {
                format!(" [{short_hash}-{attempt}]")
            };
            let candidate_name = format!("{stem}{suffix}.{extension}");
            let mut candidate = PathBuf::new();
            for part in &destination_parts {
                candidate.push(part);
            }
            candidate.push(candidate_name);
            let candidate_absolute = root.join(&candidate);
            if !candidate_absolute.exists() {
                destination_relative = candidate;
                destination_absolute = candidate_absolute;
                break;
            }
            attempt += 1;
        }
    }

    let _ = destination_absolute;
    Ok((destination_relative, destination_parts))
}

/// Build the inactive filename used by the flat duplicate quarantine. The
/// marker is deliberately not a loadable `.package` suffix, so a quarantined
/// copy cannot become active merely because Resource.cfg also covers the
/// quarantine directory.
fn duplicate_quarantine_file_name(file_name: &str, hash_key: &str, attempt: usize) -> String {
    let lower = file_name.to_ascii_lowercase();
    let suffix = if attempt == 0 {
        String::new()
    } else if attempt == 1 {
        format!(" [{}]", hash_key)
    } else {
        format!(" [{}-{}]", hash_key, attempt - 1)
    };
    let stem = if lower.ends_with(".package.disabled") {
        &file_name[..file_name.len() - ".package.disabled".len()]
    } else if lower.ends_with(".package.duplicated") {
        &file_name[..file_name.len() - ".package.duplicated".len()]
    } else if lower.ends_with(".package") {
        &file_name[..file_name.len() - ".package".len()]
    } else {
        file_name
    };
    format!("{stem}{suffix}.package.duplicated")
}

/// Allocate a reversible quarantine destination for a confirmed byte-identical
/// duplicate. All duplicate files are kept directly below #Duplicados. No
/// file is deleted or overwritten by this helper.
fn duplicate_quarantine_destination(
    root: &Path,
    language: AppLanguage,
    source: &Path,
    source_hash: &str,
    reserved: &HashSet<String>,
) -> Result<(PathBuf, Vec<String>), String> {
    let file_name = source
        .file_name()
        .ok_or_else(|| format!("Duplicate source has no filename: {}", source.display()))?;
    let hash_key = source_hash.chars().take(16).collect::<String>();
    if hash_key.is_empty() {
        return Err(format!(
            "Duplicate source has no SHA-256: {}",
            source.display()
        ));
    }
    let source_relative = source.strip_prefix(root).unwrap_or(source);
    let parts = if is_mods_root(root) {
        vec![
            "Packages".to_string(),
            language.duplicates_folder().to_string(),
        ]
    } else {
        vec![language.duplicates_folder().to_string()]
    };

    // A file already directly below #Duplicados with the inactive marker is
    // already in the canonical flat quarantine. Active/legacy names in the
    // flat root are normalized in place to the marker on the next plan.
    let direct_quarantine_parent = source_relative.parent().is_some_and(|parent| {
        let mut expected = PathBuf::new();
        for part in &parts {
            expected.push(part);
        }
        same_path_case_insensitive(parent, &expected)
    });
    if direct_quarantine_parent
        && source
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.to_ascii_lowercase().ends_with(".package.duplicated"))
    {
        return Ok((source_relative.to_path_buf(), parts));
    }

    let mut candidate = PathBuf::new();
    for part in &parts {
        candidate.push(part);
    }
    let file_name = file_name.to_string_lossy();
    candidate.push(duplicate_quarantine_file_name(&file_name, &hash_key, 0));
    let occupied = |relative: &Path| {
        root.join(relative).exists()
            || reserved.contains(&relative_key(relative).to_ascii_lowercase())
    };

    if occupied(&candidate) {
        for attempt in 0usize..10_000 {
            let name = duplicate_quarantine_file_name(&file_name, &hash_key, attempt + 1);
            let mut alternative = PathBuf::new();
            for part in &parts {
                alternative.push(part);
            }
            alternative.push(name);
            if !occupied(&alternative) {
                return Ok((alternative, parts));
            }
        }
        return Err(format!(
            "Could not allocate duplicate quarantine path for {}",
            source.display()
        ));
    }
    Ok((candidate, parts))
}

fn duplicate_logical_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let lower = name.to_ascii_lowercase();
    let stem = [".package.disabled", ".package.duplicated", ".package"]
        .into_iter()
        .find_map(|suffix| {
            lower
                .strip_suffix(suffix)
                .map(|_| &name[..name.len() - suffix.len()])
        })?;
    Some(stem.to_ascii_lowercase())
}

fn is_inside_duplicate_quarantine(path: &Path, quarantine: &Path) -> bool {
    path == quarantine || path.strip_prefix(quarantine).is_ok()
}

/// Confirm an active keeper before changing an existing duplicate-quarantine
/// entry. The quarantine tree itself is deliberately excluded: another stale
/// copy there is not proof that the user still has a preserved, loadable
/// package. The index supplies likely canonical peers, while same-name peers
/// cover current model copies that are not in the persistent index. Every
/// accepted candidate is rehashed in full before it can authorize a move.
fn confirmed_quarantine_keeper(
    root: &Path,
    quarantine: &Path,
    source: &Path,
    source_hash: &str,
    manual_model: &ManualModelIndex,
) -> Option<PathBuf> {
    let source_name = duplicate_logical_name(source)?;
    let mut candidates = BTreeSet::<PathBuf>::new();

    if let Some(files) = manual_model
        .files_by_sha256
        .get(&source_hash.to_ascii_uppercase())
    {
        for file in files {
            let candidate = root.join(&file.relative_path);
            if candidate.is_file()
                && !is_inside_duplicate_quarantine(&candidate, quarantine)
                && is_package_file(&candidate)
                && !is_disabled_package_path(&candidate)
            {
                if let Ok(canonical) = candidate.canonicalize() {
                    candidates.insert(canonical);
                }
            }
        }
    }

    // Do not hash the entire selected tree just to migrate old quarantine
    // entries. Enumerate only packages with the same logical filename, then
    // verify their complete SHA-256. This also handles a current copy that
    // predates the persistent model index.
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file()
            || !is_package_file(entry.path())
            || is_inside_duplicate_quarantine(entry.path(), quarantine)
            || is_disabled_package_path(entry.path())
            || duplicate_logical_name(entry.path()).as_deref() != Some(source_name.as_str())
        {
            continue;
        }
        if let Ok(canonical) = entry.path().canonicalize() {
            candidates.insert(canonical);
        }
    }

    let model_paths = manual_model
        .files_by_sha256
        .get(&source_hash.to_ascii_uppercase())
        .into_iter()
        .flatten()
        .map(|file| relative_key(Path::new(&file.relative_path)).to_ascii_lowercase())
        .collect::<HashSet<_>>();

    let mut confirmed = candidates
        .into_iter()
        .filter(|candidate| !same_path_case_insensitive(candidate, source))
        .filter_map(|candidate| {
            let (hash, _) = sha256_file(&candidate).ok()?;
            if !hash.eq_ignore_ascii_case(source_hash) {
                return None;
            }
            let relative = candidate
                .strip_prefix(root)
                .map(relative_key)
                .unwrap_or_else(|_| relative_key(&candidate));
            Some((
                model_paths.contains(&relative.to_ascii_lowercase()),
                relative,
                candidate,
            ))
        })
        .collect::<Vec<_>>();

    confirmed.sort_by_key(|(is_model, relative, _)| (!*is_model, relative.to_ascii_lowercase()));
    confirmed.into_iter().next().map(|(_, _, path)| path)
}

/// Move legacy nested duplicate quarantine entries into the canonical flat
/// root. This is part of the real planner, so old layouts are repaired by the
/// same executor and remain reversible through the normal restore manifest.
fn append_nested_duplicate_quarantine_items(
    root: &Path,
    language: AppLanguage,
    manual_model: &ManualModelIndex,
    items: &mut Vec<PlanItem>,
    directories: &mut BTreeSet<String>,
    stats: &mut PlanStats,
    reserved: &mut HashSet<String>,
) -> Result<(), String> {
    let quarantine = if is_mods_root(root) {
        root.join("Packages").join(language.duplicates_folder())
    } else {
        root.join(language.duplicates_folder())
    };
    if !quarantine.is_dir() {
        return Ok(());
    }

    let mut nested = WalkDir::new(&quarantine)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(root).ok()?.to_path_buf();
            let parent = relative.parent()?;
            let flat_parent = if is_mods_root(root) {
                Path::new("Packages").join(language.duplicates_folder())
            } else {
                PathBuf::from(language.duplicates_folder())
            };
            let is_flat = same_path_case_insensitive(parent, &flat_parent);
            let inactive = entry
                .path()
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.to_ascii_lowercase().ends_with(".package.duplicated"));
            if is_flat && inactive {
                None
            } else if is_package_file(entry.path()) {
                Some(entry.path().to_path_buf())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    nested.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());

    for source in nested {
        let source_canonical = source.canonicalize().map_err(|error| {
            format!(
                "Could not resolve nested duplicate {}: {error}",
                source.display()
            )
        })?;
        let (hash, size) = sha256_file(&source_canonical).map_err(|error| {
            format!(
                "Could not hash nested duplicate {}: {error}",
                source.display()
            )
        })?;
        let keeper =
            confirmed_quarantine_keeper(root, &quarantine, &source_canonical, &hash, manual_model);
        if keeper.is_none() {
            let relative = source_canonical
                .strip_prefix(root)
                .map_err(|_| format!("Nested duplicate escaped root: {}", source.display()))?;
            stats.blocked += 1;
            items.push(PlanItem {
                id: format!("duplicate-quarantine-blocked:{}", relative_key(relative)),
                name: source_canonical
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "duplicate.package".to_string()),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: relative_key(relative),
                destination_path: None,
                destination_relative_path: None,
                classification_status: "duplicate_quarantine".to_string(),
                classification_reason: Some(
                    "Existing quarantine entry was preserved because no active keeper was confirmed outside #Duplicados.".to_string(),
                ),
                plan_status: "blocked_duplicate_no_keeper".to_string(),
                sha256: Some(hash),
                size,
                warnings: vec![
                    "No active byte-identical keeper was confirmed with a full SHA-256 outside #Duplicados. The original extension and bytes remain untouched for review.".to_string(),
                ],
            });
            continue;
        }
        let (destination_relative, destination_parts) =
            duplicate_quarantine_destination(root, language, &source_canonical, &hash, reserved)?;
        let destination = root.join(&destination_relative);
        if same_path_case_insensitive(&source_canonical, &destination) {
            continue;
        }
        add_missing_directories(root, &destination_parts, directories);
        reserved.insert(relative_key(&destination_relative).to_ascii_lowercase());
        stats.ready += 1;
        stats.duplicate_quarantine_ready += 1;
        let relative = source_canonical
            .strip_prefix(root)
            .map_err(|_| format!("Nested duplicate escaped root: {}", source.display()))?;
        items.push(PlanItem {
            id: format!("duplicate-quarantine-migration:{}", relative_key(relative)),
            name: source_canonical.file_name().map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "duplicate.package".to_string()),
            source_path: source_canonical.to_string_lossy().to_string(),
            source_relative_path: relative_key(relative),
            destination_path: Some(destination.to_string_lossy().to_string()),
            destination_relative_path: Some(relative_key(&destination_relative)),
            classification_status: "duplicate_quarantine".to_string(),
            classification_reason: Some("Confirmed duplicate quarantine migration".to_string()),
            plan_status: "ready_duplicate_quarantine_migration".to_string(),
            sha256: Some(hash.clone()),
            size,
            warnings: vec![format!(
                "Flatten legacy duplicate quarantine entry from '{}' to '{}'; SHA-256 {}. No overwrite or deletion is permitted.",
                relative_key(relative), relative_key(&destination_relative), hash
            )],
        });
    }
    Ok(())
}

fn preview_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff"
            )
        })
}

/// Legacy creator folders often contain screenshots next to packages. Keep
/// those references, but move them only when the same selected legacy root
/// contains package/Sims3Pack content. Their complete origin path is retained
/// below the localized preview bucket, so equal filenames cannot overwrite.
fn append_preview_image_items(
    root: &Path,
    language: AppLanguage,
    selected: &HashSet<PathBuf>,
    selected_roots: &[PathBuf],
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    items: &mut Vec<PlanItem>,
    directories: &mut BTreeSet<String>,
    stats: &mut PlanStats,
) -> Result<(), String> {
    let mut legacy_roots = BTreeSet::<String>::new();
    for selected_path in selected {
        let Some(item) = scan_by_canonical.get(selected_path) else {
            continue;
        };
        let relative = Path::new(&item.relative_path);
        let Some(first) = relative.components().find_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        }) else {
            continue;
        };
        if !first.eq_ignore_ascii_case(language.previews_folder())
            && !first.eq_ignore_ascii_case(language.duplicates_folder())
        {
            legacy_roots.insert(first);
        }
    }
    for selected_root in selected_roots {
        let Ok(relative) = selected_root.strip_prefix(root) else {
            continue;
        };
        if let Some(Component::Normal(first)) = relative.components().next() {
            let first = first.to_string_lossy().to_string();
            if !first.eq_ignore_ascii_case(language.previews_folder())
                && !first.eq_ignore_ascii_case(language.duplicates_folder())
            {
                legacy_roots.insert(first);
            }
        }
    }

    for legacy_root in legacy_roots {
        let directory = root.join(&legacy_root);
        if !directory.is_dir() {
            continue;
        }
        let explicitly_selected = selected_roots.iter().any(|selected_root| {
            selected_root
                .strip_prefix(root)
                .ok()
                .and_then(|relative| relative.components().next())
                .and_then(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                    _ => None,
                })
                .is_some_and(|first| first.eq_ignore_ascii_case(&legacy_root))
        });
        let has_package_context = WalkDir::new(&directory)
            .follow_links(false)
            .into_iter()
            .flatten()
            .any(|entry| {
                entry.file_type().is_file() && {
                    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                    name.ends_with(".package")
                        || name.ends_with(".package.disabled")
                        || name.ends_with(".package.duplicated")
                        || name.ends_with(".sims3pack")
                }
            });
        if !has_package_context && !explicitly_selected {
            continue;
        }

        for entry in WalkDir::new(&directory)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() || !preview_extension(entry.path()) {
                continue;
            }
            let source = entry.path().canonicalize().map_err(|error| {
                format!(
                    "Could not resolve preview {}: {error}",
                    entry.path().display()
                )
            })?;
            let relative = source
                .strip_prefix(root)
                .map_err(|_| format!("Preview escaped selected root: {}", source.display()))?;
            let mut relative_parts = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if relative_parts.is_empty() {
                continue;
            }
            let branch_offset = usize::from(
                relative_parts.first().is_some_and(|part| {
                    part.eq_ignore_ascii_case("Packages")
                        || part.eq_ignore_ascii_case("Overrides")
                }),
            );
            let mut collapsed_nested_preview_bucket = false;
            if relative_parts
                .get(branch_offset)
                .is_some_and(|part| part.eq_ignore_ascii_case(language.previews_folder()))
            {
                let duplicate_index = branch_offset + 1;
                while relative_parts
                    .get(duplicate_index)
                    .is_some_and(|part| part.eq_ignore_ascii_case(language.previews_folder()))
                {
                    relative_parts.remove(duplicate_index);
                    collapsed_nested_preview_bucket = true;
                }
            }
            // Packages/Previews de CCs and Overrides/Previews de CCs are
            // already canonical after the preview bucket, regardless of the
            // preserved origin context (which may itself be "Packages").
            // Only a repeated preview bucket is legacy nesting that must be
            // collapsed; do not prepend the bucket on a second pass.
            if relative_parts
                .get(branch_offset)
                .is_some_and(|part| part.eq_ignore_ascii_case(language.previews_folder()))
                && !collapsed_nested_preview_bucket
            {
                let file_name = source
                    .file_name()
                    .ok_or_else(|| "Preview has no filename".to_string())?;
                let (sha256, size) = sha256_file(&source).map_err(|error| {
                    format!("Could not hash preview {}: {error}", source.display())
                })?;
                stats.already_organized += 1;
                items.push(PlanItem {
                    id: format!("preview:{}", sha256),
                    name: file_name.to_string_lossy().to_string(),
                    source_path: source.to_string_lossy().to_string(),
                    source_relative_path: relative_key(relative),
                    destination_path: Some(source.to_string_lossy().to_string()),
                    destination_relative_path: Some(relative_key(relative)),
                    classification_status: "preview".to_string(),
                    classification_reason: Some(
                        "Image preview is already in the canonical preview bucket.".to_string(),
                    ),
                    plan_status: "already_organized_preview".to_string(),
                    sha256: Some(sha256),
                    size,
                    warnings: Vec::new(),
                });
                continue;
            }
            // A Mods-root scan includes the loading branch in the relative
            // path. Remove that branch before preserving the legacy origin;
            // otherwise ensure_packages_destination() prepends Packages (or
            // Overrides) to an origin that already contains the same branch,
            // producing Packages/Previews/Packages/... on the next pass.
            let source_branch = relative_parts
                .first()
                .filter(|part| {
                    part.eq_ignore_ascii_case("Packages")
                        || part.eq_ignore_ascii_case("Overrides")
                })
                .cloned();
            if source_branch.is_some() {
                relative_parts.remove(0);
            }
            if relative_parts
                .first()
                .is_some_and(|part| part.eq_ignore_ascii_case(language.previews_folder()))
            {
                relative_parts.remove(0);
            }
            let Some(origin) = relative_parts.first().cloned() else {
                continue;
            };
            relative_parts.remove(0);
            // The filename is appended by destination_path; keep only its
            // parent directories in destination_parts.
            relative_parts.pop();
            let mut destination_parts = vec![language.previews_folder().to_string(), origin];
            destination_parts.extend(relative_parts.iter().cloned());
            destination_parts = if source_branch
                .as_deref()
                .is_some_and(|branch| branch.eq_ignore_ascii_case("Overrides"))
            {
                ensure_source_loading_branch(root, &source, &destination_parts)
            } else {
                ensure_packages_destination(root, &destination_parts)
            };
            let file_name = source
                .file_name()
                .ok_or_else(|| "Preview has no filename".to_string())?;
            let destination = destination_path(root, &destination_parts, file_name);
            let destination_relative = destination
                .strip_prefix(root)
                .map(relative_key)
                .map_err(|_| "Preview destination escaped selected root.".to_string())?;
            let (sha256, size) = sha256_file(&source)
                .map_err(|error| format!("Could not hash preview {}: {error}", source.display()))?;
            if same_path_case_insensitive(&source, &destination) {
                stats.already_organized += 1;
                items.push(PlanItem {
                    id: format!("preview:{}", sha256),
                    name: file_name.to_string_lossy().to_string(),
                    source_path: source.to_string_lossy().to_string(),
                    source_relative_path: relative_key(relative),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(destination_relative),
                    classification_status: "preview".to_string(),
                    classification_reason: Some(
                        "Image preview preserved with its legacy origin context.".to_string(),
                    ),
                    plan_status: "already_organized_preview".to_string(),
                    sha256: Some(sha256),
                    size,
                    warnings: Vec::new(),
                });
                continue;
            }
            if destination.exists() {
                let (target_hash, _) = sha256_file(&destination).map_err(|error| {
                    format!(
                        "Could not hash preview collision {}: {error}",
                        destination.display()
                    )
                })?;
                let same = target_hash.eq_ignore_ascii_case(&sha256);
                if same {
                    stats.duplicate_skipped += 1;
                } else {
                    stats.collision_different_content += 1;
                }
                items.push(PlanItem {
                    id: format!("preview:{}", sha256),
                    name: file_name.to_string_lossy().to_string(),
                    source_path: source.to_string_lossy().to_string(),
                    source_relative_path: relative_key(relative),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(destination_relative),
                    classification_status: "preview".to_string(),
                    classification_reason: Some(
                        "Image preview preserved with its legacy origin context.".to_string(),
                    ),
                    plan_status: if same {
                        "duplicate_skipped"
                    } else {
                        "collision_different_content"
                    }
                    .to_string(),
                    sha256: Some(sha256),
                    size,
                    warnings: vec![if same {
                        "Preview destination already contains identical bytes; no overwrite."
                            .to_string()
                    } else {
                        "Preview destination contains different bytes; no overwrite.".to_string()
                    }],
                });
                continue;
            }
            add_missing_directories(root, &destination_parts, directories);
            stats.ready += 1;
            items.push(PlanItem {
                id: format!("preview:{}", sha256), name: file_name.to_string_lossy().to_string(),
                source_path: source.to_string_lossy().to_string(), source_relative_path: relative_key(relative),
                destination_path: Some(destination.to_string_lossy().to_string()), destination_relative_path: Some(destination_relative),
                classification_status: "preview".to_string(), classification_reason: Some("Image preview moved with its complete legacy origin context; it is not a DBPF package.".to_string()),
                plan_status: "ready_preview_image".to_string(), sha256: Some(sha256), size,
                warnings: Vec::new(),
            });
        }
    }
    Ok(())
}

/// Move non-DBPF companions out of explicitly selected legacy roots. They
/// cannot be classified as packages, but leaving them behind would keep the
/// obsolete creator/category tree alive forever. The complete relative origin
/// is retained below #Sem Categoria\Arquivos Auxiliares and collisions are
/// handled by the same no-overwrite rule as packages.
fn append_auxiliary_items(
    root: &Path,
    language: AppLanguage,
    selected_roots: &[PathBuf],
    items: &mut Vec<PlanItem>,
    directories: &mut BTreeSet<String>,
    stats: &mut PlanStats,
) -> Result<(), String> {
    let roots = selected_roots.iter().cloned().collect::<BTreeSet<_>>();
    let preview_folder = language.previews_folder();
    let duplicate_folder = language.duplicates_folder();
    let uncategorized_folder = language.uncategorized_fallback_folder();

    for legacy_root in roots {
        if !legacy_root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&legacy_root)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let source = entry.path().canonicalize().map_err(|error| {
                format!(
                    "Could not resolve auxiliary file {}: {error}",
                    entry.path().display()
                )
            })?;
            let relative = source.strip_prefix(root).map_err(|_| {
                format!("Auxiliary file escaped selected root: {}", source.display())
            })?;
            let relative_parts = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if relative_parts.is_empty() {
                continue;
            }
            let name = relative_parts.last().cloned().unwrap_or_default();
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".package")
                || lower.ends_with(".package.disabled")
                || lower.ends_with(".package.duplicated")
                || preview_extension(&source)
            {
                continue;
            }
            if relative_parts.first().is_some_and(|part| {
                part.eq_ignore_ascii_case(preview_folder)
                    || part.eq_ignore_ascii_case(duplicate_folder)
                    || part.eq_ignore_ascii_case(uncategorized_folder)
            }) {
                continue;
            }

            // A previous execution may already have placed a companion under
            // the canonical Packages/#Sem Categoria/Arquivos Auxiliares
            // bucket. The selected Mods root includes the loading branch in
            // relative_parts, so recognize that complete physical prefix
            // before constructing a new destination. This is intentionally
            // exact: an Overrides companion still follows the normal
            // migration path, while a canonical Packages companion is
            // idempotent even when its preserved origin is named Packages.
            if relative_parts.len() >= 4
                && relative_parts[0].eq_ignore_ascii_case("Packages")
                && relative_parts[1].eq_ignore_ascii_case(uncategorized_folder)
                && relative_parts[2].eq_ignore_ascii_case("Arquivos Auxiliares")
            {
                let file_name = source
                    .file_name()
                    .ok_or_else(|| "Auxiliary file has no filename".to_string())?;
                let (sha256, size) = sha256_file(&source).map_err(|error| {
                    format!("Could not hash auxiliary file {}: {error}", source.display())
                })?;
                stats.already_organized += 1;
                items.push(PlanItem {
                    id: format!("auxiliary:{sha256}"),
                    name: file_name.to_string_lossy().to_string(),
                    source_path: source.to_string_lossy().to_string(),
                    source_relative_path: relative_key(relative),
                    destination_path: Some(source.to_string_lossy().to_string()),
                    destination_relative_path: Some(relative_key(relative)),
                    classification_status: "auxiliary".to_string(),
                    classification_reason: Some(
                        "Auxiliary file is already in the canonical preservation bucket."
                            .to_string(),
                    ),
                    plan_status: "already_organized_auxiliary".to_string(),
                    sha256: Some(sha256),
                    size,
                    warnings: Vec::new(),
                });
                continue;
            }

            let origin = relative_parts[0].clone();
            let mut destination_parts = vec![
                uncategorized_folder.to_string(),
                "Arquivos Auxiliares".to_string(),
                origin,
            ];
            destination_parts.extend(
                relative_parts[1..relative_parts.len().saturating_sub(1)]
                    .iter()
                    .cloned(),
            );
            destination_parts = ensure_packages_destination(root, &destination_parts);
            let mut destination =
                destination_path(root, &destination_parts, std::ffi::OsStr::new(&name));
            let (sha256, size) = sha256_file(&source).map_err(|error| {
                format!(
                    "Could not hash auxiliary file {}: {error}",
                    source.display()
                )
            })?;
            let mut status = "ready_auxiliary".to_string();
            let mut warning = "Non-package legacy companion moved intact to #Sem Categoria with its source context.".to_string();

            if destination.exists() && !same_path_case_insensitive(&source, &destination) {
                let (existing_hash, _) = sha256_file(&destination).map_err(|error| {
                    format!(
                        "Could not hash auxiliary collision {}: {error}",
                        destination.display()
                    )
                })?;
                if existing_hash.eq_ignore_ascii_case(&sha256) {
                    status = "duplicate_skipped".to_string();
                    warning = "An identical auxiliary file already exists in #Sem Categoria; no overwrite was performed.".to_string();
                } else {
                    let file = Path::new(&name);
                    let stem = file
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("auxiliary");
                    let extension = file
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or("");
                    for attempt in 0usize..10_000 {
                        let candidate_name = if extension.is_empty() {
                            format!("{stem} [{}-{}]", &sha256[..sha256.len().min(8)], attempt)
                        } else {
                            format!(
                                "{stem} [{}-{}].{extension}",
                                &sha256[..sha256.len().min(8)],
                                attempt
                            )
                        };
                        let candidate = destination_path(
                            root,
                            &destination_parts,
                            std::ffi::OsStr::new(&candidate_name),
                        );
                        if !candidate.exists() {
                            destination = candidate;
                            break;
                        }
                    }
                    warning = "A different auxiliary file used a deterministic hash suffix; no overwrite was performed.".to_string();
                }
            }

            let destination_relative = destination
                .strip_prefix(root)
                .map(relative_key)
                .map_err(|_| "Auxiliary destination escaped selected root.".to_string())?;
            let already = same_path_case_insensitive(&source, &destination);
            if already {
                status = "already_organized_auxiliary".to_string();
                stats.already_organized += 1;
            } else if status == "duplicate_skipped" {
                stats.duplicate_skipped += 1;
            } else {
                add_missing_directories(root, &destination_parts, directories);
                stats.ready += 1;
            }
            items.push(PlanItem {
                id: format!("auxiliary:{}", sha256),
                name,
                source_path: source.to_string_lossy().to_string(),
                source_relative_path: relative_key(relative),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative),
                classification_status: "auxiliary".to_string(),
                classification_reason: Some(
                    "Non-package file preserved without DBPF classification.".to_string(),
                ),
                plan_status: status,
                sha256: Some(sha256),
                size,
                warnings: vec![warning],
            });
        }
    }
    Ok(())
}

fn same_path_case_insensitive(left: &Path, right: &Path) -> bool {
    normalized_path_key(Path::new(""), left) == normalized_path_key(Path::new(""), right)
}

fn retarget_item_to_not_categorized(
    root: &Path,
    language: AppLanguage,
    item: &mut PlanItem,
    directories: &mut BTreeSet<String>,
    status: &str,
) -> Result<(), String> {
    let hash = item
        .sha256
        .as_deref()
        .ok_or_else(|| format!("Missing SHA-256 for {}", item.name))?;
    let (destination_relative, destination_parts) =
        fallback_relative_path(root, language, &item.source_relative_path, hash)?;
    let destination = root.join(&destination_relative);
    let destination_relative_text = relative_key(&destination_relative);

    if relative_key(Path::new(&item.source_relative_path))
        .eq_ignore_ascii_case(&destination_relative_text)
    {
        item.plan_status = "already_organized".to_string();
        item.destination_path = Some(destination.to_string_lossy().to_string());
        item.destination_relative_path = Some(destination_relative_text);
        return Ok(());
    }

    add_missing_directories(root, &destination_parts, directories);
    item.plan_status = status.to_string();
    item.destination_path = Some(destination.to_string_lossy().to_string());
    item.destination_relative_path = Some(destination_relative_text);
    Ok(())
}

// Exact duplicates must stay untouched and be reviewed in Duplicates, even
// when a duplicate happens to have a different proposed destination.
// Organize one selected representative of each exact-content group. Moving
// none leaves entire legacy source folders stranded. A second copy is never
// deleted here: it remains available for an explicitly confirmed quarantine.
fn is_selected_content_keeper(
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
) -> bool {
    let inactive = is_disabled_package_path(source);
    let keeper = selected_hashes
        .iter()
        .filter(|(path, hash)| {
            hash.eq_ignore_ascii_case(sha256) && is_disabled_package_path(path) == inactive
        })
        .map(|(path, _)| path)
        .min_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    keeper.is_none_or(|path| path == source)
}

// Select the duplicate keeper using the current MANUAL MODEL, never an old
// scanner suggestion that happens to resemble a category name. If a model
// hash is ambiguous, no legacy folder may gain priority on that basis.
fn canonical_destination_for_keeper(
    root: &Path,
    source: &Path,
    sha256: &str,
    item: &ScanPackageItem,
    manual_model: &ManualModelIndex,
    resource_cfg: Option<&ResourceCfgContext>,
) -> Option<PathBuf> {
    let file_name = Path::new(&item.name).file_name()?;
    if !source_uses_overrides(root, source) {
        let model_destination =
            resolve_model_destination(manual_model, root, source, sha256, &item.destination_parts)
                .ok()?;
        return Some(destination_path(root, &model_destination.parts, file_name));
    }
    // Overrides are a separate loading branch: the Packages-only model must
    // never turn an Override into a Packages move.
    let (parts, _) = fit_destination_to_resource_cfg(
        root,
        source,
        file_name,
        &item.destination_parts,
        resource_cfg,
    )
    .ok()?;
    Some(destination_path(root, &parts, file_name))
}

fn is_canonical_duplicate_peer(
    root: &Path,
    path: &Path,
    sha256: &str,
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    manual_model: &ManualModelIndex,
    resource_cfg: Option<&ResourceCfgContext>,
) -> bool {
    // Do not treat a raw model path as canonical.  The model can contain
    // historical folders, and the SHA-256 index intentionally records those
    // paths for evidence.  Canonicality is proven only when the planner's
    // taxonomy-resolved destination equals the physical path.
    scan_by_canonical
        .get(path)
        .and_then(|item| {
            canonical_destination_for_keeper(root, path, sha256, item, manual_model, resource_cfg)
        })
        .is_some_and(|destination| same_path_case_insensitive(path, &destination))
}

/// Add model exemplars that are present in the selected root to a duplicate
/// comparison.  The persistent model index can legitimately contain more
/// than one exemplar for a SHA-256 (for example a migrated or disabled copy),
/// so a hash match alone must never choose a keeper.  A file that is actually
/// present at an exact model path is stronger evidence and is safe to use as
/// the keeper candidate for the current execution.
fn add_existing_model_peer_paths(
    root: &Path,
    sha256: &str,
    inactive: bool,
    manual_model: &ManualModelIndex,
    candidate_paths: &mut BTreeSet<PathBuf>,
) {
    if let Some(files) = manual_model
        .files_by_sha256
        .get(&sha256.to_ascii_uppercase())
    {
        for file in files {
            let path = root.join(&file.relative_path);
            if path.is_file() && is_disabled_package_path(&path) == inactive {
                candidate_paths.insert(path);
            }
        }
    }
}

fn is_selected_content_keeper_for_scan(
    root: &Path,
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
    peer_relative_paths: &[String],
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    manual_model: &ManualModelIndex,
    resource_cfg: Option<&ResourceCfgContext>,
) -> bool {
    let inactive = is_disabled_package_path(source);
    let mut candidate_paths = selected_hashes
        .iter()
        .filter(|(path, hash)| {
            hash.eq_ignore_ascii_case(sha256) && is_disabled_package_path(path) == inactive
        })
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    for relative in peer_relative_paths {
        let path = root.join(relative);
        if path.is_file() && is_disabled_package_path(&path) == inactive {
            candidate_paths.insert(path);
        }
    }
    add_existing_model_peer_paths(root, sha256, inactive, manual_model, &mut candidate_paths);
    let mut candidates = candidate_paths
        .into_iter()
        .map(|path| {
            let canonical = is_canonical_duplicate_peer(
                root,
                &path,
                sha256,
                scan_by_canonical,
                manual_model,
                resource_cfg,
            );
            let relative = path
                .strip_prefix(root)
                .map(relative_key)
                .unwrap_or_else(|_| relative_key(&path));
            (path, canonical, relative.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();

    // If no selected copy can be proven to occupy a unique canonical model
    // destination, do not let lexical ordering promote an arbitrary legacy
    // folder to keeper. Every copy remains available for manual review.
    if !candidates.iter().any(|(_, canonical, _)| *canonical) {
        return false;
    }

    candidates.sort_by_key(|(_, canonical, relative)| (!*canonical, relative.clone()));
    candidates
        .first()
        .is_none_or(|(path, _, _)| *path == source)
}

fn selected_content_keeper_relative_path(
    root: &Path,
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
    peer_relative_paths: &[String],
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    manual_model: &ManualModelIndex,
    resource_cfg: Option<&ResourceCfgContext>,
) -> Option<String> {
    let inactive = is_disabled_package_path(source);
    let mut candidate_paths = selected_hashes
        .iter()
        .filter(|(path, hash)| {
            hash.eq_ignore_ascii_case(sha256) && is_disabled_package_path(path) == inactive
        })
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    for relative in peer_relative_paths {
        let path = root.join(relative);
        if path.is_file() && is_disabled_package_path(&path) == inactive {
            candidate_paths.insert(path);
        }
    }
    add_existing_model_peer_paths(root, sha256, inactive, manual_model, &mut candidate_paths);
    let mut candidates = candidate_paths
        .into_iter()
        .map(|path| {
            let canonical = is_canonical_duplicate_peer(
                root,
                &path,
                sha256,
                scan_by_canonical,
                manual_model,
                resource_cfg,
            );
            let relative = path
                .strip_prefix(root)
                .map(relative_key)
                .unwrap_or_else(|_| relative_key(&path));
            (canonical, relative.to_ascii_lowercase(), relative)
        })
        .collect::<Vec<_>>();
    if !candidates.iter().any(|(canonical, _, _)| *canonical) {
        return None;
    }
    candidates.sort_by_key(|(canonical, key, _)| (!*canonical, key.clone()));
    candidates
        .into_iter()
        .next()
        .map(|(_, _, relative)| relative)
}

/// When a persistent index was generated before a model folder was renamed,
/// its hash entries may still point at paths that no longer exist.  A current
/// classification can nevertheless provide an exact model directory.  Use
/// that path only if the real peer is present there and it is one of the
/// already-indexed duplicate peers; this never promotes a guessed or merely
/// similarly named folder.
fn current_model_destination_keeper_relative_path(
    root: &Path,
    source: &Path,
    peer_relative_paths: &[String],
    model_destination: Option<&ModelDestination>,
    classified_parts: &[String],
) -> Option<String> {
    let file_name = source.file_name()?;
    let candidate_parts = model_destination
        .map(|destination| vec![destination.parts.clone()])
        .unwrap_or_else(|| {
            // Scanner classifications can carry a non-physical wrapper such
            // as `CAS`. Test the complete classified path and each exact
            // suffix, but accept only a real peer at a path already present
            // in the duplicate index. This is not a name-based fallback.
            (0..classified_parts.len())
                .map(|start| classified_parts[start..].to_vec())
                .collect()
        });

    let mut matches = BTreeSet::<String>::new();
    for parts in candidate_parts {
        let mut relative = PathBuf::new();
        for part in parts {
            relative.push(part);
        }
        relative.push(file_name);
        let relative_text = relative_key(&relative);
        let path = root.join(&relative);
        if path.is_file()
            && !same_path_case_insensitive(source, &path)
            && peer_relative_paths
                .iter()
                .any(|peer| same_relative_path(peer, &relative_text))
        {
            matches.insert(relative_text);
        }
    }
    (matches.len() == 1).then(|| matches.into_iter().next().unwrap())
}

fn index_exact_duplicates(
    root: &Path,
    scan_items: &[ScanPackageItem],
    selected_hashes: &HashMap<PathBuf, String>,
) -> HashMap<String, Vec<String>> {
    let selected_sizes = selected_hashes
        .keys()
        .filter_map(|path| fs::metadata(path).ok().map(|metadata| metadata.len()))
        .collect::<HashSet<_>>();
    if selected_sizes.is_empty() {
        return HashMap::new();
    }
    let cache = load_cache(root);
    let mut by_hash = HashMap::<String, Vec<String>>::new();
    let mut indexed_paths = HashSet::<PathBuf>::new();
    for item in scan_items {
        if !selected_sizes.contains(&item.file_size) {
            continue;
        }
        let Ok(path) = PathBuf::from(&item.path).canonicalize() else {
            continue;
        };
        indexed_paths.insert(path.clone());
        let hash = if let Some(hash) = selected_hashes.get(&path) {
            hash.clone()
        } else {
            // Reuse the same fingerprints the Duplicates analysis already
            // recorded. Never trust a cached hash if size or mtime changed.
            let cached_hash =
                cache
                    .entries
                    .get(path.to_string_lossy().as_ref())
                    .and_then(|cached| {
                        let metadata = fs::metadata(&path).ok()?;
                        let modified = metadata
                            .modified()
                            .ok()?
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_nanos();
                        (cached.size == metadata.len() && cached.modified_ns == modified)
                            .then(|| cached.file_sha256.clone())
                    });
            match cached_hash {
                Some(hash) => hash,
                None => match sha256_file(&path) {
                    Ok((hash, _)) => hash,
                    Err(_) => continue,
                },
            }
        };
        by_hash
            .entry(hash.to_ascii_uppercase())
            .or_default()
            .push(item.relative_path.clone());
    }
    // The targeted scan may omit canonical peers in other roots. The
    // persistent fingerprint cache remains valid evidence when the current
    // size and modification timestamp match the file on disk.
    for (raw_path, cached) in &cache.entries {
        let Ok(path) = PathBuf::from(raw_path).canonicalize() else {
            continue;
        };
        if indexed_paths.contains(&path)
            || !path_is_within_root(root, &path)
            || !selected_sizes.contains(&cached.size)
        {
            continue;
        }
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        let Ok(modified) = metadata.modified().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| std::io::Error::other(error.to_string()))
        }) else {
            continue;
        };
        if metadata.len() != cached.size || modified.as_nanos() != cached.modified_ns {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map(relative_key)
            .unwrap_or_else(|_| relative_key(&path));
        by_hash
            .entry(cached.file_sha256.to_ascii_uppercase())
            .or_default()
            .push(relative);
    }
    by_hash
}

// Pure indexed comparison makes the partial-merge heuristic regression-testable.
// Only stronger-than-STBL evidence can hold a larger package for review.
fn detect_merged_resource_supersets(
    indexed: &[(String, BTreeSet<String>, bool)],
) -> HashMap<String, Vec<String>> {
    let mut anchored = HashMap::<String, Vec<usize>>::new();
    for (index, (_, resources, slider)) in indexed.iter().enumerate() {
        if *slider && resources.len() >= 2 {
            if let Some(first) = resources.iter().next() {
                anchored.entry(first.clone()).or_default().push(index);
            }
        }
    }
    let mut related = HashMap::<String, Vec<String>>::new();
    for (merged_name, merged_resources, _) in indexed {
        let mut candidates = HashSet::<usize>::new();
        for key in merged_resources {
            if let Some(indices) = anchored.get(key) {
                candidates.extend(indices);
            }
        }
        for index in candidates {
            let (loose_name, loose_resources, _) = &indexed[index];
            if loose_name != merged_name
                && loose_resources.len() < merged_resources.len()
                && loose_resources.is_subset(merged_resources)
            {
                related
                    .entry(merged_name.to_lowercase())
                    .or_default()
                    .push(loose_name.clone());
            }
        }
    }
    related
}

// A merged package can contain byte-identical morph resources from several
// standalone sliders without having the same whole-file SHA-256. This is a
// conservative *suspected* relation, not a deletion verdict: require full
// inclusion of at least two non-localization resources, with a real slider
// morph and a strictly larger package. Data comes from the valid DBPF cache;
// missing or stale cache entries never trigger a move/block.
fn index_suspected_merged_sliders(
    root: &Path,
    scan_items: &[ScanPackageItem],
) -> HashMap<String, Vec<String>> {
    type Indexed = (String, BTreeSet<String>, bool);
    let cache = load_cache(root);
    let mut indexed = Vec::<Indexed>::new();
    for item in scan_items {
        let Ok(path) = PathBuf::from(&item.path).canonicalize() else {
            continue;
        };
        let Some(cached) = cache.entries.get(path.to_string_lossy().as_ref()) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        let Ok(modified_ns) = metadata
            .modified()
            .and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| std::io::Error::other(error.to_string()))
            })
            .map(|duration| duration.as_nanos())
        else {
            continue;
        };
        if cached.size != metadata.len()
            || cached.modified_ns != modified_ns
            || cached.parse_error.is_some()
        {
            continue;
        }
        let substantive = cached
            .resources
            .iter()
            .filter(|resource| !matches!(resource.type_id, 0x2205_57DA | 0x0166_038C))
            .map(|resource| {
                format!(
                    "{:08X}-{:08X}-{:016X}-{}",
                    resource.type_id, resource.group, resource.instance, resource.payload_sha256
                )
            })
            .collect::<BTreeSet<_>>();
        let direct_slider = item.sub_category.as_deref() == Some("Sliders")
            && cached.resources.iter().any(|r| {
                matches!(
                    r.type_id,
                    0x0358_B08A | 0xB52F_5055 | 0x0355_E0A6 | 0x067C_AA11
                )
            });
        indexed.push((item.relative_path.clone(), substantive, direct_slider));
    }
    detect_merged_resource_supersets(&indexed)
}

fn mark_intra_plan_destination_collisions(
    root: &Path,
    language: AppLanguage,
    items: &mut [PlanItem],
    directories: &mut BTreeSet<String>,
    reserved_duplicate_destinations: &mut HashSet<String>,
    stats: &mut PlanStats,
) -> Result<(), String> {
    let mut destinations = HashMap::<String, Vec<usize>>::new();

    for (index, item) in items.iter().enumerate() {
        if !item.plan_status.starts_with("ready") {
            continue;
        }
        let Some(destination) = item.destination_relative_path.as_ref() else {
            continue;
        };
        let key = normalized_path_key(root, Path::new(destination));
        destinations.entry(key).or_default().push(index);
    }

    for indices in destinations.values().filter(|indices| indices.len() > 1) {
        let first_index = indices[0];
        let first_hash = items[first_index].sha256.clone().unwrap_or_default();
        let first_size = items[first_index].size;
        let first_source_relative = items[first_index].source_relative_path.clone();
        let same_content = indices.iter().all(|index| {
            let item = &items[*index];
            item.size == first_size
                && item
                    .sha256
                    .as_deref()
                    .map(|hash| hash.eq_ignore_ascii_case(&first_hash))
                    .unwrap_or(false)
                && !matches!(item.classification_status.as_str(), "preview" | "auxiliary")
        });

        if same_content {
            // Keep one representative ready for the destination. Every other
            // byte-identical source receives a reversible flat-quarantine move
            // during planning, so the executor never discovers this collision
            // only after it has started moving files.
            for index in indices.iter().skip(1) {
                let item = &mut items[*index];
                let source = PathBuf::from(&item.source_path);
                let hash = item.sha256.as_deref().unwrap_or_default();
                let (quarantine_relative, quarantine_parts) = duplicate_quarantine_destination(
                    root,
                    language,
                    &source,
                    hash,
                    reserved_duplicate_destinations,
                )?;
                let quarantine = root.join(&quarantine_relative);
                if same_path_case_insensitive(&source, &quarantine) {
                    item.plan_status = "duplicate_skipped".to_string();
                    stats.duplicate_skipped += 1;
                    item.warnings.push(
                        "The byte-identical copy is already in flat #Duplicados and remains there."
                            .to_string(),
                    );
                    continue;
                }
                add_missing_directories(root, &quarantine_parts, directories);
                reserved_duplicate_destinations
                    .insert(relative_key(&quarantine_relative).to_ascii_lowercase());
                item.destination_path = Some(quarantine.to_string_lossy().to_string());
                item.destination_relative_path = Some(relative_key(&quarantine_relative));
                item.plan_status = "ready_duplicate_quarantine".to_string();
                item.classification_reason = Some(format!(
                    "{} | Exact duplicate; canonical keeper is '{}'.",
                    item.classification_reason.clone().unwrap_or_default(),
                    first_source_relative
                ));
                item.warnings.push(format!(
                    "Byte-identical extra will move directly to flat #Duplicados as '{}'; keeper remains '{}'.",
                    relative_key(&quarantine_relative),
                    first_source_relative
                ));
                stats.duplicate_quarantine_ready += 1;
            }
        } else {
            for index in indices {
                let item = &mut items[*index];
                item.plan_status = "collision_different_content".to_string();
                stats.collision_different_content += 1;
                item.warnings.push(
                    "Multiple selected packages resolve to the same destination but contain different data. These files will be left in place and skipped during organization."
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}

/// A frozen plan must not contain a move whose destination is another ready
/// item's source. The executor intentionally uses non-overwriting renames and
/// cannot safely perform swaps or chains in arbitrary order. Detect the
/// overlap before any filesystem write so the old
/// `Destination appeared after preflight` protection remains meaningful for
/// external changes, not for a flaw already present in the plan.
fn mark_intra_plan_source_destination_collisions(root: &Path, items: &mut [PlanItem]) {
    let mut sources = HashMap::<String, Vec<usize>>::new();
    for (index, item) in items.iter().enumerate() {
        if !item.plan_status.starts_with("ready") {
            continue;
        }
        sources
            .entry(normalized_path_key(root, Path::new(&item.source_path)))
            .or_default()
            .push(index);
    }

    let mut blocked = BTreeSet::<usize>::new();
    for (index, item) in items.iter().enumerate() {
        if !item.plan_status.starts_with("ready") {
            continue;
        }
        let Some(destination) = item.destination_path.as_ref() else {
            continue;
        };
        let key = normalized_path_key(root, Path::new(destination));
        if let Some(peers) = sources.get(&key) {
            if peers.iter().any(|peer| *peer != index) {
                blocked.insert(index);
                blocked.extend(peers.iter().copied());
            }
        }
    }

    for index in blocked {
        let item = &mut items[index];
        item.plan_status = "blocked_intra_plan_path_collision".to_string();
        item.warnings.push(
            "This plan contains a source/destination overlap with another selected move. No operation in the overlap will run; review the plan order or destination before retrying.".to_string(),
        );
    }
}

/// A source may be present in more than one planner input.  This happens in
/// particular when a package selected by the normal organizer is also found
/// by the legacy nested-duplicate migration.  A filesystem rename consumes
/// its source entry, so allowing both ready items to survive would make the
/// second operation fail with "Source disappeared before move" after the
/// first one had already changed the tree.
///
/// Resolve this while the plan is still in memory.  A confirmed duplicate
/// quarantine migration is the authoritative operation for a source already
/// below #Duplicados; it preserves the flat-quarantine invariant and is safer
/// than letting a normal category move win merely because it was appended
/// first.  If there is no such explicit winner, differing destinations are
/// genuinely ambiguous and all competing operations remain blocked for
/// review.  Identical duplicate operations are reduced to one deterministic
/// representative without deleting or silently changing the source.
fn resolve_intra_plan_source_conflicts(root: &Path, items: &mut [PlanItem]) {
    let mut by_source = HashMap::<String, Vec<usize>>::new();
    for (index, item) in items.iter().enumerate() {
        if item.plan_status.starts_with("ready") {
            by_source
                .entry(normalized_path_key(root, Path::new(&item.source_path)))
                .or_default()
                .push(index);
        }
    }

    for indices in by_source.values().filter(|indices| indices.len() > 1) {
        let migration_indices = indices
            .iter()
            .copied()
            .filter(|index| items[*index].plan_status == "ready_duplicate_quarantine_migration")
            .collect::<Vec<_>>();

        let winner = if migration_indices.len() == 1 {
            Some(migration_indices[0])
        } else if migration_indices.len() > 1 {
            None
        } else {
            let first_destination = items[indices[0]]
                .destination_path
                .as_deref()
                .map(|path| normalized_path_key(root, Path::new(path)));
            let same_destination = first_destination.is_some()
                && indices.iter().all(|index| {
                    items[*index]
                        .destination_path
                        .as_deref()
                        .map(|path| normalized_path_key(root, Path::new(path)))
                        == first_destination
                });
            let same_identity = indices.iter().all(|index| {
                items[*index].size == items[indices[0]].size
                    && items[*index]
                        .sha256
                        .as_deref()
                        .zip(items[indices[0]].sha256.as_deref())
                        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
            });
            if same_destination && same_identity {
                indices
                    .iter()
                    .copied()
                    .min_by_key(|index| items[*index].id.to_ascii_lowercase())
            } else {
                None
            }
        };

        if let Some(winner) = winner {
            let winner_id = items[winner].id.clone();
            for index in indices.iter().copied().filter(|index| *index != winner) {
                let item = &mut items[index];
                item.plan_status = "duplicate_source_skipped".to_string();
                item.warnings.push(format!(
                    "This source was proposed by more than one planner path. Only '{}' remains executable for the normalized source; this competing operation was skipped before execution.",
                    winner_id
                ));
            }
        } else {
            let competing = indices
                .iter()
                .map(|index| {
                    format!(
                        "{} -> {}",
                        items[*index].source_relative_path,
                        items[*index]
                            .destination_relative_path
                            .as_deref()
                            .unwrap_or("<none>")
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            for index in indices.iter().copied() {
                let item = &mut items[index];
                item.plan_status = "blocked_duplicate_source_conflict".to_string();
                item.warnings.push(format!(
                    "The same normalized source was proposed by multiple incompatible operations; no move is executable until the conflict is reviewed: {competing}"
                ));
            }
        }
    }
}

fn is_manual_review_status(status: &str) -> bool {
    matches!(status, "needs_review" | "mixed")
}

fn plan_can_execute(stats: &PlanStats, read_only: bool) -> bool {
    // Blocked items are excluded from movement. They must not prevent the
    // independently validated ready items from being organized and restored.
    !read_only && (stats.ready > 0 || stats.empty_folders_to_clean > 0)
}

fn plan_item_moves_source(item: &PlanItem) -> bool {
    item.plan_status.starts_with("ready")
}

fn path_matches_item_source(path: &Path, item: &PlanItem) -> bool {
    same_path_case_insensitive(path, Path::new(&item.source_path))
}

fn is_stable_review_or_quarantine_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(
            value.to_ascii_lowercase().as_str(),
            "#sem categoria"
                | "#not categorized"
                | "#sin categoría"
                | "#duplicados"
                | "#duplicates"
                | "previews de ccs"
                | "cc previews"
        )
    })
}

fn projected_remaining_legacy_files(
    selected: &HashSet<PathBuf>,
    selected_roots: &[PathBuf],
    items: &[PlanItem],
) -> usize {
    let mut files = BTreeSet::<PathBuf>::new();
    for path in selected {
        if path.is_file() {
            files.insert(path.clone());
        }
    }
    for selected_root in selected_roots {
        if !selected_root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(selected_root)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                files.insert(entry.path().to_path_buf());
            }
        }
    }

    files
        .into_iter()
        .filter(|path| {
            match items
                .iter()
                .find(|item| path_matches_item_source(path, item))
            {
                Some(item) => {
                    let destination_is_same =
                        item.destination_path.as_deref().is_some_and(|destination| {
                            same_path_case_insensitive(path, Path::new(destination))
                        }) || item.destination_relative_path.as_deref().is_some_and(
                            |destination| {
                                same_relative_path(&item.source_relative_path, destination)
                            },
                        );
                    !plan_item_moves_source(item)
                        && !item.plan_status.starts_with("already_")
                        && !destination_is_same
                }
                None => !is_stable_review_or_quarantine_path(path),
            }
        })
        .count()
}

fn projected_empty_folders_to_clean(
    root: &Path,
    selected_roots: &[PathBuf],
    items: &[PlanItem],
    protected_relative_directories: &[String],
) -> usize {
    let mut candidates = BTreeSet::<PathBuf>::new();
    // Residual cleanup is global to the selected Packages root.  It is a
    // directory-only inventory: no DBPF is opened here, and it allows a
    // cleanup-only plan after a prior organization has already moved files.
    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_dir() && !entry.file_type().is_symlink() {
            candidates.insert(entry.path().to_path_buf());
        }
    }
    for selected_root in selected_roots {
        if selected_root.is_dir() && selected_root.starts_with(root) {
            candidates.insert(selected_root.clone());
        }
    }
    for item in items {
        let source = PathBuf::from(&item.source_path);
        let Some(parent) = source.parent() else {
            continue;
        };
        if !parent.is_dir() || !parent.starts_with(root) || parent == root {
            continue;
        }
        candidates.insert(parent.to_path_buf());
        let mut ancestor = parent.parent();
        while let Some(folder) = ancestor {
            if folder == root || !folder.starts_with(root) {
                break;
            }
            candidates.insert(folder.to_path_buf());
            ancestor = folder.parent();
        }
    }

    candidates
        .into_iter()
        .filter(|folder| {
            if protected_relative_directories.iter().any(|relative| {
                normalized_path_key(root, folder) == normalized_path_key(root, Path::new(relative))
            }) || is_stable_review_or_quarantine_path(folder)
            {
                return false;
            }
            if !folder.is_dir() {
                return false;
            }
            let source_files_occupy = WalkDir::new(folder)
                .follow_links(false)
                .into_iter()
                .flatten()
                .filter(|entry| entry.file_type().is_file())
                .any(|entry| {
                    let source = entry.path();
                    match items
                        .iter()
                        .find(|item| path_matches_item_source(source, item))
                    {
                        Some(item) if plan_item_moves_source(item) => item
                            .destination_path
                            .as_deref()
                            .map(Path::new)
                            .is_some_and(|destination| destination.starts_with(folder)),
                        Some(_) | None => true,
                    }
                });
            let destinations_occupy = items.iter().any(|item| {
                plan_item_moves_source(item)
                    && item
                        .destination_path
                        .as_deref()
                        .map(Path::new)
                        .is_some_and(|destination| destination.starts_with(folder))
            });
            !source_files_occupy && !destinations_occupy
        })
        .count()
}

fn projected_legacy_folder_counts(
    root: &Path,
    selected_roots: &[PathBuf],
    items: &[PlanItem],
    protected_relative_directories: &[String],
) -> (usize, usize) {
    let mut candidates = BTreeSet::<PathBuf>::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_dir() && !entry.file_type().is_symlink() {
            candidates.insert(entry.path().to_path_buf());
        }
    }
    for selected_root in selected_roots {
        if selected_root.is_dir() && selected_root.starts_with(root) {
            candidates.insert(selected_root.clone());
        }
    }
    for item in items {
        let source = PathBuf::from(&item.source_path);
        let Some(parent) = source.parent() else {
            continue;
        };
        if !parent.is_dir() || !parent.starts_with(root) || parent == root {
            continue;
        }
        candidates.insert(parent.to_path_buf());
        let mut ancestor = parent.parent();
        while let Some(folder) = ancestor {
            if folder == root || !folder.starts_with(root) {
                break;
            }
            candidates.insert(folder.to_path_buf());
            ancestor = folder.parent();
        }
    }

    let mut removable = 0usize;
    let mut retained = 0usize;
    for folder in candidates {
        if protected_relative_directories.iter().any(|relative| {
            normalized_path_key(root, &folder) == normalized_path_key(root, Path::new(relative))
        }) || is_stable_review_or_quarantine_path(&folder)
            || !folder.is_dir()
        {
            continue;
        }
        let occupied = WalkDir::new(&folder)
            .follow_links(false)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .any(|entry| {
                let path = entry.path();
                match items
                    .iter()
                    .find(|item| path_matches_item_source(path, item))
                {
                    Some(item) if plan_item_moves_source(item) => false,
                    Some(_) | None => true,
                }
            });
        if occupied {
            retained += 1;
        } else {
            removable += 1;
        }
    }
    (removable, retained)
}

fn manifest_preview(root: &Path, language: AppLanguage, ready_items: &[PlanItem]) -> String {
    let mut out = String::new();
    out.push_str("S3CC MANAGER RESTORE MANIFEST\n");
    out.push_str("version=1\n");
    out.push_str(&format!(
        "created_at={}\n",
        Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!("mode=PREVIEW\n"));
    out.push_str(&format!(
        "organization_language={}\n",
        language_code(language)
    ));
    out.push_str(&format!("root={}\n", root.display()));
    out.push_str(&format!("files={}\n\n", ready_items.len()));

    for item in ready_items {
        out.push_str("[file]\n");
        if let Some(hash) = &item.sha256 {
            out.push_str(&format!("sha256={hash}\n"));
        }
        out.push_str(&format!("size={}\n", item.size));
        out.push_str(&format!("original={}\n", item.source_relative_path));
        if let Some(destination) = &item.destination_relative_path {
            out.push_str(&format!("organized={destination}\n"));
        }
        out.push_str("[/file]\n\n");
    }

    out
}

fn make_blocked(item: &ScanPackageItem, reason: String) -> PlanItem {
    PlanItem {
        id: item.id.clone(),
        name: item.name.clone(),
        source_path: item.path.clone(),
        source_relative_path: item.relative_path.clone(),
        destination_path: None,
        destination_relative_path: None,
        classification_status: item.status.clone(),
        classification_reason: item.classification_reason.clone(),
        plan_status: "blocked".to_string(),
        sha256: None,
        size: item.file_size,
        warnings: vec![reason],
    }
}

/// Planner selections are commonly a small set of legacy roots. Scanning the
/// entire Packages tree in that case turns a targeted operation into a second
/// general audit. Scan only the selected top-level roots, while the duplicate
/// index below still consults the persistent fingerprint cache for peers that
/// live elsewhere in the tree.
fn scan_selected_roots(
    root: &Path,
    language: AppLanguage,
    selected: &HashSet<PathBuf>,
) -> Result<ScanResult, String> {
    // A selected directory may contain only previews or auxiliary files.
    // There are no DBPF packages to scan in that case, so returning an empty
    // package scan prevents a targeted operation from reopening the complete
    // Packages tree.
    if selected.is_empty() {
        return Ok(ScanResult {
            root: root.to_string_lossy().to_string(),
            items: Vec::new(),
            stats: ScanStats::default(),
            cache_hit: false,
            reused_packages: 0,
        });
    }
    if selected.len() >= 512 {
        return scan_packages_core(
            root.to_string_lossy().to_string(),
            language,
            Some("planning"),
        );
    }
    let mut paths = selected.iter().cloned().collect::<Vec<_>>();
    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    scan_selected_package_paths(root, &paths, language)
}

fn resource_cfg_destinations_for_plan(plan: &OrganizationPlan) -> Vec<PathBuf> {
    let root = PathBuf::from(&plan.root);
    let Some(mods) = root.ancestors().find(|ancestor| is_mods_root(ancestor)) else {
        return Vec::new();
    };

    plan.items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready"))
        .filter_map(|item| item.destination_path.as_ref().map(PathBuf::from))
        .filter(|path| {
            let Some(name) = path.file_name().map(|value| value.to_string_lossy()) else {
                return false;
            };
            if !name.to_ascii_lowercase().ends_with(".package") {
                return false;
            }
            let Ok(relative) = path.strip_prefix(mods) else {
                return false;
            };
            let mut components = relative.components();
            matches!(
                components.next(),
                Some(Component::Normal(branch))
                    if branch.to_string_lossy().eq_ignore_ascii_case("Packages")
                        || branch.to_string_lossy().eq_ignore_ascii_case("Overrides")
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Refresh only the Resource.cfg preview for an already built plan. Package
/// parsing, hashing, duplicate detection and destination resolution have
/// already been completed and are intentionally not repeated here.
#[tauri::command]
pub fn preview_resource_cfg_for_plan(
    plan: OrganizationPlan,
) -> Result<Option<ResourceCfgUpdate>, String> {
    let root = PathBuf::from(&plan.root)
        .canonicalize()
        .map_err(|error| format!("Could not resolve organization-plan root: {error}"))?;
    let destinations = resource_cfg_destinations_for_plan(&plan);
    if destinations.is_empty() {
        return Ok(None);
    }
    preview_resource_cfg_update(&root, &destinations)
}

pub(crate) fn build_organization_plan_with_cfg_core(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
    update_resource_cfg: bool,
) -> Result<OrganizationPlan, String> {
    if crate::operation::is_cancelled("planning") {
        return Err(crate::operation::CANCELLED_ERROR.to_string());
    }
    let planning_started = Instant::now();
    let root_input = PathBuf::from(folder.trim());
    let root = root_input
        .canonicalize()
        .map_err(|error| format!("Could not resolve root folder: {error}"))?;

    if !root.is_dir() {
        return Err(format!("Root is not a directory: {}", root.display()));
    }
    // Read the official model for every plan. Folder names that merely look
    // like categories are never enough to mark a package organized.
    let manual_model_root = manual_model_packages_root()?;
    let manual_model = load_or_build_manual_model_index(&manual_model_root)?;
    let protected_empty_directories = canonical_model_directories_for_root(&root, &manual_model);
    // The selected root can be Mods, any Packages/Overrides subfolder, or a
    // self-contained CC staging library elsewhere. All moves remain inside
    // this canonical root. Never invent an out-of-root destination.
    //
    // A Mods root may also contain Overrides, DCCache and other directories.
    // Never organize those as Packages or create category folders beside Packages.
    let packages_root = if is_mods_root(&root) {
        let path = root.join("Packages").canonicalize().map_err(|_| {
            "The selected Mods folder must contain a Packages directory.".to_string()
        })?;
        if !path.is_dir() || !path_is_within_root(&root, &path) {
            return Err(
                "Mods/Packages is not a safe directory inside the selected Mods root.".to_string(),
            );
        }
        Some(path)
    } else {
        None
    };

    let mut selected = HashSet::<PathBuf>::new();
    let mut selected_roots = Vec::<PathBuf>::new();
    for raw in &selected_paths {
        let canonical = PathBuf::from(raw)
            .canonicalize()
            .map_err(|error| format!("Could not resolve selected file {raw}: {error}"))?;
        if !path_is_within_root(&root, &canonical) {
            return Err(format!("Selected file escaped the selected root: {raw}"));
        }
        if canonical.is_dir() {
            selected_roots.push(canonical.clone());
            for entry in WalkDir::new(&canonical)
                .follow_links(false)
                .into_iter()
                .flatten()
            {
                if entry.file_type().is_file() && is_package_file(entry.path()) {
                    selected.insert(entry.path().canonicalize().map_err(|error| {
                        format!(
                            "Could not resolve selected package {}: {error}",
                            entry.path().display()
                        )
                    })?);
                }
            }
        } else {
            selected.insert(canonical);
        }
    }

    let cleanup_roots = selected_roots
        .iter()
        .filter_map(|path| path.strip_prefix(&root).ok())
        .map(|relative| {
            let key = relative_key(relative);
            if key.is_empty() {
                ".".to_string()
            } else {
                key
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let workspace = load_workspace_for_root(&root);
    let profile = active_profile(&workspace);
    let resource_cfg = resource_cfg_context(&root);
    let scan = if selected.is_empty() {
        // The selected roots are still handled below for previews and
        // auxiliary companions; no package parser is needed for them.
        ScanResult {
            root: root.to_string_lossy().to_string(),
            items: Vec::new(),
            stats: ScanStats::default(),
            cache_hit: false,
            reused_packages: 0,
        }
    } else {
        match cached_scan_for(&root, language) {
            Some(scan) => scan,
            None => scan_selected_roots(&root, language, &selected)?,
        }
    };
    if crate::operation::is_cancelled("planning") {
        return Err(crate::operation::CANCELLED_ERROR.to_string());
    }

    let mut scan_by_canonical = HashMap::<PathBuf, &ScanPackageItem>::new();
    for (item_index, item) in scan.items.iter().enumerate() {
        if item_index % 64 == 0 && crate::operation::is_cancelled("planning") {
            return Err(crate::operation::CANCELLED_ERROR.to_string());
        }
        if let Ok(path) = PathBuf::from(&item.path).canonicalize() {
            scan_by_canonical.insert(path, item);
        }
    }

    let unknown_selection = selected
        .iter()
        .filter(|path| !scan_by_canonical.contains_key(*path))
        .take(3)
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    if !unknown_selection.is_empty() {
        return Err(format!(
            "Selection contains files outside the current scan: {}",
            unknown_selection.join(", ")
        ));
    }

    // The unified analysis has already populated this resource cache.  The
    // plan preview needs a current planning identity, not a mutation-grade
    // integrity proof, so validate with the bounded fingerprint instead of
    // reading every selected package a second time.  Execution still hashes
    // every ready source before its first write.
    let mut planning_cache = load_cache(&root);
    let mut selected_hashes = HashMap::<PathBuf, String>::new();
    let mut selected_hash_set = HashSet::<String>::new();
    for path in &selected {
        let (cached, _) =
            get_or_build_analysis_with_metrics(path, &mut planning_cache).map_err(|error| {
                format!(
                    "Could not validate selected package {} for planning: {error}",
                    path.display()
                )
            })?;
        selected_hashes.insert(path.clone(), cached.file_sha256.clone());
        selected_hash_set.insert(cached.file_sha256);
    }
    // Persist a one-off cache miss, but ignore a write failure here: it must
    // never invalidate a safe, read-only plan preview.
    let _ = save_cache(&root, &planning_cache);

    let duplicate_peers = index_exact_duplicates(&root, &scan.items, &selected_hashes);
    let possible_merged_peers = index_suspected_merged_sliders(&root, &scan.items);

    let mut partial_group_hashes = HashSet::<String>::new();

    for group in workspace.groups.iter().filter(|group| group.keep_together) {
        let intersects = group
            .member_sha256
            .iter()
            .any(|hash| selected_hash_set.contains(hash));
        let complete = group
            .member_sha256
            .iter()
            .all(|hash| selected_hash_set.contains(hash));
        if intersects && !complete {
            for hash in &group.member_sha256 {
                if selected_hash_set.contains(hash) {
                    partial_group_hashes.insert(hash.clone());
                }
            }
        }
    }

    let mut stats = PlanStats {
        selected: selected.len(),
        ..PlanStats::default()
    };
    let mut items = Vec::with_capacity(selected.len());
    let mut directories = BTreeSet::new();
    let mut reserved_duplicate_destinations = HashSet::<String>::new();

    let mut ordered = selected
        .iter()
        .filter_map(|path| scan_by_canonical.get(path).copied())
        .collect::<Vec<_>>();
    ordered.sort_by_key(|item| item.relative_path.to_ascii_lowercase());

    for item in ordered {
        if is_protected(&item.relative_path, &profile) {
            stats.blocked += 1;
            items.push(make_blocked(
                item,
                "The active profile protects this folder from automatic organization.".to_string(),
            ));
            continue;
        }

        let source = PathBuf::from(&item.path);
        let source_canonical = source
            .canonicalize()
            .map_err(|error| format!("Could not resolve source {}: {error}", source.display()))?;
        let source_hash = selected_hashes
            .get(&source_canonical)
            .cloned()
            .ok_or_else(|| format!("Missing selected package hash: {}", source.display()))?;

        // A user's explicit selection authorizes migration of categorized
        // packages from legacy source folders. Never select from system caches
        // or recovery trees, and keep Overrides in Overrides.
        if is_mods_system_source(&root, &source_canonical) {
            stats.blocked += 1;
            items.push(make_blocked(
                item,
                "System/cache/recovery folder is protected: this package will not be moved."
                    .to_string(),
            ));
            continue;
        }

        // An exported Sim with embedded CC is one atomic user asset. Keep
        // every internal CASP/GEOM/texture resource together and place the
        // package directly at the Packages root; it is never routed through
        // the ordinary category tree or the generic merged-package review.
        if item.status == "sim_export_merged" {
            let file_name = source
                .file_name()
                .ok_or_else(|| format!("Source has no filename: {}", source.display()))?;
            if source_uses_overrides(&root, &source_canonical) {
                stats.kept_uncategorized += 1;
                items.push(PlanItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    source_path: source_canonical.to_string_lossy().to_string(),
                    source_relative_path: item.relative_path.clone(),
                    destination_path: None,
                    destination_relative_path: None,
                    classification_status: item.status.clone(),
                    classification_reason: item.classification_reason.clone(),
                    plan_status: "keep_sim_export_merged_override".to_string(),
                    sha256: Some(source_hash),
                    size: item.file_size,
                    warnings: vec!["Merged Sim export detected inside Overrides; it was kept there because Overrides never moves into Packages.".to_string()],
                });
                continue;
            }

            let package_root = packages_root.as_deref().unwrap_or(&root);
            let destination = package_root.join(file_name);
            if !path_is_within_root(&root, &destination) {
                stats.blocked += 1;
                items.push(make_blocked(
                    item,
                    "Merged Sim export destination is outside the selected loading root."
                        .to_string(),
                ));
                continue;
            }
            let already = same_path_case_insensitive(&source_canonical, &destination);
            let collision = !already && destination.exists();
            let collision_same = if collision {
                sha256_file(&destination)
                    .map(|(hash, _)| hash.eq_ignore_ascii_case(&source_hash))
                    .unwrap_or(false)
            } else {
                false
            };
            let destination_relative = destination
                .strip_prefix(&root)
                .map(relative_key)
                .map_err(|_| "Merged Sim export destination escaped selected root.".to_string())?;
            let plan_status = if already {
                stats.already_organized += 1;
                "already_sim_export_merged_root"
            } else if collision_same {
                stats.collision_same_content += 1;
                "sim_export_merged_collision_same_content"
            } else if collision {
                stats.collision_different_content += 1;
                "sim_export_merged_collision_different_content"
            } else {
                stats.ready += 1;
                "ready_sim_export_merged_root"
            };
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative),
                classification_status: item.status.clone(),
                classification_reason: item.classification_reason.clone(),
                plan_status: plan_status.to_string(),
                sha256: Some(source_hash.clone()),
                size: item.file_size,
                warnings: vec![if already {
                    "Merged Sim export is already at the Packages root; the package remains intact.".to_string()
                } else if collision_same {
                    "An identical root-level package already exists; no overwrite or split was performed.".to_string()
                } else if collision {
                    "A different root-level package has the same filename; no overwrite or split was performed.".to_string()
                } else {
                    "Move the complete exported Sim package as one file to the Packages root; embedded CC resources are never separated.".to_string()
                }],
            });
            continue;
        }
        let is_override_source = source_uses_overrides(&root, &source_canonical);
        let hash_model_destination = if is_override_source {
            None
        } else {
            Some(resolve_model_destination(
                &manual_model,
                &root,
                &source_canonical,
                &source_hash,
                &item.destination_parts,
            ))
        };
        let outside_active_branch = packages_root
            .as_ref()
            .is_some_and(|packages| !path_is_within_root(packages, &source_canonical))
            && !source_uses_overrides(&root, &source_canonical);

        let duplicates = duplicate_peers.get(&source_hash.to_ascii_uppercase());
        let duplicate_group = duplicates.filter(|peers| peers.len() > 1);
        let duplicate_keeper = duplicate_group.and_then(|peers| {
            selected_content_keeper_relative_path(
                &root,
                &source_canonical,
                &source_hash,
                &selected_hashes,
                peers,
                &scan_by_canonical,
                &manual_model,
                resource_cfg.as_ref(),
            )
            .or_else(|| {
                current_model_destination_keeper_relative_path(
                    &root,
                    &source_canonical,
                    peers,
                    hash_model_destination
                        .as_ref()
                        .and_then(|result| result.as_ref().ok()),
                    &item.destination_parts,
                )
            })
        });
        if let Some(peers) = duplicate_group {
            let Some(keeper) = duplicate_keeper.as_deref() else {
                stats.blocked += 1;
                let mut blocked = make_blocked(
                    item,
                    format!(
                        "Exact duplicate group has no confirmed canonical keeper; preserved in place for review. Matching paths: {}.",
                        peers.iter().take(6).cloned().collect::<Vec<_>>().join(" | ")
                    ),
                );
                blocked.plan_status = "blocked_duplicate_no_keeper".to_string();
                blocked.sha256 = Some(source_hash.clone());
                items.push(blocked);
                continue;
            };
            if same_relative_path(&item.relative_path, keeper) {
                // The confirmed keeper remains in its canonical location and
                // continues through normal classification/idempotence checks.
            } else {
                // A confirmed byte-identical extra copy is not silently stranded
                // in a legacy folder. Move it to a reversible quarantine bucket;
                // the keeper continues through normal canonical-model handling.
                // Suspected merged packages are handled separately below and are
                // intentionally never auto-quarantined.
                let (quarantine_relative, quarantine_parts) = duplicate_quarantine_destination(
                    &root,
                    language,
                    &source_canonical,
                    &source_hash,
                    &reserved_duplicate_destinations,
                )?;
                let quarantine = root.join(&quarantine_relative);
                let already_quarantined =
                    same_path_case_insensitive(&source_canonical, &quarantine);
                if already_quarantined {
                    stats.already_organized += 1;
                    items.push(PlanItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    source_path: source_canonical.to_string_lossy().to_string(),
                    source_relative_path: item.relative_path.clone(),
                    destination_path: Some(quarantine.to_string_lossy().to_string()),
                    destination_relative_path: Some(relative_key(&quarantine_relative)),
                    classification_status: item.status.clone(),
                    classification_reason: item.classification_reason.clone(),
                    plan_status: "already_duplicate_quarantine".to_string(),
                    sha256: Some(source_hash.clone()),
                    size: item.file_size,
                    warnings: vec!["Confirmed byte-identical duplicate is already in the reversible #Duplicados quarantine.".to_string()],
                });
                    continue;
                }
                add_missing_directories(&root, &quarantine_parts, &mut directories);
                reserved_duplicate_destinations
                    .insert(relative_key(&quarantine_relative).to_ascii_lowercase());
                stats.ready += 1;
                stats.duplicate_quarantine_ready += 1;
                items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(quarantine.to_string_lossy().to_string()),
                destination_relative_path: Some(relative_key(&quarantine_relative)),
                classification_status: item.status.clone(),
                classification_reason: item.classification_reason.clone(),
                plan_status: "ready_duplicate_quarantine".to_string(),
                sha256: Some(source_hash.clone()),
                size: item.file_size,
                warnings: vec![format!(
                    "Confirmed byte-identical duplicate moved reversibly to '{}'. The canonical keeper is '{}'; {} matching scan paths: {}. No file was overwritten or deleted.",
                    relative_key(&quarantine_relative),
                    selected_content_keeper_relative_path(
                        &root,
                        &source_canonical,
                        &source_hash,
                        &selected_hashes,
                        peers,
                        &scan_by_canonical,
                        &manual_model,
                        resource_cfg.as_ref(),
                    ).or_else(|| {
                        current_model_destination_keeper_relative_path(
                            &root,
                            &source_canonical,
                            peers,
                            hash_model_destination.as_ref().and_then(|result| result.as_ref().ok()),
                            &item.destination_parts,
                        )
                    }).unwrap_or_else(|| "unresolved selected path".to_string()),
                    peers.len(),
                    peers.iter().take(4).cloned().collect::<Vec<_>>().join(" | ")
                )],
            });
                continue;
            }
        }

        if let Some(loose_files) = possible_merged_peers.get(&item.relative_path.to_lowercase()) {
            stats.duplicate_skipped += 1;
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: None,
                destination_relative_path: None,
                classification_status: item.status.clone(),
                classification_reason: item.classification_reason.clone(),
                plan_status: "duplicate_skipped".to_string(),
                sha256: Some(source_hash),
                size: item.file_size,
                warnings: vec![format!(
                    "Possible merged package: its non-localization resources contain the exact TGIs and payloads of standalone slider package(s): {}. This is a review candidate, not proof that the packages are interchangeable. Kept in place; inspect Duplicates before any change.",
                    loose_files.iter().take(4).cloned().collect::<Vec<_>>().join(" | ")
                )],
            });
            continue;
        }

        if partial_group_hashes.contains(&source_hash) {
            stats.blocked += 1;
            items.push(make_blocked(
                item,
                "This package belongs to a Keep Together group. Select every member of the group before organizing it.".to_string(),
            ));
            continue;
        }

        // A package can be structurally unreadable or lack enough metadata
        // for a new classification while still being an exact, known model
        // asset already placed at its complete canonical destination. Do not
        // send that canonical copy back to #Sem Categoria on a later pass.
        // This is intentionally stricter than a category-name comparison:
        // it requires both the model's full SHA-256 identity and the exact
        // relative destination path. A legacy `Creator\\Sliders` copy can
        // therefore never qualify as already organized here.
        if item.status != "classified" {
            if let Some(Ok(model_destination)) = hash_model_destination.as_ref() {
                let mut expected_relative = PathBuf::new();
                for part in &model_destination.parts {
                    expected_relative.push(part);
                }
                expected_relative.push(
                    source
                        .file_name()
                        .ok_or_else(|| format!("Source has no filename: {}", source.display()))?,
                );
                let expected_destination = root.join(&expected_relative);
                if same_path_case_insensitive(&source_canonical, &expected_destination) {
                    stats.already_organized += 1;
                    items.push(PlanItem {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        source_path: source_canonical.to_string_lossy().to_string(),
                        source_relative_path: item.relative_path.clone(),
                        destination_path: Some(expected_destination.to_string_lossy().to_string()),
                        destination_relative_path: Some(relative_key(&expected_relative)),
                        classification_status: item.status.clone(),
                        classification_reason: item.classification_reason.clone(),
                        plan_status: "already_organized".to_string(),
                        sha256: Some(source_hash),
                        size: item.file_size,
                        warnings: vec![
                            "Exact SHA-256 model identity is already at the complete canonical path; no fallback move is proposed.".to_string(),
                        ],
                    });
                    continue;
                }
            }
        }

        // A manual-review result is a protected decision state, not an
        // invitation to move the package into a guessed fallback. Unknown
        // packages may still use #Sem Categoria when the user explicitly
        // selects them, but mixed/needs_review records stay in place until
        // their evidence is resolved in the interface.
        if is_manual_review_status(&item.status) {
            stats.kept_uncategorized += 1;
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: None,
                destination_relative_path: None,
                classification_status: item.status.clone(),
                classification_reason: item.classification_reason.clone(),
                plan_status: "keep_manual_review".to_string(),
                sha256: Some(source_hash),
                size: item.file_size,
                warnings: vec![
                    "The package has multiple or insufficient technical destinations. It remains in place for manual review; no fallback or inferred category move is proposed.".to_string(),
                ],
            });
            continue;
        }

        // Unknown is a classification result, not an instruction to leave the
        // original folder forever. Move safely selected unknowns to the flat
        // review area; their complete source path remains in the manifest and
        // history. Do not redirect Overrides or invalid/mixed packages without
        // a manual decision.
        if item.status != "classified" && !is_override_source {
            let (fallback_relative, fallback_parts) =
                fallback_relative_path(&root, language, &item.relative_path, &source_hash)?;
            // The fallback may choose a unique SHA-named file if its normal
            // destination exists. Never create that extra copy when the
            // existing destination is already byte-identical.
            if let Some(parent) = fallback_relative.parent() {
                let original_target = root.join(parent).join(&item.name);
                if !same_path_case_insensitive(&original_target, &source_canonical)
                    && original_target.is_file()
                {
                    let (existing_hash, _) = sha256_file(&original_target).map_err(|error| {
                        format!("Could not verify existing Not Categorized package: {error}")
                    })?;
                    if existing_hash.eq_ignore_ascii_case(&source_hash) {
                        stats.duplicate_skipped += 1;
                        items.push(PlanItem {
                            id: item.id.clone(),
                            name: item.name.clone(),
                            source_path: source_canonical.to_string_lossy().to_string(),
                            source_relative_path: item.relative_path.clone(),
                            destination_path: Some(original_target.to_string_lossy().to_string()),
                            destination_relative_path: original_target.strip_prefix(&root)
                                .map(relative_key).ok(),
                            classification_status: item.status.clone(),
                            classification_reason: item.classification_reason.clone(),
                            plan_status: "duplicate_skipped".to_string(),
                            sha256: Some(source_hash),
                            size: item.file_size,
                            warnings: vec!["An identical package already exists in Not Categorized. This extra source copy remains for Duplicates/Quarantine review.".to_string()],
                        });
                        continue;
                    }
                }
            }
            if let Err(error) = validate_destination_parts(&fallback_parts) {
                stats.blocked += 1;
                items.push(make_blocked(
                    item,
                    format!(
                        "{error} (fallback='{}', parts={:?})",
                        fallback_relative.display(),
                        fallback_parts
                    ),
                ));
                continue;
            }
            let target_name = fallback_relative
                .file_name()
                .ok_or_else(|| "Uncategorized fallback has no filename.".to_string())?;
            let inactive = is_disabled_package_path(Path::new(&item.name));
            let cfg_opt_in = update_resource_cfg
                && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
            let adjusted = if cfg_opt_in || inactive || item.status != "classified" {
                Ok((
                    ensure_source_loading_branch(&root, &source_canonical, &fallback_parts),
                    None,
                ))
            } else {
                fit_destination_to_resource_cfg(
                    &root,
                    &source_canonical,
                    target_name,
                    &fallback_parts,
                    resource_cfg.as_ref(),
                )
            };
            let (destination_parts, note) = match adjusted {
                Ok(ok) => ok,
                Err(reason) => {
                    stats.blocked += 1;
                    items.push(make_blocked(item, format!(
                        "Cannot move Unknown package into a loadable Not Categorized folder: {reason}"
                    )));
                    continue;
                }
            };
            if let Err(reason) = validate_destination_parts(&destination_parts) {
                stats.blocked += 1;
                items.push(make_blocked(
                    item,
                    format!("{reason} (adjusted parts={:?})", destination_parts),
                ));
                continue;
            }
            let destination = destination_path(&root, &destination_parts, target_name);
            if !path_is_within_root(&root, &destination)
                || (is_mods_root(&root) && !destination.starts_with(root.join("Packages")))
            {
                stats.blocked += 1;
                items.push(make_blocked(
                    item,
                    "Not Categorized fallback escaped the Packages loading branch.".to_string(),
                ));
                continue;
            }
            let destination_relative = destination
                .strip_prefix(&root)
                .map(relative_key)
                .map_err(|_| "Not Categorized destination escaped selected root.".to_string())?;
            let already = same_path_case_insensitive(&source_canonical, &destination);
            let collision = !already && destination.exists();
            let collision_identical = if collision {
                let (existing_hash, _) = sha256_file(&destination)
                    .map_err(|error| format!("Cannot inspect fallback collision: {error}"))?;
                existing_hash.eq_ignore_ascii_case(&source_hash)
            } else {
                false
            };
            let destination_status = if already {
                "already_organized"
            } else if collision_identical {
                "duplicate_skipped"
            } else if collision {
                "collision_different_content"
            } else {
                "ready_uncategorized"
            };
            if already {
                stats.already_organized += 1;
            }
            if collision_identical {
                stats.duplicate_skipped += 1;
            } else if collision {
                stats.collision_different_content += 1;
            }
            if !already && !collision {
                add_missing_directories(&root, &destination_parts, &mut directories);
                stats.ready += 1;
            }
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative),
                classification_status: item.status.clone(),
                classification_reason: Some(format!(
                    "No safe CASP/OBJD/family classification; moved to Not Categorized for review.{}",
                    note.map(|n| format!(" {n}")).unwrap_or_default()
                )),
                plan_status: destination_status.to_string(),
                sha256: Some(source_hash),
                size: item.file_size,
                warnings: vec![if collision_identical {
                    "Not Categorized already contains byte-identical data. Extra copy kept for Duplicates/Quarantine.".to_string()
                } else if collision {
                    "The Not Categorized destination already exists. No file will be overwritten; review this collision.".to_string()
                } else if already {
                    "Already in its safe Not Categorized destination.".to_string()
                } else {
                    "Classification is unknown; this is a reviewed fallback destination, NOT a verified semantic category. The package is never deleted.".to_string()
                }],
            });
            continue;
        }

        let verified_companion = item.status == "classified"
            && item.classification_confidence == "medium"
            && item
                .detected_from
                .iter()
                .any(|source| source == "ModFolderCompanion");
        if is_override_source
            && (item.status != "classified"
                || item.destination_parts.is_empty()
                || (!matches!(item.classification_confidence.as_str(), "high" | "manual")
                    && !verified_companion))
        {
            let source_size = fs::metadata(&source)
                .map_err(|error| format!("Could not stat {}: {error}", source.display()))?
                .len();
            stats.kept_uncategorized += 1;
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: None,
                destination_relative_path: None,
                classification_status: item.status.clone(),
                classification_reason: item.classification_reason.clone(),
                plan_status: "keep_uncategorized".to_string(),
                sha256: Some(source_hash),
                size: source_size,
                warnings: vec![format!(
                    "Classification status '{}' with confidence '{}' has no safe automatic destination. The package will stay in place and remain visible as '{}'.",
                    item.status,
                    item.classification_confidence,
                    language.not_categorized_folder()
                )],
            });
            continue;
        }

        let mut destination_parts = item.destination_parts.clone();
        let mut classification_reason = item.classification_reason.clone();

        if let Some(rule) = matching_rule(
            &profile,
            &item.name,
            &item.relative_path,
            item.category.as_deref(),
            item.sub_category.as_deref(),
            &item.detected_from,
        ) {
            destination_parts = split_destination(&rule.destination);
            classification_reason = Some(format!(
                "{} | Custom rule '{}' => {}",
                classification_reason.unwrap_or_default(),
                rule.name,
                destination_parts.join("\\")
            ));
        } else if profile.collapse_to_category
            && !item
                .destination_parts
                .first()
                .is_some_and(|first| first == "Scripts")
        {
            // A compacting profile must not destroy the canonical
            // Scripts > Gameplay > Verified Creator hierarchy.
            if let Some(category) = &item.category {
                destination_parts = vec![category.clone()];
                classification_reason = Some(format!(
                    "{} | Profile '{}' collapsed destination to category '{}'.",
                    classification_reason.unwrap_or_default(),
                    profile.name,
                    category
                ));
            }
        }

        let prefix = split_destination(&profile.destination_prefix);
        if !prefix.is_empty() {
            let mut prefixed = prefix;
            prefixed.extend(destination_parts);
            destination_parts = prefixed;
        }

        let model_destination = if is_override_source {
            None
        } else if let Some(result) = hash_model_destination.as_ref() {
            match result {
                Ok(destination) => Some(destination.clone()),
                Err(reason) => {
                    let (_, fallback_parts) =
                        fallback_relative_path(&root, language, &item.relative_path, &source_hash)?;
                    destination_parts = fallback_parts;
                    classification_reason = Some(format!(
                        "{} | Review fallback: {reason}",
                        classification_reason.clone().unwrap_or_default()
                    ));
                    None
                }
            }
        } else {
            match resolve_model_destination(
                &manual_model,
                &root,
                &source_canonical,
                &source_hash,
                &destination_parts,
            ) {
                Ok(destination) => Some(destination),
                Err(reason) => {
                    let (_, fallback_parts) =
                        fallback_relative_path(&root, language, &item.relative_path, &source_hash)?;
                    destination_parts = fallback_parts;
                    classification_reason = Some(format!(
                        "{} | Review fallback: {reason}",
                        classification_reason.clone().unwrap_or_default()
                    ));
                    None
                }
            }
        };
        let model_locked = model_destination.is_some();
        if let Some(model_destination) = &model_destination {
            destination_parts = model_destination.parts.clone();
            classification_reason = Some(format!(
                "{} | Official model destination: {}",
                classification_reason.unwrap_or_default(),
                model_destination.evidence
            ));
        }

        let file_name = source
            .file_name()
            .ok_or_else(|| format!("Source has no filename: {}", source.display()))?;

        // Updating Resource.cfg is explicitly opt-in. With consent, preserve
        // the full category tree rather than compacting it to existing depth.
        let cfg_opt_in = update_resource_cfg
            && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
        let inactive_package = is_disabled_package_path(Path::new(&item.name));
        let cfg_fit = if cfg_opt_in || inactive_package {
            // Disabled packages are never loaded by The Sims 3. Their
            // classification must not be blocked by PackedFile depth rules,
            // and the inactive suffix must be preserved on relocation.
            Ok((
                ensure_source_loading_branch(&root, &source_canonical, &destination_parts),
                None,
            ))
        } else {
            fit_destination_to_resource_cfg(
                &root,
                &source_canonical,
                file_name,
                &destination_parts,
                resource_cfg.as_ref(),
            )
        };
        match cfg_fit {
            Ok((fitted, note)) => {
                if model_locked && fitted != destination_parts {
                    stats.blocked += 1;
                    items.push(make_blocked(item, format!("The exact manual-model destination '{}' is not loadable under the current Resource.cfg; no alternate folder will be invented.", destination_parts.join("\\"))));
                    continue;
                }
                destination_parts = fitted;
                if let Some(note) = note {
                    classification_reason = Some(format!(
                        "{} | {}",
                        classification_reason.unwrap_or_default(),
                        note
                    ));
                }
            }
            Err(reason) => {
                if source_uses_overrides(&root, &source_canonical) {
                    // Do not block the other Packages in a mixed Mods scan.
                    // Preserve the Override and its logical classification
                    // whenever the actual Resource.cfg cannot load a category.
                    stats.kept_uncategorized += 1;
                    items.push(PlanItem {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        source_path: source_canonical.to_string_lossy().to_string(),
                        source_relative_path: item.relative_path.clone(),
                        destination_path: None,
                        destination_relative_path: None,
                        classification_status: item.status.clone(),
                        classification_reason: classification_reason.clone(),
                        plan_status: "keep_uncategorized".to_string(),
                        sha256: Some(source_hash),
                        size: item.file_size,
                        warnings: vec![format!("Override kept at its original location: {reason}")],
                    });
                } else {
                    stats.blocked += 1;
                    items.push(make_blocked(item, reason));
                }
                continue;
            }
        }

        // With a flat Overrides/*.package rule, the sole safe destination
        // when Overrides itself was selected is its root (zero categories).
        // This empty path is allowed only after fit_destination_to_resource_cfg
        // has verified that Resource.cfg loads the resulting root-level file.
        let validated = if is_within_overrides(&root) && destination_parts.is_empty() {
            Ok(())
        } else {
            validate_destination_parts(&destination_parts)
        };
        if let Err(reason) = validated {
            stats.blocked += 1;
            items.push(make_blocked(item, reason));
            continue;
        }

        if !path_is_within_root(&root, &source_canonical) {
            return Err(format!(
                "Source escaped the selected root: {}",
                source.display()
            ));
        }

        let mut destination_relative = PathBuf::new();
        for part in &destination_parts {
            destination_relative.push(part);
        }
        destination_relative.push(file_name);

        let destination = root.join(&destination_relative);
        if is_mods_root(&root) {
            let branch = if source_uses_overrides(&root, &source_canonical) {
                "Overrides"
            } else {
                "Packages"
            };
            if !destination.starts_with(root.join(branch)) {
                stats.blocked += 1;
                items.push(make_blocked(
                    item,
                    format!("Destination escaped its original Mods/{branch} loading branch."),
                ));
                continue;
            }
        }
        let destination_relative_text = relative_key(&destination_relative);

        // Review fallbacks are canonical destinations too. Once a package
        // has been moved to `#Sem Categoria` with its preserved source
        // context, a later incremental plan must not propose the same move
        // again merely because that fallback path is not an exemplar in the
        // manual model. The destination was already validated above; this
        // is an exact full-path comparison, never a folder-name heuristic.
        if same_path_case_insensitive(&source_canonical, &destination) {
            stats.already_organized += 1;
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative_text),
                classification_status: item.status.clone(),
                classification_reason: classification_reason.clone(),
                plan_status: "already_organized_review".to_string(),
                sha256: Some(source_hash),
                size: item.file_size,
                warnings: vec![
                    "The package is already at the exact validated review destination; no move is proposed.".to_string(),
                ],
            });
            continue;
        }

        if exact_model_relative_path_matches(&item.relative_path, &destination_relative_text) {
            stats.already_organized += 1;
            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative_text),
                classification_status: item.status.clone(),
                classification_reason: classification_reason.clone(),
                plan_status: "already_organized".to_string(),
                sha256: Some(source_hash),
                size: item.file_size,
                warnings: Vec::new(),
            });
            continue;
        }

        let source_size = fs::metadata(&source)
            .map_err(|error| format!("Could not stat {}: {error}", source.display()))?
            .len();

        if destination.exists() {
            let (target_hash, target_size) = sha256_file(&destination).map_err(|error| {
                format!(
                    "Could not hash collision target {}: {error}",
                    destination.display()
                )
            })?;

            let same = source_size == target_size && source_hash.eq_ignore_ascii_case(&target_hash);
            if same {
                // An exact copy that is stranded in a legacy folder must not
                // be left there merely because its canonical destination is
                // already occupied. Move the extra copy to the flat,
                // reversible quarantine instead; the canonical destination is
                // retained byte-for-byte and no overwrite is attempted.
                let (quarantine_relative, quarantine_parts) = duplicate_quarantine_destination(
                    &root,
                    language,
                    &source_canonical,
                    &source_hash,
                    &reserved_duplicate_destinations,
                )?;
                let quarantine = root.join(&quarantine_relative);
                if !same_path_case_insensitive(&source_canonical, &quarantine) {
                    add_missing_directories(&root, &quarantine_parts, &mut directories);
                    reserved_duplicate_destinations
                        .insert(relative_key(&quarantine_relative).to_ascii_lowercase());
                    stats.ready += 1;
                    stats.duplicate_quarantine_ready += 1;
                    items.push(PlanItem {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        source_path: source_canonical.to_string_lossy().to_string(),
                        source_relative_path: item.relative_path.clone(),
                        destination_path: Some(quarantine.to_string_lossy().to_string()),
                        destination_relative_path: Some(relative_key(&quarantine_relative)),
                        classification_status: item.status.clone(),
                        classification_reason: Some(format!(
                            "{} | Exact byte-identical copy quarantined; canonical keeper remains '{}'.",
                            classification_reason.unwrap_or_default(),
                            destination_relative_text
                        )),
                        plan_status: "ready_duplicate_quarantine".to_string(),
                        sha256: Some(source_hash),
                        size: source_size,
                        warnings: vec![format!(
                            "The canonical destination '{}' already contains identical bytes. The legacy copy will move intact to flat #Duplicados as '{}'; no overwrite or reactivation is performed.",
                            destination_relative_text,
                            relative_key(&quarantine_relative)
                        )],
                    });
                    continue;
                }

                // A file already directly below #Duplicados is already in
                // the requested flat quarantine and must remain idempotent.
                stats.duplicate_skipped += 1;
                items.push(PlanItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    source_path: source_canonical.to_string_lossy().to_string(),
                    source_relative_path: item.relative_path.clone(),
                    destination_path: Some(quarantine.to_string_lossy().to_string()),
                    destination_relative_path: Some(relative_key(&quarantine_relative)),
                    classification_status: item.status.clone(),
                    classification_reason: classification_reason.clone(),
                    plan_status: "duplicate_skipped".to_string(),
                    sha256: Some(source_hash),
                    size: source_size,
                    warnings: vec![
                        "The byte-identical file is already directly in flat #Duplicados; no overwrite was performed.".to_string(),
                    ],
                });
                continue;
            }

            if !is_override_source {
                // A same-name/different-bytes collision is not an unknown
                // package and must not be silently relocated to
                // #Sem Categoria. Keep the source untouched, expose the
                // canonical candidate for review, and let independent ready
                // operations proceed.
                classification_reason = Some(format!(
                    "{} | Content conflict with canonical destination; source preserved for review.",
                    classification_reason.unwrap_or_default()
                ));
                stats.collision_different_content += 1;
                items.push(PlanItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    source_path: source_canonical.to_string_lossy().to_string(),
                    source_relative_path: item.relative_path.clone(),
                    destination_path: Some(destination.to_string_lossy().to_string()),
                    destination_relative_path: Some(destination_relative_text),
                    classification_status: item.status.clone(),
                    classification_reason,
                    plan_status: "collision_different_content".to_string(),
                    sha256: Some(source_hash),
                    size: source_size,
                    warnings: vec![
                        "The canonical destination contains different bytes with the same filename. The source stays in place; no file is overwritten or sent to #Sem Categoria automatically.".to_string(),
                    ],
                });
                continue;
            }

            stats.collision_different_content += 1;

            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative_text),
                classification_status: item.status.clone(),
                classification_reason: classification_reason.clone(),
                plan_status: "collision_different_content".to_string(),
                sha256: Some(source_hash),
                size: source_size,
                warnings: vec![
                    "The categorized destination already contains different data with the same name. This package will stay in place until the collision is reviewed.".to_string()
                ],
            });
            continue;
        }

        add_missing_directories(&root, &destination_parts, &mut directories);
        stats.ready += 1;
        items.push(PlanItem {
            id: item.id.clone(),
            name: item.name.clone(),
            source_path: source_canonical.to_string_lossy().to_string(),
            source_relative_path: item.relative_path.clone(),
            destination_path: Some(destination.to_string_lossy().to_string()),
            destination_relative_path: Some(destination_relative_text),
            classification_status: item.status.clone(),
            classification_reason: classification_reason.clone(),
            plan_status: "ready".to_string(),
            sha256: Some(source_hash),
            size: source_size,
            warnings: if outside_active_branch {
                vec!["Migrating a selected legacy file from outside Packages/Overrides into Packages. It may become active in-game after organization; review the plan.".to_string()]
            } else {
                Vec::new()
            },
        });
    }

    // Image previews are companions to selected legacy packages. They are
    // planned as ordinary reversible moves, while DBPF classification remains
    // package-only and never invents categories for screenshots.
    append_preview_image_items(
        &root,
        language,
        &selected,
        &selected_roots,
        &scan_by_canonical,
        &mut items,
        &mut directories,
        &mut stats,
    )?;

    // Explicitly selected legacy directories may contain only documentation,
    // Sims3Pack files, Windows metadata or other companions. Move those
    // files through the real executor as well, so an old root is not kept
    // alive merely because it contains a non-DBPF file.
    append_auxiliary_items(
        &root,
        language,
        &selected_roots,
        &mut items,
        &mut directories,
        &mut stats,
    )?;

    // Also repair duplicate entries created by older versions of the Manager.
    // This runs independently of the selected legacy roots, but never touches
    // files already directly below the canonical #Duplicados root.
    append_nested_duplicate_quarantine_items(
        &root,
        language,
        &manual_model,
        &mut items,
        &mut directories,
        &mut stats,
        &mut reserved_duplicate_destinations,
    )?;

    // Detect collisions created by the plan itself before any filesystem write occurs.
    // This catches multiple selected packages that resolve to the same final path.
    mark_intra_plan_destination_collisions(
        &root,
        language,
        &mut items,
        &mut directories,
        &mut reserved_duplicate_destinations,
        &mut stats,
    )?;
    resolve_intra_plan_source_conflicts(&root, &mut items);
    mark_intra_plan_source_destination_collisions(&root, &mut items);

    for group in workspace.groups.iter().filter(|group| group.keep_together) {
        if !group
            .member_sha256
            .iter()
            .all(|hash| selected_hash_set.contains(hash))
        {
            continue;
        }

        let destinations = items
            .iter()
            .filter(|item| {
                item.sha256
                    .as_ref()
                    .map(|hash| group.member_sha256.contains(hash))
                    .unwrap_or(false)
            })
            .filter_map(|item| item.destination_relative_path.as_ref())
            .filter_map(|path| Path::new(path).parent().map(relative_key))
            .collect::<HashSet<_>>();

        if destinations.len() > 1 {
            for item in items.iter_mut().filter(|item| {
                item.sha256
                    .as_ref()
                    .map(|hash| group.member_sha256.contains(hash))
                    .unwrap_or(false)
            }) {
                if item.plan_status.starts_with("ready") {
                    item.plan_status = "blocked".to_string();
                    item.warnings.push(format!(
                        "Keep Together group '{}' would be split across multiple destination folders.",
                        group.name
                    ));
                }
            }
        }
    }

    stats.kept_uncategorized = items
        .iter()
        .filter(|item| item.plan_status == "keep_uncategorized")
        .count();
    stats.ready = items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready"))
        .count();
    stats.duplicate_quarantine_ready = items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready_duplicate_quarantine"))
        .count();
    stats.duplicate_skipped = items
        .iter()
        .filter(|item| item.plan_status == "duplicate_skipped")
        .count();
    stats.collision_different_content = items
        .iter()
        .filter(|item| item.plan_status == "collision_different_content")
        .count();
    stats.blocked = items
        .iter()
        .filter(|item| item.plan_status.starts_with("blocked"))
        .count();

    let ready_items = items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready"))
        .cloned()
        .collect::<Vec<_>>();

    stats.directories_to_create = directories.len();
    // Project the post-move state instead of counting unrelated empty folders
    // across the whole Packages tree. This keeps cleanup indicators aligned
    // with the focused branches the executor is actually allowed to visit.
    stats.remaining_legacy_files =
        projected_remaining_legacy_files(&selected, &selected_roots, &items);
    stats.empty_folders_to_clean = projected_empty_folders_to_clean(
        &root,
        &selected_roots,
        &items,
        &protected_empty_directories,
    );
    let (_, legacy_folders_retained) = projected_legacy_folder_counts(
        &root,
        &selected_roots,
        &items,
        &protected_empty_directories,
    );
    stats.legacy_folders_retained = legacy_folders_retained;

    let can_execute = plan_can_execute(&stats, workspace.read_only);
    let cfg_opt_in = update_resource_cfg
        && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
    let resource_cfg_update = if cfg_opt_in {
        // Include both new destinations and existing packages in the scanned
        // branch. Partial organization must not cause old CC to stop loading.
        let mut paths = ready_items
            .iter()
            .filter_map(|item| item.destination_path.as_ref().map(PathBuf::from))
            .collect::<Vec<_>>();
        paths.extend(
            scan.items
                .iter()
                .map(|item| PathBuf::from(&item.path))
                .filter(|path| {
                    root.ancestors()
                        .find(|parent| is_mods_root(parent))
                        .is_some_and(|mods| {
                            path.starts_with(mods.join("Packages"))
                                || path.starts_with(mods.join("Overrides"))
                        })
                }),
        );
        preview_resource_cfg_update(&root, &paths)?
    } else {
        None
    };

    stats.planning_ms = planning_started.elapsed().as_millis();
    Ok(OrganizationPlan {
        root: root.to_string_lossy().to_string(),
        manifest_preview: manifest_preview(&root, language, &ready_items),
        directories_to_create: directories.into_iter().collect(),
        protected_empty_directories,
        cleanup_roots,
        resource_cfg_update,
        stats: stats.clone(),
        items,
        can_execute,
        analysis_before: OrganizationAnalysisSnapshot::from_scan("analysis_before", &scan.stats)
            .with_plan(&stats, stats.remaining_legacy_files),
    })
}

#[tauri::command]
pub async fn build_organization_plan(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
) -> Result<OrganizationPlan, String> {
    crate::operation::run_blocking("planning", "building", move || {
        build_organization_plan_with_cfg_core(folder, language, selected_paths, false)
    })
    .await
}

#[tauri::command]
pub async fn build_organization_plan_with_cfg(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
    update_resource_cfg: bool,
) -> Result<OrganizationPlan, String> {
    crate::operation::run_blocking("planning", "building", move || {
        build_organization_plan_with_cfg_core(folder, language, selected_paths, update_resource_cfg)
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn persistent_model_index_accepts_windows_verbatim_path_identity() {
        let canonical = std::path::Path::new(
            r"C:\Users\David\Downloads\Mods - ESTRUTURA MODELO\Packages",
        );
        assert!(manual_model_root_matches(
            r"\\?\C:\Users\David\Downloads\Mods - ESTRUTURA MODELO\Packages",
            canonical
        ));
        assert!(manual_model_root_matches(
            r"c:/users/david/downloads/mods - estrutura modelo/packages",
            canonical
        ));
        assert!(!manual_model_root_matches(
            r"C:\Users\David\Downloads\Mods - ESTRUTURA MODELO\Packages\Legacy",
            canonical
        ));
    }

    #[test]
    fn copied_mods_root_preserves_packages_overrides_and_safe_destinations() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("s3cc-copied-{}-{nonce}", std::process::id()))
            .join("Mods - Copia");
        std::fs::create_dir_all(root.join("Packages")).unwrap();
        std::fs::create_dir_all(root.join("Overrides")).unwrap();
        assert!(is_mods_root(&root));
        let packages = root.join("Packages").join("Legacy").join("hair.package");
        let overrides = root.join("Overrides").join("Mods").join("fix.package");
        assert!(is_within_packages(&packages.parent().unwrap()));
        assert!(is_within_overrides(&overrides.parent().unwrap()));
        assert!(source_uses_overrides(&root, &overrides));
        assert!(!source_uses_overrides(&root, &packages));
        assert_eq!(
            ensure_packages_destination(&root, &["Cabelos".into(), "Feminino".into()]),
            vec!["Packages", "Cabelos", "Feminino"]
        );
        assert_eq!(
            ensure_source_loading_branch(&root, &overrides, &["Acessórios".into()]),
            vec!["Overrides", "Acessórios"]
        );
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn unknown_fallback_is_flat_without_losing_manifest_source_context() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-unknown-fallback-{}-{nonce}",
            std::process::id()
        ));
        let mods = base.join("Mods - Copia");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(mods.join("Overrides")).unwrap();
        let (path, parts) = fallback_relative_path(
            &mods,
            AppLanguage::Pt,
            "#+18\\Pns (TS3)\\Rigged\\PenisButtBones.package",
            "AABBCCDD",
        )
        .unwrap();
        assert!(path.starts_with("Packages"));
        assert_eq!(
            path,
            PathBuf::from("Packages\\#Sem Categoria\\PenisButtBones.package")
        );
        assert!(!path.to_string_lossy().contains("Pns (TS3)"));
        assert!(!path.to_string_lossy().contains("Rigged"));
        assert!(parts.iter().any(|part| part == "#Sem Categoria"));
        let (override_path, override_parts) = fallback_relative_path(
            &mods,
            AppLanguage::Pt,
            "Overrides\\Overhaul\\unknown.package",
            "AABBCCDD",
        )
        .unwrap();
        assert_eq!(
            override_path,
            PathBuf::from("Overrides\\Overhaul\\unknown.package")
        );
        assert!(override_parts.is_empty());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn duplicate_quarantine_is_reversible_and_never_uses_a_legacy_folder() {
        let base =
            std::env::temp_dir().join(format!("s3cc-duplicate-quarantine-{}", std::process::id()));
        let packages = base.join("Packages");
        let source = packages.join("#6 Slider").join("same.package");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        let reserved = HashSet::new();
        let (relative, parts) = duplicate_quarantine_destination(
            &packages,
            AppLanguage::Pt,
            &source,
            &"A".repeat(64),
            &reserved,
        )
        .unwrap();
        assert_eq!(parts.first().map(String::as_str), Some("#Duplicados"));
        assert_eq!(relative.parent(), Some(Path::new("#Duplicados")));
        assert!(!relative.starts_with(Path::new("#6 Slider")));
        assert!(relative.to_string_lossy().contains("same.package"));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn duplicate_quarantine_collision_uses_flat_deterministic_suffix() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-duplicate-flat-collision-{}",
            std::process::id()
        ));
        let packages = base.join("Packages");
        let quarantine = packages.join("#Duplicados");
        let source = packages.join("Old").join("same.package");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::write(quarantine.join("same.package"), b"existing").unwrap();
        std::fs::write(
            quarantine.join("same.package.duplicated"),
            b"existing marker",
        )
        .unwrap();
        let mut reserved = HashSet::new();
        reserved.insert(String::from(
            "#duplicados\\same [aaaaaaaaaaaaaaaa].package.duplicated",
        ));
        let (relative, parts) = duplicate_quarantine_destination(
            &packages,
            AppLanguage::Pt,
            &source,
            &"A".repeat(64),
            &reserved,
        )
        .unwrap();
        assert_eq!(parts, vec!["#Duplicados"]);
        assert_eq!(relative.parent(), Some(Path::new("#Duplicados")));
        assert_eq!(
            relative.file_name().unwrap(),
            "same [AAAAAAAAAAAAAAAA-1].package.duplicated"
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn duplicate_quarantine_normalizes_disabled_names_without_nested_paths() {
        assert_eq!(
            duplicate_quarantine_file_name("ESkin-Natural+Light.package.disabled", "ABC", 0),
            "ESkin-Natural+Light.package.duplicated"
        );
        assert_eq!(
            duplicate_quarantine_file_name("ESkin-Natural+Light.package", "ABC", 1),
            "ESkin-Natural+Light [ABC].package.duplicated"
        );
        assert_eq!(
            duplicate_quarantine_file_name("ESkin-Natural+Light.package.duplicated", "ABC", 0),
            "ESkin-Natural+Light.package.duplicated"
        );
    }

    #[test]
    fn eskin_same_name_different_bytes_stays_for_review() {
        let mut items = vec![
            PlanItem {
                id: "eskin-a".into(),
                name: "ESkin-Natural+Light.package".into(),
                source_path: r"C:\Mods\#5 Skin\ESkin-Natural+Light.package".into(),
                source_relative_path: r"#5 Skin\ESkin-Natural+Light.package".into(),
                destination_path: Some(
                    r"C:\Mods\Packages\Skins\ESkin-Natural+Light.package".into(),
                ),
                destination_relative_path: Some(
                    r"Packages\Skins\ESkin-Natural+Light.package".into(),
                ),
                classification_status: "classified".into(),
                classification_reason: Some("CASP skin".into()),
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 10,
                warnings: vec![],
            },
            PlanItem {
                id: "eskin-b".into(),
                name: "ESkin-Natural+Light.package".into(),
                source_path: r"C:\Mods\Skin\ESkin-Natural+Light.package".into(),
                source_relative_path: r"Skin\ESkin-Natural+Light.package".into(),
                destination_path: Some(
                    r"C:\Mods\Packages\Skins\ESkin-Natural+Light.package".into(),
                ),
                destination_relative_path: Some(
                    r"Packages\Skins\ESkin-Natural+Light.package".into(),
                ),
                classification_status: "classified".into(),
                classification_reason: Some("CASP skin".into()),
                plan_status: "ready".into(),
                sha256: Some("B".repeat(64)),
                size: 10,
                warnings: vec![],
            },
        ];
        let mut stats = PlanStats::default();
        let mut directories = BTreeSet::new();
        let mut reserved = HashSet::new();
        mark_intra_plan_destination_collisions(
            Path::new(r"C:\Mods"),
            AppLanguage::En,
            &mut items,
            &mut directories,
            &mut reserved,
            &mut stats,
        )
        .unwrap();
        assert_eq!(stats.collision_different_content, 2);
        assert!(items
            .iter()
            .all(|item| item.plan_status == "collision_different_content"));
        assert!(items.iter().all(|item| {
            item.destination_relative_path.as_deref()
                == Some(r"Packages\Skins\ESkin-Natural+Light.package")
        }));
        assert!(!items.iter().any(|item| {
            item.destination_relative_path
                .as_deref()
                .is_some_and(|path| path.contains("#Sem Categoria"))
        }));
    }

    #[test]
    fn nested_duplicate_quarantine_is_planned_for_flat_migration() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-duplicate-flat-migration-{}",
            std::process::id()
        ));
        let packages = base.join("Packages");
        let nested = packages
            .join("#Duplicados")
            .join("abcdef0123456789")
            .join("Old");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("same.package"), b"duplicate bytes").unwrap();
        let keeper = packages.join("Sliders").join("same.package");
        std::fs::create_dir_all(keeper.parent().unwrap()).unwrap();
        std::fs::write(&keeper, b"duplicate bytes").unwrap();
        let mut items = Vec::new();
        let mut directories = BTreeSet::new();
        let mut stats = PlanStats::default();
        let mut reserved = HashSet::new();
        let manual_model = ManualModelIndex {
            directories: BTreeMap::new(),
            files_by_sha256: HashMap::new(),
        };
        let packages = packages.canonicalize().unwrap();
        append_nested_duplicate_quarantine_items(
            &packages,
            AppLanguage::Pt,
            &manual_model,
            &mut items,
            &mut directories,
            &mut stats,
            &mut reserved,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].plan_status, "ready_duplicate_quarantine_migration");
        assert_eq!(
            Path::new(items[0].destination_relative_path.as_deref().unwrap()).parent(),
            Some(Path::new("#Duplicados"))
        );
        assert!(!items[0]
            .destination_relative_path
            .as_deref()
            .unwrap()
            .contains("abcdef"));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn flat_duplicate_migration_is_idempotent_after_relocation() {
        // Keep the fixture under the workspace: sandboxed Windows temp folders
        // can deny rename even though the production move is correct.
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
            "s3cc-duplicate-flat-idempotent-{}",
            std::process::id()
        ));
        let packages = base.join("Packages");
        let nested = packages.join("#Duplicados").join("legacy").join("Old");
        std::fs::create_dir_all(&nested).unwrap();
        let source = nested.join("same.package");
        std::fs::write(&source, b"duplicate bytes").unwrap();
        let keeper = packages.join("Sliders").join("same.package");
        std::fs::create_dir_all(keeper.parent().unwrap()).unwrap();
        std::fs::write(&keeper, b"duplicate bytes").unwrap();
        let packages = packages.canonicalize().unwrap();
        let source = source.canonicalize().unwrap();

        let mut items = Vec::new();
        let mut directories = BTreeSet::new();
        let mut stats = PlanStats::default();
        let mut reserved = HashSet::new();
        let manual_model = ManualModelIndex {
            directories: BTreeMap::new(),
            files_by_sha256: HashMap::new(),
        };
        append_nested_duplicate_quarantine_items(
            &packages,
            AppLanguage::Pt,
            &manual_model,
            &mut items,
            &mut directories,
            &mut stats,
            &mut reserved,
        )
        .unwrap();
        let destination = packages.join(items[0].destination_relative_path.as_deref().unwrap());
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::rename(&source, &destination).unwrap();

        let mut second_items = Vec::new();
        let mut second_directories = BTreeSet::new();
        let mut second_stats = PlanStats::default();
        let mut second_reserved = HashSet::new();
        append_nested_duplicate_quarantine_items(
            &packages,
            AppLanguage::Pt,
            &manual_model,
            &mut second_items,
            &mut second_directories,
            &mut second_stats,
            &mut second_reserved,
        )
        .unwrap();
        assert!(second_items.is_empty());
        assert_eq!(second_stats.ready, 0);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn existing_quarantine_without_active_keeper_is_preserved() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-duplicate-no-keeper-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        let packages = base.join("Packages");
        let nested = packages.join("#Duplicados").join("legacy");
        std::fs::create_dir_all(&nested).unwrap();
        let source = nested.join("orphan.package");
        std::fs::write(&source, b"no keeper").unwrap();
        let packages = packages.canonicalize().unwrap();
        let manual_model = ManualModelIndex {
            directories: BTreeMap::new(),
            files_by_sha256: HashMap::new(),
        };
        let mut items = Vec::new();
        let mut directories = BTreeSet::new();
        let mut stats = PlanStats::default();
        let mut reserved = HashSet::new();

        append_nested_duplicate_quarantine_items(
            &packages,
            AppLanguage::Pt,
            &manual_model,
            &mut items,
            &mut directories,
            &mut stats,
            &mut reserved,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].plan_status, "blocked_duplicate_no_keeper");
        assert!(items[0].destination_path.is_none());
        assert!(source.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn disabled_quarantine_entry_requires_keeper_and_stays_inactive() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-duplicate-disabled-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        let packages = base.join("Packages");
        let nested = packages.join("#Duplicados").join("legacy");
        std::fs::create_dir_all(&nested).unwrap();
        let source = nested.join("disabled.package.disabled");
        std::fs::write(&source, b"disabled duplicate").unwrap();
        let keeper = packages.join("Skins").join("disabled.package");
        std::fs::create_dir_all(keeper.parent().unwrap()).unwrap();
        std::fs::write(&keeper, b"disabled duplicate").unwrap();
        let packages = packages.canonicalize().unwrap();
        let manual_model = ManualModelIndex {
            directories: BTreeMap::new(),
            files_by_sha256: HashMap::new(),
        };
        let mut items = Vec::new();
        let mut directories = BTreeSet::new();
        let mut stats = PlanStats::default();
        let mut reserved = HashSet::new();

        append_nested_duplicate_quarantine_items(
            &packages,
            AppLanguage::Pt,
            &manual_model,
            &mut items,
            &mut directories,
            &mut stats,
            &mut reserved,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].plan_status, "ready_duplicate_quarantine_migration");
        assert!(items[0]
            .destination_relative_path
            .as_deref()
            .is_some_and(|path| path.ends_with(".package.duplicated")));
        assert!(items[0]
            .source_relative_path
            .to_ascii_lowercase()
            .ends_with(".package.disabled"));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn preview_extensions_exclude_windows_metadata_and_crdownload() {
        assert!(preview_extension(Path::new("reference.PNG")));
        assert!(preview_extension(Path::new("reference.jpeg")));
        assert!(!preview_extension(Path::new("desktop.ini")));
        assert!(!preview_extension(Path::new("download.package.crdownload")));
    }

    #[test]
    fn legacy_user_folders_are_migratable_but_sims_caches_are_protected() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-migrate-source-{}-{nonce}",
            std::process::id()
        ));
        let mods = base.join("Mods - Copia");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(mods.join("Overrides")).unwrap();
        assert!(!is_mods_system_source(
            &mods,
            &mods.join("#+18").join("AnimatedWoohoo.package")
        ));
        assert!(!is_mods_system_source(
            &mods,
            &mods.join("Careers").join("Modeling.package")
        ));
        assert!(is_mods_system_source(
            &mods,
            &mods.join("DCCache").join("dcdb0.dbc")
        ));
        assert!(is_mods_system_source(
            &mods,
            &mods.join("Downloads").join("download.package")
        ));
        assert!(is_mods_system_source(
            &mods,
            &mods
                .join("S3CC Manager")
                .join("Restore Manifests")
                .join("x.package")
        ));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn selected_exact_duplicates_choose_one_keeper_without_reactivating_disabled() {
        let mut hashes = HashMap::new();
        hashes.insert(PathBuf::from("Packages/Old/C.package"), "AAA".to_string());
        hashes.insert(PathBuf::from("Packages/Old/B.package"), "AAA".to_string());
        hashes.insert(
            PathBuf::from("Packages/Old/A.package.disabled"),
            "AAA".to_string(),
        );
        hashes.insert(
            PathBuf::from("Packages/Old/Z.package.disabled"),
            "AAA".to_string(),
        );
        assert!(is_selected_content_keeper(
            Path::new("Packages/Old/B.package"),
            "AAA",
            &hashes
        ));
        assert!(!is_selected_content_keeper(
            Path::new("Packages/Old/C.package"),
            "AAA",
            &hashes
        ));
        assert!(is_selected_content_keeper(
            Path::new("Packages/Old/A.package.disabled"),
            "AAA",
            &hashes
        ));
        assert!(!is_selected_content_keeper(
            Path::new("Packages/Old/Z.package.disabled"),
            "AAA",
            &hashes
        ));
    }

    use super::*;

    #[test]
    fn empty_manual_model_root_is_not_a_destination_for_unknown_content() {
        let mut directories = BTreeMap::new();
        directories.insert(String::new(), Vec::new());
        let index = ManualModelIndex {
            directories,
            files_by_sha256: HashMap::new(),
        };
        let result = resolve_model_destination(
            &index,
            Path::new("Mods - Copia"),
            Path::new("Mods - Copia/Packages/unknown.package"),
            "UNKNOWN",
            &[],
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("not a classification destination"));
    }

    fn synthetic_scan_item(
        root: &Path,
        relative_path: &str,
        destination_parts: &[&str],
    ) -> (PathBuf, ScanPackageItem) {
        let path = root.join(relative_path.replace('\\', std::path::MAIN_SEPARATOR_STR));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let destination_parts = destination_parts
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>();
        (
            path.clone(),
            ScanPackageItem {
                id: relative_path.to_string(),
                name,
                path: path.to_string_lossy().into_owned(),
                relative_path: relative_path.to_string(),
                file_size: 12,
                resource_count: 1,
                catalog_resource_count: 1,
                resource_types: Vec::new(),
                instances: Vec::new(),
                scripted: false,
                content_source: "test".into(),
                source_confidence: None,
                status: "classified".into(),
                classification_confidence: "high".into(),
                creator: None,
                mod_name: None,
                gameplay_category: None,
                detected_from: Vec::new(),
                category: Some("CAS".into()),
                sub_category: Some("Hair".into()),
                gender: None,
                age: None,
                species: None,
                usage_categories: Vec::new(),
                destination_parts,
                destination_path: None,
                candidate_destinations: Vec::new(),
                classifications: Vec::new(),
                classification_reason: Some("synthetic regression".into()),
                warnings: Vec::new(),
                project_id: None,
                project_role: None,
                project_confidence: None,
                project_evidence: crate::project_grouping::ProjectTechnicalEvidence::default(),
                project_relationships: Vec::new(),
                reference_evidence: None,
                profile: None,
            },
        )
    }

    #[test]
    fn nested_exact_duplicates_prefer_full_canonical_relative_path() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-nested-keeper-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let packages_root = root.join("Packages");
        std::fs::create_dir_all(&packages_root).unwrap();
        std::fs::create_dir_all(root.join("Overrides")).unwrap();
        let canonical_relative = "Sliders/shared.package";
        let legacy_relative = "Danny/Sliders/shared.package";
        let second_legacy_relative = "Brian Vicente/Brian Vicente/Sliders/shared.package";
        // Simulate stale path-based classification, including the canonical file.
        // The manual model hash must win over these scanner-suggested folders.
        let (canonical_path, canonical_item) =
            synthetic_scan_item(&packages_root, canonical_relative, &["#9 Sliders"]);
        let (legacy_path, legacy_item) =
            synthetic_scan_item(&packages_root, legacy_relative, &["Danny", "Sliders"]);
        let (second_legacy_path, second_legacy_item) =
            synthetic_scan_item(&packages_root, second_legacy_relative, &["Sliders"]);
        let mut selected_hashes = HashMap::new();
        selected_hashes.insert(canonical_path.clone(), "SAME".into());
        selected_hashes.insert(legacy_path.clone(), "SAME".into());
        selected_hashes.insert(second_legacy_path.clone(), "SAME".into());
        let mut scan = HashMap::new();
        scan.insert(canonical_path.clone(), &canonical_item);
        scan.insert(legacy_path.clone(), &legacy_item);
        scan.insert(second_legacy_path.clone(), &second_legacy_item);
        let mut directories = BTreeMap::new();
        directories.insert(
            model_key(&["Sliders".to_string()]),
            vec!["Sliders".to_string()],
        );
        let mut files_by_sha256 = HashMap::new();
        files_by_sha256.insert(
            "SAME".to_string(),
            vec![ManualModelFile {
                relative_path: "Sliders\\shared.package".to_string(),
                directory_parts: vec!["Sliders".to_string()],
                file_name: "shared.package".to_string(),
            }],
        );
        let manual_model = ManualModelIndex {
            directories,
            files_by_sha256,
        };

        assert!(is_selected_content_keeper_for_scan(
            &packages_root,
            &canonical_path,
            "SAME",
            &selected_hashes,
            &[],
            &scan,
            &manual_model,
            None
        ));
        assert!(!is_selected_content_keeper_for_scan(
            &packages_root,
            &legacy_path,
            "SAME",
            &selected_hashes,
            &[],
            &scan,
            &manual_model,
            None
        ));
        assert!(!is_selected_content_keeper_for_scan(
            &packages_root,
            &second_legacy_path,
            "SAME",
            &selected_hashes,
            &[],
            &scan,
            &manual_model,
            None
        ));
        assert_eq!(
            selected_content_keeper_relative_path(
                &packages_root,
                &legacy_path,
                "SAME",
                &selected_hashes,
                &[],
                &scan,
                &manual_model,
                None,
            ),
            Some(relative_key(Path::new(canonical_relative)))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planned_sources_use_the_same_canonical_root_as_cleanup() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "s3cc-canonical-root-{}-{nonce}",
            std::process::id()
        ));
        let source = root.join("Packages").join("Legacy").join("a.package");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"DBPF source identity test").unwrap();
        let canonical_root = root.canonicalize().unwrap();
        let canonical_source = source.canonicalize().unwrap();
        assert!(canonical_source.starts_with(&canonical_root));
        assert!(canonical_source
            .parent()
            .unwrap()
            .starts_with(&canonical_root));
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn cfg_with_rules(root: &Path, patterns: &[&str]) -> ResourceCfgContext {
        use crate::resource_cfg::ResourceCfgRule;
        ResourceCfgContext {
            directory: root.to_path_buf(),
            info: ResourceCfgInfo {
                path: root.join("Resource.cfg").to_string_lossy().to_string(),
                precedence_reliable: true,
                warnings: Vec::new(),
                rules: patterns
                    .iter()
                    .enumerate()
                    .map(|(index, rule)| ResourceCfgRule {
                        priority: 500,
                        pattern: (*rule).to_string(),
                        source_line: index + 1,
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn overrides_support_anatomical_folders_when_resource_cfg_matches() {
        let mods = Path::new("The Sims 3").join("Mods");
        let source = mods.join("Overrides").join("patch.package");
        let context = cfg_with_rules(
            &mods,
            &[
                "Packages/*.package",
                "Overrides/*.package",
                "Overrides/*/*/*/*.package",
            ],
        );
        let category = vec!["Sliders".into(), "Face".into(), "Nose".into()];
        let (destination, _) = fit_destination_to_resource_cfg(
            &mods,
            &source,
            std::ffi::OsStr::new("patch.package"),
            &category,
            Some(&context),
        )
        .unwrap();
        assert_eq!(destination, vec!["Overrides", "Sliders", "Face", "Nose"]);

        let packages_source = mods.join("Packages").join("hair.package");
        assert!(!source_uses_overrides(&mods, &packages_source));
        assert!(source_uses_overrides(&mods, &source));
    }

    #[test]
    fn overrides_flat_resource_cfg_keeps_physical_file_at_override_root() {
        let mods = Path::new("The Sims 3").join("Mods");
        let overrides = mods.join("Overrides");
        let context = cfg_with_rules(&mods, &["Overrides/*.package"]);
        let cat = vec!["Sliders".into(), "Face".into(), "Nose".into()];
        let (destination, note) = fit_destination_to_resource_cfg(
            &mods,
            &overrides.join("patch.package"),
            std::ffi::OsStr::new("patch.package"),
            &cat,
            Some(&context),
        )
        .unwrap();
        assert_eq!(destination, vec!["Overrides"]);
        assert!(note.unwrap().contains("category is retained"));

        let (selected_override, note) = fit_destination_to_resource_cfg(
            &overrides,
            &overrides.join("patch.package"),
            std::ffi::OsStr::new("patch.package"),
            &cat,
            Some(&context),
        )
        .unwrap();
        assert!(selected_override.is_empty());
        assert!(note.unwrap().contains("category is retained"));
    }

    #[test]
    fn missing_override_rule_never_redirects_to_packages() {
        let mods = Path::new("The Sims 3").join("Mods");
        let override_file = mods.join("Overrides").join("ui.package");
        let context = cfg_with_rules(&mods, &["Packages/*.package", "Packages/*/*.package"]);
        let category = vec!["Gameplay".into(), "Tuning".into()];
        assert!(fit_destination_to_resource_cfg(
            &mods,
            &override_file,
            std::ffi::OsStr::new("ui.package"),
            &category,
            Some(&context),
        )
        .is_err());
        assert!(fit_destination_to_resource_cfg(
            &mods,
            &override_file,
            std::ffi::OsStr::new("ui.package"),
            &category,
            None,
        )
        .is_err());
        let (fallback, dirs) =
            fallback_relative_path(&mods, AppLanguage::En, "Overrides/UI/ui.package", "ABCD")
                .unwrap();
        assert_eq!(fallback, Path::new("Overrides/UI/ui.package"));
        assert!(dirs.is_empty());
    }

    #[test]
    fn selected_overrides_root_preserves_original_loading_branch() {
        let mods = Path::new("The Sims 3").join("Mods");
        let root = mods.join("Overrides");
        assert!(is_overrides_root(&root));
        assert!(source_uses_overrides(&root, &root.join("file.package")));
        assert_eq!(
            ensure_source_loading_branch(
                &root,
                &root.join("file.package"),
                &["Packages".into(), "CAS".into(), "Sliders".into()],
            ),
            vec!["Sliders"]
        );
        assert_eq!(
            ensure_source_loading_branch(
                &mods,
                &mods.join("Overrides").join("file.package"),
                &["Packages".into(), "Sliders".into()],
            ),
            vec!["Overrides", "Sliders"]
        );
    }

    #[test]
    fn nested_packages_and_overrides_are_valid_organization_roots() {
        let mods = Path::new("The Sims 3").join("Mods");
        let deep_packages = mods.join("Packages").join("Clothing").join("Male");
        let deep_overrides = mods.join("Overrides").join("Gameplay").join("Tuning");
        assert!(is_within_packages(&deep_packages));
        assert!(!is_within_overrides(&deep_packages));
        assert!(is_within_overrides(&deep_overrides));
        assert!(source_uses_overrides(
            &deep_overrides,
            &deep_overrides.join("ui.package")
        ));
        assert_eq!(
            ensure_source_loading_branch(
                &deep_packages,
                &deep_packages.join("file.package"),
                &[
                    "Packages".into(),
                    "CAS".into(),
                    "Clothing".into(),
                    "Male".into(),
                    "YA-A".into(),
                    "Top".into()
                ],
            ),
            vec!["YA-A", "Top"]
        );
        assert_eq!(
            ensure_source_loading_branch(
                &deep_overrides,
                &deep_overrides.join("ui.package"),
                &[
                    "Overrides".into(),
                    "Gameplay".into(),
                    "Tuning".into(),
                    "Scripts".into()
                ],
            ),
            vec!["Scripts"]
        );
        let staging = Path::new("Downloads").join("CC Incoming");
        assert!(!is_within_overrides(&staging));
        assert_eq!(
            ensure_source_loading_branch(
                &staging,
                &staging.join("file.package"),
                &["Clothing".into()]
            ),
            vec!["Clothing"]
        );
    }

    #[test]
    fn identifies_only_known_legacy_manager_folders_for_migration() {
        let mods = Path::new("The Sims 3").join("Mods");
        assert!(legacy_manager_source(
            &mods,
            &mods.join("CAS").join("Sliders").join("a.package")
        ));
        assert!(legacy_manager_source(
            &mods,
            &mods.join("Roupas").join("Masculino").join("x.package")
        ));
        assert!(!legacy_manager_source(
            &mods,
            &mods.join("Overrides").join("ui.package")
        ));
        assert!(!legacy_manager_source(
            &mods,
            &mods.join("DCCache").join("a.package")
        ));
        assert!(!legacy_manager_source(
            &mods,
            &mods.join("Random Folder").join("x.package")
        ));
    }

    #[test]
    fn mods_root_always_routes_categories_into_packages() {
        let mods_base = Path::new("The Sims 3").join("Mods");
        let mods = mods_base.as_path();
        let categories = vec!["CAS".into(), "Clothing".into(), "Female".into()];
        assert_eq!(
            ensure_packages_destination(mods, &categories),
            vec!["Packages", "Clothing", "Female"]
        );
        assert_eq!(
            ensure_packages_destination(mods, &["Packages".into(), "CAS".into()]),
            vec!["Packages"]
        );
    }

    #[test]
    fn already_selected_packages_root_does_not_duplicate_folder_name() {
        let packages_base = Path::new("The Sims 3").join("Mods").join("Packages");
        let packages = packages_base.as_path();
        let categories = vec!["CAS".into(), "Hair".into()];
        assert_eq!(
            ensure_packages_destination(packages, &categories),
            vec!["Hair"]
        );
        assert_eq!(
            ensure_packages_destination(
                packages,
                &["Packages".into(), "CAS".into(), "Hair".into()]
            ),
            vec!["Hair"]
        );
    }

    #[test]
    fn uncovered_resource_cfg_source_still_keeps_packages_prefix() {
        let mods_base = Path::new("The Sims 3").join("Mods");
        let root = mods_base.as_path();
        let parts = vec!["Scripts".to_string(), "Gameplay".to_string()];
        let (resolved, _) = fit_destination_to_resource_cfg(
            root,
            &root.join("Packages/Unmatched.package"),
            std::ffi::OsStr::new("Unmatched.package"),
            &parts,
            None,
        )
        .unwrap();
        assert_eq!(resolved, vec!["Packages", "Scripts", "Gameplay"]);
    }

    #[test]
    fn resource_cfg_compaction_preserves_packages_directory() {
        use crate::resource_cfg::ResourceCfgRule;

        let root = Path::new("Temporary").join("Mods");
        let context = ResourceCfgContext {
            directory: root.clone(),
            info: ResourceCfgInfo {
                path: root.join("Resource.cfg").to_string_lossy().to_string(),
                precedence_reliable: true,
                warnings: vec![],
                rules: vec![
                    ResourceCfgRule {
                        priority: 500,
                        pattern: "Packages/*.package".into(),
                        source_line: 1,
                    },
                    ResourceCfgRule {
                        priority: 500,
                        pattern: "Packages/*/*.package".into(),
                        source_line: 2,
                    },
                ],
            },
        };
        let parts = vec![
            "Clothing".into(),
            "Female".into(),
            "YA-A".into(),
            "Top".into(),
        ];
        let (fitted, _) = fit_destination_to_resource_cfg(
            &root,
            &root.join("Packages").join("source.package"),
            std::ffi::OsStr::new("new.package"),
            &parts,
            Some(&context),
        )
        .unwrap();
        assert_eq!(fitted.len(), 2);
        assert_eq!(fitted[0], "Packages");
        assert!(fitted[1].starts_with("Clothing"));
    }

    #[test]
    fn four_cas_folders_fit_inside_packages_with_matching_resource_cfg() {
        use crate::resource_cfg::ResourceCfgRule;

        let root = Path::new("Temporary").join("Mods");
        let context = ResourceCfgContext {
            directory: root.clone(),
            info: ResourceCfgInfo {
                path: root.join("Resource.cfg").to_string_lossy().to_string(),
                precedence_reliable: true,
                warnings: vec![],
                rules: vec![
                    ResourceCfgRule {
                        priority: 500,
                        pattern: "Packages/*.package".into(),
                        source_line: 1,
                    },
                    ResourceCfgRule {
                        priority: 500,
                        pattern: "Packages/*/*/*/*/*.package".into(),
                        source_line: 2,
                    },
                ],
            },
        };
        let parts = vec![
            "Clothing".into(),
            "Female".into(),
            "YA-A".into(),
            "Top".into(),
        ];
        let (fitted, _) = fit_destination_to_resource_cfg(
            &root,
            &root.join("Packages").join("source.package"),
            std::ffi::OsStr::new("new.package"),
            &parts,
            Some(&context),
        )
        .unwrap();
        assert_eq!(
            fitted,
            vec!["Packages", "Clothing", "Female", "YA-A", "Top"]
        );
    }

    #[test]
    fn destination_components_reject_path_escape_and_windows_invalid_names() {
        assert!(validate_destination_parts(&["CAS".into(), "Roupas".into()]).is_ok());
        assert!(validate_destination_parts(&["..".into()]).is_err());
        assert!(validate_destination_parts(&["Bad/Folder".into()]).is_err());
        assert!(validate_destination_parts(&["CON".into()]).is_err());
        assert!(validate_destination_parts(&["Name.".into()]).is_err());
    }

    #[test]
    fn exact_duplicate_index_finds_identical_contents_across_folders() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "s3cc-duplicate-preflight-{}-{nonce}",
            std::process::id()
        ));
        let packages = root.join("Packages");
        std::fs::create_dir_all(packages.join("Sliders")).unwrap();
        let first = packages.join("Jonha_BASE.package");
        let second = packages.join("Sliders").join("Jonha_Sliders_BASE.package");
        std::fs::write(&first, b"identical Jonha STBL resources").unwrap();
        std::fs::write(&second, b"identical Jonha STBL resources").unwrap();

        let mut selected = HashMap::new();
        let (hash, _) = sha256_file(&first).unwrap();
        selected.insert(first.canonicalize().unwrap(), hash.clone());
        let scan_items = vec![&first, &second]
            .into_iter()
            .map(|path| ScanPackageItem {
                id: path.to_string_lossy().to_string(),
                name: path.file_name().unwrap().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                relative_path: path
                    .strip_prefix(&packages)
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                file_size: std::fs::metadata(path).unwrap().len(),
                resource_count: 0,
                catalog_resource_count: 0,
                resource_types: Vec::new(),
                instances: Vec::new(),
                scripted: false,
                content_source: "unknown".into(),
                source_confidence: None,
                status: "classified".into(),
                classification_confidence: "high".into(),
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
                warnings: Vec::new(),
                project_id: None,
                project_role: None,
                project_confidence: None,
                project_evidence: crate::project_grouping::ProjectTechnicalEvidence::default(),
                project_relationships: Vec::new(),
                reference_evidence: None,
                profile: None,
            })
            .collect::<Vec<_>>();
        let groups = index_exact_duplicates(&packages, &scan_items, &selected);
        assert_eq!(groups[&hash.to_ascii_uppercase()].len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn partial_merged_morph_resources_hold_only_the_larger_package() {
        let keys = |items: &[&str]| {
            items
                .iter()
                .map(|key| key.to_string())
                .collect::<BTreeSet<_>>()
        };
        let candidates = vec![
            (
                "single-slider.package".into(),
                keys(&["FACE-a", "BGEO-a"]),
                true,
            ),
            (
                "merged-sliders.package".into(),
                keys(&["FACE-a", "BGEO-a", "FACE-b"]),
                true,
            ),
            (
                "related-translation.package".into(),
                keys(&["STBL-a"]),
                false,
            ),
            (
                "unrelated.package".into(),
                keys(&["FACE-x", "BGEO-x", "FACE-y"]),
                true,
            ),
        ];
        let related = detect_merged_resource_supersets(&candidates);
        assert_eq!(
            related["merged-sliders.package"],
            vec!["single-slider.package"]
        );
        assert!(!related.contains_key("single-slider.package"));
        assert!(!related.contains_key("unrelated.package"));
    }

    #[test]
    fn planned_destination_collision_is_detected_before_execution() {
        let mut items = vec![
            PlanItem {
                id: "1".into(),
                name: "same.package".into(),
                source_path: r"C:\\Mods\\A\\same.package".into(),
                source_relative_path: r"A\\same.package".into(),
                destination_path: Some(r"C:\\Mods\\CAS\\Sliders\\same.package".into()),
                destination_relative_path: Some(r"CAS\\Sliders\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test".into()),
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 10,
                warnings: vec![],
            },
            PlanItem {
                id: "2".into(),
                name: "same.package".into(),
                source_path: r"C:\\Mods\\B\\same.package".into(),
                source_relative_path: r"B\\same.package".into(),
                destination_path: Some(r"C:\\Mods\\CAS\\Sliders\\same.package".into()),
                destination_relative_path: Some(r"CAS\\Sliders\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test".into()),
                plan_status: "ready".into(),
                sha256: Some("B".repeat(64)),
                size: 10,
                warnings: vec![],
            },
        ];
        let mut stats = PlanStats::default();

        let mut directories = BTreeSet::new();
        let mut reserved = HashSet::new();
        mark_intra_plan_destination_collisions(
            Path::new(r"C:\\Mods"),
            AppLanguage::En,
            &mut items,
            &mut directories,
            &mut reserved,
            &mut stats,
        )
        .unwrap();

        assert_eq!(stats.collision_different_content, 2);
        assert!(items
            .iter()
            .all(|item| item.plan_status == "collision_different_content"));
    }

    #[test]
    fn identical_planned_destination_keeps_one_ready_and_skips_extra_copy() {
        let hash = "A".repeat(64);
        let mut items = vec![
            PlanItem {
                id: "1".into(),
                name: "same.package".into(),
                source_path: r"C:\\Mods\\A\\same.package".into(),
                source_relative_path: r"A\\same.package".into(),
                destination_path: Some(r"C:\\Mods\\CAS\\same.package".into()),
                destination_relative_path: Some(r"CAS\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test".into()),
                plan_status: "ready".into(),
                sha256: Some(hash.clone()),
                size: 10,
                warnings: vec![],
            },
            PlanItem {
                id: "2".into(),
                name: "same.package".into(),
                source_path: r"C:\\Mods\\B\\same.package".into(),
                source_relative_path: r"B\\same.package".into(),
                destination_path: Some(r"C:\\Mods\\CAS\\same.package".into()),
                destination_relative_path: Some(r"CAS\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: Some("test".into()),
                plan_status: "ready".into(),
                sha256: Some(hash),
                size: 10,
                warnings: vec![],
            },
        ];
        let mut stats = PlanStats::default();

        let mut directories = BTreeSet::new();
        let mut reserved = HashSet::new();
        mark_intra_plan_destination_collisions(
            Path::new(r"C:\\Mods"),
            AppLanguage::En,
            &mut items,
            &mut directories,
            &mut reserved,
            &mut stats,
        )
        .unwrap();

        assert_eq!(items[0].plan_status, "ready");
        assert_eq!(items[1].plan_status, "ready_duplicate_quarantine");
        assert_eq!(stats.duplicate_quarantine_ready, 1);
        assert_eq!(stats.collision_same_content, 0);
        assert_eq!(stats.collision_different_content, 0);
    }

    #[test]
    fn normalized_plan_paths_match_verbatim_case_and_relative_variants() {
        let root = Path::new(r"C:\Mods");
        let canonical =
            normalized_path_key(root, Path::new(r"Packages\#Sem Categoria\item.package"));
        let verbatim = normalized_path_key(
            root,
            Path::new(r"\\?\c:\mods\.\Packages\#SEM CATEGORIA\item.package"),
        );
        assert_eq!(canonical, verbatim);
    }

    #[test]
    fn planned_source_destination_overlap_is_blocked_before_execution() {
        let root = Path::new(r"C:\Mods");
        let mut items = vec![
            PlanItem {
                id: "a".into(),
                name: "a.package".into(),
                source_path: r"C:\Mods\Legacy\a.package".into(),
                source_relative_path: r"Legacy\a.package".into(),
                destination_path: Some(r"C:\Mods\Packages\a.package".into()),
                destination_relative_path: Some(r"Packages\a.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 1,
                warnings: vec![],
            },
            PlanItem {
                id: "b".into(),
                name: "b.package".into(),
                source_path: r"C:\Mods\Packages\a.package".into(),
                source_relative_path: r"Packages\a.package".into(),
                destination_path: Some(r"C:\Mods\Canonical\b.package".into()),
                destination_relative_path: Some(r"Canonical\b.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("B".repeat(64)),
                size: 1,
                warnings: vec![],
            },
        ];

        mark_intra_plan_source_destination_collisions(root, &mut items);
        assert!(items
            .iter()
            .all(|item| item.plan_status == "blocked_intra_plan_path_collision"));
    }

    #[test]
    fn resource_cfg_preview_uses_only_active_loading_branch_packages() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-resource-cfg-plan-filter-{}-{nonce}",
            std::process::id()
        ));
        let mods = base.join("Mods");
        let packages = mods.join("Packages");
        fs::create_dir_all(&packages).unwrap();
        fs::create_dir_all(mods.join("Overrides")).unwrap();

        let item = |name: &str, destination: &Path, status: &str| PlanItem {
            id: name.to_string(),
            name: name.to_string(),
            source_path: packages.join(name).display().to_string(),
            source_relative_path: name.to_string(),
            destination_path: Some(destination.display().to_string()),
            destination_relative_path: Some(
                destination
                    .strip_prefix(&mods)
                    .map(|relative| relative.display().to_string())
                    .unwrap_or_else(|_| destination.display().to_string()),
            ),
            classification_status: "classified".to_string(),
            classification_reason: None,
            plan_status: status.to_string(),
            sha256: None,
            size: 1,
            warnings: Vec::new(),
        };

        let plan = OrganizationPlan {
            root: packages.display().to_string(),
            items: vec![
                item(
                    "active.package",
                    &packages.join("Sliders").join("active.package"),
                    "ready",
                ),
                item(
                    "override.package",
                    &mods
                        .join("Overrides")
                        .join("Fixes")
                        .join("override.package"),
                    "ready",
                ),
                item(
                    "disabled.package.disabled",
                    &packages.join("disabled.package.disabled"),
                    "ready",
                ),
                item(
                    "preview.png",
                    &packages.join("Previews de CCs").join("preview.png"),
                    "ready",
                ),
                item(
                    "external.package",
                    &base.join("Other").join("external.package"),
                    "ready",
                ),
                item(
                    "blocked.package",
                    &packages.join("blocked.package"),
                    "blocked_collision",
                ),
            ],
            stats: PlanStats::default(),
            directories_to_create: Vec::new(),
            protected_empty_directories: Vec::new(),
            cleanup_roots: Vec::new(),
            manifest_preview: String::new(),
            resource_cfg_update: None,
            can_execute: true,
            analysis_before: OrganizationAnalysisSnapshot::from_scan("test", &ScanStats::default()),
        };

        let destinations = resource_cfg_destinations_for_plan(&plan);
        assert_eq!(destinations.len(), 2);
        assert!(destinations
            .iter()
            .any(|path| path.ends_with(Path::new("Sliders").join("active.package"))));
        assert!(destinations
            .iter()
            .any(|path| path.ends_with(Path::new("Fixes").join("override.package"))));
        assert!(!destinations
            .iter()
            .any(|path| path.to_string_lossy().contains("disabled")));
        assert!(!destinations
            .iter()
            .any(|path| path.to_string_lossy().contains("preview")));

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn skippable_collisions_do_not_block_ready_organization() {
        let stats = PlanStats {
            selected: 1356,
            ready: 1060,
            duplicate_skipped: 171,
            collision_different_content: 125,
            blocked: 0,
            ..PlanStats::default()
        };

        assert!(plan_can_execute(&stats, false));
    }

    #[test]
    fn unresolved_packages_can_stay_in_place_while_safe_moves_execute() {
        let stats = PlanStats {
            selected: 2,
            kept_uncategorized: 1,
            ready: 1,
            blocked: 0,
            ..PlanStats::default()
        };

        assert!(plan_can_execute(&stats, false));
    }

    #[test]
    fn manual_review_items_are_protected_from_automatic_fallback_moves() {
        assert!(is_manual_review_status("needs_review"));
        assert!(is_manual_review_status("mixed"));
        assert!(!is_manual_review_status("unknown"));
        assert!(!is_manual_review_status("classified"));
    }

    #[test]
    fn blocked_items_are_skipped_without_blocking_independent_ready_moves() {
        let stats = PlanStats {
            selected: 2,
            ready: 1,
            blocked: 1,
            ..PlanStats::default()
        };

        assert!(plan_can_execute(&stats, false));
        assert!(!plan_can_execute(
            &PlanStats {
                ready: 0,
                blocked: 1,
                ..PlanStats::default()
            },
            false
        ));
        assert!(plan_can_execute(
            &PlanStats {
                ready: 0,
                empty_folders_to_clean: 2,
                ..PlanStats::default()
            },
            false
        ));
        assert!(!plan_can_execute(
            &PlanStats {
                ready: 0,
                empty_folders_to_clean: 2,
                ..PlanStats::default()
            },
            true
        ));
        assert!(!plan_can_execute(
            &PlanStats {
                ready: 1,
                ..PlanStats::default()
            },
            true
        ));
    }

    #[test]
    fn manifest_preview_contains_original_and_organized_paths() {
        let item = PlanItem {
            id: "1".into(),
            name: "x.package".into(),
            source_path: "C:\\Mods\\Old\\x.package".into(),
            source_relative_path: "Old\\x.package".into(),
            destination_path: Some("C:\\Mods\\CAS\\Hair\\x.package".into()),
            destination_relative_path: Some("CAS\\Hair\\x.package".into()),
            classification_status: "classified".into(),
            classification_reason: Some("CASP clothingType=0x00000005 => CAS\\Hair".into()),
            plan_status: "ready".into(),
            sha256: Some("ABC".into()),
            size: 123,
            warnings: vec![],
        };

        let text = manifest_preview(Path::new("C:\\Mods"), AppLanguage::Pt, &[item]);
        assert!(text.contains("mode=PREVIEW"));
        assert!(text.contains("organization_language=pt"));
        assert!(text.contains("original=Old\\x.package"));
        assert!(text.contains("organized=CAS\\Hair\\x.package"));
    }

    #[test]
    fn manual_model_rejects_partial_category_names_and_resolves_exact_hash_paths() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "s3cc-exact-model-destination-{}-{nonce}",
            std::process::id()
        ));
        let model = base.join("Packages");
        let selected = base.join("Copy").join("Packages");
        std::fs::create_dir_all(model.join("Sliders")).unwrap();
        std::fs::create_dir_all(model.join("Roupas").join("Feminino")).unwrap();
        std::fs::create_dir_all(selected.join("#9 Sliders")).unwrap();
        std::fs::create_dir_all(selected.join("#6 Slider")).unwrap();
        std::fs::create_dir_all(selected.join("Criador").join("Sliders")).unwrap();
        let canonical = model.join("Sliders").join("slider.package");
        let misplaced = selected.join("#9 Sliders").join("slider.package");
        let misplaced_six = selected.join("#6 Slider").join("slider.package");
        std::fs::write(&canonical, b"real slider bytes").unwrap();
        std::fs::write(&misplaced, b"real slider bytes").unwrap();
        std::fs::copy(&canonical, &misplaced_six).unwrap();
        let index = build_manual_model_index(&model).unwrap();
        let (hash, _) = sha256_file(&misplaced).unwrap();
        let resolved = resolve_model_destination(
            &index,
            &selected,
            &misplaced,
            &hash,
            &["#9 Sliders".to_string()],
        )
        .unwrap();
        assert_eq!(resolved.parts, vec!["Sliders"]);
        assert_ne!(
            relative_key(Path::new("#9 Sliders\\slider.package")),
            format!("{}\\slider.package", resolved.parts.join("\\"))
        );
        let six_resolved = resolve_model_destination(
            &index,
            &selected,
            &misplaced_six,
            &hash,
            &["#6 Slider".to_string()],
        )
        .unwrap();
        assert_eq!(six_resolved.parts, vec!["Sliders"]);
        assert!(!exact_model_relative_path_matches(
            "#6 Slider\\slider.package",
            "Sliders\\slider.package"
        ));
        assert!(exact_model_relative_path_matches(
            "Sliders\\slider.package",
            "Sliders\\slider.package"
        ));
        let wrong = resolve_model_destination(
            &index,
            &selected,
            &selected.join("Criador").join("Sliders").join("new.package"),
            "NOT-IN-MODEL",
            &["Criador".to_string(), "Sliders".to_string()],
        );
        assert!(wrong.is_err());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn exact_model_hash_never_keeps_legacy_taxonomy_directory_as_canonical() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
            "s3cc-legacy-model-reconciliation-{}-{nonce}",
            std::process::id()
        ));
        let model = base.join("Model");
        let selected = base.join("Selected").join("Packages");
        let cases = [
            (
                vec!["Cabelos", "Masculino", "Adolescente-Jovem Adulto-Adulto-Idoso"],
                vec![
                    "Cabelos e Pêlos",
                    "Cabelos",
                    "Masculino",
                    "Adolescente a Idoso",
                ],
                "hair.package",
            ),
            (
                vec!["Nraas Mods", "MasterController"],
                vec!["Scripts", "NRaas", "MasterController"],
                "nraas.package",
            ),
            (
                vec!["Fixes"],
                vec!["Correções"],
                "fix.package",
            ),
            (vec!["Sem Categoria"], vec!["#Sem Categoria"], "review.package"),
        ];
        for (legacy, canonical, name) in &cases {
            std::fs::create_dir_all(model.join(legacy.iter().collect::<PathBuf>())).unwrap();
            std::fs::create_dir_all(model.join(canonical.iter().collect::<PathBuf>())).unwrap();
            let model_file = model.join(legacy.iter().collect::<PathBuf>()).join(name);
            std::fs::write(&model_file, format!("bytes for {name}")).unwrap();
            let source = selected.join("Legacy").join(name);
            std::fs::create_dir_all(source.parent().unwrap()).unwrap();
            std::fs::copy(&model_file, &source).unwrap();
        }

        let index = build_manual_model_index(&model).unwrap();
        for (legacy, canonical, name) in &cases {
            let source = selected.join("Legacy").join(name);
            let (hash, _) = sha256_file(&source).unwrap();
            let result = resolve_model_destination(
                &index,
                &selected,
                &source,
                &hash,
                &legacy.iter().map(|part| (*part).to_string()).collect::<Vec<_>>(),
            )
            .unwrap();
            assert_eq!(result.parts, canonical.iter().map(|part| (*part).to_string()).collect::<Vec<_>>());
            assert!(!result.parts.starts_with(&legacy.iter().map(|part| (*part).to_string()).collect::<Vec<_>>()));
        }
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn model_aliases_are_component_exact_and_do_not_accept_intermediate_folders() {
        assert!(model_components_match("Iluminador", "Iluminadores"));
        assert!(model_components_match("Gameplay", "Jogabilidade"));
        assert!(!exact_model_relative_path_matches(
            "#6 Sliders\\hat.package",
            "Sliders\\hat.package"
        ));
        assert!(!exact_model_relative_path_matches(
            "Creator\\Sliders\\hat.package",
            "Sliders\\hat.package"
        ));
        assert!(exact_model_relative_path_matches(
            "Sliders\\hat.package",
            "Sliders\\hat.package"
        ));
    }

    #[test]
    fn manual_model_blocks_lookalike_without_matching_hash_or_exact_category() {
        let mut directories = BTreeMap::new();
        directories.insert(
            model_key(&["Sliders".to_string()]),
            vec!["Sliders".to_string()],
        );
        let index = ManualModelIndex {
            directories,
            files_by_sha256: HashMap::new(),
        };
        let root = Path::new("Mods").join("Packages");
        let source = root.join("#9 Sliders").join("unrecognized.package");
        let result = resolve_model_destination(
            &index,
            &root,
            &source,
            "MISSING-HASH",
            &["#9 Sliders".to_string()],
        );
        assert!(result
            .unwrap_err()
            .contains("No authorized taxonomy directory"));
    }

    #[test]
    fn manual_model_resolves_language_and_age_taxonomy_aliases_without_a_matching_file() {
        let mut directories = BTreeMap::new();
        directories.insert(
            model_key(&["Scripts".to_string(), "Gameplay".to_string()]),
            vec!["Scripts".to_string(), "Gameplay".to_string()],
        );
        directories.insert(
            model_key(&[
                "Cabelos".to_string(),
                "Masculino".to_string(),
                "Adolescente-Jovem Adulto-Adulto-Idoso".to_string(),
                "Pelos Corporais".to_string(),
            ]),
            vec![
                "Cabelos".to_string(),
                "Masculino".to_string(),
                "Adolescente-Jovem Adulto-Adulto-Idoso".to_string(),
                "Pelos Corporais".to_string(),
            ],
        );
        let index = ManualModelIndex {
            directories,
            files_by_sha256: HashMap::new(),
        };
        let root = Path::new("Mods").join("Packages");
        let source = root.join("Legacy").join("script.package");
        let gameplay = resolve_model_destination(
            &index,
            &root,
            &source,
            "MISSING-HASH",
            &["Scripts".into(), "Jogabilidade".into()],
        )
        .unwrap();
        assert_eq!(gameplay.parts, vec!["Scripts", "Gameplay"]);
        let body = resolve_model_destination(
            &index,
            &root,
            &source,
            "MISSING-HASH",
            &[
                "Cabelos".into(),
                "Masculino".into(),
                "Pelos Corporais".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            body.parts,
            vec![
                "Cabelos",
                "Masculino",
                "Adolescente-Jovem Adulto-Adulto-Idoso",
                "Pelos Corporais"
            ]
        );
    }

    #[test]
    fn manual_model_canonicalizes_unique_makeup_age_bucket_without_inventing_a_folder() {
        let mut directories = BTreeMap::new();
        directories.insert(
            model_key(&[
                "Maquiagem".into(),
                "Unissex".into(),
                "Jovem Adulto-Adulto".into(),
                "Blush".into(),
            ]),
            vec![
                "Maquiagem".into(),
                "Unissex".into(),
                "Jovem Adulto-Adulto".into(),
                "Blush".into(),
            ],
        );
        directories.insert(
            model_key(&[
                "Maquiagem".into(),
                "Unissex".into(),
                "Múltiplas Idades".into(),
                "Sombra".into(),
            ]),
            vec![
                "Maquiagem".into(),
                "Unissex".into(),
                "Múltiplas Idades".into(),
                "Sombra".into(),
            ],
        );
        let index = ManualModelIndex {
            directories,
            files_by_sha256: HashMap::new(),
        };
        let root = Path::new("Mods").join("Packages");
        let source = root.join("Jonas").join("makeup.package");
        let blush = resolve_model_destination(
            &index,
            &root,
            &source,
            "MISSING-BLUSH",
            &[
                "Maquiagem".into(),
                "Unissex".into(),
                "Adolescente a Idoso".into(),
                "Blush".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            blush.parts,
            vec!["Maquiagem", "Unissex", "Jovem Adulto-Adulto", "Blush"]
        );
        let shadow = resolve_model_destination(
            &index,
            &root,
            &source,
            "MISSING-SHADOW",
            &[
                "Maquiagem".into(),
                "Unissex".into(),
                "Adolescente a Idoso".into(),
                "Sombra".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            shadow.parts,
            vec!["Maquiagem", "Unissex", "Múltiplas Idades", "Sombra"]
        );
    }

    #[test]
    fn manual_model_keeps_same_hash_in_distinct_canonical_folders_ambiguous() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // This test renames a model fixture; place it in the workspace so the
        // test exercises the planner rather than a sandbox TEMP restriction.
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!(
            "s3cc-ambiguous-model-destination-{}-{nonce}",
            std::process::id()
        ));
        let model = base.join("Packages");
        std::fs::create_dir_all(model.join("A")).unwrap();
        std::fs::create_dir_all(model.join("B")).unwrap();
        let bytes = b"same bytes in two model destinations";
        std::fs::write(model.join("A").join("same.package"), bytes).unwrap();
        std::fs::write(model.join("B").join("same.package"), bytes).unwrap();
        let source = base.join("source").join("same.package");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, bytes).unwrap();
        let index = build_manual_model_index(&model).unwrap();
        let (hash, _) = sha256_file(&source).unwrap();
        let result = resolve_model_destination(&index, &base.join("source"), &source, &hash, &[]);
        assert!(result
            .unwrap_err()
            .contains("multiple different canonical model destinations"));
        // Even an apparently matching scanner category must not override a
        // conflicting destination explicitly present in the manual model.
        let preferred = resolve_model_destination(
            &index,
            &base.join("source"),
            &source,
            &hash,
            &["A".to_string()],
        );
        assert!(preferred
            .unwrap_err()
            .contains("multiple different canonical model destinations"));
        // A different filename must not conceal that same-content packages
        // exist in two distinct model destinations.
        std::fs::rename(
            model.join("B").join("same.package"),
            model.join("B").join("renamed.package"),
        )
        .unwrap();
        let differently_named_index = build_manual_model_index(&model).unwrap();
        let differently_named = resolve_model_destination(
            &differently_named_index,
            &base.join("source"),
            &source,
            &hash,
            &["A".to_string()],
        );
        assert!(differently_named
            .unwrap_err()
            .contains("multiple different canonical model destinations"));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ambiguous_model_duplicate_group_has_no_arbitrary_keeper() {
        let root = Path::new("Mods").join("Packages");
        let (first_path, first_item) =
            synthetic_scan_item(&root, "Legacy\\same.package", &["Sliders"]);
        let (second_path, second_item) =
            synthetic_scan_item(&root, "Other\\same.package", &["Sliders"]);
        let mut selected_hashes = HashMap::new();
        selected_hashes.insert(first_path.clone(), "SAME".to_string());
        selected_hashes.insert(second_path.clone(), "SAME".to_string());
        let mut scan = HashMap::new();
        scan.insert(first_path.clone(), &first_item);
        scan.insert(second_path.clone(), &second_item);
        let mut directories = BTreeMap::new();
        directories.insert(model_key(&["A".to_string()]), vec!["A".to_string()]);
        directories.insert(model_key(&["B".to_string()]), vec!["B".to_string()]);
        let mut files_by_sha256 = HashMap::new();
        files_by_sha256.insert(
            "SAME".to_string(),
            vec![
                ManualModelFile {
                    relative_path: "A\\same.package".to_string(),
                    directory_parts: vec!["A".to_string()],
                    file_name: "same.package".to_string(),
                },
                ManualModelFile {
                    relative_path: "B\\same.package".to_string(),
                    directory_parts: vec!["B".to_string()],
                    file_name: "same.package".to_string(),
                },
            ],
        );
        let manual_model = ManualModelIndex {
            directories,
            files_by_sha256,
        };
        assert!(!is_selected_content_keeper_for_scan(
            &root,
            &first_path,
            "SAME",
            &selected_hashes,
            &[],
            &scan,
            &manual_model,
            None
        ));
        assert!(!is_selected_content_keeper_for_scan(
            &root,
            &second_path,
            "SAME",
            &selected_hashes,
            &[],
            &scan,
            &manual_model,
            None
        ));
        assert_eq!(
            selected_content_keeper_relative_path(
                &root,
                &first_path,
                "SAME",
                &selected_hashes,
                &[],
                &scan,
                &manual_model,
                None
            ),
            None
        );
    }

    #[test]
    fn empty_selected_directory_does_not_trigger_a_package_scan() {
        let scan = scan_selected_roots(
            Path::new(r"C:\isolated\Packages"),
            AppLanguage::Pt,
            &HashSet::new(),
        )
        .unwrap();
        assert!(scan.items.is_empty());
        assert_eq!(scan.stats.packages, 0);
    }

    #[test]
    fn incremental_residual_plan_reads_only_explicit_paths_when_requested() {
        let Ok(root) = std::env::var("S3CC_RESIDUAL_ROOT") else {
            return;
        };
        let Ok(raw_selected) = std::env::var("S3CC_RESIDUAL_SELECTED") else {
            return;
        };
        let selected = raw_selected
            .split('|')
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let plan = build_organization_plan_with_cfg_core(root, AppLanguage::Pt, selected, false)
            .expect("incremental residual plan");
        if let Ok(expected) = std::env::var("S3CC_RESIDUAL_EXPECTED_SELECTED") {
            assert_eq!(plan.stats.selected, expected.parse::<usize>().unwrap());
        }
        if let Ok(output) = std::env::var("S3CC_RESIDUAL_PLAN_OUTPUT") {
            let output = PathBuf::from(output);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(output, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
        }
    }

    #[test]
    fn projected_remaining_ignores_stable_review_companions() {
        let base = std::env::temp_dir().join(format!(
            "s3cc-projected-remaining-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stable = base.join("#Sem Categoria").join("Arquivos Auxiliares");
        let legacy = base.join("Jonas");
        std::fs::create_dir_all(&stable).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(stable.join("Jonas_Scott.sim"), b"stable").unwrap();
        std::fs::write(legacy.join("pending.txt"), b"pending").unwrap();

        let stable_count =
            projected_remaining_legacy_files(&HashSet::new(), &[stable.clone()], &[]);
        let legacy_count =
            projected_remaining_legacy_files(&HashSet::new(), &[legacy.clone()], &[]);
        assert_eq!(stable_count, 0);
        assert_eq!(legacy_count, 1);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn translated_fix_and_replacement_names_match_one_model_component() {
        assert!(model_components_match("Fixes", "Correções"));
        assert!(model_components_match("Correcciones", "Fixes"));
        assert!(model_components_match("Substituições", "Replacements"));
        assert!(model_components_match("Reemplazos", "Substituições"));
    }

    #[test]
    fn duplicate_source_from_normal_selection_and_nested_migration_is_executable_once() {
        let root = std::env::temp_dir().join(format!(
            "s3cc-duplicate-source-intersection-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let source = root
            .join("Packages")
            .join("#Duplicados")
            .join("legacy")
            .join("eruwen.package");
        let source_text = source.to_string_lossy().into_owned();
        let mut items = vec![
            PlanItem {
                id: "normal-selection".into(),
                name: "eruwen.package".into(),
                source_path: source_text.clone(),
                source_relative_path: "Packages\\#Duplicados\\legacy\\eruwen.package".into(),
                destination_path: Some(
                    root.join("Packages\\Genética\\eruwen.package")
                        .to_string_lossy()
                        .into_owned(),
                ),
                destination_relative_path: Some("Packages\\Genética\\eruwen.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 10,
                warnings: Vec::new(),
            },
            PlanItem {
                id: "nested-quarantine-migration".into(),
                name: "eruwen.package".into(),
                source_path: format!(r"\\?\{}", source.display()),
                source_relative_path: "Packages\\#Duplicados\\legacy\\eruwen.package".into(),
                destination_path: Some(
                    root.join("Packages\\#Duplicados\\eruwen.package.package.duplicated")
                        .to_string_lossy()
                        .into_owned(),
                ),
                destination_relative_path: Some(
                    "Packages\\#Duplicados\\eruwen.package.package.duplicated".into(),
                ),
                classification_status: "duplicate_quarantine".into(),
                classification_reason: Some("confirmed keeper".into()),
                plan_status: "ready_duplicate_quarantine_migration".into(),
                sha256: Some("A".repeat(64)),
                size: 10,
                warnings: Vec::new(),
            },
        ];

        resolve_intra_plan_source_conflicts(&root, &mut items);

        assert_eq!(
            items
                .iter()
                .filter(|item| item.plan_status.starts_with("ready"))
                .count(),
            1
        );
        assert_eq!(items[1].plan_status, "ready_duplicate_quarantine_migration");
        assert_eq!(items[0].plan_status, "duplicate_source_skipped");
        assert!(items[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("more than one planner path")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn incompatible_duplicate_source_proposals_are_blocked_before_execution() {
        let root = PathBuf::from(r"C:\Mods");
        let source = r"C:\Mods\Legacy\same.package";
        let mut items = vec![
            PlanItem {
                id: "first".into(),
                name: "same.package".into(),
                source_path: source.into(),
                source_relative_path: "Legacy\\same.package".into(),
                destination_path: Some(r"C:\Mods\Packages\A\same.package".into()),
                destination_relative_path: Some("Packages\\A\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 1,
                warnings: Vec::new(),
            },
            PlanItem {
                id: "second".into(),
                name: "same.package".into(),
                source_path: format!(r"\\?\{source}"),
                source_relative_path: "Legacy\\same.package".into(),
                destination_path: Some(r"C:\Mods\Packages\B\same.package".into()),
                destination_relative_path: Some("Packages\\B\\same.package".into()),
                classification_status: "classified".into(),
                classification_reason: None,
                plan_status: "ready".into(),
                sha256: Some("A".repeat(64)),
                size: 1,
                warnings: Vec::new(),
            },
        ];

        resolve_intra_plan_source_conflicts(&root, &mut items);

        assert!(items
            .iter()
            .all(|item| item.plan_status == "blocked_duplicate_source_conflict"));
    }
}
