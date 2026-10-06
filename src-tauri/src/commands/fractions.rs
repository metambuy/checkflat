//! Per-project fraction list (project settings screen; the observation sheet adds fractions
//! through `create_observation` instead).
use checkflat_core::models::Fraction;
use checkflat_core::repo::fractions;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

#[tauri::command]
pub async fn list_fractions(app: AppHandle, project_id: String) -> AppResult<Vec<Fraction>> {
    run_db(app, move |c, _| fractions::list(c, &project_id)).await
}

#[tauri::command]
pub async fn add_fraction(app: AppHandle, project_id: String, code: String) -> AppResult<Fraction> {
    run_db(app, move |c, _| fractions::add(c, &project_id, &code)).await
}

/// Refused (`fraction_in_use`) once the fraction issued a number or an observation uses it.
#[tauri::command]
pub async fn delete_fraction(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, _| fractions::delete(c, &id)).await
}
