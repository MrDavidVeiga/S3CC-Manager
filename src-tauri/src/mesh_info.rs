use crate::dbpf::Package;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

pub const TYPE_GEOM: u32 = 0x015A_1849;
pub const TYPE_MODL: u32 = 0x0166_1233;
pub const TYPE_MLOD: u32 = 0x01D1_0F34;
const TYPE_NMAP: u32 = 0x0166_038C;
const MAX_MESH_RESOURCE_BYTES: u32 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshGroupInfo {
    pub index: usize,
    pub name_hash: Option<String>,
    pub vertices: u64,
    pub primitives: u64,
    pub triangles: Option<u64>,
    pub primitive_type: String,
    pub shadow: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshResourceInfo {
    pub type_label: String,
    pub group_hex: String,
    pub instance_hex: String,
    pub tgi: String,
    pub internal_name: Option<String>,
    pub lod: String,
    pub shadow: bool,
    pub vertices: u64,
    pub triangles: Option<u64>,
    pub groups: Vec<MeshGroupInfo>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LodPolycountInfo {
    pub lod: String,
    pub vertices: u64,
    pub triangles: u64,
    pub resource_count: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackageMeshInfo {
    pub has_meshes: bool,
    pub highest_visible_triangles: Option<u64>,
    pub highest_visible_vertices: Option<u64>,
    pub lods: Vec<LodPolycountInfo>,
    pub resources: Vec<MeshResourceInfo>,
    pub warnings: Vec<String>,
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn geom_at(data: &[u8], mut cursor: usize) -> Result<(u64, u64), String> {
    // mergeGroup, sortOrder, NumVerts, FCount
    if cursor.checked_add(16).map(|v| v <= data.len()) != Some(true) {
        return Err("GEOM header is truncated.".into());
    }
    let num_vertices = read_u32(data, cursor + 8).unwrap() as u64;
    let format_count = read_u32(data, cursor + 12).unwrap() as usize;
    if num_vertices > 20_000_000 || format_count == 0 || format_count > 128 {
        return Err("GEOM vertex/format counts are outside safe limits.".into());
    }
    cursor += 16;

    // Each vertex format entry is DWORD DataType, DWORD SubType, BYTE BytesPerElement.
    let format_bytes = format_count
        .checked_mul(9)
        .ok_or_else(|| "GEOM vertex format overflow.".to_string())?;
    if cursor.checked_add(format_bytes).map(|v| v <= data.len()) != Some(true) {
        return Err("GEOM vertex format list is truncated.".into());
    }

    let mut vertex_stride = 0usize;
    for index in 0..format_count {
        let bytes_per_element = data[cursor + index * 9 + 8] as usize;
        if bytes_per_element == 0 || bytes_per_element > 64 {
            return Err("GEOM vertex element width is invalid.".into());
        }
        vertex_stride = vertex_stride
            .checked_add(bytes_per_element)
            .ok_or_else(|| "GEOM vertex stride overflow.".to_string())?;
    }
    if vertex_stride == 0 || vertex_stride > 512 {
        return Err("GEOM vertex stride is outside safe limits.".into());
    }
    cursor += format_bytes;

    let vertex_bytes = (num_vertices as usize)
        .checked_mul(vertex_stride)
        .ok_or_else(|| "GEOM vertex data size overflow.".to_string())?;
    cursor = cursor
        .checked_add(vertex_bytes)
        .ok_or_else(|| "GEOM vertex data offset overflow.".to_string())?;
    if cursor + 4 > data.len() {
        return Err("GEOM vertex data is truncated.".into());
    }

    let item_count = read_u32(data, cursor).unwrap() as usize;
    cursor += 4;
    if item_count == 0 || item_count > 32 || cursor + item_count + 4 > data.len() {
        return Err("GEOM face-point format is invalid.".into());
    }
    let bytes_per_face_point = data[cursor..cursor + item_count]
        .iter()
        .try_fold(0usize, |sum, width| {
            if *width == 0 || *width > 8 {
                None
            } else {
                sum.checked_add(*width as usize)
            }
        })
        .ok_or_else(|| "GEOM face-point width is invalid.".to_string())?;
    cursor += item_count;

    let face_points = read_u32(data, cursor).unwrap() as u64;
    cursor += 4;
    if face_points > 200_000_000 || face_points % 3 != 0 {
        return Err("GEOM face-point count is not a valid triangle list.".into());
    }
    let face_bytes = (face_points as usize)
        .checked_mul(bytes_per_face_point)
        .ok_or_else(|| "GEOM face data size overflow.".to_string())?;
    if cursor.checked_add(face_bytes).map(|v| v <= data.len()) != Some(true) {
        return Err("GEOM face data is truncated.".into());
    }

    Ok((num_vertices, face_points / 3))
}

fn parse_geom(data: &[u8]) -> Result<(u64, u64), String> {
    if data.len() < 36 || data.get(..4) != Some(&b"GEOM"[..]) {
        return Err("Resource does not start with a GEOM tag.".into());
    }
    let version = read_u32(data, 4).unwrap();
    if version != 5 {
        return Err(format!("Unsupported The Sims 3 GEOM version 0x{version:08X}."));
    }
    let embedded_id = read_u32(data, 16).unwrap();
    if embedded_id == 0 {
        return geom_at(data, 20);
    }

    if data.len() < 24 {
        return Err("Embedded GEOM material header is truncated.".into());
    }
    let chunk_size = read_u32(data, 20).unwrap() as usize;
    if chunk_size == 0 || chunk_size > data.len() {
        return Err("Embedded GEOM material chunk size is invalid.".into());
    }

    // SimsWiki describes ChunkSize followed by the embedded MTNF chunk.
    // Exporters have historically differed on whether the size includes the
    // size DWORD, so accept only a candidate that fully validates downstream.
    let mut errors = Vec::new();
    for cursor in [
        24usize.checked_add(chunk_size),
        20usize.checked_add(chunk_size),
    ]
    .into_iter()
    .flatten()
    {
        if cursor > data.len() {
            continue;
        }
        match geom_at(data, cursor) {
            Ok(value) => return Ok(value),
            Err(error) => errors.push(error),
        }
    }
    Err(format!(
        "Could not locate GEOM counts after embedded material: {}",
        errors.join(" | ")
    ))
}

fn primitive_name(value: u32) -> &'static str {
    match value {
        0 => "PointList",
        1 => "LineList",
        2 => "LineStrip",
        3 => "TriangleList",
        4 => "TriangleFan",
        5 => "TriangleStrip",
        6 => "QuadList",
        7 => "DisplayList",
        _ => "Unknown",
    }
}

fn primitive_triangles(kind: u32, primitives: u64) -> Option<u64> {
    match kind {
        3 | 4 | 5 => Some(primitives),
        6 => primitives.checked_mul(2),
        0 | 1 | 2 => Some(0),
        _ => None,
    }
}

fn parse_mlod_block(data: &[u8], offset: usize) -> Result<Vec<MeshGroupInfo>, String> {
    if offset.checked_add(12).map(|v| v <= data.len()) != Some(true)
        || data.get(offset..offset + 4) != Some(&b"MLOD"[..])
    {
        return Err("MLOD block header is missing.".into());
    }
    let version = read_u32(data, offset + 4).unwrap();
    if version < 0x0000_0201 || version > 0x0000_0300 {
        return Err(format!("Unsupported MLOD block version 0x{version:08X}."));
    }
    let group_count = read_u32(data, offset + 8).unwrap() as usize;
    if group_count == 0 || group_count > 4096 {
        return Err("MLOD group count is outside safe limits.".into());
    }

    let mut cursor = offset + 12;
    let mut groups = Vec::with_capacity(group_count);
    for index in 0..group_count {
        let subset_size = read_u32(data, cursor)
            .ok_or_else(|| "MLOD subset size is truncated.".to_string())? as usize;
        let body = cursor + 4;
        let end = body
            .checked_add(subset_size)
            .ok_or_else(|| "MLOD subset size overflow.".to_string())?;
        if subset_size < 48 || end > data.len() {
            return Err("MLOD subset is truncated or too small.".into());
        }

        let name_hash = read_u32(data, body).unwrap();
        let flags = read_u32(data, body + 20).unwrap();
        let primitive_type = flags & 0xFF;
        let mesh_flags = flags >> 8;
        let vertices = read_u32(data, body + 40).unwrap() as u64;
        let primitives = read_u32(data, body + 44).unwrap() as u64;
        if vertices > 100_000_000 || primitives > 200_000_000 {
            return Err("MLOD group counts exceed safe limits.".into());
        }

        groups.push(MeshGroupInfo {
            index,
            name_hash: Some(format!("0x{name_hash:08X}")),
            vertices,
            primitives,
            triangles: primitive_triangles(primitive_type, primitives),
            primitive_type: primitive_name(primitive_type).to_string(),
            // DropShadow is shadow-only. ShadowCaster describes behavior of
            // ordinary visible geometry and must not be excluded from polycount.
            shadow: mesh_flags & 0x08 != 0,
        });
        cursor = end;
    }
    Ok(groups)
}

fn parse_mlod(data: &[u8]) -> Result<Vec<MeshGroupInfo>, String> {
    let mut last_error = None;
    for (offset, window) in data.windows(4).enumerate() {
        if window != b"MLOD" {
            continue;
        }
        match parse_mlod_block(data, offset) {
            Ok(groups) => return Ok(groups),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| "No valid MLOD block was found.".into()))
}

fn parse_nmap_names(data: &[u8]) -> Vec<(u64, String)> {
    if data.len() < 8 || read_u32(data, 0) != Some(1) {
        return Vec::new();
    }
    let count = read_u32(data, 4).unwrap_or(0) as usize;
    if count > 100_000 {
        return Vec::new();
    }
    let mut cursor = 8usize;
    let mut result = Vec::new();
    for _ in 0..count {
        if cursor + 12 > data.len() {
            return Vec::new();
        }
        let hash = u64::from_le_bytes(data[cursor..cursor + 8].try_into().unwrap());
        let length = read_u32(data, cursor + 8).unwrap() as usize;
        cursor += 12;
        if cursor.checked_add(length).map(|v| v <= data.len()) != Some(true) {
            return Vec::new();
        }
        let name = String::from_utf8_lossy(&data[cursor..cursor + length])
            .trim()
            .to_string();
        cursor += length;
        if !name.is_empty() {
            result.push((hash, name));
        }
    }
    result
}

fn lod_from_name(name: &str) -> Option<String> {
    let compact = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    for (needle, label) in [
        ("lod0", "LOD 0 / High"),
        ("lod1", "LOD 1 / Medium"),
        ("lod2", "LOD 2 / Low"),
        ("lod3", "LOD 3 / Lowest"),
    ] {
        if compact.contains(needle) {
            return Some(label.into());
        }
    }
    None
}

fn object_lod(type_id: u32, group: u32) -> (String, bool) {
    match (type_id, group) {
        (TYPE_MLOD, 0x0000_0000) => ("LOD 0 / High".into(), false),
        (TYPE_MODL, 0x0000_0001) => ("LOD 1 / Low".into(), false),
        (TYPE_MLOD, 0x0001_0000) => ("Shadow / High".into(), true),
        (TYPE_MLOD, 0x0001_0001) => ("Shadow / Low".into(), true),
        (TYPE_MLOD, _) => (format!("MLOD 0x{group:08X}"), false),
        (TYPE_MODL, _) => (format!("MODL 0x{group:08X}"), false),
        _ => ("Mesh".into(), false),
    }
}

fn aggregate_lods(resources: &[MeshResourceInfo]) -> Vec<LodPolycountInfo> {
    let mut map = BTreeMap::<String, LodPolycountInfo>::new();
    for resource in resources {
        if resource.shadow {
            continue;
        }
        let Some(triangles) = resource.triangles else {
            continue;
        };
        let item = map.entry(resource.lod.clone()).or_insert_with(|| LodPolycountInfo {
            lod: resource.lod.clone(),
            ..LodPolycountInfo::default()
        });
        item.vertices = item.vertices.saturating_add(resource.vertices);
        item.triangles = item.triangles.saturating_add(triangles);
        item.resource_count += 1;
    }
    let mut values = map.into_values().collect::<Vec<_>>();
    values.sort_by_key(|item| {
        if item.lod.starts_with("LOD 0") {
            (0, item.lod.clone())
        } else if item.lod.starts_with("LOD 1") {
            (1, item.lod.clone())
        } else if item.lod.starts_with("LOD 2") {
            (2, item.lod.clone())
        } else if item.lod.starts_with("LOD 3") {
            (3, item.lod.clone())
        } else if item.lod == "GEOM / Unknown LOD" {
            (10, item.lod.clone())
        } else {
            (9, item.lod.clone())
        }
    });
    values
}

pub fn analyze_package_meshes(path: &Path) -> PackageMeshInfo {
    let package = match Package::load(path) {
        Ok(package) => package,
        Err(error) => {
            return PackageMeshInfo {
                warnings: vec![format!("Mesh analysis could not open package: {error}")],
                ..PackageMeshInfo::default()
            }
        }
    };

    let mut nmap_names = HashMap::<u64, String>::new();
    for entry in &package.entries {
        if entry.type_id != TYPE_NMAP || entry.mem_size > 32 * 1024 * 1024 {
            continue;
        }
        if let Ok(data) = package.data(entry) {
            for (hash, name) in parse_nmap_names(&data) {
                nmap_names.entry(hash).or_insert(name);
            }
        }
    }

    let mut resources = Vec::new();
    let mut package_warnings = Vec::new();
    for entry in &package.entries {
        if !matches!(entry.type_id, TYPE_GEOM | TYPE_MLOD | TYPE_MODL) {
            continue;
        }
        if entry.mem_size > MAX_MESH_RESOURCE_BYTES {
            package_warnings.push(format!(
                "{} skipped: decoded mesh resource exceeds 256 MiB.",
                entry.key_string()
            ));
            continue;
        }

        let data = match package.data(entry) {
            Ok(data) => data,
            Err(error) => {
                package_warnings.push(format!(
                    "{} could not be decoded for polycount: {error}",
                    entry.key_string()
                ));
                continue;
            }
        };

        let internal_name = nmap_names.get(&entry.instance).cloned();
        let type_label = if entry.type_id == TYPE_GEOM {
            "GEOM"
        } else if entry.type_id == TYPE_MLOD {
            "MLOD"
        } else {
            "MODL"
        }
        .to_string();

        if entry.type_id == TYPE_GEOM {
            match parse_geom(&data) {
                Ok((vertices, triangles)) => {
                    let lod = internal_name
                        .as_deref()
                        .and_then(lod_from_name)
                        .unwrap_or_else(|| "GEOM / Unknown LOD".into());
                    resources.push(MeshResourceInfo {
                        type_label,
                        group_hex: format!("0x{:08X}", entry.group),
                        instance_hex: format!("0x{:016X}", entry.instance),
                        tgi: entry.key_string(),
                        internal_name,
                        lod,
                        shadow: false,
                        vertices,
                        triangles: Some(triangles),
                        groups: vec![MeshGroupInfo {
                            index: 0,
                            name_hash: None,
                            vertices,
                            primitives: triangles,
                            triangles: Some(triangles),
                            primitive_type: "TriangleList".into(),
                            shadow: false,
                        }],
                        warnings: Vec::new(),
                    });
                }
                Err(error) => package_warnings.push(format!(
                    "{} GEOM polycount unavailable: {error}",
                    entry.key_string()
                )),
            }
            continue;
        }

        match parse_mlod(&data) {
            Ok(groups) => {
                let (lod, resource_shadow) = object_lod(entry.type_id, entry.group);
                let vertices = groups
                    .iter()
                    .filter(|group| !group.shadow)
                    .map(|group| group.vertices)
                    .sum::<u64>();
                let triangles = groups
                    .iter()
                    .filter(|group| !group.shadow)
                    .try_fold(0u64, |sum, group| group.triangles.and_then(|v| sum.checked_add(v)));
                let mut warnings = Vec::new();
                if groups.iter().any(|group| group.triangles.is_none()) {
                    warnings.push(
                        "At least one mesh group uses a primitive type whose triangle count is not safely derived."
                            .into(),
                    );
                }
                resources.push(MeshResourceInfo {
                    type_label,
                    group_hex: format!("0x{:08X}", entry.group),
                    instance_hex: format!("0x{:016X}", entry.instance),
                    tgi: entry.key_string(),
                    internal_name,
                    lod,
                    shadow: resource_shadow,
                    vertices,
                    triangles,
                    groups,
                    warnings,
                });
            }
            Err(error) => package_warnings.push(format!(
                "{} object mesh polycount unavailable: {error}",
                entry.key_string()
            )),
        }
    }

    let lods = aggregate_lods(&resources);
    let (highest_visible_triangles, highest_visible_vertices) = lods
        .iter()
        .max_by_key(|lod| lod.triangles)
        .map(|lod| (Some(lod.triangles), Some(lod.vertices)))
        .unwrap_or((None, None));

    // If every CAS GEOM lacks an internal LOD name, summing them could combine
    // several LODs and exaggerate polycount. Keep the per-resource counts but
    // deliberately avoid inventing a package-level maximum.
    let only_unknown_geom = !resources.is_empty()
        && resources.iter().all(|resource| {
            resource.type_label == "GEOM" && resource.lod == "GEOM / Unknown LOD"
        });
    let (highest_visible_triangles, highest_visible_vertices) = if only_unknown_geom {
        package_warnings.push(
            "GEOM resources have no safe internal LOD identity; per-resource counts are shown without a combined package polycount."
                .into(),
        );
        (None, None)
    } else {
        (highest_visible_triangles, highest_visible_vertices)
    };

    PackageMeshInfo {
        has_meshes: !resources.is_empty(),
        highest_visible_triangles,
        highest_visible_vertices,
        lods,
        resources,
        warnings: package_warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32le(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn geom_triangle_count_comes_from_face_points() {
        let mut data = Vec::new();
        data.extend_from_slice(b"GEOM");
        u32le(&mut data, 5);
        u32le(&mut data, 0);
        u32le(&mut data, 0);
        u32le(&mut data, 0); // no embedded material
        u32le(&mut data, 0); // merge
        u32le(&mut data, 0); // sort
        u32le(&mut data, 4); // vertices
        u32le(&mut data, 1); // one vertex element
        u32le(&mut data, 1); // position type (synthetic)
        u32le(&mut data, 1); // subtype
        data.push(12);       // bytes per element
        data.extend_from_slice(&vec![0u8; 4 * 12]);
        u32le(&mut data, 1); // item count
        data.push(2);        // WORD indices
        u32le(&mut data, 6); // two triangles
        data.extend_from_slice(&[0u8; 12]);
        assert_eq!(parse_geom(&data).unwrap(), (4, 2));
    }

    #[test]
    fn mlod_reads_vertices_primitives_and_triangle_list() {
        let mut body = vec![0u8; 88];
        body[0..4].copy_from_slice(&0x1234u32.to_le_bytes());
        body[20..24].copy_from_slice(&3u32.to_le_bytes()); // TriangleList
        body[40..44].copy_from_slice(&120u32.to_le_bytes());
        body[44..48].copy_from_slice(&200u32.to_le_bytes());
        let mut data = Vec::new();
        data.extend_from_slice(b"MLOD");
        u32le(&mut data, 0x201);
        u32le(&mut data, 1);
        u32le(&mut data, body.len() as u32);
        data.extend_from_slice(&body);
        let groups = parse_mlod(&data).unwrap();
        assert_eq!(groups[0].vertices, 120);
        assert_eq!(groups[0].triangles, Some(200));
    }

    #[test]
    fn quad_primitives_convert_to_two_triangles_each() {
        assert_eq!(primitive_triangles(6, 50), Some(100));
        assert_eq!(primitive_triangles(7, 50), None);
    }

    #[test]
    fn internal_names_can_identify_cas_lods() {
        assert_eq!(lod_from_name("afHairFancy_lod0"), Some("LOD 0 / High".into()));
        assert_eq!(lod_from_name("mesh-lod_2-group"), Some("LOD 2 / Low".into()));
        assert_eq!(lod_from_name("unnamed"), None);
    }

    #[test]
    fn shadow_caster_flag_does_not_hide_visible_geometry() {
        let mut body = vec![0u8; 88];
        body[20..24].copy_from_slice(&(3u32 | (0x10u32 << 8)).to_le_bytes());
        body[40..44].copy_from_slice(&50u32.to_le_bytes());
        body[44..48].copy_from_slice(&75u32.to_le_bytes());
        let mut data = Vec::new();
        data.extend_from_slice(b"MLOD");
        u32le(&mut data, 0x201);
        u32le(&mut data, 1);
        u32le(&mut data, body.len() as u32);
        data.extend_from_slice(&body);
        let groups = parse_mlod(&data).unwrap();
        assert!(!groups[0].shadow);
        assert_eq!(groups[0].triangles, Some(75));
    }

    #[test]
    fn object_resource_groups_map_main_and_shadow_lods() {
        assert_eq!(object_lod(TYPE_MLOD, 0), ("LOD 0 / High".into(), false));
        assert_eq!(object_lod(TYPE_MODL, 1), ("LOD 1 / Low".into(), false));
        assert_eq!(object_lod(TYPE_MLOD, 0x0001_0000).1, true);
    }
}
