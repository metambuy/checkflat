//! Report generation (Sprint 5). Temporary action: the PDF is written to `reports/` in the app
//! data dir (one per project, `paths::write_report`) and its path shown; share (Android) / save
//! (Windows) follow in Sprint 6. The database lock is held only for `collect`; the image work and
//! the Typst compile run without it.
use std::time::Instant;

use checkflat_core::i18n::Lang;
use checkflat_core::{clock, paths};
use checkflat_report::{self as report, Stats};
use serde::Serialize;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::{run_db_with, run_fs_with};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    /// Absolute path of the written PDF.
    pub path: String,
    pub bytes: usize,
    pub pages: usize,
    pub stats: Stats,
    pub compile_ms: u64,
    pub pdf_ms: u64,
    pub total_ms: u64,
}

/// `lang`: `en` | `pt` (chosen at generation time); `time_zone`: the device's IANA zone name;
/// `utc_offset_min`: its current offset, the fallback when the zone is unknown (D-024).
#[tauri::command]
pub async fn generate_report(app: AppHandle, project_id: String, lang: String, time_zone: String, utc_offset_min: i32) -> AppResult<ReportResult> {
    let started = Instant::now();
    let lang = Lang::parse(&lang);
    let pid = project_id.clone();
    let source = run_db_with(app.clone(), move |c, dir| report::collect(c, dir, &pid, lang, &time_zone, utc_offset_min)).await?;
    #[cfg(feature = "dev-seed")]
    let app_out = app.clone();
    let result = run_fs_with(app, move |dir| {
        let input = report::prepare(dir, source)?;
        let rendered = report::render(&input)?;
        let dest = paths::write_report(dir, &project_id, &clock::now_compact(), &rendered.pdf)?;
        Ok::<_, report::ReportError>(ReportResult {
            path: dest.to_string_lossy().into_owned(),
            bytes: rendered.pdf.len(),
            pages: rendered.pages,
            stats: input.stats,
            compile_ms: rendered.compile_ms,
            pdf_ms: rendered.pdf_ms,
            total_ms: started.elapsed().as_millis() as u64,
        })
    })
    .await?;
    // Local measurement builds (D-026): a release build is not debuggable, so `run-as` cannot read
    // `reports/`; a copy in the Documents dir can be pulled with adb. Runs after `total_ms`.
    #[cfg(feature = "dev-seed")]
    {
        let src = std::path::Path::new(&result.path);
        if let (Ok(out), Some(name)) = (crate::devlog::log_dir(&app_out), src.file_name()) {
            if let Err(e) = std::fs::copy(src, out.join(name)) {
                log::warn!("dev-seed: report copy failed: {e}");
            }
        }
    }
    Ok(result)
}
