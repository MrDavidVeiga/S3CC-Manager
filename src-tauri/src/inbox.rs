use crate::{dbpf::Package,i18n::AppLanguage,manifest::sha256_file,scanner::{scan_packages_core,ScanResult},workspace::ensure_writable};
use chrono::Local;
use serde::Serialize;
use std::{fs,path::{Path,PathBuf}};

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct InboxPlanItem{
    pub source_path:String,
    pub relative_path:String,
    pub destination_path:String,
    pub sha256:String,
    pub size:u64,
    pub status:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct InboxImportPlan{
    pub source_root:String,
    pub mods_root:String,
    pub destination_root:String,
    pub items:Vec<InboxPlanItem>,
    pub ready:usize,
    pub blocked:usize,
    pub can_execute:bool,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct InboxImportResult{
    pub imported:usize,
    pub manifest_path:String,
    pub destination_root:String,
    pub warnings:Vec<String>,
}
fn canonical_dir(raw:&str)->Result<PathBuf,String>{
    let p=PathBuf::from(raw.trim()).canonicalize().map_err(|e|format!("Could not resolve folder: {e}"))?;
    if !p.is_dir(){return Err("Selected path is not a directory.".into());}Ok(p)
}
fn is_package(path:&Path)->bool{path.extension().and_then(|v|v.to_str()).map(|v|v.eq_ignore_ascii_case("package")).unwrap_or(false)}
fn scan_inbox_core(source_folder:String,language:AppLanguage)->Result<ScanResult,String>{
    let source=canonical_dir(&source_folder)?;
    scan_packages_core(source.to_string_lossy().to_string(),language,Some("inbox_scan"))
}
fn build_inbox_import_plan_core(folder:String,source_folder:String,selected_paths:Vec<String>)->Result<InboxImportPlan,String>{
    let root=canonical_dir(&folder)?;let source_root=canonical_dir(&source_folder)?;
    if source_root.starts_with(&root) || root.starts_with(&source_root) {
        return Err("Inbox must be outside the selected Mods root and its parent tree.".into());
    }
    let destination_root=root.join("New CC");
    let mut items=Vec::new();let mut ready=0;let mut blocked=0;
    for raw in selected_paths{
        let source=PathBuf::from(&raw).canonicalize().map_err(|e|format!("Could not resolve inbox file: {e}"))?;
        if !source.starts_with(&source_root)||!source.is_file()||!is_package(&source){blocked+=1;continue;}
        let rel=source.strip_prefix(&source_root).unwrap_or(&source);
        let destination=destination_root.join(rel);
        let (sha,size)=sha256_file(&source).map_err(|e|format!("Could not hash {}: {e}",source.display()))?;
        let status=if destination.exists(){
            "collision"
        }else if Package::load(&source).is_err(){
            "invalid"
        }else{
            "ready"
        }.to_string();
        if status=="ready"{ready+=1}else{blocked+=1}
        items.push(InboxPlanItem{
            source_path:source.to_string_lossy().to_string(),
            relative_path:rel.to_string_lossy().replace('/',"\\"),
            destination_path:destination.to_string_lossy().to_string(),sha256:sha,size,status
        });
    }
    Ok(InboxImportPlan{
        source_root:source_root.to_string_lossy().to_string(),mods_root:root.to_string_lossy().to_string(),
        destination_root:destination_root.to_string_lossy().to_string(),can_execute:ready>0&&blocked==0,items,ready,blocked
    })
}
fn execute_inbox_import_core(folder:String,source_folder:String,selected_paths:Vec<String>)->Result<InboxImportResult,String>{
    let root=canonical_dir(&folder)?;ensure_writable(&root)?;
    let plan=build_inbox_import_plan_core(folder.clone(),source_folder,selected_paths)?;
    if !plan.can_execute{return Err("Inbox import is blocked by the preflight plan.".into());}
    let destination_root=PathBuf::from(&plan.destination_root);
    let mut copied=Vec::<PathBuf>::new();
    for item in &plan.items{
        let src=PathBuf::from(&item.source_path);let dst=PathBuf::from(&item.destination_path);
        if dst.exists(){for p in copied.iter().rev(){let _=fs::remove_file(p);}return Err(format!("Destination appeared during import: {}",dst.display()));}
        let Some(parent)=dst.parent() else{
            for p in copied.iter().rev(){let _=fs::remove_file(p);}
            return Err("Import destination has no parent.".to_string());
        };
        if let Err(error)=fs::create_dir_all(parent){
            for p in copied.iter().rev(){let _=fs::remove_file(p);}
            return Err(format!("Could not create import folder: {error}"));
        }
        if let Err(error)=fs::copy(&src,&dst){
            let _=fs::remove_file(&dst);
            for p in copied.iter().rev(){let _=fs::remove_file(p);}
            return Err(format!("Could not copy {}: {error}",src.display()));
        }
        let (hash,size)=match sha256_file(&dst){
            Ok(value)=>value,
            Err(error)=>{
                let _=fs::remove_file(&dst);
                for p in copied.iter().rev(){let _=fs::remove_file(p);}
                return Err(format!("Could not verify imported file: {error}"));
            }
        };
        if hash!=item.sha256||size!=item.size{
            let _=fs::remove_file(&dst);for p in copied.iter().rev(){let _=fs::remove_file(p);}
            return Err(format!("Imported copy failed verification: {}",dst.display()));
        }
        copied.push(dst);
    }
    let reports=root.parent().unwrap_or(&root).join("S3CC Organizer").join("Inbox Manifests");
    let path=reports.join(format!("Inbox-Import-{}.json",Local::now().format("%Y%m%d-%H%M%S")));
    let mut warnings=Vec::new();
    let mut manifest_path=String::new();
    match serde_json::to_vec_pretty(&plan){
        Ok(data)=>{
            if let Err(error)=fs::create_dir_all(&reports).and_then(|_|fs::write(&path,data)){
                warnings.push(format!("Import completed, but the Inbox manifest could not be saved: {error}"));
            }else{
                manifest_path=path.to_string_lossy().to_string();
            }
        }
        Err(error)=>warnings.push(format!("Import completed, but the Inbox manifest could not be serialized: {error}")),
    }
    Ok(InboxImportResult{imported:copied.len(),manifest_path,destination_root:destination_root.to_string_lossy().to_string(),warnings})
}

#[tauri::command]
pub async fn scan_inbox(source_folder:String,language:AppLanguage)->Result<ScanResult,String>{
    crate::operation::run_blocking("inbox_scan", "scanning", move || {
        scan_inbox_core(source_folder, language)
    }).await
}

#[tauri::command]
pub async fn build_inbox_import_plan(folder:String,source_folder:String,selected_paths:Vec<String>)->Result<InboxImportPlan,String>{
    crate::operation::run_blocking("inbox_plan", "planning", move || {
        build_inbox_import_plan_core(folder, source_folder, selected_paths)
    }).await
}

#[tauri::command]
pub async fn execute_inbox_import(folder:String,source_folder:String,selected_paths:Vec<String>)->Result<InboxImportResult,String>{
    crate::operation::run_mutating_blocking("inbox_import", "importing", move || {
        execute_inbox_import_core(folder, source_folder, selected_paths)
    }).await
}
