//! Pins (observations placed on a plan). `create_pin` is the confirm of a draft pin.
use checkflat_core::models::Observation;
use checkflat_core::repo::observations;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

#[tauri::command]
pub async fn list_pins(app: AppHandle, plan_id: String) -> AppResult<Vec<Observation>> {
    run_db(app, move |c, _| observations::list_for_plan(c, &plan_id)).await
}

#[tauri::command]
pub async fn create_pin(app: AppHandle, plan_id: String, x: f64, y: f64) -> AppResult<Observation> {
    run_db(app, move |c, _| observations::create_pin(c, &plan_id, x, y)).await
}

#[tauri::command]
pub async fn move_pin(app: AppHandle, id: String, x: f64, y: f64) -> AppResult<Observation> {
    run_db(app, move |c, _| observations::move_pin(c, &id, x, y)).await
}

#[tauri::command]
pub async fn delete_pin(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, _| observations::delete_pin(c, &id)).await
}
