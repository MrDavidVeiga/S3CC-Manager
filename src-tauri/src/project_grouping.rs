//! Evidence-driven grouping of packages that form one mod project.
//!
//! This module deliberately does not know any creator, mod, or filename.  It
//! extracts stable technical identifiers while the scanner already has the
//! DBPF open and builds relations only when a component points at a script
//! package through a class, assembly identity, or exact TGI reference.

use crate::dbpf::Package;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

pub const TYPE_OBJK: u32 = 0x02DC_343F;
pub const TYPE_XML: u32 = 0x0333_406C;
pub const TYPE_ITUN: u32 = 0x03B3_3DDF;
pub const TYPE_S3SA: u32 = 0x073F_AA07;
pub const TYPE_STBL: u32 = 0x2205_57DA;
pub const TYPE_NMAP: u32 = 0x0166_038C;
pub const TYPE_MANIFEST: u32 = 0x73E9_3EEB;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTechnicalEvidence {
    pub has_s3sa: bool,
    pub assembly_identities: Vec<String>,
    pub namespaces: Vec<String>,
    pub classes: Vec<String>,
    pub script_classes: Vec<String>,
    pub identifiers: Vec<String>,
    pub resource_tgis: Vec<String>,
    pub referenced_tgis: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRelationship {
    pub related_path: String,
    pub relation: String,
    pub evidence: String,
    pub confidence: String,
    pub dependency_kind: String,
}

#[derive(Debug, Clone)]
struct Edge {
    left: usize,
    right: usize,
    relation: String,
    evidence: String,
    confidence: String,
    dependency_kind: String,
}

#[derive(Debug, Clone)]
pub struct ProjectAssignment {
    pub group_id: String,
    pub label: String,
    pub confidence: String,
    pub role: String,
    pub relationships: Vec<ProjectRelationship>,
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn meaningful(value: &str) -> bool {
    let value = value.trim();
    if value.len() < 6 || value.len() > 220 || value.chars().any(|ch| ch.is_control()) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    ![
        "sims3.",
        "system.",
        "microsoft.",
        "mscorlib",
        "tuning",
        "resource",
        "object",
        "gameplay",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn printable_strings(data: &[u8]) -> Vec<String> {
    let mut values = BTreeSet::new();
    let mut ascii = Vec::new();
    let flush_ascii = |ascii: &mut Vec<u8>, values: &mut BTreeSet<String>| {
        if ascii.len() >= 5 {
            if let Ok(value) = String::from_utf8(std::mem::take(ascii)) {
                let value = value.trim().to_string();
                if meaningful(&value) {
                    values.insert(value);
                }
            }
        } else {
            ascii.clear();
        }
    };
    for byte in data {
        if (0x20..=0x7e).contains(byte) {
            ascii.push(*byte);
        } else {
            flush_ascii(&mut ascii, &mut values);
        }
    }
    flush_ascii(&mut ascii, &mut values);

    // CLR metadata and many XML resources also carry UTF-16LE strings.  Only
    // accept printable runs with a zero high byte so arbitrary binary data is
    // not promoted to an identifier.
    let mut utf16 = Vec::<u16>::new();
    let flush_utf16 = |utf16: &mut Vec<u16>, values: &mut BTreeSet<String>| {
        if utf16.len() >= 4 {
            let value = String::from_utf16_lossy(utf16).trim().to_string();
            if meaningful(&value) {
                values.insert(value);
            }
        }
        utf16.clear();
    };
    for pair in data.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if (0x20..=0x7e).contains(&unit) {
            utf16.push(unit);
        } else {
            flush_utf16(&mut utf16, &mut values);
        }
    }
    flush_utf16(&mut utf16, &mut values);
    values.into_iter().collect()
}

fn dotted_identifiers(values: &[String]) -> Vec<String> {
    let mut result = BTreeSet::new();
    for value in values {
        let mut candidate = String::new();
        let flush = |candidate: &mut String, result: &mut BTreeSet<String>| {
            let normalized = candidate
                .trim_matches(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_'))
                .to_string();
            let parts = normalized.split('.').collect::<Vec<_>>();
            if parts.len() >= 2
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                })
            {
                result.insert(normalized.trim_end_matches(".dll").to_string());
            }
            candidate.clear();
        };
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
                candidate.push(ch);
            } else {
                flush(&mut candidate, &mut result);
            }
        }
        flush(&mut candidate, &mut result);
    }
    result.into_iter().collect()
}

fn script_class_like(value: &str) -> bool {
    let Some(last) = value.rsplit('.').next() else {
        return false;
    };
    last.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        && value.matches('.').count() >= 1
}

fn tgi_string(type_id: u32, group: u32, instance: u64) -> String {
    format!("0x{type_id:08X}-0x{group:08X}-0x{instance:016X}")
}

fn known_reference_type(type_id: u32) -> bool {
    matches!(
        type_id,
        0x015A_1849
            | 0x0166_1233
            | 0x01D1_0F34
            | 0x02DC_343F
            | 0x0333_406C
            | 0x034A_EECB
            | 0x03B3_3DDF
            | 0x067C_AA11
            | 0x073F_AA07
            | 0x7368_84F1
            | 0xB52F_5055
    )
}

fn referenced_tgis(data: &[u8]) -> Vec<String> {
    let mut result = BTreeSet::new();
    for offset in (0..data.len().saturating_sub(16)).step_by(4) {
        let Some(type_bytes) = data.get(offset..offset + 4) else {
            continue;
        };
        let type_id = u32::from_le_bytes(type_bytes.try_into().unwrap());
        if !known_reference_type(type_id) {
            continue;
        }
        let Some(group_bytes) = data.get(offset + 4..offset + 8) else {
            continue;
        };
        let Some(instance_bytes) = data.get(offset + 8..offset + 16) else {
            continue;
        };
        let group = u32::from_le_bytes(group_bytes.try_into().unwrap());
        let instance = u64::from_le_bytes(instance_bytes.try_into().unwrap());
        result.insert(tgi_string(type_id, group, instance));
    }
    result.into_iter().collect()
}

fn project_label(evidence: &ProjectTechnicalEvidence) -> String {
    // A full assembly identity is a better project label than its final
    // segment: "Creator.First.Mod" and "Creator.Second.Mod" must not both
    // become the same ambiguous directory named "Mod".
    let candidate = evidence
        .assembly_identities
        .iter()
        .find_map(|raw| {
            let value = raw.trim();
            let value = if value.to_ascii_lowercase().ends_with(".dll") {
                &value[..value.len() - 4]
            } else {
                value
            };
            let lower = value.to_ascii_lowercase();
            if value.len() > 160
                || !value.chars().any(|ch| ch.is_ascii_alphabetic())
                || matches!(lower.as_str(), "mscorlib" | "system" | "system.core" | "sims3.gameplay")
                || lower.starts_with("microsoft.")
            {
                None
            } else {
                Some(value.to_owned())
            }
        })
        .or_else(|| {
            evidence.namespaces.iter().find_map(|raw| {
                let value = raw.trim();
                (value.len() <= 160 && value.contains('.')
                    && value.chars().any(|ch| ch.is_ascii_alphabetic()))
                .then(|| value.to_owned())
            })
        })
        .or_else(|| {
            evidence.classes.iter().find_map(|raw| {
                // Remove the implementation-class name; do not call every
                // project "MainController" or "Instantiator".
                let (namespace, _) = raw.rsplit_once('.')?;
                (namespace.len() <= 160 && namespace.chars().any(|ch| ch.is_ascii_alphabetic()))
                    .then(|| namespace.to_owned())
            })
        })
        .unwrap_or_default();

    // Windows-safe directory segment; dots inside assembly identities are
    // meaningful and must not be erased. Never introduce path separators.
    let mut label = candidate
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    label = label.trim_matches(|ch: char| ch == ' ' || ch == '.').to_string();
    let reserved = label.to_ascii_uppercase();
    if label.is_empty()
        || matches!(reserved.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (reserved.len() == 4
            && (reserved.starts_with("COM") || reserved.starts_with("LPT"))
            && reserved.as_bytes()[3].is_ascii_digit())
    {
        // A generic "Projeto" for all unidentified sources would conflate
        // unrelated mods. Derive a stable fallback from the evidence itself.
        let stable_input = evidence
            .assembly_identities
            .first()
            .or_else(|| evidence.namespaces.first())
            .or_else(|| evidence.classes.first())
            .map(String::as_str)
            .unwrap_or("unknown-project");
        let hash = stable_project_id(stable_input);
        return format!("Projeto-{}", &hash[8..16]);
    }
    if label.chars().count() > 64 {
        let hash = stable_project_id(&candidate);
        let suffix = format!("-{}", &hash[8..16]);
        let prefix = label.chars().take(64 - suffix.len()).collect::<String>();
        label = format!("{}{}", prefix.trim_end_matches(|ch: char| ch == ' ' || ch == '.'), suffix);
    }
    label
}


pub fn extract_package_evidence(path: &Path, package: &Package) -> ProjectTechnicalEvidence {
    let mut evidence = ProjectTechnicalEvidence {
        has_s3sa: package
            .entries
            .iter()
            .any(|entry| entry.type_id == TYPE_S3SA),
        ..ProjectTechnicalEvidence::default()
    };
    let mut textual = Vec::new();
    for entry in &package.entries {
        evidence
            .resource_tgis
            .push(tgi_string(entry.type_id, entry.group, entry.instance));
        let relevant = matches!(
            entry.type_id,
            TYPE_S3SA | TYPE_OBJK | TYPE_XML | TYPE_ITUN | TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST
        );
        if !relevant {
            continue;
        }
        let Ok(data) = package.data(entry) else {
            continue;
        };
        let strings = printable_strings(&data);
        if matches!(
            entry.type_id,
            TYPE_S3SA | TYPE_OBJK | TYPE_XML | TYPE_ITUN | TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST
        ) {
            textual.extend(strings.iter().cloned());
        }
        if matches!(
            entry.type_id,
            TYPE_S3SA | TYPE_XML | TYPE_ITUN | TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST
        ) {
            evidence.identifiers.extend(strings.iter().cloned());
        }
        if entry.type_id == TYPE_OBJK {
            evidence.script_classes.extend(
                dotted_identifiers(&strings)
                    .into_iter()
                    .filter(|value| script_class_like(value)),
            );
        }
        if entry.type_id == TYPE_S3SA {
            let dotted = dotted_identifiers(&strings);
            evidence.assembly_identities.extend(
                strings
                    .iter()
                    .filter(|value| value.to_ascii_lowercase().ends_with(".dll"))
                    .map(|value| value.trim_end_matches(".dll").to_string()),
            );
            evidence.assembly_identities.extend(
                dotted
                    .iter()
                    .filter(|value| {
                        !value.to_ascii_lowercase().starts_with("sims3.")
                            && !value.to_ascii_lowercase().starts_with("system.")
                            && value.len() <= 120
                    })
                    .cloned(),
            );
            evidence.classes.extend(
                dotted
                    .iter()
                    .filter(|value| script_class_like(value))
                    .cloned(),
            );
        }
        if entry.type_id != TYPE_S3SA {
            evidence.referenced_tgis.extend(referenced_tgis(&data));
        }
    }
    evidence.namespaces.extend(
        evidence
            .classes
            .iter()
            .filter_map(|value| value.rfind('.').map(|index| value[..index].to_string())),
    );
    evidence.identifiers.extend(textual);
    let unique = |values: &mut Vec<String>| {
        values.retain(|value| meaningful(value));
        values.sort_by_key(|value| normalize(value));
        values.dedup_by(|left, right| normalize(left) == normalize(right));
    };
    unique(&mut evidence.assembly_identities);
    unique(&mut evidence.namespaces);
    unique(&mut evidence.classes);
    unique(&mut evidence.script_classes);
    unique(&mut evidence.identifiers);
    evidence.resource_tgis.sort();
    evidence.resource_tgis.dedup();
    evidence.referenced_tgis.sort();
    evidence.referenced_tgis.dedup();
    let _ = path; // retained in the signature to make call sites self-documenting
    evidence
}

fn stable_project_id(path: &str) -> String {
    let hash = path
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |acc, byte| {
            (acc ^ *byte as u64).wrapping_mul(0x100000001b3)
        });
    format!("project-{hash:016X}")
}

fn usable_tgi(value: &str) -> bool {
    !value.ends_with("-0x0000000000000000") && !value.ends_with("-0x00000000-0x0000000000000000")
}

fn usable_identity(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let version_like = lower
        .strip_prefix('v')
        .unwrap_or(&lower)
        .split('.')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()));
    value.chars().any(|ch| ch.is_ascii_alphabetic())
        && !version_like
        && !lower.contains("mscoree")
        && !lower.contains("version")
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    let right = right
        .iter()
        .map(|value| normalize(value))
        .collect::<HashSet<_>>();
    left.iter()
        .filter(|value| right.contains(&normalize(value)))
        .cloned()
        .collect()
}

pub fn assign_project_groups(
    packages: &[(String, ProjectTechnicalEvidence)],
) -> HashMap<String, ProjectAssignment> {
    let anchors = packages
        .iter()
        .enumerate()
        .filter(|(_, (_, evidence))| evidence.has_s3sa && !evidence.assembly_identities.is_empty())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (index, (path, evidence)) in packages.iter().enumerate() {
        for anchor in &anchors {
            if index == *anchor {
                continue;
            }
            let (_, anchor_evidence) = &packages[*anchor];
            let script = intersection(&evidence.script_classes, &anchor_evidence.classes);
            let assembly =
                intersection(&evidence.identifiers, &anchor_evidence.assembly_identities)
                    .into_iter()
                    .filter(|value| usable_identity(value))
                    .collect::<Vec<_>>();
            let tgi = intersection(&evidence.referenced_tgis, &anchor_evidence.resource_tgis)
                .into_iter()
                .filter(|value| usable_tgi(value))
                .collect::<Vec<_>>();
            let identifier = intersection(&evidence.identifiers, &anchor_evidence.identifiers)
                .into_iter()
                .filter(|value| value.len() >= 10 && value.contains('.'))
                .take(3)
                .collect::<Vec<_>>();
            let (relation, evidence_text, confidence, dependency_kind) = if !script.is_empty() {
                (
                    "ScriptClassExact",
                    format!(
                        "OBJK Script Class matches S3SA class: {}",
                        script.join(", ")
                    ),
                    "confirmed",
                    "project_component",
                )
            } else if !assembly.is_empty() {
                (
                    "AssemblyReference",
                    format!(
                        "Component metadata references S3SA assembly identity: {}",
                        assembly.join(", ")
                    ),
                    "probable",
                    "external_dependency",
                )
            } else if !tgi.is_empty() {
                (
                    "TGIReference",
                    format!(
                        "Exact TGI reference targets the script package resource: {}",
                        tgi.join(", ")
                    ),
                    "probable",
                    "external_dependency",
                )
            } else if !identifier.is_empty() && !evidence.has_s3sa {
                (
                    "SharedIdentifier",
                    format!(
                        "Shared non-generic tuning/identifier evidence: {}",
                        identifier.join(", ")
                    ),
                    "probable",
                    "shared_library",
                )
            } else {
                continue;
            };
            edges.push(Edge {
                left: index,
                right: *anchor,
                relation: relation.to_string(),
                evidence: evidence_text,
                confidence: confidence.to_string(),
                dependency_kind: dependency_kind.to_string(),
            });
            let _ = path;
        }
    }

    let mut assignments = HashMap::new();
    for (index, (path, _evidence)) in packages.iter().enumerate() {
        let all_candidates = edges
            .iter()
            .filter(|edge| edge.left == index || edge.right == index)
            .collect::<Vec<_>>();
        if all_candidates.is_empty() {
            continue;
        }
        let mut confirmed_anchors = all_candidates
            .iter()
            .filter(|edge| edge.confidence == "confirmed")
            .map(|edge| {
                if edge.right == index {
                    edge.left
                } else {
                    edge.right
                }
            })
            .filter(|candidate| anchors.contains(candidate))
            .collect::<BTreeSet<_>>();
        if anchors.contains(&index)
            && all_candidates.iter().any(|edge| {
                edge.confidence == "confirmed"
                    && edge.right == index
                    && !anchors.contains(&edge.left)
            })
        {
            confirmed_anchors.insert(index);
        }
        let probable_anchors = all_candidates
            .iter()
            .map(|edge| {
                if edge.right == index {
                    edge.left
                } else {
                    edge.right
                }
            })
            .filter(|candidate| anchors.contains(candidate))
            .collect::<BTreeSet<_>>();

        let (anchor, role, confidence) = if confirmed_anchors.len() == 1 {
            let anchor = *confirmed_anchors.iter().next().unwrap();
            (
                anchor,
                if index == anchor {
                    "script_primary"
                } else {
                    "project_component"
                },
                "confirmed",
            )
        } else if confirmed_anchors.len() > 1 {
            let anchor = *confirmed_anchors.iter().next().unwrap();
            (anchor, "shared_dependency", "confirmed")
        } else if probable_anchors.len() == 1 {
            let anchor = *probable_anchors.iter().next().unwrap();
            (anchor, "association_suggestion", "probable")
        } else {
            continue;
        };
        let (anchor_path, anchor_evidence) = &packages[anchor];
        let group_id = if role == "shared_dependency" {
            let shared = confirmed_anchors
                .iter()
                .map(|candidate| packages[*candidate].0.as_str())
                .collect::<Vec<_>>()
                .join("|");
            stable_project_id(&shared)
        } else {
            stable_project_id(anchor_path)
        };
        let label = project_label(anchor_evidence);
        let relationships = all_candidates
            .iter()
            .filter(|edge| edge.left == index || edge.right == index)
            .map(|edge| {
                let related_index = if edge.left == index {
                    edge.right
                } else {
                    edge.left
                };
                ProjectRelationship {
                    related_path: packages[related_index].0.clone(),
                    relation: edge.relation.clone(),
                    evidence: edge.evidence.clone(),
                    confidence: edge.confidence.clone(),
                    dependency_kind: edge.dependency_kind.clone(),
                }
            })
            .collect::<Vec<_>>();
        assignments.insert(
            path.clone(),
            ProjectAssignment {
                group_id,
                label,
                confidence: confidence.to_string(),
                role: role.to_string(),
                relationships,
            },
        );
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn package_with_resources(resources: &[(u32, &[u8])]) -> Vec<u8> {
        let data_offset = 96u32;
        let mut bytes = vec![0u8; data_offset as usize];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&(resources.len() as u32).to_le_bytes());
        bytes[44..48].copy_from_slice(&((4 + resources.len() * 32) as u32).to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        let mut payload_offsets = Vec::new();
        for (_, payload) in resources {
            payload_offsets.push(bytes.len() as u32);
            bytes.extend_from_slice(payload);
        }
        let index_position = bytes.len() as u32;
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for ((type_id, payload), offset) in resources.iter().zip(payload_offsets) {
            for value in [
                *type_id,
                0,
                0,
                (0x1000 + *type_id) as u32,
                offset,
                payload.len() as u32,
                payload.len() as u32,
                0,
            ] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        bytes[64..68].copy_from_slice(&index_position.to_le_bytes());
        bytes
    }

    fn evidence(script: bool, assembly: &str) -> ProjectTechnicalEvidence {
        ProjectTechnicalEvidence {
            has_s3sa: script,
            assembly_identities: if script {
                vec![assembly.into()]
            } else {
                Vec::new()
            },
            classes: if script {
                vec![format!("{assembly}.MainController")]
            } else {
                Vec::new()
            },
            ..ProjectTechnicalEvidence::default()
        }
    }

    #[test]
    fn custom_objk_script_class_creates_one_confirmed_project_assignment() {
        let mut component = evidence(false, "");
        component.script_classes = vec!["Example.Mod.MainController".into()];
        let anchor = evidence(true, "Example.Mod");
        let assignments = assign_project_groups(&[
            ("object.package".into(), component),
            ("main.package".into(), anchor),
        ]);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments["object.package"].confidence, "confirmed");
        assert_eq!(assignments["object.package"].role, "project_component");
        assert_eq!(assignments["object.package"].label, "Example.Mod");
    }

    #[test]
    fn project_labels_preserve_distinct_assembly_identities() {
        let left = evidence(true, "FirstCreator.Mod");
        let right = evidence(true, "SecondCreator.Mod");
        assert_eq!(project_label(&left), "FirstCreator.Mod");
        assert_eq!(project_label(&right), "SecondCreator.Mod");
        assert_ne!(project_label(&left), project_label(&right));
    }

    #[test]
    fn project_label_accepts_custom_sims3_prefixed_assemblies() {
        let anchor = evidence(true, "Sims3.NeoH4x0r.Core");
        assert_eq!(project_label(&anchor), "Sims3.NeoH4x0r.Core");
    }

    #[test]
    fn project_label_is_safe_as_a_single_windows_directory() {
        let mut anchor = evidence(true, "Creator:Example/Mod\\Variant");
        let label = project_label(&anchor);
        assert!(!label.is_empty());
        assert!(!label.contains([':', '/', '\\']));
        assert!(!label.ends_with(' ') && !label.ends_with('.'));
        anchor.assembly_identities = vec!["AB".repeat(80)];
        assert!(project_label(&anchor).chars().count() <= 64);
    }

    #[test]
    fn generic_game_class_does_not_create_an_assignment() {
        let mut component = evidence(false, "");
        component.script_classes = vec!["Sims3.Gameplay.Objects.CookingObjects.FoodProp".into()];
        let anchor = evidence(true, "Example.Mod");
        let assignments = assign_project_groups(&[
            ("object.package".into(), component),
            ("main.package".into(), anchor),
        ]);
        assert!(assignments.is_empty());
    }

    #[test]
    fn probable_shared_identifier_is_not_physical_assignment() {
        let mut component = evidence(false, "");
        component.identifiers = vec!["Example.Mod.SharedTuning".into()];
        let mut anchor = evidence(true, "Example.Mod");
        anchor.identifiers = vec!["Example.Mod.SharedTuning".into()];
        let assignments = assign_project_groups(&[
            ("object.package".into(), component),
            ("main.package".into(), anchor),
        ]);
        assert_eq!(assignments["object.package"].confidence, "probable");
        assert_eq!(assignments["object.package"].role, "association_suggestion");
    }

    #[test]
    fn assembly_reference_is_probable_external_dependency() {
        let mut component = evidence(false, "");
        component.identifiers = vec!["Example.Core".into()];
        let anchor = evidence(true, "Example.Core");
        let assignments = assign_project_groups(&[
            ("library.package".into(), component),
            ("main.package".into(), anchor),
        ]);
        assert_eq!(assignments["library.package"].confidence, "probable");
        assert_eq!(
            assignments["library.package"].role,
            "association_suggestion"
        );
        assert_eq!(
            assignments["library.package"].relationships[0].dependency_kind,
            "external_dependency"
        );
    }

    #[test]
    fn component_linked_to_two_script_anchors_is_shared_not_grouped() {
        let mut component = evidence(false, "");
        component.script_classes = vec![
            "Example.First.SharedObject".into(),
            "Example.Second.SharedObject".into(),
        ];
        let mut first = evidence(true, "Example.First");
        first.classes = vec!["Example.First.SharedObject".into()];
        let mut second = evidence(true, "Example.Second");
        second.classes = vec!["Example.Second.SharedObject".into()];
        let assignments = assign_project_groups(&[
            ("shared.package".into(), component),
            ("first.package".into(), first),
            ("second.package".into(), second),
        ]);
        assert_eq!(assignments["shared.package"].role, "shared_dependency");
        assert_eq!(assignments["shared.package"].confidence, "confirmed");
    }

    #[test]
    fn real_dbpf_payloads_produce_script_and_objk_evidence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("s3cc-project-evidence-{nonce}.package"));
        let bytes = package_with_resources(&[
            (
                TYPE_S3SA,
                b"ExampleProject.dll\0ExampleProject.MainController\0",
            ),
            (TYPE_OBJK, b"ExampleProject.MainController\0"),
        ]);
        fs::write(&path, bytes).unwrap();
        let package = Package::load(&path).unwrap();
        let evidence = extract_package_evidence(&path, &package);
        assert!(evidence.has_s3sa);
        assert!(evidence
            .assembly_identities
            .iter()
            .any(|value| value.eq_ignore_ascii_case("ExampleProject")));
        assert!(evidence
            .script_classes
            .iter()
            .any(|value| value == "ExampleProject.MainController"));
        fs::remove_file(path).unwrap();
    }
}
