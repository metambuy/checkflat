use checkflat_core::models::{Project, ProjectSummary};
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
