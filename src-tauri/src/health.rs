use crate::{
    cache::{get_or_build_analysis_with_metrics, load_cache, retain_existing, save_cache},
    operation,
    resource_cfg::{find_resource_cfg, package_priority, parse_resource_cfg, ResourceCfgInfo},
    structure_manager::{append_log, ManualOperationRecord},
    workspace::{active_profile, ensure_writable, is_protected, load_workspace_for_root},
};
use serde::Serialize;
use std::{collections::HashSet, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct CoverageEntry {
    pub path:String,
    pub relative_path:String,
    pub covered:bool,
    pub coverage_known:bool,
    pub priority:Option<i32>,
    pub rule:Option<String>,
    pub depth:usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HealthFinding {
    pub id: String,
    pub category: String,
    pub severity: String,
    pub relative_path: String,
    pub detail: String,
}

fn finding(category: &str, severity: &str, path: &str, detail: impl Into<String>) -> HealthFinding {
    let detail = detail.into();
    HealthFinding {
        id: format!("{category}|{path}|{detail}"),
        category: category.to_string(),
        severity: severity.to_string(),
        relative_path: path.to_string(),
        detail,
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all="camelCase")]
pub struct HealthStats {
    pub packages:usize,
    pub readable:usize,
    pub unreadable:usize,
    pub empty_folders:usize,
    pub resource_cfg_uncovered:usize,
    pub packages_outside_root:usize,
    pub invalid_headers:usize,
    pub unsupported_versions:usize,
    pub damaged_indices:usize,
    pub empty_packages:usize,
    pub repeated_tgis:usize,
    pub disabled_files:usize,
    pub dbc_not_analyzed:usize,
    pub scan_errors:usize,
    pub resource_cfg_errors:usize,
    pub cache_hits:usize,
    pub cache_misses:usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct ModsHealthReport {
    pub root:String,
    pub stats:HealthStats,
    pub empty_folders:Vec<String>,
    pub unreadable_packages:Vec<String>,
    pub outside_packages:Vec<String>,
    pub coverage:Vec<CoverageEntry>,
    pub resource_cfg:Option<ResourceCfgInfo>,
    pub resource_cfg_error:Option<String>,
    pub findings:Vec<HealthFinding>,
}

fn canonical_root(folder:&str)->Result<PathBuf,String>{
    let root=PathBuf::from(folder.trim()).canonicalize().map_err(|e|format!("Could not resolve root: {e}"))?;
    if !root.is_dir(){return Err("Selected root is not a directory.".into());}
    Ok(root)
}
fn is_package(path:&Path)->bool{
    path.extension().and_then(|v|v.to_str()).map(|v|v.eq_ignore_ascii_case("package")).unwrap_or(false)
}
fn rel(root:&Path,path:&Path)->String{
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('/',"\\")
}
fn empty_dirs(root:&Path)->Vec<String>{
    let mut out=Vec::new();
    for e in walkdir::WalkDir::new(root).follow_links(false).min_depth(1).into_iter().filter_map(Result::ok){
        if !e.file_type().is_dir(){continue;}
        let is_empty=fs::read_dir(e.path()).map(|mut it|it.next().is_none()).unwrap_or(false);
        if is_empty{out.push(rel(root,e.path()));}
    }
    out.sort_by_key(|v|v.to_ascii_lowercase());out
}
fn outside_packages(root:&Path)->Vec<String>{
    let Some(mods_root)=root
        .ancestors()
        .find(|candidate| crate::resource_cfg::is_mods_layout_root(candidate))
    else{return Vec::new();};

    let mut out=Vec::new();
    let mut visited=std::collections::HashSet::new();
    for branch in [mods_root.join("Packages"), mods_root.join("Overrides")] {
        if !branch.is_dir(){continue;}
        for item in walkdir::WalkDir::new(&branch).follow_links(false).into_iter() {
            let Ok(entry)=item else { continue };
            if !entry.file_type().is_file() || !is_package(entry.path()) || entry.path().starts_with(root){continue;}
            let Ok(path)=entry.path().canonicalize() else { continue };
            if visited.insert(path.clone()) {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
    out.sort_by_key(|v|v.to_ascii_lowercase());out
}
#[tauri::command]
fn remove_empty_folder_core(folder:String,relative_path:String)->Result<bool,String>{
    let root=canonical_root(&folder)?;
    ensure_writable(&root)?;
    let normalized=relative_path.replace('\\',"/");
    let relative=Path::new(&normalized);
    if relative.is_absolute() || relative.components().any(|c|matches!(c,std::path::Component::ParentDir|std::path::Component::RootDir|std::path::Component::Prefix(_))){
        return Err("Unsafe empty-folder path.".into());
    }
    let profile=active_profile(&load_workspace_for_root(&root));
    if is_protected(&relative_path,&profile){
        return Err("The selected folder is protected by the active workspace profile.".into());
    }
    let candidate=root.join(relative);
    let meta=match fs::symlink_metadata(&candidate){
        Ok(meta)=>meta,
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(error)=>return Err(format!("Could not inspect folder: {error}")),
    };
    if meta.file_type().is_symlink() || !meta.is_dir(){return Err("Target is not a regular directory.".into());}
    let canonical=candidate.canonicalize().map_err(|e|e.to_string())?;
    if !canonical.starts_with(&root)||canonical==root{return Err("Target is outside the selected root.".into());}
    let mut entries=match fs::read_dir(&canonical){
        Ok(entries)=>entries,
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(error)=>return Err(format!("Could not inspect folder contents: {error}")),
    };
    if entries.next().is_some(){return Err("Folder is not empty.".into());}
    match fs::remove_dir(&canonical){
        Ok(())=>{}
        Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
        Err(error)=>return Err(format!("Could not remove empty folder: {error}")),
    }
    let _ = append_log(
        &root,
        &ManualOperationRecord {
            created_at: chrono::Local::now().to_rfc3339(),
            operation: "remove_empty_folder".to_string(),
            source_relative_path: Some(relative_path),
            destination_relative_path: None,
        },
    );
    Ok(true)
}

#[tauri::command(rename = "remove_empty_folder")]
pub async fn remove_empty_folder(folder: String, relative_path: String) -> Result<bool, String> {
    let scope = format!("{}\\{}", folder, relative_path);
    operation::run_mutating_blocking_scoped(
        "folder_write",
        "removing_empty_folder",
        &[scope.as_str()],
        move || remove_empty_folder_core(folder, relative_path),
    )
    .await
}

// Error classification is intentionally conservative. A failed read is not
// automatically proof of a corrupt DBPF, and a valid empty package is not corrupt.
fn dbpf_error_category(message: &str) -> &'static str {
    if message.contains("file header is smaller than 96 bytes")
        || message.contains("file magic does not match DBPF") {
        "invalid_header"
    } else if message.contains("unsupported DBPF major version")
        || message.contains("unsupported DBPF index version") {
        "unsupported_version"
    } else if message.contains("corrupted DBPF index length")
        || message.contains("DBPF index points outside the file")
        || message.contains("DBPF has entries but no index position")
        || message.contains("DBPF index range overflow")
        || message.contains("resource range overflow")
        || (message.starts_with("resource ") && message.contains("points outside the file")) {
        "damaged_index"
    } else {
        "unreadable_package"
    }
}

fn is_disabled_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".package.disabled")
        || name.ends_with(".package.duplicated")
        || name.ends_with(".package.off")
        || name.ends_with(".dbc.disabled")
}

fn is_unanalyzed_cache(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".dbc") || name.ends_with(".ebc")
}

fn analyze_mods_health_core(folder:String)->Result<ModsHealthReport,String>{
    let root=canonical_root(&folder)?;
    let cfg_path=find_resource_cfg(&root);
    let mut findings=Vec::new();
    let mut stats=HealthStats::default();
    let (resource_cfg,resource_cfg_error)=match cfg_path.as_ref(){
        Some(path)=>match parse_resource_cfg(path){
            Ok(cfg)=>(Some(cfg),None),
            Err(error)=>{
                stats.resource_cfg_errors+=1;
                findings.push(finding("resource_cfg_error","error","Resource.cfg",error.clone()));
                (None,Some(error))
            }
        },
        None=>{
            findings.push(finding(
                "missing_resource_cfg","warning","Resource.cfg",
                "No Resource.cfg found; package load coverage cannot be verified.",
            ));
            (None,None)
        }
    };
    if let Some(cfg)=resource_cfg.as_ref() {
        for warning in &cfg.warnings {
            findings.push(finding("resource_cfg_warning","warning","Resource.cfg",warning.clone()));
        }
    }
    let cfg_dir=cfg_path.as_ref().and_then(|p|p.parent()).map(PathBuf::from);

    let mut coverage=Vec::new();
    let mut unreadable=Vec::new();
    let mut package_paths=Vec::new();
    let mut cache=load_cache(&root);

    let mut processed_files = 0usize;
    for item in walkdir::WalkDir::new(&root).follow_links(false).into_iter() {
        if operation::is_cancelled("health") {
            return Err(operation::CANCELLED_ERROR.to_string());
        }
        let e=match item {
            Ok(entry)=>entry,
            Err(error)=>{
                stats.scan_errors+=1;
                let path=error.path().map(|p|rel(&root,p)).unwrap_or_default();
                findings.push(finding("scan_error","warning",&path,error.to_string()));
                continue;
            }
        };
        if !e.file_type().is_file(){continue;}
        processed_files += 1;
        operation::update("health", processed_files, Some(rel(&root, e.path())), "analyzing");
        let name=e.file_name().to_string_lossy();
        let relative=rel(&root,e.path());
        if is_disabled_file(&name) {
            stats.disabled_files+=1;
            findings.push(finding("disabled_file","info",&relative,
                "File is disabled and is not loaded as an active package."));
            continue;
        }
        if is_unanalyzed_cache(&name) {
            stats.dbc_not_analyzed+=1;
            findings.push(finding("dbc_not_analyzed","info",&relative,
                "DBC/EBC cache file found but not inspected as a DBPF package by Health."));
            continue;
        }
        if !is_package(e.path()){continue;}
        stats.packages+=1;
        package_paths.push(e.path().to_path_buf());

        match get_or_build_analysis_with_metrics(e.path(),&mut cache) {
            Ok((cached,metrics))=>{
                if metrics.cache_hit {stats.cache_hits+=1;} else {stats.cache_misses+=1;}
                if let Some(error)=cached.parse_error {
                    stats.unreadable+=1;
                    unreadable.push(relative.clone());
                    let category=dbpf_error_category(&error);
                    match category {
                        "invalid_header"=>stats.invalid_headers+=1,
                        "unsupported_version"=>stats.unsupported_versions+=1,
                        "damaged_index"=>stats.damaged_indices+=1,
                        _=>{},
                    }
                    findings.push(finding(category,"error",&relative,error));
                } else {
                    stats.readable+=1;
                    if cached.resources.is_empty() {
                        stats.empty_packages+=1;
                        findings.push(finding("empty_package","info",&relative,
                            "Valid DBPF package with zero indexed resources."));
                    }
                    let mut seen=HashSet::new();
                    for resource in &cached.resources {
                        if !seen.insert((resource.type_id,resource.group,resource.instance)) {
                            stats.repeated_tgis+=1;
                            findings.push(finding("repeated_tgi","warning",&relative,
                                format!("Repeated TGI within this package: 0x{:08X}-0x{:08X}-0x{:016X}",resource.type_id,resource.group,resource.instance)));
                        }
                    }
                }
            }
            Err(error)=>{
                stats.scan_errors+=1;
                findings.push(finding("scan_error","warning",&relative,error));
            }
        }

        let priority=match (&resource_cfg,&cfg_dir){
            (Some(cfg),Some(dir))=>package_priority(cfg,dir,e.path()),
            _=>None
        };
        let coverage_known=resource_cfg.is_some() && resource_cfg_error.is_none();
        let covered=coverage_known && priority.is_some();
        if coverage_known && !covered {
            stats.resource_cfg_uncovered+=1;
            findings.push(finding("uncovered_resource_cfg","warning",&relative,
                "No PackedFile rule covers this package."));
        }
        coverage.push(CoverageEntry{
            path:e.path().to_string_lossy().to_string(),
            depth:Path::new(&relative).components().count().saturating_sub(1),
            relative_path:relative,
            covered,
            coverage_known,
            priority:priority.as_ref().map(|p|p.priority),
            rule:priority.map(|p|p.rule),
        });
    }
    retain_existing(&mut cache,&package_paths);
    if let Err(error)=save_cache(&root,&cache){
        findings.push(finding("cache_warning","warning","Health cache",error));
    }
    coverage.sort_by_key(|e|e.relative_path.to_ascii_lowercase());
    let empty=empty_dirs(&root);
    stats.empty_folders=empty.len();
    for path in &empty {
        findings.push(finding("empty_folder","info",path,
            "Directory is empty; removal requires explicit confirmation."));
    }
    let outside=outside_packages(&root);
    stats.packages_outside_root=outside.len();
    for path in &outside {
        findings.push(finding("outside_packages","info",path,
            "Package is outside the selected root; no corruption was inferred."));
    }
    findings.sort_by(|a,b|a.relative_path.to_ascii_lowercase()
        .cmp(&b.relative_path.to_ascii_lowercase())
        .then_with(||a.category.cmp(&b.category))
        .then_with(||a.detail.cmp(&b.detail)));
    Ok(ModsHealthReport{
        root:root.to_string_lossy().to_string(),stats,empty_folders:empty,
        unreadable_packages:unreadable,outside_packages:outside,coverage,resource_cfg,
        resource_cfg_error,findings,
    })
}

#[tauri::command]
pub async fn analyze_mods_health(folder:String)->Result<ModsHealthReport,String>{
    operation::run_blocking("health", "analyzing", move || analyze_mods_health_core(folder)).await
}

#[cfg(test)]
mod health_diagnostic_tests {
    use super::*;

    #[test]
    fn structural_failures_are_classified_conservatively() {
        assert_eq!(dbpf_error_category("file magic does not match DBPF"),"invalid_header");
        assert_eq!(dbpf_error_category("file header is smaller than 96 bytes"),"invalid_header");
        assert_eq!(dbpf_error_category("unsupported DBPF major version 4"),"unsupported_version");
        assert_eq!(dbpf_error_category("unsupported DBPF index version 5"),"unsupported_version");
        assert_eq!(dbpf_error_category("DBPF index points outside the file"),"damaged_index");
        assert_eq!(dbpf_error_category("corrupted DBPF index length 8 (expected 36)"),"damaged_index");
        assert_eq!(dbpf_error_category("permission denied"),"unreadable_package");
        assert_eq!(dbpf_error_category("failed to fill whole buffer"),"unreadable_package");
    }

    #[test]
    fn disabled_and_dbc_are_not_mistaken_for_corrupted_packages() {
        assert!(is_disabled_file("MyMod.PACKAGE.DISABLED"));
        assert!(is_disabled_file("MyMod.package.off"));
        assert!(!is_disabled_file("MyMod.package"));
        assert!(is_unanalyzed_cache("dcdb0.dbc"));
        assert!(is_unanalyzed_cache("dcdb0.EBC"));
        assert!(!is_unanalyzed_cache("MyMod.package"));
    }

    #[test]
    fn finding_ids_do_not_merge_different_repeated_tgis() {
        let a=finding("repeated_tgi","warning","a.package","0x0001");
        let b=finding("repeated_tgi","warning","a.package","0x0002");
        assert_ne!(a.id,b.id);
    }

    #[test]
    fn outside_packages_ignores_unrelated_mods_siblings() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mods = std::env::temp_dir().join(format!("s3cc-health-outside-{}-{unique}", std::process::id())).join("Mods");
        let packages = mods.join("Packages");
        let overrides = mods.join("Overrides");
        let unrelated = mods.join("Unrelated");
        std::fs::create_dir_all(&packages).unwrap();
        std::fs::create_dir_all(&overrides).unwrap();
        std::fs::create_dir_all(&unrelated).unwrap();
        std::fs::write(overrides.join("outside.package"), b"not a DBPF fixture").unwrap();
        std::fs::write(unrelated.join("ignored.package"), b"not a DBPF fixture").unwrap();

        let result = outside_packages(&packages);
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("outside.package"));
        assert!(!result.iter().any(|path| path.ends_with("ignored.package")));
        std::fs::remove_dir_all(mods.parent().unwrap()).unwrap();
    }
}
