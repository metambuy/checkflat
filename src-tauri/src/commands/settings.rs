use checkflat_core::repo::settings;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

#[tauri::command]
pub async fn get_setting(app: AppHandle, key: String) -> AppResult<Option<String>> {
    run_db(app, move |c, _| settings::get(c, &key)).await
}

#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: String) -> AppResult<()> {
    run_db(app, move |c, _| settings::set(c, &key, &value)).await
}
