use crate::cache::{get_or_build,load_cache,save_cache};
use serde::Serialize;
use std::{collections::{BTreeMap,BTreeSet},path::PathBuf};

#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct CompareResource{
    pub tgi:String,
    pub left_payload:Option<String>,
    pub right_payload:Option<String>,
    pub relation:String,
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct PackageCompareResult{
    pub left_path:String,
    pub right_path:String,
    pub left_sha256:String,
    pub right_sha256:String,
    pub left_resource_count:usize,
    pub right_resource_count:usize,
    pub identical_resources:usize,
    pub changed_resources:usize,
    pub only_left:usize,
    pub only_right:usize,
    pub resources:Vec<CompareResource>,
}
fn canonical_inside(root:&PathBuf,raw:&str)->Result<PathBuf,String>{
    let p=PathBuf::from(raw).canonicalize().map_err(|e|format!("Could not resolve package: {e}"))?;
    if !p.starts_with(root)||!p.is_file(){return Err("Package is outside selected root.".into());}
    Ok(p)
}
fn compare_packages_core(folder:String,left_path:String,right_path:String)->Result<PackageCompareResult,String>{
    let root=PathBuf::from(folder.trim()).canonicalize().map_err(|e|format!("Could not resolve root: {e}"))?;
    let left=canonical_inside(&root,&left_path)?;let right=canonical_inside(&root,&right_path)?;
    let mut cache=load_cache(&root);
    let (l,_)=get_or_build(&left,&mut cache)?;let (r,_)=get_or_build(&right,&mut cache)?;
    let _=save_cache(&root,&cache);
    if let Some(error)=&l.parse_error{return Err(format!("Left package could not be decoded: {error}"));}
    if let Some(error)=&r.parse_error{return Err(format!("Right package could not be decoded: {error}"));}
    let mut lm=BTreeMap::new();let mut rm=BTreeMap::new();
    for x in &l.resources{lm.insert((x.type_id,x.group,x.instance),x.payload_sha256.clone());}
    for x in &r.resources{rm.insert((x.type_id,x.group,x.instance),x.payload_sha256.clone());}
    let keys=lm.keys().chain(rm.keys()).cloned().collect::<BTreeSet<_>>();
    let mut resources=Vec::new();let mut identical=0;let mut changed=0;let mut only_l=0;let mut only_r=0;
    for (t,g,i) in keys{
        let a=lm.get(&(t,g,i)).cloned();let b=rm.get(&(t,g,i)).cloned();
        let relation=match (&a,&b){
            (Some(x),Some(y)) if x==y=>{identical+=1;"identical"},
            (Some(_),Some(_))=>{changed+=1;"changed"},
            (Some(_),None)=>{only_l+=1;"only_left"},
            (None,Some(_))=>{only_r+=1;"only_right"},
            _=>"unknown"
        }.to_string();
        resources.push(CompareResource{tgi:format!("0x{t:08X}-0x{g:08X}-0x{i:016X}"),left_payload:a,right_payload:b,relation});
    }
    Ok(PackageCompareResult{
        left_path:left.to_string_lossy().to_string(),right_path:right.to_string_lossy().to_string(),
        left_sha256:l.file_sha256,right_sha256:r.file_sha256,left_resource_count:l.resources.len(),right_resource_count:r.resources.len(),
        identical_resources:identical,changed_resources:changed,only_left:only_l,only_right:only_r,resources
    })
}

#[tauri::command]
pub async fn compare_packages(folder:String,left_path:String,right_path:String)->Result<PackageCompareResult,String>{
    crate::operation::run_blocking("package_compare", "comparing", move || {
        compare_packages_core(folder, left_path, right_path)
    }).await
}
