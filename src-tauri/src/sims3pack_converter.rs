use crate::{catalog::TYPE_CASP, dbpf::Package, i18n::AppLanguage};
use byteorder::{LittleEndian, WriteBytesExt};
use quick_xml::{events::Event, Reader};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const TYPE_MANIFEST: u32 = 0x73E9_3EEB;
const TYPE_KEY: u32 = 0x0166_038C;
const TYPE_MERGE_SKIP: u32 = 0x7672_F0C5;
const TYPE_OBJD: u32 = 0x319E_4F1D;
const TYPE_OBJK: u32 = 0x02DC_343F;
// Store Featured Set membership list (not a normal lamp or lighting resource).
const TYPE_FEATURED_SET_MEMBERS: u32 = 0x082D_0369;
const CACHE_THUMB_TYPES: &[u32] = &[
    0x0580_A2B4, 0x0580_A2B5, 0x0580_A2B6,
    0x0589_DC44, 0x0589_DC45, 0x0589_DC46,
    0x05B1_7698, 0x05B1_7699, 0x05B1_769A,
    0x05B1_B524, 0x05B1_B525, 0x05B1_B526,
    0x2653_E3C8, 0x2653_E3C9, 0x2653_E3CA,
    0x2D42_84F0, 0x2D42_84F1, 0x2D42_84F2,
    0x2E75_C764, 0x2E75_C765, 0x2E75_C766, 0x2E75_C767,
    0x5DE9_DBA0, 0x5DE9_DBA1, 0x5DE9_DBA2,
    0x626F_60CC, 0x626F_60CD, 0x626F_60CE,
    0xFCEA_B65B,
];
const MAX_XML_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
struct ManifestNames {
    display_name: Option<String>,
    package_title: Option<String>,
    package_id: Option<String>,
    localized_names: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct PackagedFile {
    name: String,
    guid: String,
    length: u32,
    offset: u64,
    content_type: String,
}

#[derive(Debug, Clone)]
struct Sims3Pack {
    xml_offset: u64,
    manifest: ManifestNames,
    packaged_files: Vec<PackagedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sims3PackConversionItem {
    pub index: usize,
    pub packaged_name: String,
    pub proposed_file_name: String,
    pub display_name: String,
    pub name_source: String,
    pub content_type: String,
    pub store_set: bool,
    pub recommended_folder: String,
    pub package_title: Option<String>,
    pub package_id: Option<String>,
    pub size: u64,
    pub convertible: bool,
    pub warning: Option<String>,
    pub casp_name: Option<String>,
    pub manifest_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sims3PackInspection {
    pub path: String,
    pub file_name: String,
    pub display_name: String,
    pub package_count: usize,
    pub set: bool,
    pub items: Vec<Sims3PackConversionItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sims3PackConvertedItem {
    pub source_path: String,
    pub packaged_name: String,
    pub output_path: String,
    pub output_file_name: String,
    pub name_source: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Sims3PackConversionResult {
    pub converted: usize,
    pub skipped: usize,
    pub items: Vec<Sims3PackConvertedItem>,
    pub errors: Vec<String>,
}

fn locale_preferences(language: AppLanguage) -> &'static [&'static str] {
    match language {
        AppLanguage::En => &["en-US", "en-GB", "en"],
        AppLanguage::Pt => &["pt-BR", "pt-PT", "pt"],
        AppLanguage::Es => &["es-ES", "es-MX", "es"],
    }
}

fn localized_name(manifest: &ManifestNames, language: AppLanguage) -> Option<String> {
    for wanted in locale_preferences(language) {
        if let Some((_, value)) = manifest
            .localized_names
            .iter()
            .find(|(locale, _)| locale.eq_ignore_ascii_case(wanted))
        {
            if !value.trim().is_empty() {
                return Some(clean_manifest_text(value));
            }
        }
    }

    let prefix = match language {
        AppLanguage::En => "en",
        AppLanguage::Pt => "pt",
        AppLanguage::Es => "es",
    };
    if let Some((_, value)) = manifest
        .localized_names
        .iter()
        .find(|(locale, _)| locale.to_ascii_lowercase().starts_with(prefix))
    {
        if !value.trim().is_empty() {
            return Some(clean_manifest_text(value));
        }
    }

    None
}

fn best_manifest_name(manifest: &ManifestNames, language: AppLanguage) -> Option<String> {
    localized_name(manifest, language)
        .or_else(|| manifest.package_title.as_ref().filter(|v| !v.trim().is_empty()).cloned())
        .or_else(|| manifest.display_name.as_ref().filter(|v| !v.trim().is_empty()).cloned())
}

fn clean_manifest_text(value: &str) -> String {
    let mut text = value.trim().to_string();

    for prefix in ["<![CDATA[", "![CDATA[", "[CDATA["] {
        if text.starts_with(prefix) {
            text = text[prefix.len()..].to_string();
            break;
        }
    }
    for suffix in ["]]>", "]]"] {
        if text.ends_with(suffix) {
            let new_len = text.len().saturating_sub(suffix.len());
            text.truncate(new_len);
            break;
        }
    }

    text.trim().to_string()
}

fn sanitize_file_stem(value: &str) -> String {
    let cleaned = clean_manifest_text(value);
    let mut out = String::with_capacity(cleaned.len());
    for ch in cleaned.chars() {
        match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => out.push('_'),
            c if c.is_control() => out.push('_'),
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "Converted Package".to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_guid_like_name(value: &str) -> bool {
    let stem = Path::new(value)
        .file_stem()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| value.to_string());
    let compact = stem.trim().trim_start_matches("0x").replace('-', "");
    compact.len() >= 16 && compact.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn meaningful_packaged_name(value: &str) -> Option<String> {
    let stem = Path::new(value).file_stem()?.to_string_lossy().trim().to_string();
    if stem.is_empty() || is_guid_like_name(&stem) {
        None
    } else {
        Some(stem)
    }
}

fn attr_language(
    event: &quick_xml::events::BytesStart<'_>,
    reader: &Reader<&[u8]>,
) -> Option<String> {
    for attr in event.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_ascii_lowercase();
        if key == "language" {
            return attr
                .decode_and_unescape_value(reader)
                .ok()
                .map(|value| value.to_string());
        }
    }
    None
}

fn parse_manifest_and_files(xml: &str) -> Result<(ManifestNames, Vec<PackagedFile>), String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut manifest = ManifestNames::default();
    let mut files = Vec::new();
    let mut in_localized_names = false;
    let mut in_packaged_file = false;
    let mut cur_name = String::new();
    let mut cur_guid = String::new();
    let mut cur_length = 0u32;
    let mut cur_offset = 0u64;
    let mut cur_type = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_ascii_lowercase();
                match tag.as_str() {
                    "localizednames" => in_localized_names = true,
                    "localizedname" if in_localized_names => {
                        let locale = attr_language(&event, &reader).unwrap_or_default();
                        let value = reader.read_text(event.name()).unwrap_or_default().into_owned();
                        let value = clean_manifest_text(&value);
                        if !locale.trim().is_empty() && !value.is_empty() {
                            manifest.localized_names.insert(locale, value);
                        }
                    }
                    "displayname" if !in_packaged_file => {
                        let value = reader.read_text(event.name()).unwrap_or_default().into_owned();
                        let value = clean_manifest_text(&value);
                        if !value.is_empty() {
                            manifest.display_name = Some(value);
                        }
                    }
                    "packagetitle" if !in_packaged_file => {
                        let value = reader.read_text(event.name()).unwrap_or_default().into_owned();
                        let value = clean_manifest_text(&value);
                        if !value.is_empty() {
                            manifest.package_title = Some(value);
                        }
                    }
                    "packageid" if !in_packaged_file => {
                        let value = reader.read_text(event.name()).unwrap_or_default().into_owned();
                        if !value.trim().is_empty() {
                            manifest.package_id = Some(value.trim().to_string());
                        }
                    }
                    "packagedfile" => {
                        in_packaged_file = true;
                        cur_name.clear();
                        cur_guid.clear();
                        cur_length = 0;
                        cur_offset = 0;
                        cur_type.clear();
                    }
                    "name" if in_packaged_file => {
                        cur_name = reader.read_text(event.name()).unwrap_or_default().into_owned();
                    }
                    "guid" if in_packaged_file => {
                        cur_guid = reader.read_text(event.name()).unwrap_or_default().into_owned();
                    }
                    "length" if in_packaged_file => {
                        cur_length = reader
                            .read_text(event.name())
                            .unwrap_or_default()
                            .trim()
                            .parse::<u32>()
                            .unwrap_or(0);
                    }
                    "offset" if in_packaged_file => {
                        cur_offset = reader
                            .read_text(event.name())
                            .unwrap_or_default()
                            .trim()
                            .parse::<u64>()
                            .unwrap_or(0);
                    }
                    "contenttype" if in_packaged_file => {
                        cur_type = reader
                            .read_text(event.name())
                            .unwrap_or_default()
                            .to_ascii_lowercase();
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_ascii_lowercase();
                match tag.as_str() {
                    "localizednames" => in_localized_names = false,
                    "packagedfile" if in_packaged_file => {
                        in_packaged_file = false;
                        files.push(PackagedFile {
                            name: cur_name.trim().to_string(),
                            guid: cur_guid.trim().to_string(),
                            length: cur_length,
                            offset: cur_offset,
                            content_type: cur_type.trim().to_string(),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Sims3Pack XML parse error: {error}")),
            _ => {}
        }
        buf.clear();
    }

    Ok((manifest, files))
}

fn read_sims3pack(path: &Path) -> Result<Sims3Pack, String> {
    let mut file = File::open(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let mut header = [0u8; 17];
    file.read_exact(&mut header)
        .map_err(|e| format!("Could not read Sims3Pack header: {e}"))?;

    if &header[4..11] != b"TS3Pack" {
        return Err("File is not a valid Sims3Pack.".to_string());
    }

    let xml_length = u32::from_le_bytes([header[13], header[14], header[15], header[16]]) as usize;
    if xml_length == 0 || xml_length > MAX_XML_BYTES {
        return Err(format!("Invalid Sims3Pack XML length: {xml_length}."));
    }

    let mut xml_bytes = vec![0u8; xml_length];
    file.read_exact(&mut xml_bytes)
        .map_err(|e| format!("Could not read Sims3Pack XML: {e}"))?;
    let xml = String::from_utf8(xml_bytes)
        .map_err(|e| format!("Sims3Pack XML is not valid UTF-8: {e}"))?;
    let (manifest, packaged_files) = parse_manifest_and_files(&xml)?;

    Ok(Sims3Pack {
        xml_offset: 17 + xml_length as u64,
        manifest,
        packaged_files,
    })
}

fn read_packaged_payload(path: &Path, pack: &Sims3Pack, item: &PackagedFile) -> Result<Vec<u8>, String> {
    let length = item.length as usize;
    if length == 0 || length > MAX_PAYLOAD_BYTES {
        return Err(format!("Invalid embedded payload size: {length}."));
    }
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let start = pack
        .xml_offset
        .checked_add(item.offset)
        .ok_or_else(|| "Embedded payload offset overflow.".to_string())?;
    let end = start
        .checked_add(item.length as u64)
        .ok_or_else(|| "Embedded payload range overflow.".to_string())?;
    if end > meta.len() {
        return Err("Embedded payload points outside the Sims3Pack.".to_string());
    }

    let mut file = File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0u8; length];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

fn read_i32(data: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

fn read_7bit_encoded_int(data: &[u8], offset: &mut usize) -> Option<usize> {
    let mut result = 0usize;
    let mut shift = 0usize;
    loop {
        if shift >= usize::BITS as usize {
            return None;
        }
        let value = *data.get(*offset)?;
        *offset += 1;
        result |= ((value & 0x7F) as usize) << shift;
        if value & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
    }
}

fn read_be_7bit_string(data: &[u8], offset: &mut usize) -> Option<String> {
    let byte_len = read_7bit_encoded_int(data, offset)?;
    if byte_len == 0 {
        return Some(String::new());
    }
    let end = offset.checked_add(byte_len)?;
    let raw = data.get(*offset..end)?;
    if raw.len() % 2 != 0 {
        return None;
    }
    let words = raw
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    *offset = end;
    Some(String::from_utf16_lossy(&words))
}

fn casp_internal_name(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    let mut offset = 8usize;
    let preset_count = read_u32(data, offset)? as usize;
    offset += 4;
    if preset_count > 4096 {
        return None;
    }

    for _ in 0..preset_count {
        let xml_len = read_i32(data, offset)?;
        if xml_len < 0 {
            return None;
        }
        offset += 4;
        let xml_bytes = (xml_len as usize).checked_mul(2)?;
        offset = offset.checked_add(xml_bytes)?;
        offset = offset.checked_add(4)?;
        if offset > data.len() {
            return None;
        }
    }

    let name = read_be_7bit_string(data, &mut offset)?;
    let trimmed = name.trim_matches('\0').trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn key_names(package: &Package) -> BTreeMap<u64, String> {
    let mut names = BTreeMap::new();
    for entry in package.entries.iter().filter(|entry| entry.type_id == TYPE_KEY) {
        let Ok(data) = package.data(entry) else { continue };
        if data.len() < 8 || read_u32(&data, 0) != Some(1) {
            continue;
        }
        let Some(count) = read_u32(&data, 4).map(|v| v as usize) else { continue };
        if count > 100_000 {
            continue;
        }
        let mut offset = 8usize;
        for _ in 0..count {
            if offset + 12 > data.len() {
                break;
            }
            let instance = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            let length = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().unwrap()) as usize;
            offset += 12;
            let Some(end) = offset.checked_add(length) else { break };
            if length > 4096 || end > data.len() {
                break;
            }
            let name = String::from_utf8_lossy(&data[offset..end])
                .trim_matches('\0')
                .trim()
                .to_string();
            if !name.is_empty() {
                names.entry(instance).or_insert(name);
            }
            offset = end;
        }
    }
    names
}

fn build_key_resource(names: &BTreeMap<u64, String>) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (instance, name) in names {
        let bytes = name.as_bytes();
        data.extend_from_slice(&instance.to_le_bytes());
        data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(bytes);
    }
    data
}

fn package_casp_name(package: &Package) -> Option<String> {
    let key_map = key_names(package);
    for entry in package.entries.iter().filter(|entry| entry.type_id == TYPE_CASP) {
        if let Ok(data) = package.data(entry) {
            if let Some(name) = casp_internal_name(&data) {
                return Some(name);
            }
        }
        if let Some(name) = key_map.get(&entry.instance) {
            if !name.trim().is_empty() {
                return Some(name.trim().to_string());
            }
        }
    }
    None
}

fn package_manifest_names(package: &Package) -> ManifestNames {
    for entry in package.entries.iter().filter(|entry| entry.type_id == TYPE_MANIFEST) {
        let Ok(data) = package.data(entry) else { continue };
        let Ok(text) = std::str::from_utf8(&data) else { continue };
        if let Ok((manifest, _)) = parse_manifest_and_files(text) {
            if manifest.package_title.is_some()
                || manifest.display_name.is_some()
                || !manifest.localized_names.is_empty()
            {
                return manifest;
            }
        }
    }
    ManifestNames::default()
}

fn temporary_package_path(index: usize) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "veigas_s3cc_manager_convert_{}_{}_{}.package",
        std::process::id(),
        nonce,
        index
    )))
}

fn inspect_payload(
    payload: &[u8],
    index: usize,
    language: AppLanguage,
) -> Result<(ManifestNames, Option<String>, bool), String> {
    if payload.len() < 96 || &payload[..4] != b"DBPF" {
        return Err("Embedded file is not a supported Sims 3 DBPF package.".to_string());
    }
    let temp = temporary_package_path(index)?;
    let result = (|| {
        fs::write(&temp, payload).map_err(|e| format!("Could not stage embedded package: {e}"))?;
        let package = Package::load(&temp).map_err(|e| format!("Invalid embedded package: {e}"))?;
        let manifest = package_manifest_names(&package);
        let casp_name = package_casp_name(&package);
        let store_set = package_is_featured_set(&package);
        Ok((manifest, casp_name, store_set))
    })();
    let _ = fs::remove_file(&temp);
    result
}


// The membership resource alone can also occur in override mods. Requiring the
// catalog object definitions lets us recognize actual Store Featured Set entries.
fn has_featured_set_signature(types: impl IntoIterator<Item = u32>) -> bool {
    let mut members = false;
    let mut object_definition = false;
    let mut object_key = false;
    for kind in types {
        match kind {
            TYPE_FEATURED_SET_MEMBERS => members = true,
            TYPE_OBJD => object_definition = true,
            TYPE_OBJK => object_key = true,
            _ => {}
        }
    }
    members && object_definition && object_key
}

fn package_is_featured_set(package: &Package) -> bool {
    has_featured_set_signature(package.entries.iter().map(|entry| entry.type_id))
}

fn is_safe_conversion_item(item: &Sims3PackConversionItem) -> bool {
    !item.store_set && is_merge_safe(&item.content_type)
}

fn content_extension(content_type: &str) -> &'static str {
    match content_type.trim().to_ascii_lowercase().as_str() {
        "world" => "world",
        "sim" => "sim",
        _ => "package",
    }
}

fn recommended_folder(content_type: &str) -> &'static str {
    match content_type.trim().to_ascii_lowercase().as_str() {
        "world" => "InstalledWorlds",
        "lot" | "household" | "residentiallot" | "communitylot" => "Library",
        "sim" => "SavedSims",
        "preset" => "Review",
        _ => "Mods/Packages",
    }
}

fn is_merge_safe(content_type: &str) -> bool {
    matches!(
        content_type.trim().to_ascii_lowercase().as_str(),
        "object" | "cas" | "caspart" | "clothing" | "hair" |
        "accessory" | "pattern" | "build" | "buy"
    )
}

// Use the embedded package title to distinguish collisions within the same destination.
fn disambiguate_output_names(items: &mut [Sims3PackConversionItem]) {
    let mut counts = BTreeMap::<(String, String), usize>::new();
    for item in items.iter().filter(|item| item.convertible) {
        let key = (item.recommended_folder.to_ascii_lowercase(), item.proposed_file_name.to_ascii_lowercase());
        *counts.entry(key).or_default() += 1;
    }
    let mut used = HashSet::<(String, String)>::new();
    for item in items.iter_mut().filter(|item| item.convertible) {
        let key = (item.recommended_folder.to_ascii_lowercase(), item.proposed_file_name.to_ascii_lowercase());
        if counts.get(&key).copied().unwrap_or(0) > 1 || used.contains(&key) {
            let original_stem = Path::new(&item.proposed_file_name).file_stem()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Converted Package".to_string());
            let disambiguator = item.package_title.as_deref()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| item.package_id.as_deref().filter(|v| !v.trim().is_empty()))
                .unwrap_or(&item.packaged_name);
            let suffix = sanitize_file_stem(disambiguator);
            let new_stem = if suffix.eq_ignore_ascii_case(&original_stem) {
                format!("{original_stem} - {}", item.index + 1)
            } else {
                format!("{original_stem} - {suffix}")
            };
            item.proposed_file_name = format!("{new_stem}.{}", content_extension(&item.content_type));
            item.name_source = "manifest_disambiguated".to_string();
        }
        let folder = item.recommended_folder.to_ascii_lowercase();
        let mut final_key = (folder.clone(), item.proposed_file_name.to_ascii_lowercase());
        if used.contains(&final_key) {
            let stem = Path::new(&item.proposed_file_name).file_stem()
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Converted Package".to_string());
            let mut counter = item.index + 1;
            loop {
                let candidate = format!("{stem} - {counter}.{}", content_extension(&item.content_type));
                final_key = (folder.clone(), candidate.to_ascii_lowercase());
                if !used.contains(&final_key) {
                    item.proposed_file_name = candidate;
                    break;
                }
                counter += 1;
            }
        }
        used.insert(final_key);
    }
}

fn proposed_name(
    pack: &Sims3Pack,
    packaged: &PackagedFile,
    valid_package_count: usize,
    manifest_name: Option<&str>,
    casp_name: Option<&str>,
    index: usize,
    language: AppLanguage,
) -> (String, String, String) {
    let extension = content_extension(&packaged.content_type);
    if let Some(value) = manifest_name.filter(|v| !v.trim().is_empty()) {
        let stem = sanitize_file_stem(value);
        return (format!("{stem}.{extension}"), stem, "manifest".to_string());
    }

    if valid_package_count == 1 {
        if let Some(value) = best_manifest_name(&pack.manifest, language) {
            let stem = sanitize_file_stem(&value);
            return (format!("{stem}.{extension}"), stem, "sims3pack_manifest".to_string());
        }
    }

    if let Some(value) = casp_name.filter(|v| !v.trim().is_empty()) {
        let stem = sanitize_file_stem(value);
        return (format!("{stem}.{extension}"), stem, "casp".to_string());
    }

    if let Some(value) = meaningful_packaged_name(&packaged.name) {
        let stem = sanitize_file_stem(&value);
        return (format!("{stem}.{extension}"), stem, "packaged_file".to_string());
    }

    let fallback = format!("Converted Package {}", index + 1);
    (format!("{fallback}.{extension}"), fallback, "fallback".to_string())
}

fn inspect_one(path: &Path, language: AppLanguage) -> Result<Sims3PackInspection, String> {
    let pack = read_sims3pack(path)?;
    let mut staged = Vec::new();
    let mut valid_count = 0usize;

    for (index, packaged) in pack.packaged_files.iter().enumerate() {
        if packaged.length == 0 {
            continue;
        }
        match read_packaged_payload(path, &pack, packaged) {
            Ok(payload) if payload.len() >= 4 && &payload[..4] == b"DBPF" => {
                valid_count += 1;
                staged.push((index, packaged, Some(payload), None));
            }
            Ok(_) => staged.push((
                index,
                packaged,
                None,
                Some("Embedded file is not a DBPF package and will not be converted.".to_string()),
            )),
            Err(error) => staged.push((index, packaged, None, Some(error))),
        }
    }

    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for (index, packaged, payload, prewarning) in staged {
        if let Some(payload) = payload {
            match inspect_payload(&payload, index, language) {
                Ok((manifest, casp_name, featured_set_signature)) => {
                    let store_set = featured_set_signature && packaged.content_type.eq_ignore_ascii_case("object");
                    let manifest_name = best_manifest_name(&manifest, language);
                    let package_title = manifest.package_title.clone();
                    let package_id = manifest.package_id.clone();
                    let (file_name, display_name, name_source) = proposed_name(
                        &pack,
                        packaged,
                        valid_count,
                        manifest_name.as_deref(),
                        casp_name.as_deref(),
                        index,
                        language,
                    );
                    items.push(Sims3PackConversionItem {
                        index,
                        packaged_name: packaged.name.clone(),
                        proposed_file_name: file_name,
                        display_name,
                        name_source,
                        content_type: packaged.content_type.clone(),
                        store_set,
                        recommended_folder: recommended_folder(&packaged.content_type).to_string(),
                        package_title,
                        package_id,
                        size: packaged.length as u64,
                        convertible: true,
                        warning: prewarning,
                        casp_name,
                        manifest_name,
                    });
                }
                Err(error) => {
                    warnings.push(format!("{}: {error}", packaged.name));
                    items.push(Sims3PackConversionItem {
                        index,
                        packaged_name: packaged.name.clone(),
                        proposed_file_name: String::new(),
                        display_name: packaged.name.clone(),
                        name_source: "invalid".to_string(),
                        content_type: packaged.content_type.clone(),
                        store_set: false,
                        recommended_folder: recommended_folder(&packaged.content_type).to_string(),
                        package_title: None,
                        package_id: None,
                        size: packaged.length as u64,
                        convertible: false,
                        warning: Some(error),
                        casp_name: None,
                        manifest_name: None,
                    });
                }
            }
        } else {
            let warning = prewarning.unwrap_or_else(|| "Not convertible.".to_string());
            warnings.push(format!("{}: {warning}", packaged.name));
            items.push(Sims3PackConversionItem {
                index,
                packaged_name: packaged.name.clone(),
                proposed_file_name: String::new(),
                display_name: packaged.name.clone(),
                name_source: "non_package".to_string(),
                content_type: packaged.content_type.clone(),
                store_set: false,
                recommended_folder: recommended_folder(&packaged.content_type).to_string(),
                package_title: None,
                package_id: None,
                size: packaged.length as u64,
                convertible: false,
                warning: Some(warning),
                casp_name: None,
                manifest_name: None,
            });
        }
    }

    disambiguate_output_names(&mut items);

    let display_name = best_manifest_name(&pack.manifest, language)
        .or_else(|| path.file_stem().map(|v| v.to_string_lossy().to_string()))
        .unwrap_or_else(|| "Sims3Pack".to_string());

    Ok(Sims3PackInspection {
        path: path.to_string_lossy().to_string(),
        file_name: path.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default(),
        display_name,
        package_count: valid_count,
        set: valid_count > 1,
        items,
        warnings,
    })
}

fn unique_output_path(folder: &Path, file_name: &str) -> PathBuf {
    let base = Path::new(file_name);
    let stem = base
        .file_stem()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "Converted Package".to_string());
    let ext = base.extension().and_then(|v| v.to_str()).unwrap_or("package");
    let mut attempt = 0usize;
    loop {
        let name = if attempt == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({attempt}).{ext}")
        };
        let candidate = folder.join(name);
        if !candidate.exists() {
            return candidate;
        }
        attempt += 1;
    }
}

fn write_no_replace(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;

    if let Err(error) = file.write_all(data) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("Could not write {}: {error}", path.display()));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("Could not finalize {}: {error}", path.display()));
    }
    Ok(())
}


#[derive(Debug, Clone)]
struct OwnedRawResource {
    type_id: u32,
    group: u32,
    instance: u64,
    raw: Vec<u8>,
    mem_size: u32,
    compressed: u16,
    unknown2: u16,
}

#[derive(Debug, Clone)]
struct MergedResource {
    type_id: u32,
    group: u32,
    instance: u64,
    chunk_offset: u32,
    file_size: u32,
    mem_size: u32,
    compressed: u16,
    unknown2: u16,
}

fn write_empty_dbpf_header(file: &mut File) -> Result<(), String> {
    file.write_all(b"DBPF").map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(2).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(0).map_err(|e| e.to_string())?;
    file.write_all(&[0u8; 24]).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(0).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(0).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(0).map_err(|e| e.to_string())?;
    file.write_all(&[0u8; 12]).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(3).map_err(|e| e.to_string())?;
    file.write_u32::<LittleEndian>(96).map_err(|e| e.to_string())?;
    file.write_all(&[0u8; 28]).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_owned_package_no_replace(
    target: &Path,
    raw_resources: &[OwnedRawResource],
    key_names: &BTreeMap<u64, String>,
) -> Result<usize, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| format!("Could not create {}: {e}", target.display()))?;

    let result = (|| -> Result<usize, String> {
        write_empty_dbpf_header(&mut file)?;
        file.seek(SeekFrom::Start(96)).map_err(|e| e.to_string())?;

        let mut resources = Vec::<MergedResource>::new();
        for resource in raw_resources {
            let chunk_offset = file.stream_position().map_err(|e| e.to_string())? as u32;
            file.write_all(&resource.raw).map_err(|e| e.to_string())?;
            resources.push(MergedResource {
                type_id: resource.type_id,
                group: resource.group,
                instance: resource.instance,
                chunk_offset,
                file_size: resource.raw.len() as u32,
                mem_size: resource.mem_size,
                compressed: resource.compressed,
                unknown2: resource.unknown2,
            });
        }

        if !key_names.is_empty() {
            let raw = build_key_resource(key_names);
            let chunk_offset = file.stream_position().map_err(|e| e.to_string())? as u32;
            let size = raw.len() as u32;
            file.write_all(&raw).map_err(|e| e.to_string())?;
            resources.push(MergedResource {
                type_id: TYPE_KEY,
                group: 0,
                instance: 0,
                chunk_offset,
                file_size: size,
                mem_size: size,
                compressed: 0,
                unknown2: 0,
            });
        }

        let index_position = file.stream_position().map_err(|e| e.to_string())? as u32;
        file.write_u32::<LittleEndian>(0).map_err(|e| e.to_string())?;
        for entry in &resources {
            file.write_u32::<LittleEndian>(entry.type_id).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>(entry.group).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>((entry.instance >> 32) as u32).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>(entry.instance as u32).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>(entry.chunk_offset).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>(entry.file_size | 0x8000_0000).map_err(|e| e.to_string())?;
            file.write_u32::<LittleEndian>(entry.mem_size).map_err(|e| e.to_string())?;
            file.write_u16::<LittleEndian>(entry.compressed).map_err(|e| e.to_string())?;
            file.write_u16::<LittleEndian>(entry.unknown2).map_err(|e| e.to_string())?;
        }

        let index_count = resources.len() as u32;
        let index_length = 4u32
            .checked_add(index_count.checked_mul(32).ok_or_else(|| "DBPF index is too large.".to_string())?)
            .ok_or_else(|| "DBPF index is too large.".to_string())?;

        file.seek(SeekFrom::Start(36)).map_err(|e| e.to_string())?;
        file.write_u32::<LittleEndian>(index_count).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(44)).map_err(|e| e.to_string())?;
        file.write_u32::<LittleEndian>(index_length).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(64)).map_err(|e| e.to_string())?;
        file.write_u32::<LittleEndian>(index_position).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(resources.len())
    })();

    if result.is_err() {
        drop(file);
        let _ = fs::remove_file(target);
    }
    result
}

fn cache_thumbnail_paths() -> Vec<PathBuf> {
    let Some(documents) = dirs::document_dir() else {
        return Vec::new();
    };
    let folder = documents
        .join("Electronic Arts")
        .join("The Sims 3")
        .join("Thumbnails");
    [
        folder.join("ObjectThumbnails.package"),
        folder.join("CASThumbnails.package"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect()
}

fn restore_cached_thumbnails(target: &Path) -> Result<usize, String> {
    let base = Package::load(target)
        .map_err(|e| format!("Could not reopen converted package {}: {e}", target.display()))?;

    let catalog_instances = base
        .entries
        .iter()
        .filter(|entry| matches!(entry.type_id, TYPE_CASP | TYPE_OBJD | TYPE_OBJK))
        .map(|entry| entry.instance)
        .collect::<HashSet<_>>();
    if catalog_instances.is_empty() {
        return Ok(0);
    }

    let mut existing = base
        .entries
        .iter()
        .map(|entry| (entry.type_id, entry.group, entry.instance))
        .collect::<HashSet<_>>();
    let mut additions = Vec::<OwnedRawResource>::new();
    let mut names = key_names(&base);

    for cache_path in cache_thumbnail_paths() {
        let cache = match Package::load(&cache_path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let cache_names = key_names(&cache);

        for entry in &cache.entries {
            if !CACHE_THUMB_TYPES.contains(&entry.type_id)
                || !catalog_instances.contains(&entry.instance)
            {
                continue;
            }
            let key = (entry.type_id, entry.group, entry.instance);
            if !existing.insert(key) {
                continue;
            }
            let raw = match cache.raw_data(entry) {
                Ok(value) => value,
                Err(_) => continue,
            };
            additions.push(OwnedRawResource {
                type_id: entry.type_id,
                group: entry.group,
                instance: entry.instance,
                raw,
                mem_size: entry.mem_size,
                compressed: entry.compressed,
                unknown2: entry.unknown2,
            });
            if let Some(name) = cache_names.get(&entry.instance) {
                names.entry(entry.instance).or_insert_with(|| name.clone());
            }
        }
    }

    if additions.is_empty() {
        return Ok(0);
    }

    let mut resources = Vec::<OwnedRawResource>::new();
    for entry in &base.entries {
        if entry.type_id == TYPE_KEY {
            continue;
        }
        resources.push(OwnedRawResource {
            type_id: entry.type_id,
            group: entry.group,
            instance: entry.instance,
            raw: base
                .raw_data(entry)
                .map_err(|e| format!("Could not preserve resource {}: {e}", entry.key_string()))?,
            mem_size: entry.mem_size,
            compressed: entry.compressed,
            unknown2: entry.unknown2,
        });
    }
    resources.extend(additions.iter().cloned());

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let temp = parent.join(format!(
        ".veigas-thumbnail-{}-{}.package",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let backup = parent.join(format!(
        ".veigas-thumbnail-backup-{}-{}.package",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));

    write_owned_package_no_replace(&temp, &resources, &names)?;
    fs::rename(target, &backup)
        .map_err(|e| format!("Could not stage thumbnail-preserving replacement: {e}"))?;
    if let Err(error) = fs::rename(&temp, target) {
        let _ = fs::rename(&backup, target);
        let _ = fs::remove_file(&temp);
        return Err(format!("Could not install thumbnail-preserving package: {error}"));
    }
    let _ = fs::remove_file(&backup);
    Ok(additions.len())
}

fn merge_packages_no_replace(target: &Path, packages: &[Package]) -> Result<usize, String> {
    let mut seen = HashSet::<(u32, u32, u64)>::new();
    let mut resources = Vec::<OwnedRawResource>::new();
    let mut names = BTreeMap::<u64, String>::new();

    for package in packages {
        for (instance, name) in key_names(package) {
            names.entry(instance).or_insert(name);
        }

        for entry in &package.entries {
            if matches!(entry.type_id, TYPE_MANIFEST | TYPE_KEY | TYPE_MERGE_SKIP) {
                continue;
            }
            let key = (entry.type_id, entry.group, entry.instance);
            if !seen.insert(key) {
                continue;
            }

            let raw = package
                .raw_data(entry)
                .map_err(|e| format!("Could not read resource {}: {e}", entry.key_string()))?;
            resources.push(OwnedRawResource {
                type_id: entry.type_id,
                group: entry.group,
                instance: entry.instance,
                raw,
                mem_size: entry.mem_size,
                compressed: entry.compressed,
                unknown2: entry.unknown2,
            });
        }
    }

    write_owned_package_no_replace(target, &resources, &names)
}

fn combined_output_name(source: &Path, pack: &Sims3Pack, language: AppLanguage) -> (String, String) {
    if let Some(name) = best_manifest_name(&pack.manifest, language) {
        let stem = sanitize_file_stem(&name);
        return (format!("{stem}.package"), "sims3pack_manifest".to_string());
    }
    let stem = source
        .file_stem()
        .map(|v| sanitize_file_stem(&v.to_string_lossy()))
        .unwrap_or_else(|| "Converted Sims3Pack".to_string());
    (format!("{stem}.package"), "source_file".to_string())
}

fn merge_sims3pack_payloads(
    source: &Path,
    pack: &Sims3Pack,
    inspection: &Sims3PackInspection,
    target: &Path,
) -> Result<usize, String> {
    let convertible = inspection.items.iter().filter(|item| item.convertible).collect::<Vec<_>>();
    if convertible.len() == 1 {
        let item = convertible[0];
        let packaged = pack.packaged_files.get(item.index)
            .ok_or_else(|| format!("Missing packaged item {}.", item.index))?;
        let payload = read_packaged_payload(source, pack, packaged)?;
        if payload.len() < 4 || &payload[..4] != b"DBPF" {
            return Err("Embedded file is not a DBPF package.".to_string());
        }
        write_no_replace(target, &payload)?;
        return Ok(1);
    }

    let mut staged_paths = Vec::<PathBuf>::new();
    let mut packages = Vec::<Package>::new();

    let result = (|| -> Result<usize, String> {
        for item in convertible {
            let packaged = pack.packaged_files.get(item.index)
                .ok_or_else(|| format!("Missing packaged item {}.", item.index))?;
            let payload = read_packaged_payload(source, pack, packaged)?;
            if payload.len() < 4 || &payload[..4] != b"DBPF" {
                continue;
            }
            let temp = temporary_package_path(item.index)?;
            fs::write(&temp, &payload)
                .map_err(|e| format!("Could not stage embedded package: {e}"))?;
            staged_paths.push(temp.clone());
            let package = Package::load(&temp)
                .map_err(|e| format!("Invalid embedded package {}: {e}", packaged.name))?;
            packages.push(package);
        }

        if packages.is_empty() {
            return Err("No convertible .package payloads were found.".to_string());
        }
        merge_packages_no_replace(target, &packages)
    })();

    for path in staged_paths {
        let _ = fs::remove_file(path);
    }
    result
}

#[tauri::command]
pub fn inspect_sims3packs(paths: Vec<String>, language: AppLanguage) -> Result<Vec<Sims3PackInspection>, String> {
    let mut results = Vec::new();
    for raw in paths {
        let path = PathBuf::from(raw.trim());
        if !path.is_file() {
            return Err(format!("Sims3Pack not found: {}", path.display()));
        }
        results.push(inspect_one(&path, language)?);
    }
    Ok(results)
}

#[tauri::command]
pub fn convert_sims3packs(
    paths: Vec<String>,
    destination_folder: String,
    language: AppLanguage,
    combined: bool,
) -> Result<Sims3PackConversionResult, String> {
    let destination = PathBuf::from(destination_folder.trim());
    if destination.as_os_str().is_empty() {
        return Err("No destination folder selected.".to_string());
    }
    fs::create_dir_all(&destination)
        .map_err(|e| format!("Could not create destination folder: {e}"))?;

    let mut result = Sims3PackConversionResult::default();

    for raw in paths {
        let source = PathBuf::from(raw.trim());
        let inspection = match inspect_one(&source, language) {
            Ok(value) => value,
            Err(error) => {
                result.errors.push(format!("{}: {error}", source.display()));
                continue;
            }
        };
        let pack = match read_sims3pack(&source) {
            Ok(value) => value,
            Err(error) => {
                result.errors.push(format!("{}: {error}", source.display()));
                continue;
            }
        };

        if combined {
            let forbidden = inspection.items.iter()
                .filter(|item| item.convertible && !is_safe_conversion_item(item))
                .map(|item| format!("{} ({})", item.display_name, item.content_type))
                .collect::<Vec<_>>();
            if !forbidden.is_empty() {
                result.skipped += inspection.items.len();
                result.errors.push(format!(
                    "{}: Combined conversion blocked for non-mergeable content: {}. Use separate conversion.",
                    source.display(), forbidden.join(", ")
                ));
                continue;
            }
            let (file_name, name_source) = combined_output_name(&source, &pack, language);
            let mut target = unique_output_path(&destination, &file_name);
            loop {
                match merge_sims3pack_payloads(&source, &pack, &inspection, &target) {
                    Ok(_) => break,
                    Err(_error) if target.exists() => {
                        target = unique_output_path(&destination, &file_name);
                    }
                    Err(error) => {
                        result.skipped += inspection.items.iter().filter(|item| item.convertible).count();
                        result.errors.push(format!("{}: {error}", source.display()));
                        target = PathBuf::new();
                        break;
                    }
                }
            }

            if !target.as_os_str().is_empty() {
                let _ = restore_cached_thumbnails(&target);
                result.converted += 1;
                result.skipped += inspection.items.iter().filter(|item| !item.convertible).count();
                result.items.push(Sims3PackConvertedItem {
                    source_path: source.to_string_lossy().to_string(),
                    packaged_name: inspection.display_name.clone(),
                    output_file_name: target.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default(),
                    output_path: target.to_string_lossy().to_string(),
                    name_source,
                });
            }
            continue;
        }

        let output_root = if inspection.set {
            let set_name = source
                .file_stem()
                .map(|v| sanitize_file_stem(&v.to_string_lossy()))
                .unwrap_or_else(|| "Sims3Pack Set".to_string());
            let folder = destination.join(set_name);
            if let Err(error) = fs::create_dir_all(&folder) {
                result.errors.push(format!("{}: {error}", source.display()));
                continue;
            }
            folder
        } else {
            destination.clone()
        };

        for item in inspection.items.iter().filter(|item| item.convertible) {
            let category_folder = output_root.join(&item.recommended_folder);
            if let Err(error) = fs::create_dir_all(&category_folder) {
                result.skipped += 1;
                result.errors.push(format!(
                    "{} / {}: Could not create {}: {error}",
                    source.display(), item.packaged_name, category_folder.display()
                ));
                continue;
            }
            let Some(packaged) = pack.packaged_files.get(item.index) else {
                result.skipped += 1;
                result.errors.push(format!(
                    "{}: packaged item {} disappeared during conversion.",
                    source.display(),
                    item.index
                ));
                continue;
            };
            let payload = match read_packaged_payload(&source, &pack, packaged) {
                Ok(value) if value.len() >= 4 && &value[..4] == b"DBPF" => value,
                Ok(_) => {
                    result.skipped += 1;
                    continue;
                }
                Err(error) => {
                    result.skipped += 1;
                    result.errors.push(format!("{} / {}: {error}", source.display(), packaged.name));
                    continue;
                }
            };

            let mut target = unique_output_path(&category_folder, &item.proposed_file_name);
            loop {
                match write_no_replace(&target, &payload) {
                    Ok(()) => break,
                    Err(_error) if target.exists() => {
                        target = unique_output_path(&category_folder, &item.proposed_file_name);
                    }
                    Err(error) => {
                        result.skipped += 1;
                        result.errors.push(error);
                        target = PathBuf::new();
                        break;
                    }
                }
            }
            if target.as_os_str().is_empty() {
                continue;
            }

            // Preserve byte-for-byte payloads for worlds, lots, households, and Sims.
            if is_safe_conversion_item(item) {
                let _ = restore_cached_thumbnails(&target);
            }
            result.converted += 1;
            result.items.push(Sims3PackConvertedItem {
                source_path: source.to_string_lossy().to_string(),
                packaged_name: packaged.name.clone(),
                output_file_name: target.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default(),
                output_path: target.to_string_lossy().to_string(),
                name_source: item.name_source.clone(),
            });
        }

        result.skipped += inspection.items.iter().filter(|item| !item.convertible).count();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_prefers_matching_localized_name() {
        let mut manifest = ManifestNames::default();
        manifest.display_name = Some("English fallback".into());
        manifest.localized_names.insert("pt-BR".into(), "Nome Brasileiro".into());
        manifest.localized_names.insert("es-ES".into(), "Nombre Español".into());
        assert_eq!(best_manifest_name(&manifest, AppLanguage::Pt).as_deref(), Some("Nome Brasileiro"));
        assert_eq!(best_manifest_name(&manifest, AppLanguage::Es).as_deref(), Some("Nombre Español"));
    }

    #[test]
    fn guid_packaged_names_are_not_used_as_friendly_names() {
        assert!(meaningful_packaged_name("0xc97ea9c600104fcbafdf2d64ffceae2b.package").is_none());
        assert_eq!(meaningful_packaged_name("CoolHair.package").as_deref(), Some("CoolHair"));
    }

    #[test]
    fn cdata_wrappers_are_removed_from_localized_names() {
        assert_eq!(
            clean_manifest_text("![CDATA[Cadeira Aconfortada]]"),
            "Cadeira Aconfortada"
        );
        assert_eq!(
            clean_manifest_text("<![CDATA[Cadeira Aconfortada]]>"),
            "Cadeira Aconfortada"
        );
        assert_eq!(
            sanitize_file_stem("![CDATA[Cadeira Aconfortada]]"),
            "Cadeira Aconfortada"
        );
    }

    #[test]
    fn rebuilt_key_resource_has_nmap_version_and_count() {
        let mut names = BTreeMap::new();
        names.insert(0x1234u64, "Cadeira Aconfortada".to_string());
        let data = build_key_resource(&names);
        assert_eq!(read_u32(&data, 0), Some(1));
        assert_eq!(read_u32(&data, 4), Some(1));
    }

    #[test]
    fn file_name_sanitizer_keeps_readable_names() {
        assert_eq!(sanitize_file_stem("Cool: Hair / Set"), "Cool_ Hair _ Set");
    }
    #[test]
    fn installation_folders_and_extensions_respect_sims3pack_content_types() {
        assert_eq!(content_extension("world"), "world");
        assert_eq!(recommended_folder("world"), "InstalledWorlds");
        assert_eq!(content_extension("lot"), "package");
        assert_eq!(recommended_folder("lot"), "Library");
        assert_eq!(recommended_folder("household"), "Library");
        assert_eq!(content_extension("sim"), "sim");
        assert_eq!(recommended_folder("sim"), "SavedSims");
        assert_eq!(recommended_folder("object"), "Mods/Packages");
    }

    #[test]
    fn combined_conversion_rejects_special_content_types() {
        assert!(is_merge_safe("object"));
        assert!(is_merge_safe("CASPart"));
        for value in ["world", "lot", "household", "sim", "preset", "unknown"] {
            assert!(!is_merge_safe(value));
        }
    }

    #[test]
    fn same_display_names_are_disambiguated_by_internal_titles() {
        let mut items = ["AuroraSkies_Gold", "AuroraSkies_World"].iter().enumerate()
            .map(|(index, title)| Sims3PackConversionItem {
                index,
                packaged_name: format!("{index}.package"),
                proposed_file_name: "Aurora Skies.package".to_string(),
                display_name: "Aurora Skies".to_string(),
                name_source: "manifest".to_string(),
                content_type: "object".to_string(),
                store_set: true,
                recommended_folder: "Mods/Packages".to_string(),
                package_title: Some((*title).to_string()),
                package_id: None,
                size: 100,
                convertible: true,
                warning: None,
                casp_name: None,
                manifest_name: Some("Aurora Skies".to_string()),
            })
            .collect::<Vec<_>>();
        disambiguate_output_names(&mut items);
        assert_eq!(items[0].proposed_file_name, "Aurora Skies - AuroraSkies_Gold.package");
        assert_eq!(items[1].proposed_file_name, "Aurora Skies - AuroraSkies_World.package");
    }

    #[test]
    fn store_featured_set_signature_distinguishes_world_sets_from_streetlights() {
        let featured_set = [
            TYPE_FEATURED_SET_MEMBERS, TYPE_OBJD, TYPE_OBJK, TYPE_MANIFEST
        ];
        let ordinary_streetlight = [TYPE_OBJD, TYPE_OBJK, TYPE_MANIFEST];
        let standalone_override = [TYPE_FEATURED_SET_MEMBERS];
        assert!(has_featured_set_signature(featured_set));
        assert!(!has_featured_set_signature(ordinary_streetlight));
        assert!(!has_featured_set_signature(standalone_override));
        assert!(!has_featured_set_signature([TYPE_FEATURED_SET_MEMBERS, TYPE_OBJD]));
    }

    #[test]
    fn store_featured_sets_are_not_merged_even_when_manifest_says_object() {
        let item = Sims3PackConversionItem {
            index: 0,
            packaged_name: "featured_set.package".into(),
            proposed_file_name: "AuroraSkies_Gold.package".into(),
            display_name: "Aurora Skies".into(),
            name_source: "manifest".into(),
            content_type: "object".into(),
            store_set: true,
            recommended_folder: "Mods/Packages".into(),
            package_title: Some("AuroraSkies_Gold".into()),
            package_id: None,
            size: 100,
            convertible: true,
            warning: None,
            casp_name: None,
            manifest_name: Some("Aurora Skies".into()),
        };
        assert!(!is_safe_conversion_item(&item));
        let ordinary = Sims3PackConversionItem { store_set: false, ..item };
        assert!(is_safe_conversion_item(&ordinary));
    }

}
