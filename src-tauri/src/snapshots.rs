use crate::manifest::sha256_file;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEntry {
    pub relative_path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsSnapshot {
    pub id: String,
    pub created_at: String,
    pub root: String,
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiff {
    pub added: Vec<SnapshotEntry>,
    pub removed: Vec<SnapshotEntry>,
    pub modified: Vec<SnapshotEntry>,
    pub moved: Vec<SnapshotMove>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMove {
    pub sha256: String,
    pub old_path: String,
    pub new_path: String,
}

fn canonical_root(folder:&str)->Result<PathBuf,String>{
    let root=PathBuf::from(folder.trim()).canonicalize().map_err(|e|format!("Could not resolve root: {e}"))?;
    if !root.is_dir(){return Err("Selected root is not a directory.".into());}
    Ok(root)
}
fn snapshots_dir(root:&Path)->PathBuf{
    root.parent().unwrap_or(root).join("S3CC Organizer").join("Snapshots")
}
fn package_path(path:&Path)->bool{
    path.extension().and_then(|v|v.to_str()).map(|v|v.eq_ignore_ascii_case("package")).unwrap_or(false)
}
fn capture(root:&Path, kind: Option<&str>)->Result<Vec<SnapshotEntry>,String>{
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok){
        if entry.file_type().is_file() && package_path(entry.path()) { paths.push(entry.path().to_path_buf()); }
    }
    if let Some(kind) = kind { crate::operation::set_total(kind, paths.len()); }
    let mut entries=Vec::new();
    for (processed, path) in paths.iter().enumerate(){
        if let Some(kind) = kind {
            if crate::operation::is_cancelled(kind) { return Err(crate::operation::CANCELLED_ERROR.to_string()); }
            crate::operation::update(kind, processed, Some(path.to_string_lossy().into_owned()), "hashing");
        }
        let (sha256,size)=sha256_file(path).map_err(|e|format!("Could not hash {}: {e}",path.display()))?;
        let rel=path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('/',"\\");
        entries.push(SnapshotEntry{relative_path:rel,size,sha256});
    }
    entries.sort_by_key(|e|e.relative_path.to_ascii_lowercase());
    Ok(entries)
}
fn diff_entries(old:&[SnapshotEntry],new:&[SnapshotEntry])->SnapshotDiff{
    let old_by_path=old.iter().map(|e|(e.relative_path.to_ascii_lowercase(),e)).collect::<HashMap<_,_>>();
    let new_by_path=new.iter().map(|e|(e.relative_path.to_ascii_lowercase(),e)).collect::<HashMap<_,_>>();
    let mut diff=SnapshotDiff::default();
    for (key,e) in &new_by_path{
        match old_by_path.get(key){
            None=>diff.added.push((*e).clone()),
            Some(prev) if prev.sha256!=e.sha256=>diff.modified.push((*e).clone()),
            _=>{}
        }
    }
    for (key,e) in &old_by_path{
        if !new_by_path.contains_key(key){diff.removed.push((*e).clone());}
    }
    let mut removed_by_hash=HashMap::<String,Vec<String>>::new();
    let mut added_by_hash=HashMap::<String,Vec<String>>::new();
    for entry in &diff.removed{
        removed_by_hash.entry(entry.sha256.clone()).or_default().push(entry.relative_path.clone());
    }
    for entry in &diff.added{
        added_by_hash.entry(entry.sha256.clone()).or_default().push(entry.relative_path.clone());
    }

    let mut moved=Vec::new();
    for (hash,old_paths) in &removed_by_hash{
        let Some(new_paths)=added_by_hash.get(hash) else{continue;};
        if old_paths.len()==1 && new_paths.len()==1{
            moved.push(SnapshotMove{
                sha256:hash.clone(),
                old_path:old_paths[0].clone(),
                new_path:new_paths[0].clone(),
            });
        }
    }
    if !moved.is_empty(){
        let moved_pairs=moved.iter()
            .map(|m|(m.sha256.clone(),m.old_path.to_ascii_lowercase(),m.new_path.to_ascii_lowercase()))
            .collect::<std::collections::HashSet<_>>();
        diff.removed.retain(|e|!moved_pairs.iter().any(|(hash,old,_)|hash==&e.sha256&&old==&e.relative_path.to_ascii_lowercase()));
        diff.added.retain(|e|!moved_pairs.iter().any(|(hash,_,new)|hash==&e.sha256&&new==&e.relative_path.to_ascii_lowercase()));
        diff.moved=moved;
    }
    diff
}
fn load_snapshot(path:&Path)->Result<ModsSnapshot,String>{
    let text=fs::read_to_string(path).map_err(|e|format!("Could not read snapshot: {e}"))?;
    serde_json::from_str(&text).map_err(|e|format!("Invalid snapshot: {e}"))
}
fn create_snapshot_core(folder:String)->Result<ModsSnapshot,String>{
    let root=canonical_root(&folder)?;
    let id=Local::now().format("%Y%m%d-%H%M%S").to_string();
    let snapshot=ModsSnapshot{id:id.clone(),created_at:Local::now().to_rfc3339(),root:root.to_string_lossy().to_string(),entries:capture(&root, Some("snapshot"))?};
    let dir=snapshots_dir(&root); fs::create_dir_all(&dir).map_err(|e|format!("Could not create snapshot directory: {e}"))?;
    let mut path=dir.join(format!("snapshot-{id}.json")); let mut n=1;
    while path.exists(){path=dir.join(format!("snapshot-{id}-{n}.json"));n+=1;}
    fs::write(&path,serde_json::to_vec_pretty(&snapshot).unwrap()).map_err(|e|format!("Could not save snapshot: {e}"))?;
    Ok(snapshot)
}
#[tauri::command]
pub fn list_snapshots(folder:String)->Result<Vec<ModsSnapshot>,String>{
    let root=canonical_root(&folder)?; let dir=snapshots_dir(&root);
    if !dir.is_dir(){return Ok(Vec::new());}
    let mut out=Vec::new();
    for e in fs::read_dir(dir).map_err(|e|e.to_string())?.filter_map(Result::ok){
        if e.path().extension().and_then(|v|v.to_str())==Some("json"){
            if let Ok(s)=load_snapshot(&e.path()){out.push(s);}
        }
    }
    out.sort_by(|a,b|b.created_at.cmp(&a.created_at)); Ok(out)
}
fn compare_snapshot_to_current_core(folder:String,snapshot_id:String)->Result<SnapshotDiff,String>{
    let root=canonical_root(&folder)?; let dir=snapshots_dir(&root);
    let mut selected=None;
    for e in fs::read_dir(&dir).map_err(|e|e.to_string())?.filter_map(Result::ok){
        if let Ok(s)=load_snapshot(&e.path()){if s.id==snapshot_id{selected=Some(s);break;}}
    }
    let snapshot=selected.ok_or_else(||"Snapshot not found.".to_string())?;
    Ok(diff_entries(&snapshot.entries,&capture(&root, Some("snapshot"))?))
}
fn compare_mods_roots_core(left_folder:String,right_folder:String)->Result<SnapshotDiff,String>{
    let left=canonical_root(&left_folder)?; let right=canonical_root(&right_folder)?;
    Ok(diff_entries(&capture(&left, Some("snapshot"))?,&capture(&right, Some("snapshot"))?))
}

#[tauri::command]
pub async fn create_snapshot(folder:String)->Result<ModsSnapshot,String>{
    crate::operation::run_mutating_blocking("snapshot", "capturing", move || create_snapshot_core(folder)).await
}

#[tauri::command]
pub async fn compare_snapshot_to_current(folder:String,snapshot_id:String)->Result<SnapshotDiff,String>{
    crate::operation::run_blocking("snapshot", "comparing", move || {
        compare_snapshot_to_current_core(folder, snapshot_id)
    }).await
}

#[tauri::command]
pub async fn compare_mods_roots(left_folder:String,right_folder:String)->Result<SnapshotDiff,String>{
    crate::operation::run_blocking("snapshot", "comparing", move || {
        compare_mods_roots_core(left_folder, right_folder)
    }).await
}
