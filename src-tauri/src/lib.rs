//! Checkflat Tauri shell: thin commands over `checkflat-core`, plus the Sprint 0 report spike
//! that the dev screen still uses.

mod commands;
mod devlog;
mod error;
mod state;

use base64::Engine as _;
use serde::Serialize;
use std::time::Instant;
use tauri::Manager;

use state::AppState;

#[derive(Serialize)]
struct PlatformInfo {
    os: &'static str,
    arch: &'static str,
    debug: bool,
    engines: Vec<&'static str>,
}

#[tauri::command]
fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        debug: cfg!(debug_assertions),
        engines: report_spike::Engine::available()
            .into_iter()
            .map(|e| e.name())
            .collect(),
    }
}

#[derive(Serialize)]
struct ReportResult {
    engine: String,
    path: String,
    bytes: usize,
    /// Render time only (engine call), excluding file write.
    elapsed_ms: u128,
    /// Base64 PDF so the WebView can offer it for download on desktop.
    base64: String,
}

/// Spike C: render the 1-page sample report with the chosen engine and write it to the app data dir.
#[tauri::command]
fn generate_report(app: tauri::AppHandle, engine: String) -> Result<ReportResult, String> {
    let engine = report_spike::Engine::parse(&engine).ok_or_else(|| format!("unknown engine {engine}"))?;
    let started = Instant::now();
    let pdf = report_spike::render(engine, &report_spike::Report::default()).map_err(|e| e.to_string())?;
    let elapsed_ms = started.elapsed().as_millis();

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("report-{}.pdf", engine.name()));
    std::fs::write(&path, &pdf).map_err(|e| e.to_string())?;

    Ok(ReportResult {
        engine: engine.name().to_string(),
        path: path.to_string_lossy().into_owned(),
        bytes: pdf.len(),
        elapsed_ms,
        base64: base64::engine::general_purpose::STANDARD.encode(&pdf),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_camera_capture::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let opened = checkflat_core::db::open(&data_dir.join("checkflat.db"))?;
            // Startup maintenance (quarantine, never delete; skipped for a fresh database).
            match checkflat_core::paths::sweep_orphans(&opened.conn, &data_dir, opened.freshly_created) {
                Ok(report) => log::info!("sweep: {report:?}"),
                Err(e) => log::warn!("sweep failed: {e}"),
            }
            app.manage(AppState::new(opened.conn, data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platform_info,
            generate_report,
            devlog::dev_log,
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::rename_project,
            commands::projects::update_project_address,
            commands::projects::delete_project,
            commands::projects::update_ref_settings,
            commands::projects::preview_ref_settings,
            commands::fractions::list_fractions,
            commands::fractions::add_fraction,
            commands::fractions::delete_fraction,
            commands::photos::stage_photo,
            commands::photos::discard_staged_photo,
            commands::photos::existing_staged_photos,
            commands::photos::list_photos,
            commands::plans::stage_plan_source,
            commands::plans::import_plan,
            commands::plans::discard_staged_plan,
            commands::plans::list_plans,
            commands::plans::rename_plan,
            commands::plans::delete_plan,
            commands::plans::get_plan,
            commands::observations::list_pins,
            commands::observations::create_observation,
            commands::observations::update_observation,
            commands::observations::preview_ref,
            commands::observations::move_pin,
            commands::observations::delete_pin,
            commands::tiles::plan_tiles_info,
            commands::tiles::write_plan_tile,
            commands::tiles::write_plan_tile_manifest,
            commands::tiles::clear_plan_tiles,
            commands::settings::get_setting,
            commands::settings::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
