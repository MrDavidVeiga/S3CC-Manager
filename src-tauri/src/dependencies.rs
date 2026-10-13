use crate::dbpf::Package;
use crate::operation::{self, CANCELLED_ERROR};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

const TYPE_NMAP: u32 = 0x0166_038C;
const TYPE_XML: u32 = 0x0333_406C;
const TYPE_BONE_DELTA: u32 = 0x0355_E0A6;
const TYPE_FACE: u32 = 0x0358_B08A;
const TYPE_BGEO: u32 = 0x067C_AA11;
const TYPE_STBL: u32 = 0x2205_57DA;
const TYPE_FBLN: u32 = 0xB52F_5055;
const TYPE_VPXY: u32 = 0x7368_84F1;
const TYPE_OBJK: u32 = 0x02DC_343F;
const TYPE_MODL: u32 = 0x0166_1233;
const TYPE_MLOD: u32 = 0x01D1_0F34;
// Avoid decoding/scanning enormous payloads in a quadratic-looking byte loop.
const MAX_REFERENCE_PAYLOAD: usize = 16 * 1024 * 1024;

fn oversized_reference_resource(raw: u32, decoded: u32) -> bool {
    (raw as usize) > MAX_REFERENCE_PAYLOAD || (decoded as usize) > MAX_REFERENCE_PAYLOAD
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEvidence {
    pub source_package: String,
    pub source_relative_path: String,
    pub source_resource: String,
    pub target_package: String,
    pub target_relative_path: String,
    pub target_resource: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedDependencyGroup {
    pub id: String,
    pub package_paths: Vec<String>,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyAnalysis {
    pub root: String,
    pub findings: Vec<DependencyEvidence>,
    pub suggested_groups: Vec<SuggestedDependencyGroup>,
    pub scanned_packages: usize,
    pub scanned_reference_resources: usize,
    pub skipped_large_reference_resources: usize,
    pub truncated: bool,
}

fn is_reference_type(t: u32) -> bool {
    matches!(t, TYPE_VPXY | TYPE_OBJK | TYPE_MODL | TYPE_MLOD | TYPE_XML)
}

fn is_slider_reference_target(t: u32) -> bool {
    matches!(
        t,
        TYPE_NMAP | TYPE_FBLN | TYPE_BGEO | TYPE_FACE | TYPE_BONE_DELTA | TYPE_VPXY
    )
}

fn canonical_root(folder: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|e| format!("Could not resolve root: {e}"))?;
    if !root.is_dir() {
        return Err("Selected root is not a directory.".into());
    }
    Ok(root)
}

fn is_package(path: &Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("package"))
        .unwrap_or(false)
}

fn tgi_bytes(t: u32, g: u32, i: u64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&t.to_le_bytes());
    b[4..8].copy_from_slice(&g.to_le_bytes());
    b[8..16].copy_from_slice(&i.to_le_bytes());
    b
}

fn stbl_keys(data: &[u8]) -> Vec<u64> {
    if data.len() < 17 || &data[..4] != b"STBL" {
        return Vec::new();
    }

    let Some(count_bytes) = data.get(7..11) else {
        return Vec::new();
    };
    let count = u32::from_le_bytes(count_bytes.try_into().unwrap()) as usize;
    let mut offset = 17usize;
    let mut keys = Vec::with_capacity(count.min(4096));

    for _ in 0..count {
        let Some(key_bytes) = data.get(offset..offset + 8) else {
            return Vec::new();
        };
        let key = u64::from_le_bytes(key_bytes.try_into().unwrap());
        offset += 8;

        let Some(length_bytes) = data.get(offset..offset + 4) else {
            return Vec::new();
        };
        let char_count = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
        offset += 4;

        let Some(byte_count) = char_count.checked_mul(2) else {
            return Vec::new();
        };
        let Some(next) = offset.checked_add(byte_count) else {
            return Vec::new();
        };
        if next > data.len() {
            return Vec::new();
        }

        keys.push(key);
        offset = next;
    }

    keys
}

fn suggested_groups(findings: &[DependencyEvidence]) -> Vec<SuggestedDependencyGroup> {
    let mut adjacency = HashMap::<String, HashSet<String>>::new();
    let mut edge_counts = HashMap::<(String, String), usize>::new();

    for finding in findings {
        let left = finding.source_relative_path.clone();
        let right = finding.target_relative_path.clone();
        if left == right {
            continue;
        }
        adjacency.entry(left.clone()).or_default().insert(right.clone());
        adjacency.entry(right.clone()).or_default().insert(left.clone());
        let mut pair = [left, right];
        pair.sort();
        *edge_counts.entry((pair[0].clone(), pair[1].clone())).or_default() += 1;
    }

    let mut visited = HashSet::<String>::new();
    let mut groups = Vec::new();

    for start in adjacency.keys() {
        if !visited.insert(start.clone()) {
            continue;
        }

        let mut stack = vec![start.clone()];
        let mut members = Vec::new();
        while let Some(current) = stack.pop() {
            members.push(current.clone());
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }

        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|value| value.to_ascii_lowercase());

        let member_set = members.iter().cloned().collect::<HashSet<_>>();
        let evidence_count = edge_counts
            .iter()
            .filter(|((left, right), _)| member_set.contains(left) && member_set.contains(right))
            .map(|(_, count)| *count)
            .sum::<usize>();

        let id = format!(
            "dependency-group-{:016X}",
            members.iter().fold(0xcbf29ce484222325u64, |hash, value| {
                value.as_bytes().iter().fold(hash, |h, byte| {
                    (h ^ *byte as u64).wrapping_mul(0x100000001b3)
                })
            })
        );
        groups.push(SuggestedDependencyGroup {
            id,
            package_paths: members,
            evidence_count,
        });
    }

    groups.sort_by(|a, b| {
        b.package_paths.len()
            .cmp(&a.package_paths.len())
            .then_with(|| b.evidence_count.cmp(&a.evidence_count))
            .then_with(|| a.id.cmp(&b.id))
    });
    groups
}

fn analyze_dependencies_core(folder: String) -> Result<DependencyAnalysis, String> {
    let root = canonical_root(&folder)?;
    let package_paths = walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_package(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();
    operation::set_total("dependencies", package_paths.len());
    let mut packages = Vec::new();
    let mut targets = HashMap::<[u8; 16], Vec<(usize, String)>>::new();
    let mut slider_instance_targets = HashMap::<u64, Vec<(usize, String)>>::new();

    for (processed, path) in package_paths.iter().enumerate() {
        if operation::is_cancelled("dependencies") {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update("dependencies", processed, Some(path.to_string_lossy().into_owned()), "indexing");
        if let Ok(pkg) = Package::load(path) {
            let idx = packages.len();
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('/', "\\");

            for r in &pkg.entries {
                targets
                    .entry(tgi_bytes(r.type_id, r.group, r.instance))
                    .or_default()
                    .push((idx, r.key_string()));

                if is_slider_reference_target(r.type_id) {
                    slider_instance_targets
                        .entry(r.instance)
                        .or_default()
                        .push((idx, r.key_string()));
                }
            }

            packages.push((path.to_path_buf(), rel, pkg));
        }
    }

    let mut findings = Vec::new();
    let mut seen = HashSet::new();
    let mut reference_resources = 0usize;
    let mut skipped_large_reference_resources = 0usize;
    let mut truncated = false;

    'packages: for (source_idx, (source_path, source_rel, pkg)) in packages.iter().enumerate() {
        if operation::is_cancelled("dependencies") {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update("dependencies", source_idx, Some(source_rel.clone()), "resolving");
        for r in &pkg.entries {
            if is_reference_type(r.type_id) {
                if oversized_reference_resource(r.file_size, r.mem_size) {
                    skipped_large_reference_resources += 1;
                    continue;
                }
                let Ok(data) = pkg.data(r) else {
                    continue;
                };
                reference_resources += 1;
                if data.len() > MAX_REFERENCE_PAYLOAD {
                    skipped_large_reference_resources += 1;
                    continue;
                }
                if data.len() >= 16 {
                    for offset in 0..=data.len() - 16 {
                        let mut key = [0u8; 16];
                        key.copy_from_slice(&data[offset..offset + 16]);
                        let Some(matches) = targets.get(&key) else {
                            continue;
                        };

                        for (target_idx, target_resource) in matches {
                            if *target_idx == source_idx {
                                continue;
                            }
                            let pair = (
                                source_idx,
                                *target_idx,
                                r.key_string(),
                                target_resource.clone(),
                                "tgi",
                            );
                            if !seen.insert(pair) {
                                continue;
                            }

                            findings.push(DependencyEvidence {
                                source_package: source_path.to_string_lossy().to_string(),
                                source_relative_path: source_rel.clone(),
                                source_resource: r.key_string(),
                                target_package: packages[*target_idx].0.to_string_lossy().to_string(),
                                target_relative_path: packages[*target_idx].1.clone(),
                                target_resource: target_resource.clone(),
                                evidence: format!(
                                    "Exact 16-byte little-endian TGI reference found at payload offset 0x{offset:X}."
                                ),
                            });
                            if findings.len() >= 5000 {
                                truncated = true;
                                break 'packages;
                            }
                        }
                    }
                }
            }

            if r.type_id == TYPE_STBL {
                if oversized_reference_resource(r.file_size, r.mem_size) {
                    skipped_large_reference_resources += 1;
                    continue;
                }
                let Ok(data) = pkg.data(r) else {
                    continue;
                };
                reference_resources += 1;
                if data.len() > MAX_REFERENCE_PAYLOAD {
                    skipped_large_reference_resources += 1;
                    continue;
                }

                for key in stbl_keys(&data) {
                    let Some(matches) = slider_instance_targets.get(&key) else {
                        continue;
                    };

                    for (target_idx, target_resource) in matches {
                        if *target_idx == source_idx {
                            continue;
                        }

                        let pair = (
                            source_idx,
                            *target_idx,
                            r.key_string(),
                            target_resource.clone(),
                            "stbl_slider_key",
                        );
                        if !seen.insert(pair) {
                            continue;
                        }

                        findings.push(DependencyEvidence {
                            source_package: source_path.to_string_lossy().to_string(),
                            source_relative_path: source_rel.clone(),
                            source_resource: r.key_string(),
                            target_package: packages[*target_idx].0.to_string_lossy().to_string(),
                            target_relative_path: packages[*target_idx].1.clone(),
                            target_resource: target_resource.clone(),
                            evidence: format!(
                                "STBL entry key 0x{key:016X} matches a slider/morph resource instance in the target package."
                            ),
                        });
                        if findings.len() >= 5000 {
                            truncated = true;
                            break 'packages;
                        }
                    }
                }
            }
        }
    }

    findings.sort_by_key(|f| {
        (
            f.source_relative_path.to_ascii_lowercase(),
            f.target_relative_path.to_ascii_lowercase(),
        )
    });

    let suggested_groups = suggested_groups(&findings);

    Ok(DependencyAnalysis {
        root: root.to_string_lossy().to_string(),
        findings,
        suggested_groups,
        scanned_packages: packages.len(),
        scanned_reference_resources: reference_resources,
        skipped_large_reference_resources,
        truncated,
    })
}

#[tauri::command]
pub async fn analyze_dependencies(folder: String) -> Result<DependencyAnalysis, String> {
    crate::operation::run_blocking("dependencies", "analyzing", move || {
        analyze_dependencies_core(folder)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stbl_keys_reads_version_two_entries() {
        let key = 0x745F_376D_11D5_E6C3u64;
        let text = "Butt".encode_utf16().collect::<Vec<_>>();
        let mut data = Vec::new();
        data.extend_from_slice(b"STBL");
        data.extend_from_slice(&[2, 0, 0]);
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0; 6]);
        data.extend_from_slice(&key.to_le_bytes());
        data.extend_from_slice(&(text.len() as u32).to_le_bytes());
        for unit in text {
            data.extend_from_slice(&unit.to_le_bytes());
        }

        assert_eq!(stbl_keys(&data), vec![key]);
    }

    #[test]
    fn skip_large_dependency_payloads_before_decompression() {
        assert!(oversized_reference_resource((MAX_REFERENCE_PAYLOAD + 1) as u32, 0));
        assert!(oversized_reference_resource(100, (MAX_REFERENCE_PAYLOAD + 1) as u32));
        assert!(!oversized_reference_resource(128, 512));
    }

    #[test]
    fn slider_reference_targets_include_morph_metadata() {
        assert!(is_slider_reference_target(TYPE_NMAP));
        assert!(is_slider_reference_target(TYPE_FBLN));
        assert!(is_slider_reference_target(TYPE_BGEO));
        assert!(!is_slider_reference_target(TYPE_STBL));
    }


    #[test]
    fn dependency_edges_become_reviewable_keep_together_suggestions() {
        let finding = |source: &str, target: &str| DependencyEvidence {
            source_package: source.into(),
            source_relative_path: source.into(),
            source_resource: "A".into(),
            target_package: target.into(),
            target_relative_path: target.into(),
            target_resource: "B".into(),
            evidence: "exact".into(),
        };
        let groups = suggested_groups(&[
            finding("A.package", "B.package"),
            finding("B.package", "C.package"),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].package_paths.len(), 3);
        assert_eq!(groups[0].evidence_count, 2);
    }
}
