//! Evidence-driven grouping of packages that form one mod project.
//!
//! This module deliberately does not know any creator, mod, or filename.  It
//! extracts stable technical identifiers while the scanner already has the
//! DBPF open and builds relations only when a component points at a script
//! package through a class, assembly identity, or exact TGI reference.

use crate::dbpf::Package;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

pub const TYPE_OBJK: u32 = 0x02DC_343F;
pub const TYPE_XML: u32 = 0x0333_406C;
pub const TYPE_ITUN: u32 = 0x03B3_3DDF;
pub const TYPE_S3SA: u32 = 0x073F_AA07;
pub const TYPE_STBL: u32 = 0x2205_57DA;
pub const TYPE_NMAP: u32 = 0x0166_038C;
pub const TYPE_MANIFEST: u32 = 0x73E9_3EEB;
pub const TYPE_LAYO: u32 = 0x025C_95B6;

// Directed scanner families use these identifiers as corroborating evidence.
// Keep their byte-level detection beside the shared extraction pass so a large
// decoded payload is not searched again later by the scanner.
pub const INTERNAL_IDENTIFIER_MARKERS: &[&str] = &[
    "casdetails",
    "ui/caption/cas/physical:",
    "deathbymeteorjig",
    "minerexclusionjig",
    "acbody",
    "adbody",
    "albody",
    "ambody",
    "embody",
    "tmbody",
    "ccbody",
    "cdbody",
    "genital",
    "defaultface",
    "defaultwhisker",
    "defaultbrow",
    "defaulttextures",
    "sims_need_less_space",
    "pets_need_less_space",
    "king_qings_window",
];

fn internal_markers_starting_with(byte: u8) -> &'static [&'static str] {
    match byte {
        b'a' => &["acbody", "adbody", "albody", "ambody"],
        b'c' => &["casdetails", "ccbody", "cdbody"],
        b'd' => &[
            "deathbymeteorjig",
            "defaultface",
            "defaultwhisker",
            "defaultbrow",
            "defaulttextures",
        ],
        b'e' => &["embody"],
        b'g' => &["genital"],
        b'k' => &["king_qings_window"],
        b'm' => &["minerexclusionjig"],
        b'p' => &["pets_need_less_space"],
        b's' => &["sims_need_less_space"],
        b't' => &["tmbody"],
        b'u' => &["ui/caption/cas/physical:"],
        _ => &[],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTechnicalEvidence {
    pub has_s3sa: bool,
    pub assembly_identities: Vec<String>,
    pub namespaces: Vec<String>,
    pub classes: Vec<String>,
    pub script_classes: Vec<String>,
    pub identifiers: Vec<String>,
    #[serde(default)]
    pub store_marker: bool,
    #[serde(default)]
    pub internal_markers: Vec<String>,
    pub resource_tgis: Vec<String>,
    pub referenced_tgis: Vec<String>,
}

/// Work performed while extracting a package's project evidence.  Keep this
/// separate from assignment time: reading a compressed STBL/OBJK payload and
/// matching already-extracted evidence against project anchors have very
/// different optimization paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectEvidenceMetrics {
    pub resource_decode_us: u128,
    pub decoded_resource_count: usize,
    pub payload_analysis_us: u128,
    pub stbl_resource_count: usize,
    pub nmap_resource_count: usize,
    pub objk_resource_count: usize,
    pub other_resource_count: usize,
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

/// Extract human-readable identifiers and, when requested, binary TGI
/// references in one sequential pass over a decoded resource. Large Store
/// resources previously made three independent passes here (ASCII, UTF-16LE,
/// then TGI probing). The output remains the same sorted/deduplicated sets.
fn store_marker_in_text(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("sims3.store")
        || value.contains("the sims 3 store")
        || value.contains("sims3store")
}

fn extract_textual_evidence(
    data: &[u8],
    include_references: bool,
) -> (Vec<String>, Vec<String>, bool, Vec<String>) {
    // Insertion into a BTreeSet for every STBL string dominated CPU time for
    // Store worlds with thousands of localized entries. Preserve the exact
    // sorted/deduplicated output, but collect sequentially while the payload
    // is hot and order it once after the pass.
    let mut values = Vec::new();
    let mut references = Vec::new();
    let mut store_marker = false;
    let mut internal_markers = BTreeSet::new();
    let mut ascii = Vec::new();
    let flush_ascii = |ascii: &mut Vec<u8>, values: &mut Vec<String>, store_marker: &mut bool| {
        if ascii.len() >= 5 {
            if let Ok(value) = String::from_utf8(std::mem::take(ascii)) {
                let value = value.trim().to_string();
                *store_marker |= store_marker_in_text(&value);
                if meaningful(&value) {
                    values.push(value);
                }
            }
        } else {
            ascii.clear();
        }
    };
    let mut utf16 = Vec::<u16>::new();
    let flush_utf16 = |utf16: &mut Vec<u16>, values: &mut Vec<String>| {
        if utf16.len() >= 4 {
            let value = String::from_utf16_lossy(utf16).trim().to_string();
            if meaningful(&value) {
                values.push(value);
            }
        }
        utf16.clear();
    };

    for (offset, byte) in data.iter().enumerate() {
        // Preserve the old byte-for-byte, ASCII-case-insensitive marker
        // semantics. We only compare a complete marker when its first byte
        // matches, rather than performing one complete pass per marker.
        let lowered = byte.to_ascii_lowercase();
        for marker_text in internal_markers_starting_with(lowered) {
            let marker = marker_text.as_bytes();
            if data
                .get(offset..offset.saturating_add(marker.len()))
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(marker))
            {
                internal_markers.insert((*marker_text).to_string());
            }
        }
        if (0x20..=0x7e).contains(byte) {
            ascii.push(*byte);
        } else {
            flush_ascii(&mut ascii, &mut values, &mut store_marker);
        }

        // CLR metadata and many XML resources also carry UTF-16LE strings.
        // Evaluate one unit at every even byte while retaining the same
        // printable-run semantics as the former dedicated pass.
        if offset % 2 == 0 {
            if let Some(pair) = data.get(offset..offset + 2) {
                let unit = u16::from_le_bytes([pair[0], pair[1]]);
                if (0x20..=0x7e).contains(&unit) {
                    utf16.push(unit);
                } else {
                    flush_utf16(&mut utf16, &mut values);
                }
            }
        }

        // Keep the original probing boundary: the legacy range was exclusive
        // at `len - 16`, so a byte-perfect cache/result comparison must not
        // add a new final TGI candidate merely because this pass is combined.
        if include_references
            && offset % 4 == 0
            && offset < data.len().saturating_sub(16)
        {
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
            references.push(tgi_string(type_id, group, instance));
        }
    }
    flush_ascii(&mut ascii, &mut values, &mut store_marker);
    flush_utf16(&mut utf16, &mut values);
    values.sort();
    values.dedup();
    references.sort();
    references.dedup();
    (values, references, store_marker, internal_markers.into_iter().collect())
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
    extract_package_evidence_with_metrics(path, package).0
}

pub fn extract_package_evidence_with_metrics(
    path: &Path,
    package: &Package,
) -> (ProjectTechnicalEvidence, ProjectEvidenceMetrics) {
    let mut evidence = ProjectTechnicalEvidence {
        has_s3sa: package
            .entries
            .iter()
            .any(|entry| entry.type_id == TYPE_S3SA),
        ..ProjectTechnicalEvidence::default()
    };
    let mut metrics = ProjectEvidenceMetrics::default();
    let mut textual = Vec::new();
    for entry in &package.entries {
        // `assign_project_groups` indexes a package's own resource TGIs only
        // when that package is a S3SA anchor. Components are matched through
        // their decoded *references*, not their complete DBPF index. Avoid
        // materializing and sorting thousands of otherwise-unused TGI strings
        // for large Store/world packages without scripts.
        if evidence.has_s3sa {
            evidence
                .resource_tgis
                .push(tgi_string(entry.type_id, entry.group, entry.instance));
        }
        let relevant = matches!(
            entry.type_id,
            TYPE_S3SA
                | TYPE_OBJK
                | TYPE_XML
                | TYPE_ITUN
                | TYPE_STBL
                | TYPE_NMAP
                | TYPE_MANIFEST
                | TYPE_LAYO
        );
        if !relevant {
            continue;
        }
        let decode_started = std::time::Instant::now();
        let Ok(data) = package.data(entry) else {
            metrics.resource_decode_us += decode_started.elapsed().as_micros();
            continue;
        };
        metrics.resource_decode_us += decode_started.elapsed().as_micros();
        metrics.decoded_resource_count += 1;
        match entry.type_id {
            TYPE_STBL => metrics.stbl_resource_count += 1,
            TYPE_NMAP => metrics.nmap_resource_count += 1,
            TYPE_OBJK => metrics.objk_resource_count += 1,
            _ => metrics.other_resource_count += 1,
        }
        let analysis_started = std::time::Instant::now();
        let (strings, references, store_marker, markers) = if entry.type_id == TYPE_S3SA {
            extract_textual_evidence(&data, false)
        } else {
            extract_textual_evidence(&data, true)
        };
        metrics.payload_analysis_us += analysis_started.elapsed().as_micros();
        evidence.store_marker |= store_marker;
        evidence.internal_markers.extend(markers);
        if matches!(
            entry.type_id,
            TYPE_S3SA | TYPE_OBJK | TYPE_XML | TYPE_ITUN | TYPE_STBL | TYPE_NMAP | TYPE_MANIFEST
        ) {
            textual.extend(strings.iter().cloned());
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
            evidence.referenced_tgis.extend(references);
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
        // `identifiers` can contain thousands of repeated STBL/XML strings
        // in large Store packages.  The previous sort comparator normalized
        // the same strings many times and temporarily held two full copies
        // of each value.  A normalized ordered map preserves the resulting
        // case-insensitive set and its ordering while normalizing once.
        let mut unique = BTreeMap::<String, String>::new();
        for value in std::mem::take(values) {
            if meaningful(&value) {
                unique.entry(normalize(&value)).or_insert(value);
            }
        }
        values.extend(unique.into_values());
    };
    unique(&mut evidence.assembly_identities);
    unique(&mut evidence.namespaces);
    unique(&mut evidence.classes);
    unique(&mut evidence.script_classes);
    unique(&mut evidence.identifiers);
    evidence.internal_markers.sort();
    evidence.internal_markers.dedup();
    evidence.resource_tgis.sort();
    evidence.resource_tgis.dedup();
    evidence.referenced_tgis.sort();
    evidence.referenced_tgis.dedup();
    let _ = path; // retained in the signature to make call sites self-documenting
    (evidence, metrics)
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

fn add_indexed_matches(
    values: &[String],
    index: &HashMap<String, Vec<usize>>,
    matches: &mut HashMap<usize, Vec<String>>,
    seen: &mut HashMap<usize, HashSet<String>>,
    predicate: impl Fn(&String) -> bool,
) {
    for value in values.iter().filter(|value| predicate(value)) {
        let key = normalize(value);
        let Some(anchors) = index.get(&key) else {
            continue;
        };
        for anchor in anchors {
            let already_seen = seen.entry(*anchor).or_default().insert(key.clone());
            if already_seen {
                matches.entry(*anchor).or_default().push(value.clone());
            }
        }
    }
}

pub fn assign_project_groups(
    packages: &[(String, ProjectTechnicalEvidence)],
) -> HashMap<String, ProjectAssignment> {
    let references = packages
        .iter()
        .map(|(path, evidence)| (path.as_str(), evidence))
        .collect::<Vec<_>>();
    assign_project_groups_references(&references)
}

/// Borrowed variant used by the scanner. Project evidence can be large for
/// Store packages; cloning every vector merely to establish that no script
/// anchor exists made post-scan finalization disproportionately expensive.
/// The owned-input helper remains for existing callers and tests.
pub fn assign_project_groups_references(
    packages: &[(&str, &ProjectTechnicalEvidence)],
) -> HashMap<String, ProjectAssignment> {
    let anchors = packages
        .iter()
        .enumerate()
        .filter(|(_, (_, evidence))| evidence.has_s3sa && !evidence.assembly_identities.is_empty())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    // Build inverted evidence indexes once. The previous implementation
    // rebuilt a HashSet for every package/anchor pair and therefore repeated
    // the same normalization work O(packages × anchors) times. These indexes
    // preserve the exact relation precedence below while turning candidate
    // discovery into lookups over only anchors that share evidence.
    let mut classes_by_anchor = HashMap::<String, Vec<usize>>::new();
    let mut assemblies_by_anchor = HashMap::<String, Vec<usize>>::new();
    let mut tgis_by_anchor = HashMap::<String, Vec<usize>>::new();
    let mut identifiers_by_anchor = HashMap::<String, Vec<usize>>::new();
    for anchor in &anchors {
        let (_, evidence) = &packages[*anchor];
        for value in &evidence.classes {
            classes_by_anchor
                .entry(normalize(value))
                .or_default()
                .push(*anchor);
        }
        for value in &evidence.assembly_identities {
            assemblies_by_anchor
                .entry(normalize(value))
                .or_default()
                .push(*anchor);
        }
        for value in &evidence.resource_tgis {
            tgis_by_anchor
                .entry(normalize(value))
                .or_default()
                .push(*anchor);
        }
        for value in &evidence.identifiers {
            identifiers_by_anchor
                .entry(normalize(value))
                .or_default()
                .push(*anchor);
        }
    }
    let mut edges = Vec::new();
    for (index, (path, evidence)) in packages.iter().enumerate() {
        let mut script_matches = HashMap::new();
        let mut script_seen = HashMap::new();
        let mut assembly_matches = HashMap::new();
        let mut assembly_seen = HashMap::new();
        let mut tgi_matches = HashMap::new();
        let mut tgi_seen = HashMap::new();
        let mut identifier_matches = HashMap::new();
        let mut identifier_seen = HashMap::new();
        add_indexed_matches(
            &evidence.script_classes,
            &classes_by_anchor,
            &mut script_matches,
            &mut script_seen,
            |_| true,
        );
        add_indexed_matches(
            &evidence.identifiers,
            &assemblies_by_anchor,
            &mut assembly_matches,
            &mut assembly_seen,
            |value| usable_identity(value),
        );
        add_indexed_matches(
            &evidence.referenced_tgis,
            &tgis_by_anchor,
            &mut tgi_matches,
            &mut tgi_seen,
            |value| usable_tgi(value),
        );
        add_indexed_matches(
            &evidence.identifiers,
            &identifiers_by_anchor,
            &mut identifier_matches,
            &mut identifier_seen,
            |value| value.len() >= 10 && value.contains('.'),
        );

        let mut candidate_anchors = BTreeSet::new();
        candidate_anchors.extend(script_matches.keys().copied());
        candidate_anchors.extend(assembly_matches.keys().copied());
        candidate_anchors.extend(tgi_matches.keys().copied());
        candidate_anchors.extend(identifier_matches.keys().copied());

        for anchor in candidate_anchors {
            if index == anchor {
                continue;
            }
            let script = script_matches.remove(&anchor).unwrap_or_default();
            let assembly = assembly_matches.remove(&anchor).unwrap_or_default();
            let tgi = tgi_matches.remove(&anchor).unwrap_or_default();
            let identifier = identifier_matches
                .remove(&anchor)
                .unwrap_or_default()
                .into_iter()
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
                right: anchor,
                relation: relation.to_string(),
                evidence: evidence_text,
                confidence: confidence.to_string(),
                dependency_kind: dependency_kind.to_string(),
            });
        }
        let _ = path;
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
                .map(|candidate| packages[*candidate].0)
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
                    related_path: packages[related_index].0.to_string(),
                    relation: edge.relation.clone(),
                    evidence: edge.evidence.clone(),
                    confidence: edge.confidence.clone(),
                    dependency_kind: edge.dependency_kind.clone(),
                }
            })
            .collect::<Vec<_>>();
        assignments.insert(
            (*path).to_string(),
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
        assert_eq!(evidence.resource_tgis.len(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn non_script_packages_do_not_materialize_unused_own_tgis() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("s3cc-project-non-script-{nonce}.package"));
        let bytes = package_with_resources(&[
            (TYPE_STBL, b"ExampleProject.SharedLabel\0"),
            (TYPE_OBJK, b"ExampleProject.MainController\0"),
        ]);
        fs::write(&path, bytes).unwrap();
        let package = Package::load(&path).unwrap();
        let evidence = extract_package_evidence(&path, &package);
        assert!(!evidence.has_s3sa);
        assert!(evidence.resource_tgis.is_empty());
        // Component evidence remains available for matching against a S3SA
        // anchor during the second, indexed grouping pass.
        assert!(evidence
            .script_classes
            .iter()
            .any(|value| value == "ExampleProject.MainController"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn store_marker_survives_identifier_filtering() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("s3cc-project-store-marker-{nonce}.package"));
        let bytes = package_with_resources(&[(TYPE_STBL, b"sims3.store\0")]);
        fs::write(&path, bytes).unwrap();
        let package = Package::load(&path).unwrap();
        let evidence = extract_package_evidence(&path, &package);
        assert!(evidence.store_marker);
        assert!(evidence
            .identifiers
            .iter()
            .all(|value| !value.eq_ignore_ascii_case("sims3.store")));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn internal_markers_preserve_case_insensitive_layo_evidence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("s3cc-project-directed-marker-{nonce}.package"));
        let bytes = package_with_resources(&[(
            TYPE_LAYO,
            b"prefix UI/CAPTION/CAS/PHYSICAL: suffix\0",
        )]);
        fs::write(&path, bytes).unwrap();
        let package = Package::load(&path).unwrap();
        let evidence = extract_package_evidence(&path, &package);
        assert_eq!(
            evidence.internal_markers,
            vec!["ui/caption/cas/physical:".to_string()]
        );
        fs::remove_file(path).unwrap();
    }
}
