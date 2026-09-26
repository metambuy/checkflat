//! Measurement log for spike builds (VITE_SPIKES=1, D-014): the UI appends timing lines here, and
//! they are read with adb because EMUI hides the WebView console. Normal builds never call it.
use std::io::Write;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Android: `getExternalFilesDir(DOCUMENTS)`, readable with `adb shell cat` on a release build.
/// Desktop: `<app data>/devlog`.
fn log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "android"))]
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("devlog");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Appends lines to `dev.log`.
#[tauri::command]
pub fn dev_log(app: AppHandle, lines: Vec<String>) -> Result<(), String> {
    let path = log_dir(&app)?.join("dev.log");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(f, "{l}").map_err(|e| e.to_string())?;
    }
    Ok(())
}
