//! Plan tile cache (D-014). The WebView renders tiles with PDF.js and sends each one as base64:
//! Tauri's Android IPC would turn a Uint8Array into a JSON number array (write time 8.8 s vs 2.3 s
//! for a pyramid on the P30, D-014).
use base64::Engine as _;
use checkflat_core::repo::tiles::{self, TileInfo, TileManifest};
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::state::run_db;

#[tauri::command]
pub async fn plan_tiles_info(app: AppHandle, plan_id: String) -> AppResult<TileInfo> {
    run_db(app, move |c, dir| tiles::info(c, dir, &plan_id)).await
}

#[tauri::command]
pub async fn write_plan_tile(app: AppHandle, plan_id: String, size: u32, x: u32, y: u32, data: String) -> AppResult<()> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| AppError::new("validation", format!("tile data: {e}")))?;
    run_db(app, move |c, dir| tiles::write_tile(c, dir, &plan_id, size, x, y, &bytes)).await
}

#[tauri::command]
pub async fn write_plan_tile_manifest(app: AppHandle, plan_id: String, manifest: TileManifest) -> AppResult<()> {
    run_db(app, move |c, dir| tiles::write_manifest(c, dir, &plan_id, &manifest)).await
}

#[tauri::command]
pub async fn clear_plan_tiles(app: AppHandle, plan_id: String) -> AppResult<()> {
    run_db(app, move |c, dir| tiles::clear(c, dir, &plan_id)).await
}
