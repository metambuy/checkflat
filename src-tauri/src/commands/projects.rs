use checkflat_core::models::{Project, ProjectSummary};
use checkflat_core::refs::Scope;
use checkflat_core::repo::projects;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> AppResult<Vec<ProjectSummary>> {
    run_db(app, |c, _| projects::list(c)).await
}

#[tauri::command]
pub async fn get_project(app: AppHandle, id: String) -> AppResult<Project> {
    run_db(app, move |c, _| projects::get(c, &id)).await
}

#[tauri::command]
pub async fn create_project(app: AppHandle, name: String, address: String) -> AppResult<Project> {
    run_db(app, move |c, _| projects::create(c, &name, &address)).await
}

#[tauri::command]
pub async fn rename_project(app: AppHandle, id: String, name: String) -> AppResult<Project> {
    run_db(app, move |c, _| projects::rename(c, &id, &name)).await
}

#[tauri::command]
pub async fn update_project_address(app: AppHandle, id: String, address: String) -> AppResult<Project> {
    run_db(app, move |c, _| projects::update_address(c, &id, &address)).await
}

#[tauri::command]
pub async fn delete_project(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, dir| projects::delete(c, dir, &id)).await
}

/// Ref numbering settings (project settings screen). `scope` is `"project"` or `"fraction"`.
#[tauri::command]
pub async fn update_ref_settings(app: AppHandle, id: String, code: String, template: String, scope: Scope) -> AppResult<Project> {
    run_db(app, move |c, _| projects::update_ref_settings(c, &id, &code, &template, scope)).await
}

/// Live preview on the settings screen: the next ref under unsaved settings, nothing written.
#[tauri::command]
pub async fn preview_ref_settings(
    app: AppHandle,
    id: String,
    code: String,
    template: String,
    scope: Scope,
    fraction: String,
) -> AppResult<String> {
    run_db(app, move |c, _| projects::preview_ref(c, &id, &code, &template, scope, &fraction)).await
}
