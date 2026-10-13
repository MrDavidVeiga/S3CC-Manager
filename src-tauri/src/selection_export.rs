use chrono::Local;
use crate::operation;
use serde::Serialize;
use std::{fs,path::{Path,PathBuf}};

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct SelectionExportResult{
    pub path:String,
    pub directory:String,
}
fn canonical_root(folder:&str)->Result<PathBuf,String>{
    let root=PathBuf::from(folder.trim()).canonicalize().map_err(|e|format!("Could not resolve root: {e}"))?;
    if !root.is_dir(){return Err("Selected root is not a directory.".into());}Ok(root)
}
fn export_dir(root:&Path)->PathBuf{
    root.parent().unwrap_or(root).join("S3CC Organizer").join("Exports")
}
#[tauri::command]
pub fn save_selection_export(folder:String,format:String,content:String)->Result<SelectionExportResult,String>{
    let root=canonical_root(&folder)?;
    let ext=match format.to_ascii_lowercase().as_str(){"txt"=>"txt","csv"=>"csv","json"=>"json",_=>return Err("Supported export formats are txt, csv and json.".into())};
    if content.trim().is_empty(){return Err("Export content is empty.".into());}
    if ext=="json"{serde_json::from_str::<serde_json::Value>(&content).map_err(|e|format!("Invalid JSON export: {e}"))?;}
    let dir=export_dir(&root);
    let scope=dir.to_string_lossy().to_string();
    operation::run_mutating_sync("selection_export", "saving", &[scope.as_str()], move || {
        fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        let stamp=Local::now().format("%Y%m%d-%H%M%S").to_string();
        let mut path=dir.join(format!("Selection-{stamp}.{ext}"));let mut n=1;
        while path.exists(){path=dir.join(format!("Selection-{stamp}-{n}.{ext}"));n+=1;}
        fs::write(&path,content.as_bytes()).map_err(|e|format!("Could not save export: {e}"))?;
        Ok(SelectionExportResult{path:path.to_string_lossy().to_string(),directory:dir.to_string_lossy().to_string()})
    })
}
