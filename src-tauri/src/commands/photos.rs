//! Photos: `stage_photo` takes a picked or captured image once, processes it in the core
//! (orientation, ≤ 1600 px JPEG) and returns a token the observation save consumes;
//! `discard_staged_photo` drops it; `list_photos` returns an observation's stored photos.
use checkflat_core::paths::RelPath;
use checkflat_core::photos;
use checkflat_core::repo::photos as photo_repo;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_fs::{FsExt, OpenOptions};

use super::plans::to_file_path;
use crate::error::{AppError, AppResult};
use crate::state::{run_db, run_fs};

/// A photo as the WebView shows it: `path` is absolute, for `convertFileSrc`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoView {
    pub id: String,
    pub observation_id: String,
    pub path: String,
    pub taken_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedPhotoView {
    pub token: String,
    pub path: String,
    pub taken_at: String,
}

fn absolute(dir: &std::path::Path, rel: &RelPath) -> String {
    rel.resolve(dir).to_string_lossy().into_owned()
}

/// `source` is an absolute path (Android: the plugin's cache copy; Windows: the picked file) or a
/// `content://` URI. `utc_offset_min` is the device's zone, minutes east of UTC, for EXIF times
/// that state none.
#[tauri::command]
pub async fn stage_photo(app: AppHandle, source: String, utc_offset_min: i32) -> AppResult<StagedPhotoView> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    let file = app
        .fs()
        .open(to_file_path(&source), opts)
        .map_err(|e| AppError::new("source_unreadable", e.to_string()))?;
    run_fs(app, move |dir| {
        let s = photos::stage(dir, file, utc_offset_min)?;
        let path = photos::staged_path(dir, &s.token)?.to_string_lossy().into_owned();
        Ok(StagedPhotoView { token: s.token, path, taken_at: s.taken_at })
    })
    .await
}

#[tauri::command]
pub async fn discard_staged_photo(app: AppHandle, token: String) -> AppResult<()> {
    run_fs(app, move |dir| photos::discard_staged(dir, &token)).await
}

/// The subset of `tokens` whose staged file still exists (draft recovery after a restart).
#[tauri::command]
pub async fn existing_staged_photos(app: AppHandle, tokens: Vec<String>) -> AppResult<Vec<String>> {
    run_fs(app, move |dir| Ok(photos::staged_present(dir, &tokens))).await
}

#[tauri::command]
pub async fn list_photos(app: AppHandle, observation_id: String) -> AppResult<Vec<PhotoView>> {
    run_db(app, move |c, dir| {
        Ok(photo_repo::list_for_observation(c, &observation_id)?
            .into_iter()
            .map(|p| PhotoView { path: absolute(dir, &p.file_path), id: p.id, observation_id: p.observation_id, taken_at: p.taken_at })
            .collect())
    })
    .await
}
