//! Pins (observations placed on a plan). `create_observation` is the save of the observation
//! sheet; the ref is computed in the core and returned as `ref` / `marker`.
use checkflat_core::models::Observation;
use checkflat_core::repo::{observations, projects};
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

#[tauri::command]
pub async fn list_pins(app: AppHandle, plan_id: String) -> AppResult<Vec<Observation>> {
    run_db(app, move |c, _| observations::list_for_plan(c, &plan_id)).await
}

#[tauri::command]
pub async fn create_observation(
    app: AppHandle,
    plan_id: String,
    x: f64,
    y: f64,
    fraction: String,
    description: String,
) -> AppResult<Observation> {
    run_db(app, move |c, _| observations::create_observation(c, &plan_id, x, y, &fraction, &description)).await
}

/// The ref the next observation in `fraction` would get with the project's stored settings.
#[tauri::command]
pub async fn preview_ref(app: AppHandle, project_id: String, fraction: String) -> AppResult<String> {
    run_db(app, move |c, _| {
        let p = projects::get(c, &project_id)?;
        projects::preview_ref(c, &project_id, &p.code, &p.ref_template, p.seq_scope, &fraction)
    })
    .await
}

#[tauri::command]
pub async fn move_pin(app: AppHandle, id: String, x: f64, y: f64) -> AppResult<Observation> {
    run_db(app, move |c, _| observations::move_pin(c, &id, x, y)).await
}

#[tauri::command]
pub async fn delete_pin(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, _| observations::delete_pin(c, &id)).await
}
