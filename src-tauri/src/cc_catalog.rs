use crate::{
    catalog::{classify_resource, TYPE_CASP, TYPE_OBJD},
    dbpf::Package,
    i18n::AppLanguage,
    operation::{self, CANCELLED_ERROR},
    package_family::{classify_package_family, PackageFamilyResult},
};
use calamine::{open_workbook_auto, Reader};
use csv::{ReaderBuilder, WriterBuilder};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use walkdir::WalkDir;

const SCHEMA_VERSION: u32 = 1;
const META_SHEET: &str = "_S3CC_META";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    #[serde(default)]
    pub entry_id: String,
    #[serde(default)]
    pub creator_converter: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub tumblr_handle: String,
    #[serde(default, rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub resource_type: String,
    #[serde(default)]
    pub instance: String,
    #[serde(default)]
    pub tgi: String,
    #[serde(default)]
    pub link_broken: bool,
    #[serde(default)]
    pub partnership_exception: bool,
    #[serde(default)]
    pub package_path: String,
    #[serde(default)]
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSource {
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub entry_id: Option<String>,
    pub row_index: Option<usize>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogHealth {
    pub entries: usize,
    pub with_url: usize,
    pub without_url: usize,
    pub broken_links: usize,
    pub duplicate_groups: usize,
    pub with_type: usize,
    pub compact_ready: usize,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogValidation {
    pub issues: Vec<CatalogIssue>,
    pub health: CatalogHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDocument {
    pub path: String,
    pub schema_version: u32,
    pub schema_recognized: bool,
    pub language_hint: String,
    pub recognized_columns: Vec<String>,
    pub entries: Vec<CatalogEntry>,
    pub validation: CatalogValidation,
    pub preview_total_rows: usize,
    pub preview_valid_rows: usize,
    pub preview_problem_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSaveResult {
    pub path: String,
    pub backup_path: Option<String>,
    pub schema_version: u32,
    pub entries: usize,
    pub validation: CatalogValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogIssueExportRow {
    pub entry: CatalogEntry,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogScanResult {
    pub entries: Vec<CatalogEntry>,
    pub scanned_packages: usize,
    pub skipped_ignored: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDifference {
    pub key: String,
    pub left: Option<CatalogEntry>,
    pub right: Option<CatalogEntry>,
    pub different_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CatalogComparison {
    pub only_left: Vec<CatalogDifference>,
    pub only_right: Vec<CatalogDifference>,
    pub different: Vec<CatalogDifference>,
    pub same: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MissingCcLinkItem {
    pub status: String,
    pub file_name: String,
    pub detected_name: String,
    pub creator_converter: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub url: String,
    pub used_by: String,
    pub catalog_row: String,
    pub resource_type: String,
    pub instance: String,
    pub tgi: String,
    pub extra: BTreeMap<String, String>,
    pub suggested_entry_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MissingCcLinksImport {
    pub schema_version: u32,
    pub source: String,
    pub language: String,
    pub export_mode: String,
    pub input_name: String,
    pub total: usize,
    pub items: Vec<MissingCcLinkItem>,
    pub not_in_master: usize,
    pub possible_match: usize,
    pub missing_url: usize,
    pub invalid_url: usize,
    pub missing_type: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct ColumnMap {
    creator_converter: Option<usize>,
    file_name: Option<usize>,
    url: Option<usize>,
    tumblr_handle: Option<usize>,
    type_name: Option<usize>,
    resource_type: Option<usize>,
    instance: Option<usize>,
    tgi: Option<usize>,
    entry_id: Option<usize>,
    link_broken: Option<usize>,
    partnership_exception: Option<usize>,
}

fn localized_headers(language: AppLanguage) -> [&'static str; 8] {
    match language {
        AppLanguage::Pt => ["Criador / Conversor", "Nome do Arquivo", "URL", "Usuário do Tumblr", "Tipo", "Tipo de Recurso", "Instance", "TGI"],
        AppLanguage::Es => ["Creador / Conversor", "Nombre del Archivo", "URL", "Usuario de Tumblr", "Tipo", "Tipo de Recurso", "Instance", "TGI"],
        AppLanguage::En => ["Creator / Converter", "File Name", "URL", "Tumblr Handle", "Type", "Resource Type", "Instance", "TGI"],
    }
}

fn normalize_header(value: &str) -> String {
    value.trim().to_lowercase().chars().filter(|ch| ch.is_alphanumeric()).collect()
}

fn alias_field(value: &str) -> Option<&'static str> {
    match normalize_header(value).as_str() {
        "creator" | "creatorconverter" | "creatorname" | "author" | "criador" | "criadorconversor" | "autor" | "nomecriador" | "creador" | "creadorconversor" => Some("creator_converter"),
        "filename" | "file" | "item" | "itemname" | "name" | "nomearquivo" | "nomedoarquivo" | "nomeficheiro" | "nomedoficheiro" | "nombrearchivo" | "nombredelarchivo" => Some("file_name"),
        "url" | "link" | "source" | "sourceurl" | "downloadurl" | "downloadlink" | "origem" | "enlace" => Some("url"),
        "tumblr" | "tumblrhandle" | "tumblrusername" | "tumblruser" | "tumblrname" | "tumblraccount" | "tumblrcreator" | "usuariodotumblr" | "usuariotumblr" => Some("tumblr_handle"),
        "type" | "tipo" | "category" | "categoria" => Some("type"),
        "resourcetype" | "tipoderecurso" | "tiporecurso" => Some("resource_type"),
        "instance" | "instanceid" | "instancia" => Some("instance"),
        "tgi" | "resourcekey" | "resourceid" => Some("tgi"),
        "entryid" | "s3ccentryid" | "id" => Some("entry_id"),
        "linkbroken" | "brokenlink" | "linkquebrado" | "enlaceroto" => Some("link_broken"),
        "partnershipexception" | "partnership" | "parceriaexcecao" | "colaboracionexcepcion" => Some("partnership_exception"),
        _ => None,
    }
}

fn map_columns(headers: &[String]) -> ColumnMap {
    let mut out = ColumnMap::default();
    for (index, header) in headers.iter().enumerate() {
        match alias_field(header) {
            Some("creator_converter") if out.creator_converter.is_none() => out.creator_converter = Some(index),
            Some("file_name") if out.file_name.is_none() => out.file_name = Some(index),
            Some("url") if out.url.is_none() => out.url = Some(index),
            Some("tumblr_handle") if out.tumblr_handle.is_none() => out.tumblr_handle = Some(index),
            Some("type") if out.type_name.is_none() => out.type_name = Some(index),
            Some("resource_type") if out.resource_type.is_none() => out.resource_type = Some(index),
            Some("instance") if out.instance.is_none() => out.instance = Some(index),
            Some("tgi") if out.tgi.is_none() => out.tgi = Some(index),
            Some("entry_id") if out.entry_id.is_none() => out.entry_id = Some(index),
            Some("link_broken") if out.link_broken.is_none() => out.link_broken = Some(index),
            Some("partnership_exception") if out.partnership_exception.is_none() => out.partnership_exception = Some(index),
            _ => {}
        }
    }
    out
}

fn recognized_columns(map: ColumnMap) -> Vec<String> {
    let mut out = Vec::new();
    for (name, present) in [
        ("creator_converter", map.creator_converter.is_some()),
        ("file_name", map.file_name.is_some()),
        ("url", map.url.is_some()),
        ("tumblr_handle", map.tumblr_handle.is_some()),
        ("type", map.type_name.is_some()),
        ("resource_type", map.resource_type.is_some()),
        ("instance", map.instance.is_some()),
        ("tgi", map.tgi.is_some()),
    ] {
        if present { out.push(name.to_string()); }
    }
    out
}

fn row_cell(row: &[String], index: Option<usize>) -> String {
    index.and_then(|i| row.get(i)).map(|value| value.trim().to_string()).unwrap_or_default()
}

fn parse_bool(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "sim" | "sí" | "si" | "broken" | "quebrado")
}

fn normalize_cc_name(value: &str) -> String {
    let mut value = value.trim().to_lowercase();
    for ext in [".package", ".sims3pack"] {
        if value.ends_with(ext) {
            value.truncate(value.len().saturating_sub(ext.len()));
            break;
        }
    }
    value.chars().filter(|ch| ch.is_alphanumeric()).collect()
}

fn normalize_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn looks_valid_url(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() { return false; }
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("www."))
        && lower.contains('.')
        && !lower.chars().any(char::is_whitespace)
}

fn normalized_hex(value: &str, width: usize) -> Option<String> {
    let raw = value.trim().trim_start_matches("0x").replace('-', "").replace(':', "").replace(' ', "");
    if raw.is_empty() || raw.len() > width || !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{:0>width$}", raw.to_ascii_uppercase(), width = width))
}

fn parse_tgi(value: &str) -> Option<(String, String, String)> {
    let normalized = value.trim().replace(':', "-");
    let parts = normalized.split('-').map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>();
    if parts.len() != 3 { return None; }
    Some((normalized_hex(parts[0], 8)?, normalized_hex(parts[1], 8)?, normalized_hex(parts[2], 16)?))
}

fn resource_type_id(value: &str) -> Option<String> {
    if value.trim().starts_with("0x") { return normalized_hex(value, 8); }
    let id: u32 = match value.trim().to_ascii_uppercase().as_str() {
        "CASP" => 0x034A_EECB, "OBJD" => 0x319E_4F1D, "OBJK" => 0x02DC_343F,
        "GEOM" => 0x015A_1849, "NMAP" => 0x0166_038C, "STBL" => 0x2205_57DA,
        "XML" => 0x0333_406C, "ITUN" => 0x03B3_3DDF, "S3SA" => 0x073F_AA07,
        "PTRN" | "PATTERN" => 0xD4D9_FBE5, "FACE" => 0x0358_B08A,
        _ => return None,
    };
    Some(format!("0x{id:08X}"))
}

fn compute_entry_id(entry: &CatalogEntry) -> String {
    if !entry.entry_id.trim().is_empty() { return entry.entry_id.trim().to_string(); }
    let basis = if !entry.tgi.trim().is_empty() {
        format!("tgi|{}", entry.tgi.trim().to_ascii_uppercase())
    } else if !entry.instance.trim().is_empty() {
        format!("instance|{}|{}", entry.resource_type.trim().to_ascii_uppercase(), entry.instance.trim().to_ascii_uppercase())
    } else {
        format!("editorial|{}|{}|{}", normalize_cc_name(&entry.file_name), entry.creator_converter.trim().to_ascii_lowercase(), normalize_url(&entry.url))
    };
    let digest = format!("{:X}", Sha256::digest(basis.as_bytes()));
    format!("cc-{}", &digest[..24])
}

fn entry_from_row(row: &[String], map: ColumnMap) -> CatalogEntry {
    let mut entry = CatalogEntry {
        entry_id: row_cell(row, map.entry_id),
        creator_converter: row_cell(row, map.creator_converter),
        file_name: row_cell(row, map.file_name),
        url: row_cell(row, map.url),
        tumblr_handle: row_cell(row, map.tumblr_handle).trim_start_matches('@').to_string(),
        type_name: row_cell(row, map.type_name),
        resource_type: row_cell(row, map.resource_type),
        instance: row_cell(row, map.instance),
        tgi: row_cell(row, map.tgi),
        link_broken: parse_bool(&row_cell(row, map.link_broken)),
        partnership_exception: parse_bool(&row_cell(row, map.partnership_exception)),
        package_path: String::new(),
        modified_at: String::new(),
    };
    entry.entry_id = compute_entry_id(&entry);
    entry
}

fn find_header_row(rows: &[Vec<String>]) -> Option<(usize, ColumnMap)> {
    for (index, row) in rows.iter().take(12).enumerate() {
        let map = map_columns(row);
        if map.file_name.is_some() && (map.creator_converter.is_some() || map.url.is_some()) {
            return Some((index, map));
        }
    }
    None
}

fn csv_schema_version(rows: &[Vec<String>]) -> u32 {
    for row in rows.iter().take(12) {
        let first = row.get(0).map(|value| normalize_header(value)).unwrap_or_default();
        if first == "schema" || first == "s3ccschema" {
            return row.get(1)
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn read_csv_rows(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let bytes = fs::read(path).map_err(|e| format!("Could not open CSV catalog: {e}"))?;
    let semi = bytes.iter().take(4096).filter(|&&b| b == b';').count();
    let comma = bytes.iter().take(4096).filter(|&&b| b == b',').count();
    let delimiter = if semi > comma { b';' } else { b',' };
    let mut reader = ReaderBuilder::new().delimiter(delimiter).flexible(true).has_headers(false).from_reader(bytes.as_slice());
    reader.records().map(|record| {
        record.map(|row| row.iter().map(str::to_string).collect())
            .map_err(|e| format!("Could not read CSV row: {e}"))
    }).collect()
}

fn read_workbook_rows(path: &Path) -> Result<(Vec<Vec<String>>, u32), String> {
    let mut workbook = open_workbook_auto(path).map_err(|e| format!("Could not open spreadsheet: {e}"))?;
    let range = workbook.worksheet_range_at(0)
        .ok_or_else(|| "The spreadsheet does not contain a worksheet.".to_string())?
        .map_err(|e| format!("Could not read the first worksheet: {e}"))?;
    let rows = range.rows().map(|row| row.iter().map(|cell| cell.to_string()).collect::<Vec<_>>()).collect::<Vec<_>>();
    let mut schema = 0u32;
    if workbook.sheet_names().iter().any(|name| name == META_SHEET) {
        if let Ok(meta) = workbook.worksheet_range(META_SHEET) {
            for row in meta.rows() {
                let key = row.get(0).map(|v| v.to_string()).unwrap_or_default();
                let value = row.get(1).map(|v| v.to_string()).unwrap_or_default();
                if key.trim().eq_ignore_ascii_case("SCHEMA") {
                    schema = value.trim().parse::<u32>().unwrap_or(0);
                }
            }
        }
    }
    Ok((rows, schema))
}

fn read_catalog_core(path: &Path) -> Result<(Vec<CatalogEntry>, Vec<String>, u32, String), String> {
    if !path.is_file() { return Err("Master catalog file not found.".to_string()); }
    let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase();
    let (rows, mut schema) = match ext.as_str() {
        "csv" => {
            let rows = read_csv_rows(path)?;
            let schema = csv_schema_version(&rows);
            (rows, schema)
        },
        "xlsx" | "xls" | "xlsb" | "ods" => read_workbook_rows(path)?,
        _ => return Err("Unsupported catalog format. Use CSV, XLSX, XLS, XLSB or ODS.".to_string()),
    };
    let nonempty = rows.into_iter().filter(|row| row.iter().any(|cell| !cell.trim().is_empty())).collect::<Vec<_>>();
    if nonempty.is_empty() { return Ok((Vec::new(), Vec::new(), schema, "unknown".into())); }

    let (header_index, map, data_start) = if let Some((index, map)) = find_header_row(&nonempty) {
        (index, map, index + 1)
    } else {
        if nonempty[0].len() < 3 {
            return Err("The catalog needs at least Creator, File Name and URL columns.".to_string());
        }
        (usize::MAX, ColumnMap {
            creator_converter: Some(0), file_name: Some(1), url: Some(2),
            tumblr_handle: (nonempty[0].len() > 3).then_some(3), ..ColumnMap::default()
        }, 0)
    };
    let columns = recognized_columns(map);
    if schema == 0 && columns.iter().any(|c| matches!(c.as_str(), "type" | "resource_type" | "instance" | "tgi")) {
        schema = SCHEMA_VERSION;
    }
    let language_hint = if header_index != usize::MAX {
        let joined = nonempty[header_index].join(" ").to_lowercase();
        if joined.contains("criador") || joined.contains("arquivo") { "pt" }
        else if joined.contains("creador") || joined.contains("archivo") { "es" }
        else { "en" }
    } else { "unknown" }.to_string();

    let mut entries = Vec::new();
    for row in nonempty.iter().skip(data_start) {
        let entry = entry_from_row(row, map);
        if entry.file_name.trim().is_empty() && entry.creator_converter.trim().is_empty() && entry.url.trim().is_empty() && entry.tgi.trim().is_empty() { continue; }
        entries.push(entry);
    }
    Ok((entries, columns, schema, language_hint))
}

fn apply_meta_rows(path: &Path, entries: &mut [CatalogEntry]) {
    let Ok(mut workbook) = open_workbook_auto(path) else { return; };
    if !workbook.sheet_names().iter().any(|name| name == META_SHEET) { return; }
    let Ok(meta) = workbook.worksheet_range(META_SHEET) else { return; };
    for (index, row) in meta.rows().skip(5).enumerate() {
        let Some(entry) = entries.get_mut(index) else { break; };
        let id = row.get(0).map(|v| v.to_string()).unwrap_or_default();
        if !id.trim().is_empty() { entry.entry_id = id.trim().to_string(); }
        entry.link_broken = row.get(1).map(|v| parse_bool(&v.to_string())).unwrap_or(false);
        entry.partnership_exception = row.get(2).map(|v| parse_bool(&v.to_string())).unwrap_or(false);
    }
}

fn push_issue(issues: &mut Vec<CatalogIssue>, severity: &str, code: &str, message: String, entry: Option<&CatalogEntry>, row_index: Option<usize>, field: Option<&str>) {
    issues.push(CatalogIssue {
        severity: severity.to_string(), code: code.to_string(), message,
        entry_id: entry.map(compute_entry_id), row_index, field: field.map(str::to_string),
    });
}

pub fn validate_entries(entries: &[CatalogEntry]) -> CatalogValidation {
    let mut issues = Vec::new();
    let mut file_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut url_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut instance_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut tgi_map: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, entry) in entries.iter().enumerate() {
        if entry.file_name.trim().is_empty() {
            push_issue(&mut issues, "error", "missing_file_name", "File Name is empty.".into(), Some(entry), Some(index), Some("file_name"));
        }
        if entry.creator_converter.trim().is_empty() {
            push_issue(&mut issues, "warning", "missing_creator", "Creator / Converter is empty.".into(), Some(entry), Some(index), Some("creator_converter"));
        }
        if entry.url.trim().is_empty() {
            push_issue(&mut issues, "warning", "missing_url", "URL is empty.".into(), Some(entry), Some(index), Some("url"));
        } else if !entry.link_broken && !looks_valid_url(&entry.url) {
            push_issue(&mut issues, "error", "invalid_url", "URL is invalid or incomplete.".into(), Some(entry), Some(index), Some("url"));
        }
        if entry.type_name.trim().is_empty() {
            push_issue(&mut issues, "warning", "missing_type", "Type is empty; Compact CC Links cannot use this entry.".into(), Some(entry), Some(index), Some("type"));
        }
        if !entry.tgi.trim().is_empty() {
            if let Some((type_id, _group, instance)) = parse_tgi(&entry.tgi) {
                if let Some(resource_id) = resource_type_id(&entry.resource_type) {
                    if resource_id != type_id {
                        push_issue(&mut issues, "warning", "resource_type_tgi_mismatch", "Resource Type does not match TGI.".into(), Some(entry), Some(index), Some("resource_type"));
                    }
                }
                if let Some(expected_instance) = normalized_hex(&entry.instance, 16) {
                    if expected_instance != instance {
                        push_issue(&mut issues, "warning", "instance_tgi_mismatch", "Instance does not match TGI.".into(), Some(entry), Some(index), Some("instance"));
                    }
                }
            } else {
                push_issue(&mut issues, "error", "invalid_tgi", "TGI must contain Type-Group-Instance hexadecimal values.".into(), Some(entry), Some(index), Some("tgi"));
            }
        }

        let file_key = normalize_cc_name(&entry.file_name);
        if !file_key.is_empty() { file_map.entry(file_key).or_default().push(index); }
        let url_key = normalize_url(&entry.url);
        if !url_key.is_empty() { url_map.entry(url_key).or_default().push(index); }
        if let Some(instance) = normalized_hex(&entry.instance, 16) { instance_map.entry(instance).or_default().push(index); }
        if let Some((t, g, i)) = parse_tgi(&entry.tgi) { tgi_map.entry(format!("{t}-{g}-{i}")).or_default().push(index); }
    }

    let mut duplicate_groups = 0usize;
    for (kind, map) in [("file_name", &file_map), ("url", &url_map), ("instance", &instance_map), ("tgi", &tgi_map)] {
        for (value, indexes) in map.iter().filter(|(_, indexes)| indexes.len() > 1) {
            duplicate_groups += 1;
            push_issue(&mut issues, "warning", &format!("duplicate_{kind}"), format!("Duplicate {kind}: {value} ({} rows).", indexes.len()), None, None, Some(kind));
        }
    }
    for (url, indexes) in url_map.iter().filter(|(_, indexes)| indexes.len() > 1) {
        let creators = indexes.iter().filter_map(|&i| entries.get(i))
            .filter(|entry| !entry.partnership_exception)
            .map(|entry| entry.creator_converter.trim().to_ascii_lowercase())
            .filter(|creator| !creator.is_empty())
            .collect::<HashSet<_>>();
        if creators.len() > 1 {
            push_issue(&mut issues, "warning", "url_multiple_creators", format!("The same URL is assigned to different creators: {url}."), None, None, Some("url"));
        }
    }

    let errors = issues.iter().filter(|issue| issue.severity == "error").count();
    let warnings = issues.iter().filter(|issue| issue.severity == "warning").count();
    let with_url = entries.iter().filter(|entry| !entry.url.trim().is_empty()).count();
    let broken_links = entries.iter().filter(|entry| entry.link_broken).count();
    let with_type = entries.iter().filter(|entry| !entry.type_name.trim().is_empty()).count();
    let compact_ready = entries.iter().filter(|entry| !entry.type_name.trim().is_empty() && looks_valid_url(&entry.url) && !entry.link_broken).count();
    CatalogValidation {
        issues,
        health: CatalogHealth {
            entries: entries.len(), with_url, without_url: entries.len().saturating_sub(with_url),
            broken_links, duplicate_groups, with_type, compact_ready, errors, warnings,
        },
    }
}

fn write_csv(path: &Path, language: AppLanguage, entries: &[CatalogEntry]) -> Result<(), String> {
    let mut writer = WriterBuilder::new().from_path(path).map_err(|e| format!("Could not create CSV: {e}"))?;
    writer.write_record(["S3CC Packer / Manager Master Catalog"]).map_err(|e| e.to_string())?;
    let schema_text = SCHEMA_VERSION.to_string();
    writer.write_record(["SCHEMA", schema_text.as_str()]).map_err(|e| e.to_string())?;
    writer.write_record(["LANGUAGE", match language { AppLanguage::En => "en", AppLanguage::Pt => "pt", AppLanguage::Es => "es" }]).map_err(|e| e.to_string())?;
    let mut headers = localized_headers(language).to_vec();
    headers.extend(["S3CC Entry ID", "Link Broken", "Partnership / Exception"]);
    writer.write_record(headers).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry_id = compute_entry_id(entry);
        writer.write_record([
            entry.creator_converter.as_str(), entry.file_name.as_str(), entry.url.as_str(), entry.tumblr_handle.as_str(),
            entry.type_name.as_str(), entry.resource_type.as_str(), entry.instance.as_str(), entry.tgi.as_str(),
            entry_id.as_str(),
            if entry.link_broken { "1" } else { "0" },
            if entry.partnership_exception { "1" } else { "0" },
        ]).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}

fn write_xlsx(path: &Path, language: AppLanguage, entries: &[CatalogEntry]) -> Result<(), String> {
    let mut workbook = Workbook::new();
    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("CC Catalog").map_err(|e| e.to_string())?;
        for (column, header) in localized_headers(language).iter().enumerate() {
            sheet.write_string(0, column as u16, *header).map_err(|e| e.to_string())?;
        }
        for (index, entry) in entries.iter().enumerate() {
            let row = (index + 1) as u32;
            for (column, value) in [
                entry.creator_converter.as_str(), entry.file_name.as_str(), entry.url.as_str(), entry.tumblr_handle.as_str(),
                entry.type_name.as_str(), entry.resource_type.as_str(), entry.instance.as_str(), entry.tgi.as_str(),
            ].iter().enumerate() {
                sheet.write_string(row, column as u16, *value).map_err(|e| e.to_string())?;
            }
        }
    }
    {
        let meta = workbook.add_worksheet();
        meta.set_name(META_SHEET).map_err(|e| e.to_string())?;
        for (row, (key, value)) in [
            ("CATALOG", "S3CC Packer / Manager Master Catalog".to_string()),
            ("SCHEMA", SCHEMA_VERSION.to_string()),
            ("LANGUAGE", match language { AppLanguage::En => "en", AppLanguage::Pt => "pt", AppLanguage::Es => "es" }.to_string()),
        ].iter().enumerate() {
            meta.write_string(row as u32, 0, *key).map_err(|e| e.to_string())?;
            meta.write_string(row as u32, 1, value.as_str()).map_err(|e| e.to_string())?;
        }
        meta.write_string(4, 0, "ENTRY_ID").map_err(|e| e.to_string())?;
        meta.write_string(4, 1, "LINK_BROKEN").map_err(|e| e.to_string())?;
        meta.write_string(4, 2, "PARTNERSHIP_EXCEPTION").map_err(|e| e.to_string())?;
        for (index, entry) in entries.iter().enumerate() {
            let row = (index + 5) as u32;
            meta.write_string(row, 0, compute_entry_id(entry)).map_err(|e| e.to_string())?;
            meta.write_string(row, 1, if entry.link_broken { "1" } else { "0" }).map_err(|e| e.to_string())?;
            meta.write_string(row, 2, if entry.partnership_exception { "1" } else { "0" }).map_err(|e| e.to_string())?;
        }
    }
    workbook.save(path).map_err(|e| format!("Could not save XLSX catalog: {e}"))
}

fn backup_existing(path: &Path, limit: usize) -> Result<Option<PathBuf>, String> {
    if !path.exists() { return Ok(None); }
    let parent = path.parent().ok_or_else(|| "Catalog has no parent folder.".to_string())?;
    let backup_dir = parent.join(".s3cc-master-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| format!("Could not create catalog backup folder: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let stem = path.file_stem().and_then(|v| v.to_str()).unwrap_or("catalog");
    let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("xlsx");
    let backup = backup_dir.join(format!("{stem}-{stamp}.{ext}"));
    fs::copy(path, &backup).map_err(|e| format!("Could not back up master catalog: {e}"))?;

    let mut backups = fs::read_dir(&backup_dir).map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            entry.path().is_file() && name.starts_with(&format!("{stem}-")) && name.ends_with(&format!(".{ext}"))
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH));
    let keep = limit.max(1);
    if backups.len() > keep {
        let remove_count = backups.len() - keep;
        for entry in backups.into_iter().take(remove_count) { let _ = fs::remove_file(entry.path()); }
    }
    Ok(Some(backup))
}

fn write_catalog(path: &Path, language: AppLanguage, entries: &[CatalogEntry]) -> Result<(), String> {
    match path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "csv" => write_csv(path, language, entries),
        "xlsx" => write_xlsx(path, language, entries),
        _ => Err("Writable catalog format must be CSV or XLSX.".to_string()),
    }
}

#[tauri::command]
pub fn open_cc_catalog(path: String) -> Result<CatalogDocument, String> {
    open_cc_catalog_core(path)
}

fn open_cc_catalog_core(path: String) -> Result<CatalogDocument, String> {
    let path_buf = PathBuf::from(path.trim());
    let (mut entries, columns, schema, language_hint) = read_catalog_core(&path_buf)?;
    if path_buf.extension().and_then(|v| v.to_str()).map(|v| v.eq_ignore_ascii_case("xlsx")).unwrap_or(false) {
        apply_meta_rows(&path_buf, &mut entries);
    }
    for entry in &mut entries { entry.entry_id = compute_entry_id(entry); }
    let validation = validate_entries(&entries);
    let problem_rows = validation.issues.iter().filter_map(|issue| issue.row_index).collect::<HashSet<_>>().len();
    Ok(CatalogDocument {
        path: path_buf.to_string_lossy().to_string(), schema_version: schema,
        schema_recognized: schema == SCHEMA_VERSION || schema == 0, language_hint,
        recognized_columns: columns, preview_total_rows: entries.len(),
        preview_valid_rows: entries.len().saturating_sub(problem_rows), preview_problem_rows: problem_rows,
        entries, validation,
    })
}

#[tauri::command]
fn create_cc_catalog_core(path: String, language: AppLanguage) -> Result<CatalogDocument, String> {
    let path_buf = PathBuf::from(path.trim());
    if path_buf.as_os_str().is_empty() { return Err("No catalog path was provided.".to_string()); }
    if let Some(parent) = path_buf.parent() { fs::create_dir_all(parent).map_err(|e| format!("Could not create catalog folder: {e}"))?; }
    if path_buf.exists() { return Err("A file already exists at the selected catalog path.".to_string()); }
    write_catalog(&path_buf, language, &[])?;
    open_cc_catalog_core(path_buf.to_string_lossy().to_string())
}

fn save_cc_catalog_core(path: String, language: AppLanguage, mut entries: Vec<CatalogEntry>, backup_limit: usize) -> Result<CatalogSaveResult, String> {
    let path_buf = PathBuf::from(path.trim());
    if path_buf.as_os_str().is_empty() { return Err("No catalog path was provided.".to_string()); }
    let validation = validate_entries(&entries);
    for entry in &mut entries { entry.entry_id = compute_entry_id(entry); }
    let backup = backup_existing(&path_buf, backup_limit.clamp(1, 50))?;
    let ext = path_buf.extension().and_then(|v| v.to_str()).unwrap_or("xlsx");
    let temp = path_buf.with_extension(format!("tmp.{ext}"));
    write_catalog(&temp, language, &entries)?;
    fs::copy(&temp, &path_buf).map_err(|e| format!("Could not commit catalog: {e}"))?;
    let _ = fs::remove_file(&temp);
    Ok(CatalogSaveResult {
        path: path_buf.to_string_lossy().to_string(), backup_path: backup.map(|p| p.to_string_lossy().to_string()),
        schema_version: SCHEMA_VERSION, entries: entries.len(), validation,
    })
}

#[tauri::command(rename = "create_cc_catalog")]
pub async fn create_cc_catalog(
    path: String,
    language: AppLanguage,
) -> Result<CatalogDocument, String> {
    let scope = path.clone();
    operation::run_mutating_blocking_scoped(
        "catalog_write",
        "creating",
        &[scope.as_str()],
        move || create_cc_catalog_core(path, language),
    )
    .await
}

#[tauri::command(rename = "save_cc_catalog")]
pub async fn save_cc_catalog(
    path: String,
    language: AppLanguage,
    entries: Vec<CatalogEntry>,
    backup_limit: usize,
) -> Result<CatalogSaveResult, String> {
    let scope = path.clone();
    operation::run_mutating_blocking_scoped(
        "catalog_write",
        "saving",
        &[scope.as_str()],
        move || save_cc_catalog_core(path, language, entries, backup_limit),
    )
    .await
}

#[tauri::command]
pub fn validate_cc_catalog(entries: Vec<CatalogEntry>) -> Result<CatalogValidation, String> {
    Ok(validate_entries(&entries))
}

fn resource_type_label(type_id: u32) -> String {
    match type_id {
        0x034A_EECB => "CASP".into(), 0x319E_4F1D => "OBJD".into(), 0x02DC_343F => "OBJK".into(),
        0x015A_1849 => "GEOM".into(), 0x0166_038C => "NMAP".into(), 0x2205_57DA => "STBL".into(),
        0x0333_406C => "XML".into(), 0x03B3_3DDF => "ITUN".into(), 0x073F_AA07 => "S3SA".into(),
        0xD4D9_FBE5 => "PTRN".into(), 0x0358_B08A => "FACE".into(), other => format!("0x{other:08X}"),
    }
}

fn preferred_resource_index(package: &Package) -> Option<usize> {
    for wanted in [TYPE_CASP, TYPE_OBJD, 0x073F_AA07, 0xD4D9_FBE5, 0x0354_796A, 0x0355_5BA8, 0x0358_B08A, 0xB52F_5055, 0x067C_AA11, 0x062C_8204] {
        if let Some(index) = package.entries.iter().position(|entry| entry.type_id == wanted) { return Some(index); }
    }
    package.entries.iter().position(|entry| !matches!(entry.type_id, 0x0166_038C | 0x73E9_3EEB)).or_else(|| (!package.entries.is_empty()).then_some(0))
}

fn suggested_type(package: &Package) -> String {
    for entry in &package.entries {
        if matches!(entry.type_id, TYPE_CASP | TYPE_OBJD) {
            if let Ok(data) = package.data(entry) {
                if let Some(classification) = classify_resource(entry.type_id, &data, AppLanguage::En) {
                    if entry.type_id == TYPE_CASP {
                        return match (classification.main_category.as_str(), classification.sub_category.as_deref().unwrap_or("")) {
                            ("Hair", _) => "Hair", ("Clothing", "Shoes") => "Shoes", ("Clothing", _) => "Clothing",
                            ("Accessories", _) => "Accessories", ("Makeup", _) => "Makeup", ("Genetics", _) => "Genetics",
                            ("Pets", _) => "Pets", _ => "",
                        }.to_string();
                    }
                    return match classification.main_category.as_str() {
                        "Build" => "Build",
                        "Buy" if classification.sub_category.as_deref().unwrap_or("").to_ascii_lowercase().contains("decor") => "Decor",
                        "Buy" => "Furniture",
                        _ => "",
                    }.to_string();
                }
            }
        }
    }
    let types = package.entries.iter().map(|entry| entry.type_id).collect::<BTreeSet<_>>();
    match classify_package_family(&types, AppLanguage::En) {
        PackageFamilyResult::Classified(value) => match value.main_category.as_str() {
            "Patterns" => "Patterns".into(), "Gameplay" => "Gameplay".into(), "Localization" => "Localization".into(),
            "CAS" if value.sub_category.as_deref() == Some("Sliders") => "Sliders".into(),
            "CAS" if value.sub_category.as_deref() == Some("Skin Tones") => "Skins".into(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn scan_catalog_package(path: &Path) -> Result<CatalogEntry, String> {
    let package = Package::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let primary = preferred_resource_index(&package).and_then(|index| package.entries.get(index));
    let (resource_type, instance, tgi) = if let Some(entry) = primary {
        (resource_type_label(entry.type_id), format!("0x{:016X}", entry.instance), format!("0x{:08X}-0x{:08X}-0x{:016X}", entry.type_id, entry.group, entry.instance))
    } else { (String::new(), String::new(), String::new()) };
    let modified_at = fs::metadata(path).ok().and_then(|m| m.modified().ok()).map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339()).unwrap_or_default();
    let mut entry = CatalogEntry {
        creator_converter: String::new(),
        file_name: path.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default(),
        type_name: suggested_type(&package), resource_type, instance, tgi,
        package_path: path.to_string_lossy().to_string(), modified_at,
        ..CatalogEntry::default()
    };
    entry.entry_id = compute_entry_id(&entry);
    Ok(entry)
}

fn canonical_enabled(configs: &[CatalogSource]) -> Vec<PathBuf> {
    configs.iter().filter(|config| config.enabled && !config.path.trim().is_empty())
        .filter_map(|config| PathBuf::from(config.path.trim()).canonicalize().ok()).collect()
}

fn scan_cc_catalog_sources_core(sources: Vec<CatalogSource>, ignored: Vec<CatalogSource>) -> Result<CatalogScanResult, String> {
    let source_roots = canonical_enabled(&sources);
    if source_roots.is_empty() { return Err("No enabled CC source folders were found.".to_string()); }
    let ignored_roots = canonical_enabled(&ignored);
    let mut package_paths = BTreeSet::new();
    let mut skipped_ignored = 0usize;
    let mut errors = Vec::new();
    for source in source_roots {
        if !source.is_dir() { errors.push(format!("CC source is not a folder: {}", source.display())); continue; }
        for item in WalkDir::new(&source).follow_links(false).into_iter().filter_map(Result::ok) {
            if !item.file_type().is_file() { continue; }
            let path = item.path();
            if !path.extension().and_then(|v| v.to_str()).map(|v| v.eq_ignore_ascii_case("package")).unwrap_or(false) { continue; }
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if ignored_roots.iter().any(|ignored| canonical.starts_with(ignored)) { skipped_ignored += 1; continue; }
            package_paths.insert(canonical);
        }
    }
    let mut entries = Vec::new();
    operation::set_total("catalog_scan", package_paths.len());
    for (processed, path) in package_paths.into_iter().enumerate() {
        if operation::is_cancelled("catalog_scan") {
            return Err(CANCELLED_ERROR.to_string());
        }
        operation::update("catalog_scan", processed, Some(path.to_string_lossy().into_owned()), "reading");
        match scan_catalog_package(&path) { Ok(entry) => entries.push(entry), Err(error) => errors.push(error) }
    }
    entries.sort_by(|a, b| a.file_name.to_ascii_lowercase().cmp(&b.file_name.to_ascii_lowercase()));
    Ok(CatalogScanResult { scanned_packages: entries.len(), entries, skipped_ignored, errors })
}

fn comparison_key(entry: &CatalogEntry) -> String {
    if let Some((t, g, i)) = parse_tgi(&entry.tgi) { return format!("tgi|{t}-{g}-{i}"); }
    if let Some(instance) = normalized_hex(&entry.instance, 16) { return format!("instance|{}|{instance}", entry.resource_type.trim().to_ascii_uppercase()); }
    format!("name|{}", normalize_cc_name(&entry.file_name))
}

fn differing_fields(a: &CatalogEntry, b: &CatalogEntry) -> Vec<String> {
    [
        ("creator_converter", &a.creator_converter, &b.creator_converter), ("file_name", &a.file_name, &b.file_name),
        ("url", &a.url, &b.url), ("tumblr_handle", &a.tumblr_handle, &b.tumblr_handle), ("type", &a.type_name, &b.type_name),
        ("resource_type", &a.resource_type, &b.resource_type), ("instance", &a.instance, &b.instance), ("tgi", &a.tgi, &b.tgi),
    ].into_iter().filter_map(|(name, left, right)| (!left.trim().eq_ignore_ascii_case(right.trim())).then_some(name.to_string())).collect()
}

fn compare_cc_catalogs_core(left_path: String, right_path: String) -> Result<CatalogComparison, String> {
    let (left_entries, _, _, _) = read_catalog_core(Path::new(left_path.trim()))?;
    let (right_entries, _, _, _) = read_catalog_core(Path::new(right_path.trim()))?;
    let mut left = left_entries.into_iter().map(|entry| (comparison_key(&entry), entry)).collect::<BTreeMap<_, _>>();
    let mut right = right_entries.into_iter().map(|entry| (comparison_key(&entry), entry)).collect::<BTreeMap<_, _>>();
    let keys = left.keys().chain(right.keys()).cloned().collect::<BTreeSet<_>>();
    let mut out = CatalogComparison::default();
    for key in keys {
        match (left.remove(&key), right.remove(&key)) {
            (Some(a), Some(b)) => {
                let fields = differing_fields(&a, &b);
                if fields.is_empty() { out.same += 1; } else { out.different.push(CatalogDifference { key, left: Some(a), right: Some(b), different_fields: fields }); }
            }
            (Some(a), None) => out.only_left.push(CatalogDifference { key, left: Some(a), right: None, different_fields: Vec::new() }),
            (None, Some(b)) => out.only_right.push(CatalogDifference { key, left: None, right: Some(b), different_fields: Vec::new() }),
            _ => {}
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn scan_cc_catalog_sources(sources: Vec<CatalogSource>, ignored: Vec<CatalogSource>) -> Result<CatalogScanResult, String> {
    crate::operation::run_blocking("catalog_scan", "scanning", move || {
        scan_cc_catalog_sources_core(sources, ignored)
    }).await
}

#[tauri::command]
pub async fn compare_cc_catalogs(left_path: String, right_path: String) -> Result<CatalogComparison, String> {
    crate::operation::run_blocking("catalog_compare", "comparing", move || {
        compare_cc_catalogs_core(left_path, right_path)
    }).await
}

fn missing_value(map: &BTreeMap<String, String>, key: &str) -> String {
    map.get(key).cloned().unwrap_or_default()
}

#[tauri::command]
pub fn import_missing_cc_links(path: String, entries: Vec<CatalogEntry>) -> Result<MissingCcLinksImport, String> {
    let text = fs::read_to_string(path.trim()).map_err(|e| format!("Could not read Missing CC Links file: {e}"))?;
    let mut result = MissingCcLinksImport::default();
    let mut current = BTreeMap::<String, String>::new();
    let mut header = BTreeMap::<String, String>::new();

    fn flush_item(map: &mut BTreeMap<String, String>, result: &mut MissingCcLinksImport, entries: &[CatalogEntry]) {
        if map.is_empty() { return; }
        let mut item = MissingCcLinkItem {
            status: missing_value(map, "STATUS"), file_name: missing_value(map, "FILE_NAME"),
            detected_name: missing_value(map, "DETECTED_NAME"), creator_converter: missing_value(map, "CREATOR_CONVERTER"),
            type_name: missing_value(map, "TYPE"), url: missing_value(map, "URL"), used_by: missing_value(map, "USED_BY"),
            catalog_row: missing_value(map, "CATALOG_ROW"), resource_type: missing_value(map, "RESOURCE_TYPE"),
            instance: missing_value(map, "INSTANCE"), tgi: missing_value(map, "TGI"),
            extra: BTreeMap::new(), suggested_entry_id: None,
        };
        for (key, value) in map.iter() {
            if !matches!(key.as_str(), "STATUS" | "FILE_NAME" | "DETECTED_NAME" | "CREATOR_CONVERTER" | "TYPE" | "URL" | "USED_BY" | "CATALOG_ROW" | "RESOURCE_TYPE" | "INSTANCE" | "TGI") {
                item.extra.insert(key.clone(), value.clone());
            }
        }
        let target = normalize_cc_name(if item.file_name.trim().is_empty() { &item.detected_name } else { &item.file_name });
        item.suggested_entry_id = entries.iter().find(|entry| normalize_cc_name(&entry.file_name) == target && !target.is_empty()).map(compute_entry_id);
        match item.status.as_str() {
            "NOT_IN_MASTER" => result.not_in_master += 1, "POSSIBLE_MATCH" => result.possible_match += 1,
            "MISSING_URL" => result.missing_url += 1, "INVALID_URL" => result.invalid_url += 1, "MISSING_TYPE" => result.missing_type += 1, _ => {}
        }
        result.items.push(item);
        map.clear();
    }

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line == "VEIGAS_S3CC_MISSING_LINKS" { continue; }
        if line == "[ITEM]" { flush_item(&mut current, &mut result, &entries); continue; }
        let Some((key, value)) = line.split_once('=') else { continue; };
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim().to_string();
        if current.is_empty() && matches!(key.as_str(), "SCHEMA" | "SOURCE" | "LANGUAGE" | "EXPORT_MODE" | "INPUT_NAME" | "GENERATED_AT" | "TOTAL") {
            header.insert(key, value);
        } else {
            current.insert(key, value);
        }
    }
    flush_item(&mut current, &mut result, &entries);
    result.schema_version = header.get("SCHEMA").and_then(|v| v.parse().ok()).unwrap_or(1);
    result.source = missing_value(&header, "SOURCE");
    result.language = missing_value(&header, "LANGUAGE");
    result.export_mode = missing_value(&header, "EXPORT_MODE");
    result.input_name = missing_value(&header, "INPUT_NAME");
    result.total = header.get("TOTAL").and_then(|v| v.parse().ok()).unwrap_or(result.items.len());
    Ok(result)
}

#[tauri::command]
pub fn export_catalog_issues(
    path: String,
    language: AppLanguage,
    entries: Vec<CatalogEntry>,
    extra_rows: Option<Vec<CatalogIssueExportRow>>,
) -> Result<String, String> {
    let validation = validate_entries(&entries);
    let mut issue_rows = entries.into_iter().enumerate().filter_map(|(index, entry)| {
        let codes = validation.issues.iter().filter(|issue| issue.row_index == Some(index)).map(|issue| issue.code.clone()).collect::<Vec<_>>();
        (!codes.is_empty()).then_some((entry, codes.join("; ")))
    }).collect::<Vec<_>>();
    for row in extra_rows.unwrap_or_default() {
        issue_rows.push((row.entry, row.status));
    }
    let path_buf = PathBuf::from(path.trim());
    let scope = path_buf.to_string_lossy().to_string();
    operation::run_mutating_sync("catalog_write", "exporting", &[scope.as_str()], || {
    match path_buf.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "csv" => {
            let mut writer = WriterBuilder::new().from_path(&path_buf).map_err(|e| e.to_string())?;
            let mut headers = localized_headers(language).to_vec(); headers.push("Status");
            writer.write_record(headers).map_err(|e| e.to_string())?;
            for (entry, status) in issue_rows {
                writer.write_record([entry.creator_converter, entry.file_name, entry.url, entry.tumblr_handle, entry.type_name, entry.resource_type, entry.instance, entry.tgi, status]).map_err(|e| e.to_string())?;
            }
            writer.flush().map_err(|e| e.to_string())?;
        }
        "xlsx" => {
            let mut workbook = Workbook::new();
            let sheet = workbook.add_worksheet();
            sheet.set_name("CC Issues").map_err(|e| e.to_string())?;
            let mut headers = localized_headers(language).to_vec(); headers.push("Status");
            for (col, header) in headers.iter().enumerate() { sheet.write_string(0, col as u16, *header).map_err(|e| e.to_string())?; }
            for (index, (entry, status)) in issue_rows.iter().enumerate() {
                for (col, value) in [entry.creator_converter.as_str(), entry.file_name.as_str(), entry.url.as_str(), entry.tumblr_handle.as_str(), entry.type_name.as_str(), entry.resource_type.as_str(), entry.instance.as_str(), entry.tgi.as_str(), status.as_str()].iter().enumerate() {
                    sheet.write_string((index + 1) as u32, col as u16, *value).map_err(|e| e.to_string())?;
                }
            }
            workbook.save(&path_buf).map_err(|e| e.to_string())?;
        }
        _ => return Err("Issue export must be CSV or XLSX.".to_string()),
    }
    Ok(path_buf.to_string_lossy().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_are_language_independent() {
        assert_eq!(alias_field("Creator / Converter"), Some("creator_converter"));
        assert_eq!(alias_field("Criador / Conversor"), Some("creator_converter"));
        assert_eq!(alias_field("Creador / Conversor"), Some("creator_converter"));
        assert_eq!(alias_field("Nome do Arquivo"), Some("file_name"));
        assert_eq!(alias_field("Nombre del Archivo"), Some("file_name"));
    }

    #[test]
    fn validation_separates_errors_and_warnings() {
        let entry = CatalogEntry { file_name: "A.package".into(), url: "not a url".into(), ..CatalogEntry::default() };
        let result = validate_entries(&[entry]);
        assert!(result.health.errors >= 1);
        assert!(result.health.warnings >= 2);
    }

    #[test]
    fn tgi_parser_normalizes_hex() {
        assert_eq!(parse_tgi("034AEECB-0-1234"), Some(("0x034AEECB".into(), "0x00000000".into(), "0x0000000000001234".into())));
    }
}
