//! The report action (Sprint 5; the latest report, Open and Share since Sprint 6a, D-028). The PDF
//! and its metadata are written to `reports/` in the app data dir (`checkflat_core::reports`, one
//! per project); Open and Share hand another app a copy under the cache's `reports/` through the
//! Android plugin, and the WebView never passes a path: every command takes the project id. The
//! database lock is held only for `collect`; the image work and the Typst compile run without it.
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use checkflat_core::clock;
use checkflat_core::i18n::Lang;
use checkflat_core::reports::{self, NewReport, ReportMeta};
use checkflat_report::{self as report, Stats};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_camera_capture::CameraCaptureExt;

use crate::error::{AppError, AppResult};
use crate::state::{run_db_with, run_fs, run_fs_with};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    pub report: ReportMeta,
    pub stats: Stats,
    pub compile_ms: u64,
    pub pdf_ms: u64,
    pub total_ms: u64,
}

/// The project whose report is being generated. One report at a time: two would double the
/// ≈ 635 MB peak (D-026). `delete_project` refuses that project meanwhile, or the report's write
/// would recreate the files its delete removed.
static GENERATING: Mutex<Option<String>> = Mutex::new(None);

fn generating_slot() -> std::sync::MutexGuard<'static, Option<String>> {
    GENERATING.lock().unwrap_or_else(|e| e.into_inner())
}

fn busy() -> AppError {
    AppError::new("report_busy", "a report is already being generated")
}

/// Holds [`GENERATING`] for one run; released when dropped (also on an error or a cancelled call).
#[derive(Debug)]
struct Generating;

impl Generating {
    fn start(project_id: &str) -> AppResult<Generating> {
        let mut slot = generating_slot();
        if slot.is_some() {
            return Err(busy());
        }
        *slot = Some(project_id.to_string());
        Ok(Generating)
    }
}

impl Drop for Generating {
    fn drop(&mut self) {
        *generating_slot() = None;
    }
}

/// `report_busy` if this project's report is being generated (for `delete_project`, which checks
/// it under the database lock: a run that starts later reads the project only after the delete).
pub(crate) fn refuse_while_generating(project_id: &str) -> AppResult<()> {
    if generating_slot().as_deref() == Some(project_id) {
        return Err(busy());
    }
    Ok(())
}

/// `lang`: `en` | `pt` (chosen at generation time); `time_zone`: the device's IANA zone name;
/// `utc_offset_min`: its current offset, the fallback when the zone is unknown (D-024).
#[tauri::command]
pub async fn generate_report(app: AppHandle, project_id: String, lang: String, time_zone: String, utc_offset_min: i32) -> AppResult<ReportResult> {
    let _running = Generating::start(&project_id)?;
    let started = Instant::now();
    let lang = Lang::parse(&lang);
    let pid = project_id.clone();
    let source = run_db_with(app.clone(), move |c, dir| report::collect(c, dir, &pid, lang, &time_zone, utc_offset_min)).await?;
    // Read before `prepare` takes the source: the readable name and the metadata come from the
    // same instant as the cover's date (D-028).
    let new = NewReport {
        name: reports::display_name(&source.project.name, &source.clock.iso_date(&source.created_at)),
        lang: lang.code().to_string(),
        pages: 0,
        created_at: source.created_at.clone(),
    };
    #[cfg(feature = "dev-seed")]
    let app_out = app.clone();
    let (result, _dest) = run_fs_with(app, move |dir| {
        let input = report::prepare(dir, source)?;
        let rendered = report::render(&input)?;
        let new = NewReport { pages: rendered.pages, ..new };
        let (dest, meta) = reports::write_report(dir, &project_id, &clock::now_compact(), &rendered.pdf, new)?;
        let result = ReportResult {
            report: meta,
            stats: input.stats,
            compile_ms: rendered.compile_ms,
            pdf_ms: rendered.pdf_ms,
            total_ms: started.elapsed().as_millis() as u64,
        };
        Ok::<_, report::ReportError>((result, dest))
    })
    .await?;
    // Local measurement builds (D-026): a release build is not debuggable, so `run-as` cannot read
    // `reports/`; a copy in the Documents dir can be pulled with adb. Runs after `total_ms`.
    #[cfg(feature = "dev-seed")]
    {
        if let (Ok(out), Some(name)) = (crate::devlog::log_dir(&app_out), _dest.file_name()) {
            if let Err(e) = std::fs::copy(&_dest, out.join(name)) {
                log::warn!("dev-seed: report copy failed: {e}");
            }
        }
    }
    Ok(result)
}

/// The project's latest report, or `None` (never generated, or generated before Sprint 6a).
#[tauri::command]
pub async fn latest_report(app: AppHandle, project_id: String) -> AppResult<Option<ReportMeta>> {
    run_fs(app, move |dir| reports::latest(dir, &project_id)).await
}

/// `<app cache>/reports`: the share copies (the plugin's FileProvider path `reports/`, D-028).
pub(crate) fn share_root(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app.path().app_cache_dir().map_err(AppError::internal)?.join("reports"))
}

/// The share copy of the project's latest report; `no_report` when there is none.
async fn copy_for_sharing(app: &AppHandle, project_id: String) -> AppResult<String> {
    let root = share_root(app)?;
    let copy = run_fs(app.clone(), move |dir| reports::share_copy(dir, &project_id, &root)).await?;
    let copy = copy.ok_or_else(|| AppError::new("no_report", "there is no report to open or share"))?;
    Ok(copy.to_string_lossy().into_owned())
}

/// A plugin error as the UI's code: a rejection `no_app` becomes `no_app_code`, the rest internal.
fn plugin_error(e: tauri_plugin_camera_capture::Error, no_app_code: &str) -> AppError {
    match e.rejected_code() {
        Some("no_app") => AppError::new(no_app_code, e.to_string()),
        _ => AppError::internal(e),
    }
}

/// Opens the project's latest report in a PDF viewer (Android); `no_pdf_app` when none is installed.
#[tauri::command]
pub async fn open_report(app: AppHandle, project_id: String) -> AppResult<()> {
    let path = copy_for_sharing(&app, project_id).await?;
    tauri::async_runtime::spawn_blocking(move || app.camera_capture().view_pdf(path))
        .await
        .map_err(AppError::internal)?
        .map_err(|e| plugin_error(e, "no_pdf_app"))
}

/// Shares the project's latest report through the system share sheet (Android); `title` is the
/// sheet's title where the chooser shows one.
#[tauri::command]
pub async fn share_report(app: AppHandle, project_id: String, title: String) -> AppResult<()> {
    let path = copy_for_sharing(&app, project_id).await?;
    tauri::async_runtime::spawn_blocking(move || app.camera_capture().share_pdf(path, title))
        .await
        .map_err(AppError::internal)?
        .map_err(|e| plugin_error(e, "no_share_app"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_report_at_a_time_and_the_delete_check() {
        let first = Generating::start("p1").unwrap();
        assert_eq!(Generating::start("p2").unwrap_err().code, "report_busy");
        assert_eq!(refuse_while_generating("p1").unwrap_err().code, "report_busy");
        assert!(refuse_while_generating("p2").is_ok());
        drop(first);
        assert!(refuse_while_generating("p1").is_ok());
        let again = Generating::start("p2").unwrap();
        drop(again);
    }
}
