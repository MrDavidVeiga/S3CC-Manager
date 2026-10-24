//! Read-only, evidence-based detection of CCs fully contained in merged packages.
//! Never removes resources, moves files or offers automated quarantine.
use crate::{cache::{get_or_build, load_cache, save_cache}, dbpf::Package, operation::{self, CANCELLED_ERROR}};
use quick_xml::{events::{BytesStart, Event}, Reader};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};
use walkdir::WalkDir;

type Key = (u32, u32, u64);
const KIND: &str = "embedded_duplicates";
const MANIFEST_KEY: Key = (0x0333_406C, 0x5653_4733, 0x5645_4947_414D_4E46);
const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCES: usize = 10_000;
const MAX_SOURCE_RESOURCES: usize = 100_000;

#[derive(Debug, Clone)]
struct Source {
    name: String,
    keys: BTreeSet<Key>,
    anchors: BTreeSet<Key>,
    declared_count: usize,
}

#[derive(Debug, Clone)]
struct Origin {
    merged_path: String,
    source_name: String,
    resources: usize,
    anchors: usize,
    fingerprint: String,
    from_manifest: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedFinding {
    pub kind: String,
    pub source_name: String,
    pub left_path: String,
    pub right_path: String,
    pub resource_count: usize,
    pub anchor_count: usize,
    pub evidence: String,
    pub safe_action: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedReport {
    pub root: String,
    pub scanned_packages: usize,
    pub valid_manifests: usize,
    pub findings: Vec<EmbeddedFinding>,
    pub warnings: Vec<String>,
}

fn attr(event: &BytesStart<'_>, reader: &Reader<&[u8]>, name: &[u8]) -> Option<String> {
    event.attributes().flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.decode_and_unescape_value(reader).ok())
        .map(|v| v.to_string())
}

fn parse_key(raw: &str) -> Result<Key, String> {
    let parts = raw.split('-').collect::<Vec<_>>();
    if parts.len() != 3 { return Err("Invalid manifest TGI field count".into()); }
    let hex = |v: &str| v.strip_prefix("0x").ok_or_else(|| "Invalid manifest TGI prefix".to_string());
    let t = u32::from_str_radix(hex(parts[0])?, 16).map_err(|_| "Invalid TGI type")?;
    let g = u32::from_str_radix(hex(parts[1])?, 16).map_err(|_| "Invalid TGI group")?;
    let i = u64::from_str_radix(hex(parts[2])?, 16).map_err(|_| "Invalid TGI instance")?;
    Ok((t, g, i))
}

fn parse_merge_manifest(xml: &str) -> Result<Vec<Source>, String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut found_root = false;
    let mut declared_packages = None;
    let mut sources = Vec::new();
    let mut current: Option<Source> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"VeigaS3CCMergeManifest" => {
                if found_root { return Err("Repeated merge-manifest root".into()); }
                found_root = true;
                if attr(e, &reader, b"version").as_deref() != Some("2") {
                    return Err("Unsupported merge-manifest version".into());
                }
                declared_packages = attr(e, &reader, b"packageCount")
                    .and_then(|v| v.parse::<usize>().ok());
            }
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"Package" && found_root => {
                if current.is_some() || sources.len() >= MAX_SOURCES {
                    return Err("Nested or excessive merge-manifest sources".into());
                }
                let name = attr(e, &reader, b"name").filter(|v| !v.trim().is_empty())
                    .ok_or("Manifest source has no name")?;
                let declared_count = attr(e, &reader, b"resourceCount")
                    .and_then(|v| v.parse::<usize>().ok())
                    .ok_or("Manifest source has no valid resource count")?;
                if declared_count == 0 || declared_count > MAX_SOURCE_RESOURCES {
                    return Err("Invalid or excessive manifest source resource count".into());
                }
                current = Some(Source { name, keys: BTreeSet::new(), anchors: BTreeSet::new(), declared_count });
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"Resource" => {
                let source = current.as_mut().ok_or("Resource outside manifest source")?;
                let key = parse_key(&attr(e, &reader, b"key").ok_or("Resource without TGI")?)?;
                if !source.keys.insert(key) { return Err("Repeated TGI in source manifest".into()); }
                if source.keys.len() > MAX_SOURCE_RESOURCES { return Err("Excessive source TGIs".into()); }
            }
            Ok(Event::Empty(ref e)) if e.name().as_ref() == b"Anchor" => {
                let source = current.as_mut().ok_or("Anchor outside manifest source")?;
                let key = parse_key(&attr(e, &reader, b"key").ok_or("Anchor without TGI")?)?;
                source.anchors.insert(key);
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"Package" => {
                let item = current.take().ok_or("Unexpected closing source")?;
                if item.keys.len() != item.declared_count || item.anchors.is_empty()
                    || !item.anchors.is_subset(&item.keys) {
                    return Err(format!("Incomplete manifest source: {}", item.name));
                }
                sources.push(item);
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Invalid merge manifest XML: {error}")),
            _ => {}
        }
        buf.clear();
    }
    if !found_root || current.is_some() || sources.len() != declared_packages.unwrap_or(usize::MAX) {
        return Err("Incomplete merge-manifest document or package count".into());
    }
    Ok(sources)
}

fn signature(keys: &BTreeSet<Key>, hashes: &BTreeMap<Key, String>) -> Option<String> {
    if keys.is_empty() { return None; }
    let mut hasher = Sha256::new();
    for &(t, g, i) in keys {
        let hash = hashes.get(&(t, g, i))?;
        hasher.update(t.to_le_bytes());
        hasher.update(g.to_le_bytes());
        hasher.update(i.to_le_bytes());
        hasher.update(hash.as_bytes());
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn collect_origins(folder: String) -> Result<EmbeddedReport, String> {
    let root = PathBuf::from(folder.trim()).canonicalize()
        .map_err(|e| format!("Unable to resolve the selected Mods folder: {e}"))?;
    if !root.is_dir() { return Err("Selected root is not a directory".into()); }

    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for item in WalkDir::new(&root).follow_links(false).into_iter() {
        if operation::is_cancelled(KIND) { return Err(CANCELLED_ERROR.into()); }
        match item {
            Ok(entry) if entry.file_type().is_file()
                && entry.path().extension().and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("package")) => {
                files.push(entry.into_path());
            }
            Ok(_) => {}
            Err(e) => warnings.push(format!("Directory scan error: {e}")),
        }
    }
    files.sort();
    operation::set_total(KIND, files.len().saturating_add(1));

    let mut cache = load_cache(&root);
    let mut grouped: BTreeMap<String, Vec<Origin>> = BTreeMap::new();
    let mut valid_manifests = 0usize;
    for (index, path) in files.iter().enumerate() {
        if operation::is_cancelled(KIND) { return Err(CANCELLED_ERROR.into()); }
        operation::update(KIND, index, None, "reading_packages");
        let package = match Package::load(path) {
            Ok(pkg) => pkg,
            Err(e) => { warnings.push(format!("{}: {e}", path.display())); continue; }
        };
        let manifest_entry = package.entries.iter().find(|e| (e.type_id, e.group, e.instance) == MANIFEST_KEY);
        let manifest_sources = if let Some(entry) = manifest_entry {
            if entry.mem_size as usize > MAX_MANIFEST_BYTES {
                warnings.push(format!("{}: merge manifest exceeds size limit", path.display()));
                continue;
            }
            let decoded = match package.data(entry) {
                Ok(data) if data.len() <= MAX_MANIFEST_BYTES => data,
                Ok(_) => { warnings.push(format!("{}: oversized manifest", path.display())); continue; }
                Err(e) => { warnings.push(format!("{}: cannot decode merge manifest: {e}", path.display())); continue; }
            };
            let xml = match std::str::from_utf8(&decoded) {
                Ok(xml) => xml,
                Err(_) => { warnings.push(format!("{}: invalid UTF-8 manifest", path.display())); continue; }
            };
            match parse_merge_manifest(xml) {
                Ok(sources) => Some(sources),
                Err(e) => { warnings.push(format!("{}: {e}", path.display())); continue; }
            }
        } else { None };

        let (cached, _) = match get_or_build(path, &mut cache) {
            Ok(result) => result,
            Err(e) => { warnings.push(format!("{}: {e}", path.display())); continue; }
        };
        if let Some(error) = cached.parse_error {
            warnings.push(format!("{}: {error}", path.display()));
            continue;
        }
        let mut hashes: BTreeMap<Key, String> = BTreeMap::new();
        let mut duplicate_key = false;
        for r in cached.resources {
            if hashes.insert((r.type_id, r.group, r.instance), r.payload_sha256).is_some() {
                duplicate_key = true;
            }
        }
        if duplicate_key {
            warnings.push(format!("{}: repeated TGI in package; cannot verify ownership", path.display()));
            continue;
        }
        let package_path = path.to_string_lossy().to_string();
        if let Some(sources) = manifest_sources {
            let union = sources.iter().flat_map(|s| s.keys.iter().copied()).collect::<BTreeSet<_>>();
            let indexed = hashes.keys().filter(|k| **k != MANIFEST_KEY).copied().collect::<BTreeSet<_>>();
            if union != indexed {
                warnings.push(format!("{}: source manifest does not cover package index", path.display()));
                continue;
            }
            valid_manifests += 1;
            for source in sources {
                if let Some(fingerprint) = signature(&source.keys, &hashes) {
                    grouped.entry(fingerprint.clone()).or_default().push(Origin {
                        merged_path: package_path.clone(), source_name: source.name,
                        resources: source.keys.len(), anchors: source.anchors.len(),
                        fingerprint, from_manifest: true,
                    });
                }
            }
        } else {
            let keys = hashes.keys().copied().collect::<BTreeSet<_>>();
            if let Some(fingerprint) = signature(&keys, &hashes) {
                grouped.entry(fingerprint.clone()).or_default().push(Origin {
                    merged_path: package_path, source_name: path.file_name()
                        .map(|v| v.to_string_lossy().to_string()).unwrap_or_default(),
                    resources: keys.len(), anchors: 0, fingerprint, from_manifest: false,
                });
            }
        }
    }
    operation::update(KIND, files.len(), None, "comparing");
    if operation::is_cancelled(KIND) { return Err(CANCELLED_ERROR.into()); }
    let _ = save_cache(&root, &cache);

    let mut findings = Vec::new();
    for origins in grouped.values() {
        for left_index in 0..origins.len() {
            for right_index in (left_index + 1)..origins.len() {
                if operation::is_cancelled(KIND) { return Err(CANCELLED_ERROR.into()); }
                let left = &origins[left_index];
                let right = &origins[right_index];
                if left.merged_path == right.merged_path || (!left.from_manifest && !right.from_manifest) {
                    continue; // Existing Duplicates analyzer owns ordinary whole-package duplicates.
                }
                let (left, right) = if left.from_manifest || !right.from_manifest {
                    (left, right)
                } else { (right, left) };
                findings.push(EmbeddedFinding {
                    kind: if left.from_manifest && right.from_manifest { "merged_vs_merged" }
                          else { "individual_vs_merged" }.into(),
                    source_name: left.source_name.clone(),
                    left_path: left.merged_path.clone(),
                    right_path: right.merged_path.clone(),
                    resource_count: left.resources,
                    anchor_count: left.anchors,
                    evidence: "verified_manifest_tgi_payload" .into(),
                    safe_action: "review_only".into(),
                });
            }
        }
    }
    findings.sort_by(|a,b| a.source_name.cmp(&b.source_name)
        .then_with(|| a.left_path.cmp(&b.left_path))
        .then_with(|| a.right_path.cmp(&b.right_path)));
    operation::update(KIND, files.len().saturating_add(1), None, "finalizing");
    Ok(EmbeddedReport {
        root: root.to_string_lossy().to_string(),
        scanned_packages: files.len(), valid_manifests, findings, warnings,
    })
}

#[tauri::command]
pub async fn analyze_embedded_duplicates(folder: String) -> Result<EmbeddedReport, String> {
    operation::run_blocking(KIND, "discovering", move || collect_origins(folder)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = r#"<VeigaS3CCMergeManifest version="2" packageCount="1">
    <Package id="1" name="hair.package" originalSize="10" resourceCount="2">
      <Anchor kind="CASP" instance="0x0000000000000003" key="0x034AEECB-0x18000000-0x0000000000000003" />
      <Resource key="0x034AEECB-0x18000000-0x0000000000000003" />
      <Resource key="0x00AE6C67-0x18000000-0x0000000000000004" />
    </Package></VeigaS3CCMergeManifest>"#;
    #[test] fn parse_verified_sources() {
        let sources = parse_merge_manifest(SAMPLE).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].keys.len(), 2);
        assert_eq!(sources[0].anchors.len(), 1);
    }
    #[test] fn reject_missing_ownership() {
        let bad = SAMPLE.replace("resourceCount=\"2\"", "resourceCount=\"3\"");
        assert!(parse_merge_manifest(&bad).is_err());
        let bad = SAMPLE.replace("key=\"0x034AEECB-0x18000000-0x0000000000000003\" />",
            "key=\"0x034AEECB-0x18000000-0x0000000000000999\" />");
        assert!(parse_merge_manifest(&bad).is_err());
    }
    #[test] fn exact_content_fingerprints_ignore_source_names() {
        let one = parse_merge_manifest(SAMPLE).unwrap().remove(0);
        let keys = one.keys;
        let hashes = keys.iter().copied().map(|key| (key, "same-hash".to_string())).collect();
        let fingerprint = signature(&keys, &hashes);
        assert!(fingerprint.is_some());
        let mut changed = hashes.clone();
        *changed.get_mut(keys.iter().next().unwrap()).unwrap() = "different".into();
        assert_ne!(fingerprint, signature(&keys, &changed));
    }
}
