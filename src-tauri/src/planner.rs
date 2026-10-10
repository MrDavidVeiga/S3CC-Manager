use crate::{
    i18n::AppLanguage,
    cache::load_cache,
    manifest::sha256_file,
    resource_cfg::{find_resource_cfg, package_priority, parse_resource_cfg, ResourceCfgInfo},
    resource_cfg_update::{preview_resource_cfg_update, ResourceCfgUpdate},
    scanner::{cached_scan_for, scan_packages_core, ScanPackageItem},
    workspace::{
        active_profile, is_protected, load_workspace_for_root, matching_rule,
        split_destination,
    },
};
use chrono::{Local, SecondsFormat};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlanStats {
    pub selected: usize,
    pub kept_uncategorized: usize,
    pub ready: usize,
    pub already_organized: usize,
    pub duplicate_skipped: usize,
    pub collision_same_content: usize,
    pub collision_different_content: usize,
    pub blocked: usize,
    pub directories_to_create: usize,
    pub empty_folders_to_clean: usize,
}

#[derive(Debug, Clone)]
struct ResourceCfgContext {
    info: ResourceCfgInfo,
    directory: PathBuf,
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
    root.ancestors().find(|ancestor| {
        is_branch(ancestor) && ancestor.parent().is_some_and(is_mods_root)
    })
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
fn trim_selected_branch_prefix(
    root: &Path,
    branch: &Path,
    parts: &mut Vec<String>,
) {
    let Ok(relative) = root.strip_prefix(branch) else { return };
    let existing = relative.components().filter_map(|part| match part {
        Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        _ => None,
    }).collect::<Vec<_>>();
    if !existing.is_empty() && parts.len() >= existing.len()
        && parts.iter().take(existing.len()).zip(existing.iter())
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
    source.strip_prefix(root).ok()
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
    if parts.first().is_some_and(|part| part.eq_ignore_ascii_case("Packages")
        || part.eq_ignore_ascii_case("Overrides"))
    {
        parts.remove(0);
    }
    if parts.first().is_some_and(|part| part.eq_ignore_ascii_case("CAS")) {
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
    if !is_mods_root(root) { return false; }
    let Some(Component::Normal(top)) = source.strip_prefix(root).ok()
        .and_then(|relative| relative.components().next()) else { return false; };
    let top = top.to_string_lossy().to_ascii_lowercase();
    matches!(top.as_str(),
        "dccache" | "dcbackup" | "downloads" | "savedsims" | "library"
        | "saves" | "installedworlds" | "exports" | "screenshots"
        | "collections" | "thumbnails" | "cache" | "caches"
        | "quarantine" | "restore manifests" | "s3cc manager"
        | "s3cc organizer" | "backups" | "backup")
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
    let Some(name) = relative.components().next().and_then(|component| match component {
        Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
        _ => None,
    }) else {
        return false;
    };
    [
        "cas", "sliders", "clothing", "roupas", "ropa",
        "hair", "cabelos", "cabello",
        "accessories", "acessórios", "accesorios",
        "makeup", "maquiagem", "maquillaje",
        "genetics", "genética", "genetica",
        "pets", "animais", "mascotas",
        "patterns", "padrões", "patrones",
        "buy", "compra", "build", "construção", "construcción",
        "objects", "objetos",
        "gameplay", "jogabilidade", "jugabilidad",
        "scripts", "store", "nraas",
        "localization", "localização", "localización",
        "poses and animations", "poses e animações", "poses y animaciones",
    ].contains(&name.as_str())
}

// When the user selected Mods rather than Mods/Packages, the organizer's
// destination folders must still stay inside Packages, even if the selected
// package has no reliable Resource.cfg rule.
fn ensure_packages_destination(root: &Path, parts: &[String]) -> Vec<String> {
    let mut parts = parts.to_vec();
    let has_packages_prefix = parts.first()
        .is_some_and(|part| part.eq_ignore_ascii_case("Packages"));
    // Explicit legacy rules may contain CAS or Packages/CAS. CAS must not
    // consume a Resource.cfg nesting level. Never produce Packages/Packages.
    let cas_index = usize::from(has_packages_prefix);
    if parts.get(cas_index).is_some_and(|part| part.eq_ignore_ascii_case("CAS")) {
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
        let root_relative = root.strip_prefix(&context.directory).ok()
            .map(|value| value.components()
                .filter_map(|part| match part {
                    Component::Normal(value) => Some(value.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>())
            .unwrap_or_default();
        let mut prefix = Vec::<String>::new();
        if root_relative.len() < rule_prefix.len()
            && rule_prefix.iter().take(root_relative.len()).zip(root_relative.iter())
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        {
            prefix.extend(rule_prefix.iter().skip(root_relative.len()).cloned());
        }
        if packages_parts.len() >= prefix.len()
            && prefix.iter().zip(packages_parts.iter())
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
        || (parts.first().is_some_and(|part| part == "Packages" || part == "Overrides")
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
        if is_mods_root(root) { 1 } else { 0 }
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


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlan {
    pub root: String,
    pub items: Vec<PlanItem>,
    pub stats: PlanStats,
    pub directories_to_create: Vec<String>,
    pub manifest_preview: String,
    pub resource_cfg_update: Option<ResourceCfgUpdate>,
    pub can_execute: bool,
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
    if value
        .chars()
        .any(|ch| ch < '\u{20}' || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
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

fn add_missing_directories(root: &Path, parts: &[String], output: &mut BTreeSet<String>) {
    let mut relative = PathBuf::new();
    for part in parts {
        relative.push(part);
        if !root.join(&relative).exists() {
            output.insert(relative_key(&relative));
        }
    }
}

fn is_not_categorized_root(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "not categorized"
            | "uncategorized"
            | "sem categoria"
            | "não categorizado"
            | "nao categorizado"
            | "sin categorizar"
    )
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

    let mut parent_parts = components[..components.len().saturating_sub(1)].to_vec();
    if is_mods_root(root)
        && parent_parts
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case("packages"))
    {
        parent_parts.remove(0);
    }
    if parent_parts
        .first()
        .map(|value| is_not_categorized_root(value))
        .unwrap_or(false)
    {
        parent_parts.remove(0);
    }
    let mut destination_parts = vec![language.not_categorized_folder().to_string()];
    destination_parts.extend(parent_parts);
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
        let extension = file.extension().and_then(|value| value.to_str()).unwrap_or("package");
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

fn same_path_case_insensitive(left: &Path, right: &Path) -> bool {
    relative_key(left).eq_ignore_ascii_case(&relative_key(right))
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
// Disabled files are a distinct load state and must not be selected as the
// keeper of an enabled group (or vice versa).
fn is_disabled_package_path(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name.to_string_lossy()
        .to_ascii_lowercase().ends_with(".package.disabled"))
}

fn is_selected_content_keeper(
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
) -> bool {
    let inactive = is_disabled_package_path(source);
    let keeper = selected_hashes.iter()
        .filter(|(path, hash)| hash.eq_ignore_ascii_case(sha256)
            && is_disabled_package_path(path) == inactive)
        .map(|(path, _)| path)
        .min_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    keeper.is_none_or(|path| path == source)
}

// Prefer the copy whose complete path from the selected root already is the
// canonical destination. The old lexical-only choice could make a legacy
// nested source the keeper over an already organized package.
fn canonical_destination_for_keeper(
    root: &Path,
    source: &Path,
    item: &ScanPackageItem,
    resource_cfg: Option<&ResourceCfgContext>,
) -> Option<PathBuf> {
    let file_name = Path::new(&item.name).file_name()?;
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

fn is_selected_content_keeper_for_scan(
    root: &Path,
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    resource_cfg: Option<&ResourceCfgContext>,
) -> bool {
    let inactive = is_disabled_package_path(source);
    let mut candidates = selected_hashes
        .iter()
        .filter(|(path, hash)| {
            hash.eq_ignore_ascii_case(sha256)
                && is_disabled_package_path(path) == inactive
        })
        .map(|(path, _)| {
            let canonical = scan_by_canonical
                .get(path)
                .and_then(|item| canonical_destination_for_keeper(root, path, item, resource_cfg))
                .is_some_and(|destination| same_path_case_insensitive(path, &destination));
            let relative = path
                .strip_prefix(root)
                .map(relative_key)
                .unwrap_or_else(|_| relative_key(path));
            (path, canonical, relative.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|(_, canonical, relative)| (!*canonical, relative.clone()));
    candidates.first().is_none_or(|(path, _, _)| *path == source)
}

fn selected_content_keeper_relative_path(
    root: &Path,
    source: &Path,
    sha256: &str,
    selected_hashes: &HashMap<PathBuf, String>,
    scan_by_canonical: &HashMap<PathBuf, &ScanPackageItem>,
    resource_cfg: Option<&ResourceCfgContext>,
) -> Option<String> {
    let inactive = is_disabled_package_path(source);
    let mut candidates = selected_hashes
        .iter()
        .filter(|(path, hash)| {
            hash.eq_ignore_ascii_case(sha256)
                && is_disabled_package_path(path) == inactive
        })
        .map(|(path, _)| {
            let canonical = scan_by_canonical
                .get(path)
                .and_then(|item| canonical_destination_for_keeper(root, path, item, resource_cfg))
                .is_some_and(|destination| same_path_case_insensitive(path, &destination));
            let relative = path
                .strip_prefix(root)
                .map(relative_key)
                .unwrap_or_else(|_| relative_key(path));
            (canonical, relative.to_ascii_lowercase(), relative)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(canonical, key, _)| (!*canonical, key.clone()));
    candidates.into_iter().next().map(|(_, _, relative)| relative)
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
    for item in scan_items {
        if !selected_sizes.contains(&item.file_size) {
            continue;
        }
        let Ok(path) = PathBuf::from(&item.path).canonicalize() else {
            continue;
        };
        let hash = if let Some(hash) = selected_hashes.get(&path) {
            hash.clone()
        } else {
            // Reuse the same fingerprints the Duplicates analysis already
            // recorded. Never trust a cached hash if size or mtime changed.
            let cached_hash = cache.entries.get(path.to_string_lossy().as_ref())
                .and_then(|cached| {
                    let metadata = fs::metadata(&path).ok()?;
                    let modified = metadata.modified().ok()?
                        .duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
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
        by_hash.entry(hash.to_ascii_uppercase())
            .or_default()
            .push(item.relative_path.clone());
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
                related.entry(merged_name.to_lowercase())
                    .or_default().push(loose_name.clone());
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
        let Ok(modified_ns) = metadata.modified()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| std::io::Error::other(error.to_string())))
            .map(|duration| duration.as_nanos())
        else {
            continue;
        };
        if cached.size != metadata.len() || cached.modified_ns != modified_ns
            || cached.parse_error.is_some()
        {
            continue;
        }
        let substantive = cached.resources.iter()
            .filter(|resource| !matches!(resource.type_id, 0x2205_57DA | 0x0166_038C))
            .map(|resource| {
                format!("{:08X}-{:08X}-{:016X}-{}",
                    resource.type_id, resource.group, resource.instance, resource.payload_sha256)
            })
            .collect::<BTreeSet<_>>();
        let direct_slider = item.sub_category.as_deref() == Some("Sliders")
            && cached.resources.iter().any(|r| {
                matches!(r.type_id, 0x0358_B08A | 0xB52F_5055 | 0x0355_E0A6 | 0x067C_AA11)
            });
        indexed.push((item.relative_path.clone(), substantive, direct_slider));
    }
    detect_merged_resource_supersets(&indexed)

}

fn mark_intra_plan_destination_collisions(items: &mut [PlanItem], stats: &mut PlanStats) {
    let mut destinations = HashMap::<String, Vec<usize>>::new();

    for (index, item) in items.iter().enumerate() {
        if item.plan_status != "ready" {
            continue;
        }
        let Some(destination) = item.destination_relative_path.as_ref() else {
            continue;
        };
        let key = destination.replace('/', "\\").to_ascii_lowercase();
        destinations.entry(key).or_default().push(index);
    }

    for indices in destinations.values().filter(|indices| indices.len() > 1) {
        let first_index = indices[0];
        let first = &items[first_index];
        let first_hash = first.sha256.as_deref().unwrap_or_default();
        let first_size = first.size;
        let same_content = indices.iter().all(|index| {
            let item = &items[*index];
            item.size == first_size
                && item
                    .sha256
                    .as_deref()
                    .map(|hash| hash.eq_ignore_ascii_case(first_hash))
                    .unwrap_or(false)
        });

        if same_content {
            // Keep one representative ready for the destination. Extra byte-identical
            // sources stay in place and can later be handled from Duplicates/Quarantine.
            for index in indices.iter().skip(1) {
                let item = &mut items[*index];
                item.plan_status = "duplicate_skipped".to_string();
                stats.duplicate_skipped += 1;
                item.warnings.push(
                    "Another selected package with identical bytes will be moved to this destination. This duplicate will be left in place."
                        .to_string(),
                );
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
}

fn plan_can_execute(stats: &PlanStats, read_only: bool) -> bool {
    // Blocked items are excluded from movement. They must not prevent the
    // independently validated ready items from being organized and restored.
    !read_only && (stats.ready > 0 || stats.empty_folders_to_clean > 0)
}

fn manifest_preview(
    root: &Path,
    language: AppLanguage,
    ready_items: &[PlanItem],
) -> String {
    let mut out = String::new();
    out.push_str("S3CC MANAGER RESTORE MANIFEST\n");
    out.push_str("version=1\n");
    out.push_str(&format!(
        "created_at={}\n",
        Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!("mode=PREVIEW\n"));
    out.push_str(&format!("organization_language={}\n", language_code(language)));
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

#[tauri::command]
pub fn build_organization_plan(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
) -> Result<OrganizationPlan, String> {
    build_organization_plan_with_cfg(folder, language, selected_paths, false)
}

#[tauri::command]
pub fn build_organization_plan_with_cfg(
    folder: String,
    language: AppLanguage,
    selected_paths: Vec<String>,
    update_resource_cfg: bool,
) -> Result<OrganizationPlan, String> {
    if selected_paths.is_empty() {
        return Err("No packages were selected.".to_string());
    }

    let root_input = PathBuf::from(folder.trim());
    let root = root_input
        .canonicalize()
        .map_err(|error| format!("Could not resolve root folder: {error}"))?;

    if !root.is_dir() {
        return Err(format!("Root is not a directory: {}", root.display()));
    }
    // The selected root can be Mods, any Packages/Overrides subfolder, or a
    // self-contained CC staging library elsewhere. All moves remain inside
    // this canonical root. Never invent an out-of-root destination.
    //
    // A Mods root may also contain Overrides, DCCache and other directories.
    // Never organize those as Packages or create category folders beside Packages.
    let packages_root = if is_mods_root(&root) {
        let path = root.join("Packages")
            .canonicalize()
            .map_err(|_| "The selected Mods folder must contain a Packages directory.".to_string())?;
        if !path.is_dir() || !path_is_within_root(&root, &path) {
            return Err("Mods/Packages is not a safe directory inside the selected Mods root.".to_string());
        }
        Some(path)
    } else {
        None
    };

    let workspace = load_workspace_for_root(&root);
    let profile = active_profile(&workspace);
    let resource_cfg = resource_cfg_context(&root);
    let scan = match cached_scan_for(&root, language) {
        Some(scan) => scan,
        None => scan_packages_core(root.to_string_lossy().to_string(), language, None)?,
    };

    let mut scan_by_canonical = HashMap::<PathBuf, &ScanPackageItem>::new();
    for item in &scan.items {
        if let Ok(path) = PathBuf::from(&item.path).canonicalize() {
            scan_by_canonical.insert(path, item);
        }
    }

    let mut selected = HashSet::<PathBuf>::new();
    for raw in selected_paths {
        let canonical = PathBuf::from(&raw)
            .canonicalize()
            .map_err(|error| format!("Could not resolve selected file {raw}: {error}"))?;

        if !path_is_within_root(&root, &canonical) {
            return Err(format!("Selected file escaped the selected root: {raw}"));
        }

        selected.insert(canonical);
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

    let mut selected_hashes = HashMap::<PathBuf, String>::new();
    let mut selected_hash_set = HashSet::<String>::new();
    for path in &selected {
        let (hash, _) = sha256_file(path)
            .map_err(|error| format!("Could not hash selected package {}: {error}", path.display()))?;
        selected_hashes.insert(path.clone(), hash.clone());
        selected_hash_set.insert(hash);
    }

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
            items.push(make_blocked(item,
                "System/cache/recovery folder is protected: this package will not be moved.".to_string()));
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
                    "Merged Sim export destination is outside the selected loading root.".to_string(),
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
        let outside_active_branch = packages_root
            .as_ref()
            .is_some_and(|packages| !path_is_within_root(packages, &source_canonical))
            && !source_uses_overrides(&root, &source_canonical);

        let duplicates = duplicate_peers.get(&source_hash.to_ascii_uppercase());
        if let Some(peers) = duplicates.filter(|peers| peers.len() > 1)
            .filter(|_| !is_selected_content_keeper_for_scan(
                &root,
                &source_canonical,
                &source_hash,
                &selected_hashes,
                &scan_by_canonical,
                resource_cfg.as_ref(),
            ))
        {
            // Only an additional selected same-state copy is excluded. The
            // chosen keeper continues through classification, destination
            // collision checks and normal transactional/Restore protection.
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
                sha256: Some(source_hash.clone()),
                size: item.file_size,
                warnings: vec![format!(
                    "This package is a byte-identical duplicate and remains in its full selected-root path. The canonical keeper is '{}'; {} matching scan paths: {}. It was not moved, overwritten or deleted. Review it in Duplicates/Quarantine to empty the legacy folder safely.",
                    selected_content_keeper_relative_path(
                        &root,
                        &source_canonical,
                        &source_hash,
                        &selected_hashes,
                        &scan_by_canonical,
                        resource_cfg.as_ref(),
                    ).unwrap_or_else(|| "unresolved selected path".to_string()),
                    peers.len(),
                    peers.iter().take(4).cloned().collect::<Vec<_>>().join(" | ")
                )],
            });
            continue;
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

        // Unknown is a classification result, not an instruction to leave the
        // original folder forever. Move safely selected unknowns to a review
        // area (with their source hierarchy intact). Do not redirect Overrides
        // or invalid/mixed packages without a manual decision.
        if item.status == "unknown" && !source_uses_overrides(&root, &source_canonical) {
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
                    let (existing_hash, _) = sha256_file(&original_target)
                        .map_err(|error| format!(
                            "Could not verify existing Not Categorized package: {error}"
                        ))?;
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
                items.push(make_blocked(item, error));
                continue;
            }
            let target_name = fallback_relative.file_name()
                .ok_or_else(|| "Uncategorized fallback has no filename.".to_string())?;
            let inactive = item.name.to_ascii_lowercase().ends_with(".package.disabled");
            let cfg_opt_in = update_resource_cfg
                && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
            let adjusted = if cfg_opt_in || inactive {
                Ok((ensure_source_loading_branch(
                    &root, &source_canonical, &fallback_parts), None))
            } else {
                fit_destination_to_resource_cfg(
                    &root, &source_canonical, target_name,
                    &fallback_parts, resource_cfg.as_ref(),
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
                items.push(make_blocked(item, reason));
                continue;
            }
            let destination = destination_path(&root, &destination_parts, target_name);
            if !path_is_within_root(&root, &destination)
                || (is_mods_root(&root) && !destination.starts_with(root.join("Packages")))
            {
                stats.blocked += 1;
                items.push(make_blocked(item,
                    "Not Categorized fallback escaped the Packages loading branch.".to_string()));
                continue;
            }
            let destination_relative = destination.strip_prefix(&root)
                .map(relative_key)
                .map_err(|_| "Not Categorized destination escaped selected root.".to_string())?;
            let already = same_path_case_insensitive(&source_canonical, &destination);
            let collision = !already && destination.exists();
            let collision_identical = if collision {
                let (existing_hash, _) = sha256_file(&destination)
                    .map_err(|error| format!("Cannot inspect fallback collision: {error}"))?;
                existing_hash.eq_ignore_ascii_case(&source_hash)
            } else { false };
            let destination_status = if already {
                "already_organized"
            } else if collision_identical {
                "duplicate_skipped"
            } else if collision {
                "collision_different_content"
            } else {
                "ready_uncategorized"
            };
            if already { stats.already_organized += 1; }
            if collision_identical { stats.duplicate_skipped += 1; }
            else if collision { stats.collision_different_content += 1; }
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
            && item.detected_from.iter().any(|source| source == "ModFolderCompanion");
        if item.status != "classified"
            || item.destination_parts.is_empty()
            || (!matches!(item.classification_confidence.as_str(), "high" | "manual")
                && !verified_companion)
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
            && !item.destination_parts.first().is_some_and(|first| first == "Scripts")
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

        let file_name = source
            .file_name()
            .ok_or_else(|| format!("Source has no filename: {}", source.display()))?;

        // Updating Resource.cfg is explicitly opt-in. With consent, preserve
        // the full category tree rather than compacting it to existing depth.
        let cfg_opt_in = update_resource_cfg
            && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
        let inactive_package = item.name.to_ascii_lowercase().ends_with(".package.disabled");
        let cfg_fit = if cfg_opt_in || inactive_package {
            // Disabled packages are never loaded by The Sims 3. Their
            // classification must not be blocked by PackedFile depth rules,
            // and the .disabled suffix must be preserved on relocation.
            Ok((ensure_source_loading_branch(&root, &source_canonical, &destination_parts), None))
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
                        warnings: vec![format!(
                            "Override kept at its original location: {reason}"
                        )],
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

        if relative_key(Path::new(&item.relative_path)).eq_ignore_ascii_case(&destination_relative_text) {
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
            let (target_hash, target_size) = sha256_file(&destination)
                .map_err(|error| format!("Could not hash collision target {}: {error}", destination.display()))?;

            let same = source_size == target_size && source_hash.eq_ignore_ascii_case(&target_hash);
            if same {
                stats.duplicate_skipped += 1;
            } else {
                stats.collision_different_content += 1;
            }

            items.push(PlanItem {
                id: item.id.clone(),
                name: item.name.clone(),
                source_path: source_canonical.to_string_lossy().to_string(),
                source_relative_path: item.relative_path.clone(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                destination_relative_path: Some(destination_relative_text),
                classification_status: item.status.clone(),
                classification_reason: classification_reason.clone(),
                plan_status: if same {
                    "duplicate_skipped".to_string()
                } else {
                    "collision_different_content".to_string()
                },
                sha256: Some(source_hash),
                size: source_size,
                warnings: vec![if same {
                    "The categorized destination already contains a byte-identical file. This copy will stay in place; use Duplicates/Quarantine for explicit review.".to_string()
                } else {
                    "The categorized destination already contains different data with the same name. This package will stay in place until the collision is reviewed.".to_string()
                }],
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

    // Detect collisions created by the plan itself before any filesystem write occurs.
    // This catches multiple selected packages that resolve to the same final path.
    mark_intra_plan_destination_collisions(&mut items, &mut stats);

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
    stats.blocked = items.iter().filter(|item| item.plan_status == "blocked").count();

    let ready_items = items
        .iter()
        .filter(|item| item.plan_status.starts_with("ready"))
        .cloned()
        .collect::<Vec<_>>();

    stats.directories_to_create = directories.len();
    // Enable a confirmed cleanup-only pass after a previous organization,
    // but count only actually empty directories and never loading roots.
    stats.empty_folders_to_clean = walkdir::WalkDir::new(&root)
        .follow_links(false).min_depth(1).into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .filter(|entry| {
            let path = entry.path();
            let protected = is_mods_root(&root) && path.parent() == Some(root.as_path())
                && path.file_name().is_some_and(|name| {
                    name.to_string_lossy().eq_ignore_ascii_case("Packages")
                        || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
                });
            !protected && fs::read_dir(path)
                .map(|mut items| items.next().is_none()).unwrap_or(false)
        })
        .count();

    let can_execute = plan_can_execute(&stats, workspace.read_only);
    let cfg_opt_in = update_resource_cfg
        && (is_mods_root(&root) || is_within_packages(&root) || is_within_overrides(&root));
    let resource_cfg_update = if cfg_opt_in {
        // Include both new destinations and existing packages in the scanned
        // branch. Partial organization must not cause old CC to stop loading.
        let mut paths = ready_items.iter()
            .filter_map(|item| item.destination_path.as_ref().map(PathBuf::from))
            .collect::<Vec<_>>();
        paths.extend(scan.items.iter().map(|item| PathBuf::from(&item.path)).filter(|path| {
            root.ancestors().find(|parent| is_mods_root(parent)).is_some_and(|mods| {
                path.starts_with(mods.join("Packages")) || path.starts_with(mods.join("Overrides"))
            })
        }));
        preview_resource_cfg_update(&root, &paths)?
    } else { None };

    Ok(OrganizationPlan {
        root: root.to_string_lossy().to_string(),
        manifest_preview: manifest_preview(&root, language, &ready_items),
        directories_to_create: directories.into_iter().collect(),
        resource_cfg_update,
        stats,
        items,
        can_execute,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn copied_mods_root_preserves_packages_overrides_and_safe_destinations() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-copied-{}-{nonce}", std::process::id()))
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
    fn unknown_fallback_moves_out_of_legacy_folders_without_losing_source_context() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("s3cc-unknown-fallback-{}-{nonce}",std::process::id()));
        let mods = base.join("Mods - Copia");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(mods.join("Overrides")).unwrap();
        let (path, parts) = fallback_relative_path(
            &mods, AppLanguage::Pt,
            "#+18\\Pns (TS3)\\Rigged\\PenisButtBones.package", "AABBCCDD"
        ).unwrap();
        assert!(path.starts_with("Packages"));
        assert!(path.to_string_lossy().contains("Pns (TS3)"));
        assert!(path.to_string_lossy().contains("Rigged"));
        assert!(parts.iter().any(|part| part == "Sem Categoria"));
        let (override_path, override_parts) = fallback_relative_path(
            &mods, AppLanguage::Pt, "Overrides\\Overhaul\\unknown.package", "AABBCCDD"
        ).unwrap();
        assert_eq!(override_path, PathBuf::from("Overrides\\Overhaul\\unknown.package"));
        assert!(override_parts.is_empty());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn legacy_user_folders_are_migratable_but_sims_caches_are_protected() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("s3cc-migrate-source-{}-{nonce}",std::process::id()));
        let mods = base.join("Mods - Copia");
        std::fs::create_dir_all(mods.join("Packages")).unwrap();
        std::fs::create_dir_all(mods.join("Overrides")).unwrap();
        assert!(!is_mods_system_source(&mods,&mods.join("#+18").join("AnimatedWoohoo.package")));
        assert!(!is_mods_system_source(&mods,&mods.join("Careers").join("Modeling.package")));
        assert!(is_mods_system_source(&mods,&mods.join("DCCache").join("dcdb0.dbc")));
        assert!(is_mods_system_source(&mods,&mods.join("Downloads").join("download.package")));
        assert!(is_mods_system_source(&mods,&mods.join("S3CC Manager").join("Restore Manifests").join("x.package")));
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn selected_exact_duplicates_choose_one_keeper_without_reactivating_disabled() {
        let mut hashes = HashMap::new();
        hashes.insert(PathBuf::from("Packages/Old/C.package"), "AAA".to_string());
        hashes.insert(PathBuf::from("Packages/Old/B.package"), "AAA".to_string());
        hashes.insert(PathBuf::from("Packages/Old/A.package.disabled"), "AAA".to_string());
        hashes.insert(PathBuf::from("Packages/Old/Z.package.disabled"), "AAA".to_string());
        assert!(is_selected_content_keeper(Path::new("Packages/Old/B.package"), "AAA", &hashes));
        assert!(!is_selected_content_keeper(Path::new("Packages/Old/C.package"), "AAA", &hashes));
        assert!(is_selected_content_keeper(Path::new("Packages/Old/A.package.disabled"), "AAA", &hashes));
        assert!(!is_selected_content_keeper(Path::new("Packages/Old/Z.package.disabled"), "AAA", &hashes));
    }


    use super::*;

    fn synthetic_scan_item(
        root: &Path,
        relative_path: &str,
        destination_parts: &[&str],
    ) -> (PathBuf, ScanPackageItem) {
        let path = root.join(relative_path.replace('\\', std::path::MAIN_SEPARATOR_STR));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let destination_parts = destination_parts.iter().map(|part| (*part).to_string()).collect::<Vec<_>>();
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
        let (canonical_path, canonical_item) = synthetic_scan_item(&packages_root, canonical_relative, &["Sliders"]);
        let (legacy_path, legacy_item) = synthetic_scan_item(&packages_root, legacy_relative, &["Sliders"]);
        let (second_legacy_path, second_legacy_item) = synthetic_scan_item(
            &packages_root,
            second_legacy_relative,
            &["Sliders"],
        );
        let mut selected_hashes = HashMap::new();
        selected_hashes.insert(canonical_path.clone(), "SAME".into());
        selected_hashes.insert(legacy_path.clone(), "SAME".into());
        selected_hashes.insert(second_legacy_path.clone(), "SAME".into());
        let mut scan = HashMap::new();
        scan.insert(canonical_path.clone(), &canonical_item);
        scan.insert(legacy_path.clone(), &legacy_item);
        scan.insert(second_legacy_path.clone(), &second_legacy_item);

        assert!(is_selected_content_keeper_for_scan(
            &packages_root, &canonical_path, "SAME", &selected_hashes, &scan, None
        ));
        assert!(!is_selected_content_keeper_for_scan(
            &packages_root, &legacy_path, "SAME", &selected_hashes, &scan, None
        ));
        assert!(!is_selected_content_keeper_for_scan(
            &packages_root, &second_legacy_path, "SAME", &selected_hashes, &scan, None
        ));
        assert_eq!(
            selected_content_keeper_relative_path(
                &packages_root,
                &legacy_path,
                "SAME",
                &selected_hashes,
                &scan,
                None,
            ),
            Some(relative_key(Path::new(canonical_relative)))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planned_sources_use_the_same_canonical_root_as_cleanup() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-canonical-root-{}-{nonce}", std::process::id()));
        let source = root.join("Packages").join("Legacy").join("a.package");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"DBPF source identity test").unwrap();
        let canonical_root = root.canonicalize().unwrap();
        let canonical_source = source.canonicalize().unwrap();
        assert!(canonical_source.starts_with(&canonical_root));
        assert!(canonical_source.parent().unwrap().starts_with(&canonical_root));
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
                rules: patterns.iter().enumerate().map(|(index, rule)| ResourceCfgRule {
                    priority: 500,
                    pattern: (*rule).to_string(),
                    source_line: index + 1,
                }).collect(),
            },
        }
    }

    #[test]
    fn overrides_support_anatomical_folders_when_resource_cfg_matches() {
        let mods = Path::new("The Sims 3").join("Mods");
        let source = mods.join("Overrides").join("patch.package");
        let context = cfg_with_rules(&mods, &[
            "Packages/*.package",
            "Overrides/*.package",
            "Overrides/*/*/*/*.package",
        ]);
        let category = vec!["Sliders".into(), "Face".into(), "Nose".into()];
        let (destination, _) = fit_destination_to_resource_cfg(
            &mods, &source, std::ffi::OsStr::new("patch.package"),
            &category, Some(&context),
        ).unwrap();
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
            &mods, &overrides.join("patch.package"),
            std::ffi::OsStr::new("patch.package"), &cat, Some(&context),
        ).unwrap();
        assert_eq!(destination, vec!["Overrides"]);
        assert!(note.unwrap().contains("category is retained"));

        let (selected_override, note) = fit_destination_to_resource_cfg(
            &overrides, &overrides.join("patch.package"),
            std::ffi::OsStr::new("patch.package"), &cat, Some(&context),
        ).unwrap();
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
            &mods, &override_file, std::ffi::OsStr::new("ui.package"),
            &category, Some(&context),
        ).is_err());
        assert!(fit_destination_to_resource_cfg(
            &mods, &override_file, std::ffi::OsStr::new("ui.package"),
            &category, None,
        ).is_err());
        let (fallback, dirs) = fallback_relative_path(
            &mods, AppLanguage::En, "Overrides/UI/ui.package", "ABCD",
        ).unwrap();
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
        assert!(source_uses_overrides(&deep_overrides, &deep_overrides.join("ui.package")));
        assert_eq!(
            ensure_source_loading_branch(
                &deep_packages,
                &deep_packages.join("file.package"),
                &["Packages".into(), "CAS".into(), "Clothing".into(), "Male".into(),
                    "YA-A".into(), "Top".into()],
            ),
            vec!["YA-A", "Top"]
        );
        assert_eq!(
            ensure_source_loading_branch(
                &deep_overrides,
                &deep_overrides.join("ui.package"),
                &["Overrides".into(), "Gameplay".into(), "Tuning".into(), "Scripts".into()],
            ),
            vec!["Scripts"]
        );
        let staging = Path::new("Downloads").join("CC Incoming");
        assert!(!is_within_overrides(&staging));
        assert_eq!(
            ensure_source_loading_branch(&staging, &staging.join("file.package"), &["Clothing".into()]),
            vec!["Clothing"]
        );
    }

    #[test]
    fn identifies_only_known_legacy_manager_folders_for_migration() {
        let mods = Path::new("The Sims 3").join("Mods");
        assert!(legacy_manager_source(
            &mods, &mods.join("CAS").join("Sliders").join("a.package")));
        assert!(legacy_manager_source(
            &mods, &mods.join("Roupas").join("Masculino").join("x.package")));
        assert!(!legacy_manager_source(
            &mods, &mods.join("Overrides").join("ui.package")));
        assert!(!legacy_manager_source(
            &mods, &mods.join("DCCache").join("a.package")));
        assert!(!legacy_manager_source(
            &mods, &mods.join("Random Folder").join("x.package")));
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
        assert_eq!(ensure_packages_destination(packages, &categories), vec!["Hair"]);
        assert_eq!(
            ensure_packages_destination(packages, &["Packages".into(), "CAS".into(), "Hair".into()]),
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
        ).unwrap();
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
            "Clothing".into(), "Female".into(), "YA-A".into(), "Top".into(),
        ];
        let (fitted, _) = fit_destination_to_resource_cfg(
            &root, &root.join("Packages").join("source.package"),
            std::ffi::OsStr::new("new.package"), &parts, Some(&context),
        ).unwrap();
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
            "Clothing".into(), "Female".into(), "YA-A".into(), "Top".into(),
        ];
        let (fitted, _) = fit_destination_to_resource_cfg(
            &root, &root.join("Packages").join("source.package"),
            std::ffi::OsStr::new("new.package"), &parts, Some(&context),
        ).unwrap();
        assert_eq!(fitted, vec!["Packages", "Clothing", "Female", "YA-A", "Top"]);
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

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("s3cc-duplicate-preflight-{}-{nonce}", std::process::id()));
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
                relative_path: path.strip_prefix(&packages).unwrap().to_string_lossy().to_string(),
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
            })
            .collect::<Vec<_>>();
        let groups = index_exact_duplicates(&packages, &scan_items, &selected);
        assert_eq!(groups[&hash.to_ascii_uppercase()].len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn partial_merged_morph_resources_hold_only_the_larger_package() {
        let keys = |items: &[&str]| items.iter().map(|key| key.to_string())
            .collect::<BTreeSet<_>>();
        let candidates = vec![
            ("single-slider.package".into(), keys(&["FACE-a", "BGEO-a"]), true),
            ("merged-sliders.package".into(), keys(&["FACE-a", "BGEO-a", "FACE-b"]), true),
            ("related-translation.package".into(), keys(&["STBL-a"]), false),
            ("unrelated.package".into(), keys(&["FACE-x", "BGEO-x", "FACE-y"]), true),
        ];
        let related = detect_merged_resource_supersets(&candidates);
        assert_eq!(related["merged-sliders.package"], vec!["single-slider.package"]);
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

        mark_intra_plan_destination_collisions(&mut items, &mut stats);

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

        mark_intra_plan_destination_collisions(&mut items, &mut stats);

        assert_eq!(items[0].plan_status, "ready");
        assert_eq!(items[1].plan_status, "duplicate_skipped");
        assert_eq!(stats.duplicate_skipped, 1);
        assert_eq!(stats.collision_same_content, 0);
        assert_eq!(stats.collision_different_content, 0);
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
    fn blocked_items_are_skipped_without_blocking_independent_ready_moves() {
        let stats = PlanStats {
            selected: 2,
            ready: 1,
            blocked: 1,
            ..PlanStats::default()
        };

        assert!(plan_can_execute(&stats, false));
        assert!(!plan_can_execute(&PlanStats { ready: 0, blocked: 1, ..PlanStats::default() }, false));
        assert!(plan_can_execute(&PlanStats { ready: 0, empty_folders_to_clean: 2, ..PlanStats::default() }, false));
        assert!(!plan_can_execute(&PlanStats { ready: 0, empty_folders_to_clean: 2, ..PlanStats::default() }, true));
        assert!(!plan_can_execute(&PlanStats { ready: 1, ..PlanStats::default() }, true));
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
}
