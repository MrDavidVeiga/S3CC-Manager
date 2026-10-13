use crate::operation;
use chrono::Local;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReportResult {
    pub directory: String,
    pub markdown_path: String,
    pub json_path: String,
}

fn reports_dir(root: &Path) -> PathBuf {
    let base = dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Documents")))
        .unwrap_or_else(|| root.parent().unwrap_or(root).to_path_buf());

    base.join("Veiga's S3CC Manager").join("Reports")
}

fn unique_paths(directory: &Path, kind: &str) -> (PathBuf, PathBuf) {
    // kind is validated against the explicit allowlist in save_audit_report.
    let report_label = match kind {
        "organizer" => "Organizer",
        "duplicates" => "Duplicates",
        "conflicts" => "Conflicts",
        "health" => "Health",
        _ => unreachable!("Report type must be validated"),
    };
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    for suffix in 0..1000usize {
        let base = if suffix == 0 {
            format!("S3CC-Manager-{report_label}-Audit-{stamp}")
        } else {
            format!("S3CC-Manager-{report_label}-Audit-{stamp}-{suffix}")
        };
        let markdown = directory.join(format!("{base}.md"));
        let json = directory.join(format!("{base}.json"));
        if !markdown.exists() && !json.exists() {
            return (markdown, json);
        }
    }

    (
        directory.join(format!("S3CC-Manager-{report_label}-Audit-{stamp}-overflow.md")),
        directory.join(format!("S3CC-Manager-{report_label}-Audit-{stamp}-overflow.json")),
    )
}

fn write_new(path: &Path, content: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("Refusing to overwrite report: {}", path.display()));
    }
    fs::write(path, content)
        .map_err(|error| format!("Could not write report {}: {error}", path.display()))
}

#[tauri::command]
pub fn save_audit_report(
    folder: String,
    kind: String,
    markdown: String,
    json_content: String,
) -> Result<AuditReportResult, String> {
    if !matches!(kind.as_str(), "organizer" | "duplicates" | "conflicts" | "health") {
        return Err("Invalid audit report type.".to_string());
    }
    let root = PathBuf::from(folder.trim())
        .canonicalize()
        .map_err(|error| format!("Could not resolve Mods root: {error}"))?;
    if !root.is_dir() {
        return Err(format!("Selected root is not a directory: {}", root.display()));
    }

    if markdown.trim().is_empty() {
        return Err("Markdown audit report is empty.".to_string());
    }
    let json: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|error| format!("Audit JSON is invalid: {error}"))?;
    if json.get("reportKind").and_then(|value| value.as_str()) != Some(kind.as_str()) {
        return Err("Audit JSON report type does not match the requested export.".to_string());
    }

    let directory = reports_dir(&root);
    let scope = directory.to_string_lossy().to_string();
    operation::run_mutating_sync("report_write", "saving", &[scope.as_str()], move || {
        fs::create_dir_all(&directory).map_err(|error| {
            format!("Could not create reports directory {}: {error}", directory.display())
        })?;

        let (markdown_path, json_path) = unique_paths(&directory, &kind);
        write_new(&markdown_path, markdown.as_bytes())?;

        if let Err(error) = write_new(&json_path, json_content.as_bytes()) {
            let _ = fs::remove_file(&markdown_path);
            return Err(error);
        }

        Ok(AuditReportResult {
            directory: directory.to_string_lossy().to_string(),
            markdown_path: markdown_path.to_string_lossy().to_string(),
            json_path: json_path.to_string_lossy().to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_use_manager_documents_location() {
        let root = Path::new(r"C:\Mods\Packages");
        assert!(reports_dir(root)
            .to_string_lossy()
            .contains("Veiga's S3CC Manager"));
    }
}
