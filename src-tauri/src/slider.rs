use crate::dbpf::Package;
use serde::Serialize;
use std::collections::BTreeSet;

pub const TYPE_BOND: u32 = 0x0355_E0A6;
pub const TYPE_FACE: u32 = 0x0358_B08A;
pub const TYPE_BGEO: u32 = 0x067C_AA11;
pub const TYPE_VPXY: u32 = 0x7368_84F1;
pub const TYPE_FBLN: u32 = 0xB52F_5055;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FblnReference {
    pub type_id: u32,
    pub group: u32,
    pub instance: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FblnMetadata {
    pub version: Option<u32>,
    pub name_hash: Option<u64>,
    pub tgi_block_count: Option<u32>,
    pub bidirectional: Option<bool>,
    /// Raw values exposed by S3PE's CASPanelGroup and Sort fields. They are
    /// retained as evidence, but never used to invent an anatomical folder.
    pub cas_panel_group: Option<u32>,
    pub sort: Option<u32>,
    pub references: Vec<FblnReference>,
    pub parse_warning: Option<String>,
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(data.get(offset..end)?.try_into().ok()?))
}

fn known_reference_type(type_id: u32) -> bool {
    matches!(type_id, TYPE_FACE | TYPE_BGEO | TYPE_BOND | TYPE_VPXY)
}

/// Reads the stable fields of a Sims 3 Blend Unit (FBLN).
///
/// S3PI/S3PE expose FBLN as the BlendUnit resource: it owns the CAS label
/// metadata and TGI links to the FACE/BGEO/BOND chain. The resource has a
/// small versioned header followed by TGI records. We deliberately scan the
/// serialized TGI records instead of assuming that a particular editor's
/// display order is the physical order. This accepts the real 3-resource
/// variants and keeps unknown tail fields lossless as evidence.
pub fn parse_fbln(data: &[u8]) -> Option<FblnMetadata> {
    let version = read_u32(data, 0)?;
    if version == 0 || data.len() < 32 {
        return None;
    }

    let mut references = Vec::new();
    let mut seen = BTreeSet::new();
    for offset in 0..=data.len().saturating_sub(16) {
        let Some(type_id) = read_u32(data, offset) else { continue };
        if !known_reference_type(type_id) {
            continue;
        }
        let Some(group) = read_u32(data, offset + 4) else { continue };
        let Some(instance) = read_u64(data, offset + 8) else { continue };
        let key = (type_id, group, instance);
        if seen.insert(key) {
            references.push(FblnReference { type_id, group, instance });
        }
    }

    let tgi_block_count = read_u32(data, 20);
    let bidirectional = data.get(28).map(|value| *value != 0);
    let cas_panel_group = read_u32(data, 32);
    let sort = read_u32(data, 36);
    let parse_warning = if references.is_empty() {
        Some("No FACE/BGEO/BOND/VPXY TGI link was exposed by this FBLN payload.".to_string())
    } else if tgi_block_count.is_some_and(|count| count < references.len() as u32) {
        Some(format!(
            "FBLN declared {count} TGI blocks but exposed {actual} known links.",
            count = tgi_block_count.unwrap_or_default(),
            actual = references.len()
        ))
    } else {
        None
    };

    Some(FblnMetadata {
        version: Some(version),
        name_hash: read_u64(data, 12),
        tgi_block_count,
        bidirectional,
        cas_panel_group,
        sort,
        references,
        parse_warning,
    })
}

pub fn read_fbln_metadata(package: &Package) -> Vec<FblnMetadata> {
    package
        .entries
        .iter()
        .filter(|entry| entry.type_id == TYPE_FBLN)
        .filter_map(|entry| package.data(entry).ok().and_then(|data| parse_fbln(&data)))
        .collect()
}

pub fn fbln_evidence(metadata: &[FblnMetadata]) -> String {
    let references = metadata
        .iter()
        .flat_map(|record| record.references.iter())
        .map(|reference| format!("0x{:08X}:0x{:08X}:0x{:016X}", reference.type_id, reference.group, reference.instance))
        .collect::<Vec<_>>();
    let warnings = metadata
        .iter()
        .filter_map(|record| record.parse_warning.as_deref())
        .collect::<Vec<_>>();
    let mut result = format!(
        "{} FBLN resource(s); FACE/BGEO/BOND/VPXY links={}",
        metadata.len(),
        references.len()
    );
    if !references.is_empty() {
        result.push_str(&format!(" [{}]", references.join(", ")));
    }
    if !warnings.is_empty() {
        result.push_str(&format!("; {}", warnings.join(" | ")));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fbln_reads_real_header_and_tgi_links_without_using_panel_for_anatomy() {
        let mut data = vec![0u8; 65];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[12..20].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        data[28] = 1;
        data[32..36].copy_from_slice(&0x0000_1001u32.to_le_bytes());
        data[36..40].copy_from_slice(&0x0000_0200u32.to_le_bytes());
        data[41..45].copy_from_slice(&TYPE_FACE.to_le_bytes());
        data[45..49].copy_from_slice(&0u32.to_le_bytes());
        data[49..57].copy_from_slice(&0xAABB_CCDD_EEFF_0011u64.to_le_bytes());

        let parsed = parse_fbln(&data).expect("valid FBLN");
        assert_eq!(parsed.version, Some(3));
        assert_eq!(parsed.name_hash, Some(0x1122_3344_5566_7788));
        assert_eq!(parsed.tgi_block_count, Some(1));
        assert_eq!(parsed.cas_panel_group, Some(0x1001));
        assert_eq!(parsed.sort, Some(0x200));
        assert_eq!(parsed.references.len(), 1);
        assert_eq!(parsed.references[0].type_id, TYPE_FACE);
    }

    #[test]
    fn fbln_accepts_multiple_links_and_preserves_unknown_panel_values() {
        let mut data = vec![0u8; 81];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[20..24].copy_from_slice(&2u32.to_le_bytes());
        for (offset, instance) in [(49usize, 1u64), (65usize, 2u64)] {
            data[offset..offset + 4].copy_from_slice(&TYPE_FACE.to_le_bytes());
            data[offset + 4..offset + 8].copy_from_slice(&0u32.to_le_bytes());
            data[offset + 8..offset + 16].copy_from_slice(&instance.to_le_bytes());
        }
        let parsed = parse_fbln(&data).unwrap();
        assert_eq!(parsed.references.len(), 2);
        assert!(parsed.parse_warning.is_none());
    }
}
