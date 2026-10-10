use crate::{
    manifest::sha256_file,
    workspace::{ensure_writable, load_workspace_for_root},
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinePlanItem {
    pub source_path: String,
    pub source_relative_path: String,
    pub destination_path: String,
    pub destination_relative_path: String,
    pub sha256: String,
    pub size: u64,
    pub status: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineStats {
    pub selected: usize,
    pub ready: usize,
    pub blocked: usize,
    pub collisions: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantinePlan {
    pub root: String,
    pub quarantine_root: String,
    pub items: Vec<QuarantinePlanItem>,
    pub stats: QuarantineStats,
    pub manifest_preview: String,
    pub can_execute: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineResult {
    pub moved: usize,
    pub quarantine_root: String,
    pub manifest_path: String,
    pub status: String,
    pub cleanup_removed: usize,
    pub cleanup_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuarantineManifest {
    version: u32,
    created_at: String,
    status: String,
    root: String,
    quarantine_root: String,
    items: Vec<QuarantinePlanItem>,
}

type MoveRecord = (PathBuf, PathBuf, String, u64);

fn transaction_mutex() -> &'static Mutex<()> {
    static QUARANTINE_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    QUARANTINE_MUTEX.get_or_init(|| Mutex::new(()))
}

fn transaction_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    transaction_mutex()
        .lock()
        .map_err(|_| "Quarantine transaction lock is poisoned.".to_string())
}

fn workspace_base(root: &Path) -> PathBuf {
    // Preserve the historical Mods/S3CC Organizer workspace for packages
    // selected at any depth inside a game-loading branch. Never place the
    // quarantined package in a directory that Resource.cfg may still load.
    if let Some(branch) = root.ancestors().find(|ancestor| {
        ancestor.file_name().is_some_and(|name| {
            name.to_string_lossy().eq_ignore_ascii_case("Packages")
                || name.to_string_lossy().eq_ignore_ascii_case("Overrides")
        }) && ancestor.parent().is_some_and(crate::resource_cfg::is_mods_layout_root)
    }) {
        return branch.parent().unwrap().join("S3CC Organizer");
    }
    root.parent().unwrap_or(root).join("S3CC Organizer")
}

fn quarantine_base(root: &Path) -> PathBuf {
    workspace_base(root).join("Quarantine")
}

fn manifest_base(root: &Path) -> PathBuf {
    workspace_base(root).join("Quarantine Manifests")
}

fn canonical_root(folder: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve Mods root: {error}"))?;
    if !root.is_dir() {
        return Err("Selected Mods root is not a directory.".into());
    }
    Ok(root)
}

fn relative_text(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

fn validated_relative(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.starts_with('\\') || value.starts_with('/') {
        return Err("Relative path is empty or absolute.".into());
    }
    let mut path = PathBuf::new();
    for part in value.split(|ch| ch == '\\' || ch == '/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(' ')
            || part.ends_with('.')
            || part.chars().any(|ch| {
                ch < '\u{20}' || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*')
            })
        {
            return Err(format!("Unsafe quarantine path component: {part:?}"));
        }
        let stem = part.split('.').next().unwrap_or(part).to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(format!("Reserved Windows file name in quarantine path: {part}"));
        }
        path.push(part);
    }
    if path.as_os_str().is_empty() || path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("Invalid relative quarantine path.".into());
    }
    Ok(path)
}

/// Reject existing symlink/junction components, including a redirect introduced
/// after preflight. This is applied on BOTH sides before every move.
fn checked_join(base: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut cursor = base.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("Unsafe quarantine path component.".into());
        };
        cursor.push(name);
        match fs::symlink_metadata(&cursor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(format!("Symlink in quarantine path: {}", cursor.display()));
                }
                let resolved = cursor.canonicalize()
                    .map_err(|error| format!("Could not validate path {}: {error}", cursor.display()))?;
                if !resolved.starts_with(base) {
                    return Err("Quarantine path escapes its expected root.".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect path: {error}")),
        }
    }
    Ok(cursor)
}

fn checked_workspace(root: &Path) -> Result<(), String> {
    // Reuse the same actual quarantine workspace root for security checks
    // when a library is scanned from nested Packages/Overrides folders.
    let workspace = workspace_base(root);
    let parent = workspace.parent().unwrap_or(root);
    let rel = Path::new("S3CC Organizer");
    let _ = checked_join(parent, rel)?;
    for name in ["Quarantine", "Quarantine Manifests"] {
        let sub = Path::new("S3CC Organizer").join(name);
        let _ = checked_join(parent, &sub)?;
    }
    Ok(())
}

fn validate_session_dir(root: &Path, input: &Path) -> Result<PathBuf, String> {
    let base = quarantine_base(root);
    let session = input
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Quarantine session has no valid name.".to_string())?;
    if session.len() < 15 || !session.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err("Invalid quarantine session identifier.".into());
    }
    if input != base.join(session) {
        return Err("Quarantine session is outside its workspace.".into());
    }
    checked_workspace(root)?;
    let _ = checked_join(&base, Path::new(session))?;
    Ok(base.join(session))
}

fn manifest_from_plan(plan: &QuarantinePlan, status: &str) -> QuarantineManifest {
    QuarantineManifest {
        version: 1,
        created_at: Local::now().to_rfc3339(),
        status: status.into(),
        root: plan.root.clone(),
        quarantine_root: plan.quarantine_root.clone(),
        items: plan.items.clone(),
    }
}

fn write_manifest_atomic(path: &Path, manifest: &QuarantineManifest) -> Result<(), String> {
    let parent = path.parent().ok_or("Manifest path is missing its parent.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create manifest directory: {error}"))?;
    let data = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Could not encode quarantine manifest: {error}"))?;
    let tmp = parent.join(format!(
        ".quarantine-{}-{}-{}.tmp",
        std::process::id(),
        Local::now().timestamp_micros(),
        manifest.items.len()
    ));
    let result = (|| -> Result<(), String> {
        let mut output = OpenOptions::new()
            .write(true).create_new(true).open(&tmp)
            .map_err(|error| format!("Could not open transaction journal: {error}"))?;
        output.write_all(&data)
            .map_err(|error| format!("Could not write transaction journal: {error}"))?;
        output.sync_all()
            .map_err(|error| format!("Could not flush transaction journal: {error}"))?;
        // std::fs::rename replaces an existing file where supported.
        // Never remove the previous journal before installing the new one.
        fs::rename(&tmp, path)
            .map_err(|error| format!("Could not commit transaction journal: {error}"))?;
        #[cfg(unix)]
        {
            if let Ok(directory) = File::open(parent) {
                let _ = directory.sync_all();
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn read_manifest(root: &Path, input: &str) -> Result<(PathBuf, QuarantineManifest, PathBuf), String> {
    checked_workspace(root)?;
    let parent = manifest_base(root);
    let manifest_path = PathBuf::from(input.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve quarantine manifest: {error}"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("Could not resolve manifest directory: {error}"))?;
    if manifest_path.parent() != Some(canonical_parent.as_path())
        || manifest_path.extension().and_then(|s| s.to_str()) != Some("json")
    {
        return Err("Quarantine manifest is outside its expected directory.".into());
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("Could not read quarantine manifest: {error}"))?;
    let manifest: QuarantineManifest = serde_json::from_str(&text)
        .map_err(|error| format!("Invalid quarantine manifest: {error}"))?;
    if manifest.version != 1 || manifest.items.is_empty() || manifest.items.len() > 100_000 {
        return Err("Invalid quarantine manifest version or item count.".into());
    }
    let original_root = PathBuf::from(&manifest.root).canonicalize()
        .map_err(|error| format!("Could not resolve manifest Mods root: {error}"))?;
    if original_root != root {
        return Err("Quarantine manifest belongs to a different Mods root.".into());
    }
    let qroot = validate_session_dir(root, Path::new(&manifest.quarantine_root))?;
    let mut seen = HashSet::new();
    for item in &manifest.items {
        let src = validated_relative(&item.source_relative_path)?;
        let dst = validated_relative(&item.destination_relative_path)?;
        if !quarantine_path_matches_identity(&src, &dst, &item.sha256) || !seen.insert(src.clone()) {
            return Err("Manifest has conflicting or inconsistent relative paths.".into());
        }
        if item.sha256.len() != 64 || !item.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Manifest has an invalid SHA-256 identity.".into());
        }
        // Source/destination absolute paths are audit fields, never trusted as move targets.
        let _ = checked_join(root, &src)?;
        let _ = checked_join(&qroot, &dst)?;
    }
    Ok((manifest_path, manifest, qroot))
}

// Historical manifests used identical source/destination relative paths.
// Exact-duplicate batches may instead be grouped by identical copy count.
fn quarantine_path_matches_identity(source: &Path, target: &Path, hash: &str) -> bool {
    // A manifest is serialized with Windows separators even when CI/tests run
    // on macOS or Linux. Normalize both sides before matching suffix/depth;
    // a backslash is otherwise a literal filename character on Unix.
    let Ok(source) = validated_relative(&source.to_string_lossy()) else { return false };
    let Ok(target) = validated_relative(&target.to_string_lossy()) else { return false };
    if source == target { return true; }
    if hash.len() < 16 || !target.ends_with(&source) { return false; }
    let components = target.components().filter_map(|part| match part {
        Component::Normal(text) => text.to_str(),
        _ => None,
    }).collect::<Vec<_>>();
    if components.len() != source.components().count() + 3 ||
        components.first() != Some(&"Exact Duplicates")
    {
        return false;
    }
    let count = components[1].strip_suffix(" identical files")
        .and_then(|value| value.parse::<usize>().ok());
    count.is_some_and(|n| (2..=100_000).contains(&n)) &&
        components[2].eq_ignore_ascii_case(&hash[..16])
}

fn quarantine_destination_relative(item: &QuarantinePlanItem) -> Result<PathBuf, String> {
    let source = validated_relative(&item.source_relative_path)?;
    let target = validated_relative(&item.destination_relative_path)?;
    if !quarantine_path_matches_identity(&source, &target, &item.sha256) {
        return Err("Quarantine source/destination mapping is invalid.".to_string());
    }
    Ok(target)
}

fn verified(path: &Path, item: &QuarantinePlanItem) -> Result<(), String> {
    let (hash, size) = sha256_file(path)
        .map_err(|error| format!("Could not verify {}: {error}", path.display()))?;
    if !hash.eq_ignore_ascii_case(&item.sha256) || size != item.size {
        return Err(format!("File differs from quarantine manifest: {}", path.display()));
    }
    Ok(())
}


fn transfer_no_replace(source: &Path, destination: &Path, hash: &str, size: u64) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()));
    }
    let (source_hash, source_size) = sha256_file(source)
        .map_err(|error| format!("Could not verify transfer source: {error}"))?;
    if !source_hash.eq_ignore_ascii_case(hash) || source_size != size {
        return Err(format!("Transfer source changed: {}", source.display()));
    }
    let parent = destination.parent().ok_or("Transfer destination has no parent.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create destination folder: {error}"))?;

    // Hard-link creation is atomic and fails rather than replacing an existing
    // destination. Quarantine is normally on the same volume as Mods.
    match fs::hard_link(source, destination) {
        Ok(()) => {
            let destination_ok = sha256_file(destination)
                .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == size)
                .unwrap_or(false);
            if !destination_ok {
                let _ = fs::remove_file(destination);
                return Err("Hard-linked destination failed identity verification.".into());
            }
            if let Err(error) = fs::remove_file(source) {
                // Both paths still point at preserved content; remove only the
                // link this transaction just created.
                let _ = fs::remove_file(destination);
                return Err(format!("Could not unlink transfer source: {error}"));
            }
            return Ok(());
        }
        Err(link_error) => {
            // Filesystems without hard links use create_new + verified copy.
            // create_new is the no-overwrite guarantee.
            let mut input = File::open(source)
                .map_err(|error| format!("Could not open transfer source: {error}"))?;
            let mut output = OpenOptions::new().write(true).create_new(true).open(destination)
                .map_err(|error| format!("Destination appeared during transfer: {error}"))?;
            let copied = match io::copy(&mut input, &mut output) {
                Ok(copied) => copied,
                Err(error) => {
                    drop(output);
                    let _ = fs::remove_file(destination);
                    return Err(format!("Could not copy transfer data: {error}"));
                }
            };
            if let Err(error) = output.sync_all() {
                drop(output);
                let _ = fs::remove_file(destination);
                return Err(format!("Could not flush transfer destination: {error}"));
            }
            drop(output);
            if copied != size {
                let _ = fs::remove_file(destination);
                return Err(format!("Copied byte count changed ({copied} != {size})."));
            }
            let destination_ok = sha256_file(destination)
                .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == size)
                .unwrap_or(false);
            if !destination_ok {
                let _ = fs::remove_file(destination);
                return Err("Copied destination failed identity verification.".into());
            }
            if let Err(error) = fs::remove_file(source) {
                // Preserve the original; discard only our newly-created copy.
                let _ = fs::remove_file(destination);
                return Err(format!(
                    "Transfer copy succeeded but source removal failed ({error}); hard-link fallback reason: {link_error}"
                ));
            }
            Ok(())
        }
    }
}

/// Verify before moving anything back. A changed quarantined file must NOT
/// silently replace its original location during rollback.
fn rollback_to_sources(moved: &[MoveRecord]) -> bool {
    let mut ok = true;
    for (source, destination, hash, size) in moved.iter().rev() {
        let valid = sha256_file(destination)
            .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == *size)
            .unwrap_or(false);
        if !valid || source.exists()
            || transfer_no_replace(destination, source, hash, *size).is_err() {
            ok = false;
            continue;
        }
        if sha256_file(source)
            .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == *size)
            .unwrap_or(false) == false
        {
            ok = false;
        }
    }
    ok
}

fn rollback_to_quarantine(moved: &[MoveRecord]) -> bool {
    // Each record is (original, quarantined, hash, size).
    let mut ok = true;
    for (source, destination, hash, size) in moved.iter().rev() {
        let valid = sha256_file(source)
            .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == *size)
            .unwrap_or(false);
        if !valid || destination.exists()
            || transfer_no_replace(source, destination, hash, *size).is_err() {
            ok = false;
            continue;
        }
        if sha256_file(destination)
            .map(|(actual, n)| actual.eq_ignore_ascii_case(hash) && n == *size)
            .unwrap_or(false) == false
        {
            ok = false;
        }
    }
    ok
}

fn abort_quarantine(path: &Path, manifest: &mut QuarantineManifest,
    moved: &[MoveRecord], reason: String) -> String {
    let restored = rollback_to_sources(moved);
    manifest.status = if restored { "ROLLED_BACK" } else { "ROLLBACK_INCOMPLETE" }.into();
    if let Err(journal_error) = write_manifest_atomic(path, manifest) {
        format!("{reason}; journal update failed: {journal_error}; recovery is required: {}", path.display())
    } else {
        format!("{reason}; transaction status: {}; manifest: {}", manifest.status, path.display())
    }
}

fn abort_restore(path: &Path, manifest: &mut QuarantineManifest,
    moved: &[MoveRecord], reason: String) -> String {
    let restored = rollback_to_quarantine(moved);
    manifest.status = if restored { "COMPLETE" } else { "RESTORE_INCOMPLETE" }.into();
    if let Err(journal_error) = write_manifest_atomic(path, manifest) {
        format!("{reason}; journal update failed: {journal_error}; recovery is required: {}", path.display())
    } else {
        format!("{reason}; transaction status: {}; manifest: {}", manifest.status, path.display())
    }
}

fn require_package(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("package")).unwrap_or(false)
}

#[tauri::command]
pub fn build_quarantine_plan(folder: String, selected_paths: Vec<String>) -> Result<QuarantinePlan, String> {
    if selected_paths.is_empty() {
        return Err("No packages selected for quarantine.".into());
    }
    let root = canonical_root(&folder)?;
    checked_workspace(&root)?;
    let session = Local::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let qroot = quarantine_base(&root).join(&session);
    if qroot.exists() || manifest_base(&root).join(format!("Quarantine-{session}.json")).exists() {
        return Err("Quarantine session identifier is already in use.".into());
    }

    let mut unique = HashSet::new();
    let mut items = Vec::new();
    let mut stats = QuarantineStats::default();
    for raw in selected_paths {
        let source = PathBuf::from(raw.trim()).canonicalize()
            .map_err(|error| format!("Cannot resolve selected package: {error}"))?;
        if !source.is_file() || !source.starts_with(&root) || !require_package(&source) {
            stats.blocked += 1;
            continue;
        }
        if !unique.insert(source.clone()) {
            continue;
        }
        let relative = source.strip_prefix(&root)
            .map_err(|_| "Selected package is outside Mods root.".to_string())?;
        let _ = checked_join(&root, relative)?;
        let _ = checked_join(&qroot, relative)?;
        let (sha256, size) = sha256_file(&source)
            .map_err(|error| format!("Could not hash selected package: {error}"))?;
        let destination = qroot.join(relative);
        let collision = destination.exists();
        stats.selected += 1;
        if collision {
            stats.blocked += 1;
            stats.collisions += 1;
        } else {
            stats.ready += 1;
        }
        items.push(QuarantinePlanItem {
            source_path: source.to_string_lossy().into_owned(),
            source_relative_path: relative_text(relative),
            destination_path: destination.to_string_lossy().into_owned(),
            destination_relative_path: relative_text(relative),
            sha256,
            size,
            status: if collision { "collision" } else { "ready" }.into(),
            warnings: if collision {
                vec!["Quarantine destination already exists.".into()]
            } else { vec![] },
        });
    }
    items.sort_by_key(|item| item.source_relative_path.to_ascii_lowercase());
    let mut preview = format!(
        "S3CC ORGANIZER QUARANTINE PREVIEW\nmode=PREVIEW\nroot={}\nquarantine_root={}\nfiles={}\n",
        root.display(), qroot.display(), items.len()
    );
    for item in &items {
        preview.push_str(&format!(
            "\n[file]\nsha256={}\nsize={}\noriginal={}\nquarantine={}\n[/file]\n",
            item.sha256, item.size, item.source_relative_path, item.destination_relative_path
        ));
    }
    let read_only = load_workspace_for_root(&root).read_only;
    Ok(QuarantinePlan {
        root: root.to_string_lossy().into_owned(),
        quarantine_root: qroot.to_string_lossy().into_owned(),
        items,
        can_execute: !read_only && stats.ready > 0 && stats.blocked == 0,
        stats,
        manifest_preview: preview,
    })
}

// Exact-copy batch selection is verified against at least one separate,
// live, same-SHA .package survivor for every quarantined file. Re-run the
// verification immediately before moving, not only when building the preview.
fn verify_exact_survivors(
    root: &Path,
    plan: &QuarantinePlan,
    retained_paths: &[String],
) -> Result<HashMap<String, usize>, String> {
    if retained_paths.is_empty() || plan.items.is_empty() {
        return Err("Exact-duplicate quarantine must retain at least one package.".into());
    }
    let canonical_root = root.canonicalize()
        .map_err(|error| format!("Could not resolve exact-duplicate library root: {error}"))?;
    let selected = plan.items.iter()
        .map(|item| PathBuf::from(&item.source_path))
        .collect::<HashSet<_>>();
    let mut retained_hashes = HashMap::<String, usize>::new();
    let mut seen_retained = HashSet::<PathBuf>::new();
    for raw in retained_paths {
        let survivor = PathBuf::from(raw).canonicalize()
            .map_err(|error| format!("Exact-duplicate survivor is missing: {error}"))?;
        if !survivor.is_file() || !survivor.starts_with(&canonical_root) ||
            !require_package(&survivor) || selected.contains(&survivor)
        {
            return Err("Exact-duplicate survivor is invalid or also selected for quarantine.".into());
        }
        if !seen_retained.insert(survivor.clone()) {
            return Err("Duplicate survivor path supplied more than once.".into());
        }
        let (hash, _) = sha256_file(&survivor)
            .map_err(|error| format!("Could not verify retained duplicate: {error}"))?;
        *retained_hashes.entry(hash.to_ascii_uppercase()).or_default() += 1;
    }
    for item in &plan.items {
        if !retained_hashes.contains_key(&item.sha256.to_ascii_uppercase()) {
            return Err(format!(
                "No unselected byte-identical survivor remains for {}. Quarantine blocked.",
                item.source_relative_path
            ));
        }
    }
    Ok(retained_hashes)
}

fn group_exact_duplicate_destinations(
    plan: &mut QuarantinePlan,
    retained_counts: &HashMap<String, usize>,
) -> Result<(), String> {
    let mut moved_counts = HashMap::<String, usize>::new();
    for item in &plan.items {
        *moved_counts.entry(item.sha256.to_ascii_uppercase()).or_default() += 1;
    }
    let root = Path::new(&plan.quarantine_root);
    for item in &mut plan.items {
        let hash = item.sha256.to_ascii_uppercase();
        let total = moved_counts.get(&hash).copied().unwrap_or(0) +
            retained_counts.get(&hash).copied().unwrap_or(0);
        if !(2..=100_000).contains(&total) {
            return Err("Exact duplicate group must retain a verified copy.".into());
        }
        let original = validated_relative(&item.source_relative_path)?;
        let grouped = Path::new("Exact Duplicates")
            .join(format!("{total} identical files"))
            .join(&hash[..16])
            .join(&original);
        let destination = checked_join(root, &grouped)?;
        item.destination_relative_path = relative_text(&grouped);
        item.destination_path = destination.to_string_lossy().into_owned();
    }
    let mut preview = format!(
        "S3CC ORGANIZER EXACT DUPLICATE QUARANTINE\nmode=PREVIEW\nquarantine_root={}\nfiles={}\n",
        root.display(), plan.items.len()
    );
    for item in &plan.items {
        preview.push_str(&format!(
            "\n[file]\nsha256={}\noriginal={}\nquarantine={}\n[/file]\n",
            item.sha256, item.source_relative_path, item.destination_relative_path
        ));
    }
    plan.manifest_preview = preview;
    Ok(())
}

#[tauri::command]
pub fn build_exact_duplicate_quarantine_plan(
    folder: String,
    selected_paths: Vec<String>,
    retained_paths: Vec<String>,
) -> Result<QuarantinePlan, String> {
    let root = canonical_root(&folder)?;
    let plan = build_quarantine_plan(folder, selected_paths)?;
    let retained = verify_exact_survivors(&root, &plan, &retained_paths)?;
    let mut plan = plan;
    group_exact_duplicate_destinations(&mut plan, &retained)?;
    Ok(plan)
}

fn preflight_plan(root: &Path, plan: &QuarantinePlan) -> Result<(), String> {
    let qroot = validate_session_dir(root, Path::new(&plan.quarantine_root))?;
    if qroot.exists() {
        return Err("Quarantine session destination already exists.".into());
    }
    for item in &plan.items {
        let relative = validated_relative(&item.source_relative_path)?;
        let source = checked_join(root, &relative)?;
        let destination = checked_join(&qroot, &quarantine_destination_relative(item)?)?;
        if !source.is_file() || destination.exists() {
            return Err(format!("Quarantine plan is stale for {}", source.display()));
        }
        verified(&source, item)?;
    }
    Ok(())
}

#[tauri::command]
pub fn execute_quarantine(
    folder: String,
    selected_paths: Vec<String>,
    planned_quarantine_root: Option<String>,
) -> Result<QuarantineResult, String> {
    let _guard = transaction_guard()?;
    execute_quarantine_core(folder, selected_paths, planned_quarantine_root, None)
}

#[tauri::command]
pub fn execute_exact_duplicate_quarantine(
    folder: String,
    selected_paths: Vec<String>,
    planned_quarantine_root: Option<String>,
    retained_paths: Vec<String>,
) -> Result<QuarantineResult, String> {
    let _guard = transaction_guard()?;
    execute_quarantine_core(folder, selected_paths, planned_quarantine_root, Some(retained_paths))
}

fn execute_quarantine_core(
    folder: String,
    selected_paths: Vec<String>,
    planned_quarantine_root: Option<String>,
    retained_paths: Option<Vec<String>>,
) -> Result<QuarantineResult, String> {
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let mut plan = build_quarantine_plan(folder, selected_paths)?;
    if !plan.can_execute {
        return Err("Quarantine is blocked by preflight checks.".into());
    }
    if let Some(ref survivors) = retained_paths {
        let retained = verify_exact_survivors(&root, &plan, survivors)?;
        group_exact_duplicate_destinations(&mut plan, &retained)?;
    }
    if let Some(requested) = planned_quarantine_root {
        let qroot = validate_session_dir(&root, Path::new(&requested))?;
        plan.quarantine_root = qroot.to_string_lossy().into_owned();
        for item in &mut plan.items {
            let relative = quarantine_destination_relative(item)?;
            item.destination_path = checked_join(&qroot, &relative)?.to_string_lossy().into_owned();
        }
    }
    preflight_plan(&root, &plan)?;
    let qroot = PathBuf::from(&plan.quarantine_root);
    let session = qroot.file_name().and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid quarantine session.".to_string())?;
    let manifest_path = manifest_base(&root).join(format!("Quarantine-{session}.json"));
    if manifest_path.exists() {
        return Err("Quarantine manifest already exists.".into());
    }
    let mut journal = manifest_from_plan(&plan, "PENDING");
    write_manifest_atomic(&manifest_path, &journal)?; // durable BEFORE any move
    let mut moved: Vec<MoveRecord> = Vec::new();
    for item in &plan.items {
        let relative = match validated_relative(&item.source_relative_path) {
            Ok(relative) => relative,
            Err(error) => return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error)),
        };
        let source = match checked_join(&root, &relative) {
            Ok(path) => path,
            Err(error) => return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error)),
        };
        let quarantine_relative = match quarantine_destination_relative(item) {
            Ok(relative) => relative,
            Err(error) => return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error)),
        };
        let destination = match checked_join(&qroot, &quarantine_relative) {
            Ok(path) => path,
            Err(error) => return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error)),
        };
        if destination.exists() || !source.is_file() {
            return Err(abort_quarantine(&manifest_path, &mut journal, &moved,
                format!("Source or destination changed: {}", source.display())));
        }
        if let Err(error) = verified(&source, item) {
            return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error));
        }
        if let Some(parent) = destination.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return Err(abort_quarantine(&manifest_path, &mut journal, &moved,
                    format!("Could not create quarantine folder: {error}")));
            }
        }
        if let Err(error) = checked_join(&qroot, &quarantine_relative) {
            return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error));
        }
        if let Err(error) = transfer_no_replace(&source, &destination, &item.sha256, item.size) {
            if destination.exists() {
                moved.push((source.clone(), destination.clone(), item.sha256.clone(), item.size));
            }
            return Err(abort_quarantine(&manifest_path, &mut journal, &moved,
                format!("Could not move package without overwrite: {error}")));
        }
        moved.push((source, destination.clone(), item.sha256.clone(), item.size));
    }
    journal.status = "COMPLETE".into();
    if let Err(error) = write_manifest_atomic(&manifest_path, &journal) {
        return Err(abort_quarantine(&manifest_path, &mut journal, &moved, error));
    }
    // The files are quarantined, never deleted. Reconcile the selected tree
    // after the verified move so nested legacy folders and their empty
    // ancestors do not remain as shells. Protected Packages/Overrides roots
    // are preserved by the shared cleanup routine.
    let (cleanup_removed, _, cleanup_warnings) =
        crate::executor::cleanup_empty_directories_after_organization(&root, &[]);
    Ok(QuarantineResult {
        moved: moved.len(),
        quarantine_root: plan.quarantine_root,
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        status: "COMPLETE".into(),
        cleanup_removed,
        cleanup_warnings,
    })
}

#[tauri::command]
pub fn restore_quarantine(
    folder: String,
    manifest_path: String,
    confirmed: bool,
) -> Result<QuarantineResult, String> {
    if !confirmed {
        return Err("Quarantine restore requires explicit confirmation.".to_string());
    }
    let _guard = transaction_guard()?;
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let (path, mut journal, qroot) = read_manifest(&root, &manifest_path)?;
    if journal.status != "COMPLETE" {
        return Err(format!("Manifest status {} requires recovery or is already restored.", journal.status));
    }
    for item in &journal.items {
        let relative = validated_relative(&item.source_relative_path)?;
        let source = checked_join(&root, &relative)?;
        let quarantined = checked_join(&qroot, &quarantine_destination_relative(item)?)?;
        if source.exists() || !quarantined.is_file() {
            return Err(format!("Restore blocked by occupied/missing file: {}", source.display()));
        }
        verified(&quarantined, item)?;
    }
    journal.status = "RESTORE_PENDING".into();
    write_manifest_atomic(&path, &journal)?; // durable BEFORE any restore move
    let mut restored: Vec<MoveRecord> = Vec::new();
    let items = journal.items.clone();
    for item in &items {
        let relative = match validated_relative(&item.source_relative_path) {
            Ok(relative) => relative,
            Err(error) => return Err(abort_restore(&path, &mut journal, &restored, error)),
        };
        let source = match checked_join(&root, &relative) {
            Ok(path) => path,
            Err(error) => return Err(abort_restore(&path, &mut journal, &restored, error)),
        };
        let quarantined = match checked_join(&qroot, &quarantine_destination_relative(item)?) {
            Ok(path) => path,
            Err(error) => return Err(abort_restore(&path, &mut journal, &restored, error)),
        };
        if source.exists() || !quarantined.is_file() {
            return Err(abort_restore(&path, &mut journal, &restored,
                format!("Restore destination changed: {}", source.display())));
        }
        if let Err(error) = verified(&quarantined, item) {
            return Err(abort_restore(&path, &mut journal, &restored, error));
        }
        if let Some(parent) = source.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return Err(abort_restore(&path, &mut journal, &restored,
                    format!("Could not create restore folder: {error}")));
            }
        }
        if let Err(error) = checked_join(&root, &relative) {
            return Err(abort_restore(&path, &mut journal, &restored, error));
        }
        if let Err(error) = transfer_no_replace(&quarantined, &source, &item.sha256, item.size) {
            if source.exists() {
                restored.push((source.clone(), quarantined.clone(), item.sha256.clone(), item.size));
            }
            return Err(abort_restore(&path, &mut journal, &restored,
                format!("Could not restore file without overwrite: {error}")));
        }
        restored.push((source.clone(), quarantined, item.sha256.clone(), item.size));
    }
    journal.status = "RESTORED".into();
    if let Err(error) = write_manifest_atomic(&path, &journal) {
        return Err(abort_restore(&path, &mut journal, &restored, error));
    }
    // Never delete user content or unexpected files under quarantine.
    Ok(QuarantineResult {
        moved: restored.len(),
        quarantine_root: qroot.to_string_lossy().into_owned(),
        manifest_path: path.to_string_lossy().into_owned(),
        status: "RESTORED".into(),
        cleanup_removed: 0,
        cleanup_warnings: Vec::new(),
    })
}

/// Recover an interrupted quarantine or restore by returning every validated
/// file to its ORIGINAL location. Refuse ambiguity, corruption or overwrite.
#[tauri::command]
pub fn recover_quarantine(folder: String, manifest_path: String) -> Result<QuarantineResult, String> {
    let _guard = transaction_guard()?;
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;
    let (path, mut journal, qroot) = read_manifest(&root, &manifest_path)?;
    let prior = journal.status.clone();
    if !matches!(prior.as_str(),
        "PENDING" | "ROLLBACK_INCOMPLETE" | "RESTORE_PENDING" | "RESTORE_INCOMPLETE")
    {
        return Err(format!("Manifest status {prior} is not an interrupted operation."));
    }

    // Entire set MUST pass preflight before recovering even the first file.
    let mut to_restore = Vec::<(PathBuf, PathBuf, QuarantinePlanItem)>::new();
    for item in &journal.items {
        let relative = validated_relative(&item.source_relative_path)?;
        let source = checked_join(&root, &relative)?;
        let quarantined = checked_join(&qroot, &quarantine_destination_relative(item)?)?;
        if source.is_file() && !quarantined.exists() {
            verified(&source, item)?;
        } else if !source.exists() && quarantined.is_file() {
            verified(&quarantined, item)?;
            to_restore.push((source, quarantined, item.clone()));
        } else {
            return Err(format!("Recovery cannot resolve missing/occupied pair: {}", source.display()));
        }
    }

    let mut moved: Vec<MoveRecord> = Vec::new();
    for (source, quarantined, item) in &to_restore {
        if let Some(parent) = source.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return Err(format!("Recovery stopped; retry from manifest {}: {error}", path.display()));
            }
        }
        let relative = validated_relative(&item.source_relative_path)?;
        let _ = checked_join(&root, &relative)?;
        if source.exists() || !quarantined.is_file() {
            return Err(format!("Recovery stopped due to a path change; manifest: {}", path.display()));
        }
        verified(quarantined, item)?;
        transfer_no_replace(quarantined, source, &item.sha256, item.size)
            .map_err(|error| format!("Recovery stopped; retry from manifest {}: {error}", path.display()))?;
        moved.push((source.clone(), quarantined.clone(), item.sha256.clone(), item.size));
    }
    journal.status = if prior.starts_with("RESTORE") { "RESTORED" } else { "ROLLED_BACK" }.into();
    write_manifest_atomic(&path, &journal)?;
    Ok(QuarantineResult {
        moved: moved.len(),
        quarantine_root: qroot.to_string_lossy().into_owned(),
        manifest_path: path.to_string_lossy().into_owned(),
        status: journal.status,
        cleanup_removed: 0,
        cleanup_warnings: Vec::new(),
    })
}


/// Delete only a completed transaction journal. Never remove the quarantined
/// CCs, and never remove a journal required for restore or crash recovery.
#[tauri::command]
pub fn remove_quarantine_history(
    folder: String,
    manifest_path: String,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("Removing quarantine history requires explicit confirmation.".into());
    }
    let _guard = transaction_guard()?;
    let root = canonical_root(&folder)?;
    ensure_writable(&root)?;

    let submitted = PathBuf::from(manifest_path.trim());
    let metadata = fs::symlink_metadata(&submitted)
        .map_err(|error| format!("Could not inspect quarantine manifest: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Only regular quarantine manifests may be removed.".into());
    }

    // read_manifest verifies the managed directory, the selected Mods root,
    // the session quarantine directory, and the paths/hashes of its members.
    let (path, journal, quarantine_root) =
        read_manifest(&root, &submitted.to_string_lossy())?;
    if !matches!(journal.status.as_str(), "RESTORED" | "ROLLED_BACK") {
        return Err(
            "Quarantine is active or still needs recovery. Restore or recover it before removing its history.".into(),
        );
    }

    // A completed journal is disposable only if none of its managed files
    // remain in quarantine. Never delete a journal needed to recover CCs.
    for item in &journal.items {
        let relative = validated_relative(&item.source_relative_path)?;
        let quarantined = checked_join(&quarantine_root, &quarantine_destination_relative(item)?)?;
        if quarantined.exists() {
            return Err(format!(
                "Quarantined content still exists; record removal was blocked: {}",
                quarantined.display()
            ));
        }
    }

    fs::remove_file(&path)
        .map_err(|error| format!("Could not remove quarantine history record: {error}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn copied_mods_subtree_quarantine_stays_outside_loading_branches() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("s3cc-quarantine-copy-{}-{nonce}",std::process::id()));
        let mods = base.join("Mods - Copia");
        std::fs::create_dir_all(mods.join("Packages/Legacy")).unwrap();
        std::fs::create_dir_all(mods.join("Overrides")).unwrap();
        let selected = mods.join("Packages/Legacy");
        assert_eq!(workspace_base(&selected), mods.join("S3CC Organizer"));
        assert!(!workspace_base(&selected).starts_with(mods.join("Packages")));
        std::fs::remove_dir_all(base).unwrap();
    }

    use super::*;

    #[test]
    fn nested_package_and_override_quarantine_remain_outside_loading_branches() {
        let mods = Path::new("Game").join("Mods");
        let packages = mods.join("Packages").join("CAS").join("Clothing");
        let overrides = mods.join("Overrides").join("Gameplay");
        let expected = mods.join("S3CC Organizer");
        assert_eq!(workspace_base(&packages), expected);
        assert_eq!(workspace_base(&overrides), expected);
        assert_eq!(workspace_base(&mods.join("Packages")), expected);
        assert_eq!(workspace_base(&mods), Path::new("Game").join("S3CC Organizer"));
    }

    #[test]
    fn exact_batch_cannot_quarantine_the_last_identical_copy() {
        let mods = isolated_mods_root();
        let first = test_package(&mods, "Jonha_BASE.package", b"same complete package");
        let duplicate = test_package(&mods, "Jonha_Sliders_BASE.package", b"same complete package");
        let one = vec![duplicate.to_string_lossy().to_string()];
        let retain = vec![first.to_string_lossy().to_string()];
        let plan = build_exact_duplicate_quarantine_plan(
            mods.to_string_lossy().to_string(), one.clone(), retain.clone()
        ).unwrap();
        assert_eq!(plan.stats.ready, 1);
        assert!(verify_exact_survivors(&mods, &plan, &retain).is_ok());
        assert!(verify_exact_survivors(&mods, &plan, &one).is_err());
        fs::remove_file(&first).unwrap();
        assert!(verify_exact_survivors(&mods, &plan, &retain).is_err());
        cleanup(&mods);
    }

    #[test]
    fn exact_three_copy_group_creates_a_labeled_folder_and_restores_both_files() {
        let mods = isolated_mods_root();
        let kept = test_package(&mods, "Keeper.package", b"three identical packages");
        let first = test_package(&mods, "CopyA.package", b"three identical packages");
        let second = test_package(&mods, "CopyB.package", b"three identical packages");
        let selected = vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ];
        let retained = vec![kept.to_string_lossy().to_string()];
        let folder = mods.to_string_lossy().to_string();
        let plan = build_exact_duplicate_quarantine_plan(
            folder.clone(), selected.clone(), retained.clone()
        ).unwrap();
        assert!(plan.can_execute);
        assert_eq!(plan.stats.ready, 2);
        assert!(plan.items.iter().all(|item| {
            item.destination_relative_path.contains("3 identical files")
                && item.destination_relative_path.contains("Exact Duplicates")
        }));
        assert!(plan.manifest_preview.contains("3 identical files"));
        assert!(quarantine_path_matches_identity(
            Path::new(&plan.items[0].source_relative_path),
            &validated_relative(&plan.items[0].destination_relative_path).unwrap(),
            &plan.items[0].sha256
        ));
        let result = execute_exact_duplicate_quarantine(
            folder.clone(),
            selected,
            Some(plan.quarantine_root.clone()),
            retained
        ).unwrap();
        assert_eq!(result.moved, 2);
        assert!(kept.exists());
        assert!(!first.exists() && !second.exists());
        for item in &plan.items {
            assert!(Path::new(&plan.quarantine_root)
                .join(validated_relative(&item.destination_relative_path).unwrap()).exists());
        }
        let restored = restore_quarantine(folder, result.manifest_path, true).unwrap();
        assert_eq!(restored.moved, 2);
        assert!(kept.is_file() && first.is_file() && second.is_file());
        assert!(!quarantine_path_matches_identity(
            Path::new("CAS/CopyA.package"),
            Path::new("Exact Duplicates/3 identical files/0000000000000000/CAS/CopyA.package"),
            &plan.items[0].sha256
        ));
        cleanup(&mods);
    }

    #[test]
    fn normalize_legacy_windows_paths_and_reject_traversal() {
        assert_eq!(
            validated_relative(r"CAS\Hair\one.package").unwrap(),
            PathBuf::from("CAS").join("Hair").join("one.package")
        );
        assert!(validated_relative(r"CAS\..\secret.package").is_err());
        assert!(validated_relative(r"C:\outside.package").is_err());
        assert!(validated_relative(r"CAS\CON\bad.package").is_err());
        assert!(validated_relative(r"CAS\\double.package").is_err());
    }

    #[test]
    fn transaction_journal_is_serializable_and_recoverable() {
        let item = QuarantinePlanItem {
            source_path: "ignored".into(),
            source_relative_path: r"CAS\Hair\one.package".into(),
            destination_path: "ignored".into(),
            destination_relative_path: r"CAS\Hair\one.package".into(),
            sha256: "a".repeat(64),
            size: 100,
            status: "ready".into(),
            warnings: vec![],
        };
        let manifest = QuarantineManifest {
            version: 1, created_at: "test".into(), status: "RESTORE_PENDING".into(),
            root: "Mods".into(), quarantine_root: "Quarantine".into(),
            items: vec![item],
        };
        let decoded: QuarantineManifest =
            serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(decoded.status, "RESTORE_PENDING");
        assert_eq!(decoded.items.len(), 1);
    }

    #[test]
    fn session_paths_cannot_escape_workspace() {
        let root = PathBuf::from("C:/Mods");
        assert!(validate_session_dir(&root, Path::new("C:/evil/20260930-123456-00001")).is_err());
    }


    fn isolated_mods_root() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let folder = std::env::temp_dir().join(format!(
            "s3cc-quarantine-{}-{}-{}",
            std::process::id(),
            Local::now().timestamp_micros(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mods = folder.join("Mods");
        fs::create_dir_all(mods.join("CAS")).unwrap();
        mods
    }

    fn test_package(root: &Path, name: &str, data: &[u8]) -> PathBuf {
        let path = root.join("CAS").join(name);
        fs::write(&path, data).unwrap();
        path
    }

    fn cleanup(mods: &Path) {
        if let Some(parent) = mods.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn quarantine_restore_preserves_bytes_and_paths() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "exact.package", b"sample-one");
        let root_text = mods.to_string_lossy().to_string();
        let plan = build_quarantine_plan(root_text.clone(),
            vec![source.to_string_lossy().into_owned()]).unwrap();
        assert!(plan.can_execute);
        let result = execute_quarantine(root_text.clone(),
            vec![source.to_string_lossy().into_owned()],
            Some(plan.quarantine_root)).unwrap();
        assert_eq!(result.moved, 1);
        assert!(!source.exists());
        let quarantined = PathBuf::from(&result.quarantine_root)
            .join("CAS").join("exact.package");
        assert_eq!(fs::read(&quarantined).unwrap(), b"sample-one");
        let restored = restore_quarantine(root_text, result.manifest_path.clone(), true).unwrap();
        assert_eq!(restored.status, "RESTORED");
        assert_eq!(fs::read(&source).unwrap(), b"sample-one");
        assert!(!quarantined.exists());
        cleanup(&mods);
    }

    #[test]
    fn interrupted_quarantine_can_be_recovered_without_overwrite() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "pending.package", b"recover-me");
        let root_text = mods.to_string_lossy().to_string();
        let plan = build_quarantine_plan(root_text.clone(),
            vec![source.to_string_lossy().into_owned()]).unwrap();
        let qroot = PathBuf::from(&plan.quarantine_root);
        let session = qroot.file_name().unwrap().to_string_lossy();
        let manifest_path = manifest_base(&mods)
            .join(format!("Quarantine-{session}.json"));
        write_manifest_atomic(&manifest_path, &manifest_from_plan(&plan, "PENDING")).unwrap();
        fs::create_dir_all(qroot.join("CAS")).unwrap();
        fs::rename(&source, qroot.join("CAS/pending.package")).unwrap();
        let recovered = recover_quarantine(root_text,
            manifest_path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(recovered.status, "ROLLED_BACK");
        assert_eq!(fs::read(&source).unwrap(), b"recover-me");
        cleanup(&mods);
    }

    #[test]
    fn changed_quarantined_file_blocks_restore() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "tamper.package", b"original");
        let root_text = mods.to_string_lossy().to_string();
        let result = execute_quarantine(root_text.clone(),
            vec![source.to_string_lossy().into_owned()], None).unwrap();
        let quarantined = PathBuf::from(&result.quarantine_root)
            .join("CAS").join("tamper.package");
        fs::write(&quarantined, b"tampered").unwrap();
        assert!(restore_quarantine(root_text, result.manifest_path, true).is_err());
        assert!(!source.exists());
        assert_eq!(fs::read(&quarantined).unwrap(), b"tampered");
        cleanup(&mods);
    }

    #[test]
    fn occupied_restore_destination_remains_untouched() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "occupied.package", b"original");
        let root_text = mods.to_string_lossy().to_string();
        let result = execute_quarantine(root_text.clone(),
            vec![source.to_string_lossy().into_owned()], None).unwrap();
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"later-user-file").unwrap();
        assert!(restore_quarantine(root_text, result.manifest_path, true).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"later-user-file");
        cleanup(&mods);
    }

    #[test]
    fn restore_requires_explicit_confirmation() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "confirmation.package", b"keep-quarantined");
        let root_text = mods.to_string_lossy().to_string();
        let result = execute_quarantine(root_text.clone(),
            vec![source.to_string_lossy().into_owned()], None).unwrap();
        let denied = restore_quarantine(root_text, result.manifest_path, false);
        assert!(denied.is_err());
        assert!(!source.exists());
        assert!(PathBuf::from(result.quarantine_root)
            .join("CAS").join("confirmation.package").exists());
        cleanup(&mods);
    }

    #[test]
    fn no_replace_transfer_refuses_existing_destination() {
        let mods = isolated_mods_root();
        let source = test_package(&mods, "source.package", b"source");
        let destination = mods.join("CAS").join("destination.package");
        fs::write(&destination, b"keep-me").unwrap();
        let (hash, size) = sha256_file(&source).unwrap();
        assert!(transfer_no_replace(&source, &destination, &hash, size).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&destination).unwrap(), b"keep-me");
        cleanup(&mods);
    }

    #[test]
    fn failed_later_step_rolls_back_an_earlier_move() {
        let mods = isolated_mods_root();
        let first = test_package(&mods, "first.package", b"first");
        let second = test_package(&mods, "second.package", b"second");
        let plan = build_quarantine_plan(mods.to_string_lossy().into_owned(), vec![
            first.to_string_lossy().into_owned(), second.to_string_lossy().into_owned()
        ]).unwrap();
        let qroot = PathBuf::from(&plan.quarantine_root);
        let session = qroot.file_name().unwrap().to_string_lossy();
        let journal_path = manifest_base(&mods)
            .join(format!("Quarantine-{session}.json"));
        let mut journal = manifest_from_plan(&plan, "PENDING");
        write_manifest_atomic(&journal_path, &journal).unwrap();
        let dest = qroot.join("CAS").join("first.package");
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::rename(&first, &dest).unwrap();
        // Inject a failure on step 2. The first move must be reverted.
        let (hash, size) = sha256_file(&dest).unwrap();
        let moved = vec![(first.clone(), dest.clone(), hash, size)];
        let _ = abort_quarantine(&journal_path, &mut journal, &moved,
            "injected second-step failure".into());
        assert_eq!(journal.status, "ROLLED_BACK");
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&second).unwrap(), b"second");
        assert!(!dest.exists());
        cleanup(&mods);
    }
}
