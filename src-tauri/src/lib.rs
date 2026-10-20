//! Checkflat Tauri shell: thin commands over `checkflat-core` and `checkflat-report`.

mod commands;
#[cfg(test)]
mod asset_scope;
mod devlog;
mod error;
mod state;

use serde::Serialize;
use tauri::Manager;

use state::AppState;

#[derive(Serialize)]
struct PlatformInfo {
    os: &'static str,
    arch: &'static str,
    debug: bool,
}

#[tauri::command]
fn platform_info() -> PlatformInfo {
    PlatformInfo { os: std::env::consts::OS, arch: std::env::consts::ARCH, debug: cfg!(debug_assertions) }
}

/// The command list, plus any extra commands that exist only in some builds (the Dev screen's
/// seed command is compiled out of release builds without the `dev-seed` feature, so it cannot be
/// named there).
macro_rules! handler {
    ($($extra:path),* $(,)?) => {
        tauri::generate_handler![
            platform_info,
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
            commands::report::generate_report,
            $($extra),*
        ]
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
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
        });
    #[cfg(any(debug_assertions, feature = "dev-seed"))]
    let builder = builder.invoke_handler(handler![commands::dev::dev_seed_observations]);
    #[cfg(not(any(debug_assertions, feature = "dev-seed")))]
    let builder = builder.invoke_handler(handler![]);
    builder.run(tauri::generate_context!()).expect("error while running tauri application");
}
