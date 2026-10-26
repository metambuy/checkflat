//! The report action (Sprint 5; the latest report, Open and Share since Sprint 6a, D-028; Windows
//! Open and Save a copy since Sprint 6b, D-029). The PDF and its metadata are written to `reports/`
//! in the app data dir (`checkflat_core::reports`, one per project). Open and Share hand another
//! app a copy under the cache's `reports/` (the Android plugin; the default PDF app on desktop).
//! Save a copy writes where the user chooses in a save dialog opened from here. The WebView never
//! passes a path: every command takes the project id. The database lock is held only for
//! `collect`; the image work and the Typst compile run without it.
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
use tauri_plugin_dialog::DialogExt;

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

/// A run in progress: its project and language (`en` | `pt`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Running {
    pub project_id: String,
    pub lang: String,
}

/// The report being generated. One report at a time: two would double the ≈ 635 MB peak (D-026).
/// `delete_project` refuses that project meanwhile, or the report's write would recreate the files
/// its delete removed. It outlives the page: a run goes on when the WebView reloads (an activity
/// recreated in a living process), and [`report_running`] lets the reloaded card follow it (D-029).
static GENERATING: Mutex<Option<Running>> = Mutex::new(None);

fn generating_slot() -> std::sync::MutexGuard<'static, Option<Running>> {
    GENERATING.lock().unwrap_or_else(|e| e.into_inner())
}

fn busy() -> AppError {
    AppError::new("report_busy", "a report is already being generated")
}

/// Holds [`GENERATING`] for one run; released when dropped, at the end of the run, also on an
/// error. Tauri never cancels a command, so a page that goes away does not release it.
#[derive(Debug)]
struct Generating;

impl Generating {
    fn start(project_id: &str, lang: &str) -> AppResult<Generating> {
        let mut slot = generating_slot();
        if slot.is_some() {
            return Err(busy());
        }
        *slot = Some(Running { project_id: project_id.to_string(), lang: lang.to_string() });
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
    if generating_slot().as_ref().is_some_and(|r| r.project_id == project_id) {
        return Err(busy());
    }
    Ok(())
}

/// The run in progress, if any (D-029): a card mounted after its page was reloaded shows it and
/// waits for it, instead of offering a Generate that would answer `report_busy`.
#[tauri::command]
pub fn report_running() -> Option<Running> {
    generating_slot().clone()
}

/// `lang`: `en` | `pt` (chosen at generation time); `time_zone`: the device's IANA zone name;
/// `utc_offset_min`: its current offset, the fallback when the zone is unknown (D-024).
#[tauri::command]
pub async fn generate_report(app: AppHandle, project_id: String, lang: String, time_zone: String, utc_offset_min: i32) -> AppResult<ReportResult> {
    let lang = Lang::parse(&lang);
    let _running = Generating::start(&project_id, lang.code())?;
    let started = Instant::now();
    let pid = project_id.clone();
    let source = run_db_with(app.clone(), move |c, dir| report::collect(c, dir, &pid, lang, &time_zone, utc_offset_min)).await?;
    // Read before `prepare` takes the source: the readable name and the metadata come from the
    // same instant as the cover's date (D-028).
    let new = NewReport {
        name: reports::display_name(&source.project.name, &source.clock.iso_date(&source.created_at), lang.code()),
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

/// Opens the project's latest report in a PDF viewer (Android) or the default PDF app (desktop);
/// `no_pdf_app` when there is none.
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

/// "Save a copy…" (Windows, D-029): a save dialog proposing the report's readable name in
/// Documents, then a copy of the latest report to the chosen file. Returns the saved file's name,
/// or `None` when the dialog is cancelled. The dialog asks before replacing a file. Its PDF filter
/// is also the default extension, so a name typed without `.pdf` gets it, and the replace prompt
/// sees the final name. A copy that cannot be written (the file held open by a PDF viewer, a
/// read-only folder) is `save_failed`. `title` is the dialog's title. On Android the dialog returns
/// a content URI, not a path; the card never offers the action there.
#[tauri::command]
pub async fn save_report_copy(app: AppHandle, window: tauri::Window, project_id: String, title: String) -> AppResult<Option<String>> {
    let pid = project_id.clone();
    let meta = run_fs(app.clone(), move |dir| reports::latest(dir, &pid)).await?;
    let meta = meta.ok_or_else(|| AppError::new("no_report", "there is no report to save"))?;
    let documents = app.path().document_dir().ok();
    let dialog = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut d = dialog.dialog().file().set_parent(&window).set_title(title).set_file_name(&meta.name).add_filter("PDF", &["pdf"]);
        if let Some(dir) = documents {
            d = d.set_directory(dir);
        }
        d.blocking_save_file()
    })
    .await
    .map_err(AppError::internal)?;
    let Some(picked) = picked else { return Ok(None) };
    let dest = picked.into_path().map_err(|e| AppError::new("save_failed", e.to_string()))?;
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let saved = run_fs(app, move |dir| reports::save_copy(dir, &project_id, &dest)).await;
    match saved {
        Ok(Some(_)) => Ok(Some(name)),
        Ok(None) => Err(AppError::new("no_report", "there is no report to save")),
        Err(e) if e.code == "io" => Err(AppError::new("save_failed", e.message)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_report_at_a_time_the_delete_check_and_the_running_query() {
        assert_eq!(report_running(), None);
        let first = Generating::start("p1", "pt").unwrap();
        assert_eq!(report_running(), Some(Running { project_id: "p1".into(), lang: "pt".into() }));
        assert_eq!(Generating::start("p2", "en").unwrap_err().code, "report_busy");
        assert_eq!(report_running().unwrap().project_id, "p1", "a refused start leaves the run as it was");
        assert_eq!(refuse_while_generating("p1").unwrap_err().code, "report_busy");
        assert!(refuse_while_generating("p2").is_ok());
        drop(first);
        assert_eq!(report_running(), None);
        assert!(refuse_while_generating("p1").is_ok());
        let again = Generating::start("p2", "en").unwrap();
        assert_eq!(report_running().unwrap().lang, "en");
        drop(again);
    }
}
