pub mod analyzer;
pub mod audit_report;
pub mod cache;
pub mod category_identity;
pub mod catalog;
pub mod cc_catalog;
pub mod compression;
pub mod conflicts;
pub mod dbpf;
pub mod dependencies;
pub mod duplicates;
pub mod embedded_duplicates;
pub mod executor;
pub mod fix_signatures;
pub mod health;
pub mod history;
pub mod i18n;
pub mod inbox;
pub mod manifest;
pub mod mesh_info;
pub mod navigation;
pub mod operation;
pub mod package_compare;
pub mod package_details;
pub mod package_family;
pub mod project_grouping;
pub mod reference_catalog;
pub mod planner;
pub mod quarantine;
pub mod resource_cfg;
pub mod resource_cfg_update;
pub mod restore;
pub mod restore_history;
pub mod review_store;
pub mod scanner;
pub mod selection_export;
pub mod sims3pack_converter;
pub mod slider;
pub mod snapshots;
pub mod structure_manager;
pub mod system_appearance;
pub mod taxonomy;
pub mod technical_search;
pub mod unified_analysis;
pub mod workspace;

#[cfg(target_os = "windows")]
fn apply_runtime_window_icon(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tauri::Manager;

    let encoded = include_str!("../icons/manager-icon.b64").trim();
    let bytes = STANDARD.decode(encoded)?;
    let icon = tauri::image::Image::from_bytes(&bytes)?;

    if let Some(window) = app.get_webview_window("main") {
        window.set_icon(icon)?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(not(test))]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "windows")]
            apply_runtime_window_icon(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system_appearance::get_system_appearance,
            scanner::scan_packages,
            unified_analysis::analyze_unified,
            sims3pack_converter::inspect_sims3packs,
            sims3pack_converter::convert_sims3packs,
            cc_catalog::open_cc_catalog,
            cc_catalog::create_cc_catalog,
            cc_catalog::save_cc_catalog,
            cc_catalog::validate_cc_catalog,
            cc_catalog::scan_cc_catalog_sources,
            cc_catalog::compare_cc_catalogs,
            cc_catalog::import_missing_cc_links,
            cc_catalog::export_catalog_issues,
            duplicates::analyze_duplicates,
            embedded_duplicates::analyze_embedded_duplicates,
            conflicts::analyze_conflicts,
            operation::get_operation_status,
            operation::cancel_operation,
            cache::get_cache_info,
            cache::clear_cache,
            navigation::open_directory,
            navigation::reveal_path,
            navigation::open_external_url,
            package_details::get_package_preview,
            package_details::get_package_technical_details,
            restore_history::list_restore_history,
            restore_history::remove_restore_history,
            review_store::load_conflict_decisions,
            review_store::set_conflict_decision,
            review_store::set_conflict_decisions_bulk,
            audit_report::save_audit_report,
            structure_manager::list_structure,
            structure_manager::list_structure_directories,
            structure_manager::create_structure_folder,
            structure_manager::move_structure_path,
            structure_manager::rename_structure_folder,
            structure_manager::list_manual_operations,
            structure_manager::undo_last_manual_operation,
            workspace::load_workspace,
            workspace::save_workspace,
            workspace::set_read_only,
            workspace::set_package_metadata,
            workspace::set_manual_classification,
            workspace::save_package_group,
            workspace::delete_package_group,
            snapshots::create_snapshot,
            snapshots::list_snapshots,
            snapshots::compare_snapshot_to_current,
            snapshots::compare_mods_roots,
            health::analyze_mods_health,
            health::remove_empty_folder,
            dependencies::analyze_dependencies,
            fix_signatures::build_persistent_fix_signature_index,
            fix_signatures::get_persistent_fix_signature_index_status,
            fix_signatures::rebuild_persistent_fix_signature_index,
            reference_catalog::get_reference_catalog_status,
            reference_catalog::build_reference_catalogs,
            technical_search::technical_search,
            package_compare::compare_packages,
            inbox::scan_inbox,
            inbox::build_inbox_import_plan,
            inbox::execute_inbox_import,
            selection_export::save_selection_export,
            history::get_operation_history,
            quarantine::build_quarantine_plan_async,
            quarantine::build_exact_duplicate_quarantine_plan_async,
            quarantine::list_existing_quarantine,
            quarantine::execute_quarantine_async,
            quarantine::execute_exact_duplicate_quarantine_async,
            quarantine::restore_quarantine_async,
            quarantine::recover_quarantine_async,
            quarantine::remove_quarantine_history,
            planner::build_organization_plan,
            planner::build_organization_plan_with_cfg,
            planner::preview_resource_cfg_for_plan,
            executor::execute_organization,
            executor::execute_organization_with_cfg,
            executor::execute_organization_plan_with_cfg,
            restore::preview_restore,
            restore::execute_restore_async
        ])
        .run(tauri::generate_context!())
        .expect("error while running S3CC Manager");
}

// The Tauri context macro requires Cargo's application build environment and
// cannot be expanded for a library test target. The application entry point
// remains unchanged; this test-only stub lets the Rust classifier and DBPF
// regressions run without starting or building the desktop application.
#[cfg(test)]
pub fn run() {}
