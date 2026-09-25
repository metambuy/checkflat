//! Sprint 2 viewer spike (D-014): a log file adb can read from a release build, and a tile store
//! for the import-time pyramid experiment. Used only by the Dev screen; removed after D-014.
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

/// Android: `getExternalFilesDir(DOCUMENTS)` (`adb pull`-able without run-as, EMUI hides the
/// console). Desktop: `<app data>/spike`.
fn log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    let dir = app.path().document_dir().map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "android"))]
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("spike");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

fn tiles_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("spike-tiles"))
}

/// Tile paths are `<key>/<level>/<x>_<y>.<ext>` with a short ASCII key; anything else is refused.
fn valid_tile_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.len() == 3
        && parts.iter().all(|s| {
            !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
        })
        && !parts.iter().any(|s| s.starts_with('.'))
}

/// Appends lines to `spike.log`; returns the file path.
#[tauri::command]
pub fn spike_log(app: AppHandle, lines: Vec<String>) -> Result<String, String> {
    let path = log_dir(&app)?.join("spike.log");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(f, "{l}").map_err(|e| e.to_string())?;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Raw request body = tile bytes; the relative path comes in the `x-tile-path` header.
#[tauri::command]
pub fn spike_write_tile(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    // Desktop delivers the Uint8Array as a raw body; Android's IPC turns it into a JSON number array.
    let json_bytes;
    let bytes: &[u8] = match request.body() {
        tauri::ipc::InvokeBody::Raw(b) => b,
        tauri::ipc::InvokeBody::Json(v) => {
            json_bytes = serde_json::from_value::<Vec<u8>>(v.clone()).map_err(|e| format!("body: {e}"))?;
            &json_bytes
        }
    };
    let rel = request
        .headers()
        .get("x-tile-path")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing x-tile-path")?;
    if !valid_tile_path(rel) {
        return Err(format!("invalid tile path {rel}"));
    }
    let path = tiles_root(&app)?.join(rel);
    std::fs::create_dir_all(path.parent().expect("has parent")).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct TileStats {
    dir: String,
    files: u64,
    bytes: u64,
    /// `meta/manifest.json`, written by the generator after each finished level.
    manifest: Option<String>,
}

/// Absolute dir of one pyramid (for `convertFileSrc`) and its size on disk.
#[tauri::command]
pub fn spike_tiles_info(app: AppHandle, key: String) -> Result<TileStats, String> {
    if !valid_tile_path(&format!("{key}/0/0")) {
        return Err("invalid key".into());
    }
    let dir = tiles_root(&app)?.join(&key);
    let (mut files, mut bytes) = (0, 0);
    if let Ok(levels) = std::fs::read_dir(&dir) {
        for l in levels.flatten() {
            for f in std::fs::read_dir(l.path()).into_iter().flatten().flatten() {
                files += 1;
                bytes += f.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    let manifest = std::fs::read_to_string(dir.join("meta").join("manifest.json")).ok();
    Ok(TileStats { dir: dir.to_string_lossy().into_owned(), files, bytes, manifest })
}

#[tauri::command]
pub fn spike_tiles_clear(app: AppHandle, key: String) -> Result<(), String> {
    if !valid_tile_path(&format!("{key}/0/0")) {
        return Err("invalid key".into());
    }
    let dir = tiles_root(&app)?.join(&key);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::valid_tile_path;

    #[test]
    fn tile_paths() {
        assert!(valid_tile_path("a1-legacy/2048/3_1.webp"));
        assert!(!valid_tile_path("../x/y"));
        assert!(!valid_tile_path("a/b"));
        assert!(!valid_tile_path("a/.b/c"));
        assert!(!valid_tile_path("a//c"));
        assert!(!valid_tile_path("/a/b/c"));
    }
}
