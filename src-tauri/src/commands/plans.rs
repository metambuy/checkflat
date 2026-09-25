//! Plan import: `stage_plan_source` (read the picked file once into staging, inspect, propose a
//! title) then `import_plan` (rename into the project + insert) or `discard_staged_plan`.
use std::path::PathBuf;

use checkflat_core::models::Plan;
use checkflat_core::pdf::PdfInfo;
use checkflat_core::repo::plans;
use serde::Serialize;
use tauri::{AppHandle, Url};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

use crate::error::{AppError, AppResult};
use crate::state::{run_db, run_fs};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedPlan {
    pub token: String,
    pub display_name: Option<String>,
    pub default_title: String,
    pub info: PdfInfo,
}

/// `content://…` / `file://…` become URL paths (Android picker), anything else is a filesystem path.
fn to_file_path(source: &str) -> FilePath {
    if source.contains("://") {
        if let Ok(url) = Url::parse(source) {
            return FilePath::Url(url);
        }
    }
    FilePath::Path(PathBuf::from(source))
}

#[tauri::command]
pub async fn stage_plan_source(app: AppHandle, source: String) -> AppResult<StagedPlan> {
    let path = to_file_path(&source);
    // Display name: Android resolves it through the ContentResolver (plugin, off the main thread);
    // desktop paths carry their own file name.
    let display_name = match &path {
        FilePath::Url(url) => {
            use tauri_plugin_camera_capture::CameraCaptureExt;
            let uri = url.to_string();
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || handle.camera_capture().display_name(uri))
                .await
                .map_err(AppError::internal)?
                .unwrap_or_else(|e| {
                    log::warn!("display name unavailable: {e}");
                    None
                })
        }
        FilePath::Path(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()),
    };
    // Open through the fs plugin so content:// URIs work on Android; then stream into staging.
    let mut opts = OpenOptions::new();
    opts.read(true);
    let file = app
        .fs()
        .open(path, opts)
        .map_err(|e| AppError::new("source_unreadable", e.to_string()))?;
    let staged = run_fs(app, move |dir| plans::stage(dir, file)).await?;
    let default_title = display_name
        .as_deref()
        .map(plans::default_title)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Plan".to_string());
    Ok(StagedPlan { token: staged.token, display_name, default_title, info: staged.info })
}

#[tauri::command]
pub async fn import_plan(app: AppHandle, project_id: String, token: String, title: String) -> AppResult<Plan> {
    run_db(app, move |c, dir| plans::import(c, dir, &project_id, &token, &title)).await
}

#[tauri::command]
pub async fn discard_staged_plan(app: AppHandle, token: String) -> AppResult<()> {
    run_fs(app, move |dir| plans::discard_staged(dir, &token)).await
}

#[tauri::command]
pub async fn list_plans(app: AppHandle, project_id: String) -> AppResult<Vec<Plan>> {
    run_db(app, move |c, _| plans::list_for_project(c, &project_id)).await
}

#[tauri::command]
pub async fn rename_plan(app: AppHandle, id: String, title: String) -> AppResult<Plan> {
    run_db(app, move |c, _| plans::rename(c, &id, &title)).await
}

#[tauri::command]
pub async fn delete_plan(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, dir| plans::delete(c, dir, &id)).await
}
