use checkflat_core::models::{Project, ProjectSummary};
use checkflat_core::refs::Scope;
use checkflat_core::reports;
use checkflat_core::repo::projects;
use tauri::AppHandle;

use crate::commands::report;
use crate::error::{AppError, AppResult};
use crate::state::{run_db, run_db_with, run_fs};

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

/// Refused (`report_busy`) while this project's report is being generated; the check runs under the
/// database lock, so a report started afterwards finds the project gone. The share copies in the
/// app cache go too (D-028).
#[tauri::command]
pub async fn delete_project(app: AppHandle, id: String) -> AppResult<()> {
    let pid = id.clone();
    run_db_with(app.clone(), move |c, dir| {
        report::refuse_while_generating(&pid)?;
        projects::delete(c, dir, &pid).map_err(AppError::from)
    })
    .await?;
    let root = report::share_root(&app)?;
    let share_id = id.clone();
    if let Err(e) = run_fs(app, move |_| reports::remove_share_copies(&root, &share_id)).await {
        log::warn!("project {id} deleted but its share copies could not be removed: {}", e.message);
    }
    Ok(())
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
