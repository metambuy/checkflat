//! Checkflat — Sprint 0 spike shell. Only spike commands live here; no product code yet.

use base64::Engine as _;
use serde::Serialize;
use std::time::Instant;
use tauri::Manager;

#[derive(Serialize)]
struct PlatformInfo {
    os: &'static str,
    arch: &'static str,
    debug: bool,
    engines: Vec<&'static str>,
}

#[tauri::command]
fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        debug: cfg!(debug_assertions),
        engines: report_spike::Engine::available()
            .into_iter()
            .map(|e| e.name())
            .collect(),
    }
}

#[derive(Serialize)]
struct ReportResult {
    engine: String,
    path: String,
    bytes: usize,
    /// Render time only (engine call), excluding file write.
    elapsed_ms: u128,
    /// Base64 PDF so the WebView can offer it for download on desktop.
    base64: String,
}

/// Spike C: render the 1-page sample report with the chosen engine and write it to the app data dir.
#[tauri::command]
fn generate_report(app: tauri::AppHandle, engine: String) -> Result<ReportResult, String> {
    let engine = report_spike::Engine::parse(&engine).ok_or_else(|| format!("unknown engine {engine}"))?;
    let started = Instant::now();
    let pdf = report_spike::render(engine, &report_spike::Report::default()).map_err(|e| e.to_string())?;
    let elapsed_ms = started.elapsed().as_millis();

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("report-{}.pdf", engine.name()));
    std::fs::write(&path, &pdf).map_err(|e| e.to_string())?;

    Ok(ReportResult {
        engine: engine.name().to_string(),
        path: path.to_string_lossy().into_owned(),
        bytes: pdf.len(),
        elapsed_ms,
        base64: base64::engine::general_purpose::STANDARD.encode(&pdf),
    })
}

#[derive(Serialize)]
struct PhotoResult {
    path: String,
    bytes: usize,
    /// JPEG as base64 so the WebView can show it without asset-protocol scope setup.
    base64: String,
}

/// Spike B part 2: native camera via the camera-capture plugin; the file is read here in Rust.
#[tauri::command]
async fn capture_photo(app: tauri::AppHandle) -> Result<PhotoResult, String> {
    use tauri_plugin_camera_capture::CameraCaptureExt;
    let res = app.camera_capture().capture().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&res.path).map_err(|e| format!("read {}: {e}", res.path))?;
    Ok(PhotoResult {
        path: res.path,
        bytes: bytes.len(),
        base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_camera_capture::init())
        .invoke_handler(tauri::generate_handler![platform_info, generate_report, capture_photo])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
