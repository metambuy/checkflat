//! Report content. [`collect`] reads everything the report needs from the database (fast, under
//! the connection lock); [`prepare`] turns it into the template's data plus the image files
//! (snapshots from the tile cache, photos through the annotation hook), with no database access.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use checkflat_core::i18n::{t, Lang};
use checkflat_core::models::{Observation, Photo, Plan, Project};
use checkflat_core::paths::plan_tiles_dir;
use checkflat_core::repo::tiles::{self, TileManifest};
use checkflat_core::repo::{observations, photos, plans, projects};
use checkflat_core::rusqlite::Connection;
use serde::Serialize;

use crate::snapshot;
use crate::time::Clock;
use crate::{annotate, Result};

pub struct PlanSource {
    pub plan: Plan,
    pub tiles_dir: PathBuf,
    /// Finished levels on disk (verified by `tiles::info`); `None` when the plan was never
    /// prepared on this device.
    pub manifest: Option<TileManifest>,
}

/// Everything read from the database for one report.
pub struct ReportSource {
    pub project: Project,
    pub lang: Lang,
    pub clock: Clock,
    pub plans: Vec<PlanSource>,
    /// Open observations, newest first, each with its photos in the order they were added.
    pub observations: Vec<(Observation, Vec<Photo>)>,
}

/// `zone`: IANA time-zone name of the device; `utc_offset_min`: its current offset (fallback).
pub fn collect(conn: &Connection, data_dir: &Path, project_id: &str, lang: Lang, zone: &str, utc_offset_min: i32) -> Result<ReportSource> {
    let project = projects::get(conn, project_id)?;
    let plans = plans::list_for_project(conn, project_id)?
        .into_iter()
        .map(|plan| {
            let manifest = tiles::info(conn, data_dir, &plan.id)?.manifest;
            let tiles_dir = plan_tiles_dir(&plan.project_id, &plan.id).resolve(data_dir);
            Ok(PlanSource { plan, tiles_dir, manifest })
        })
        .collect::<Result<Vec<_>>>()?;
    let observations = observations::list_open_for_project(conn, project_id)?
        .into_iter()
        .map(|o| Ok((photos::list_for_observation(conn, &o.id)?, o)))
        .map(|r: Result<_>| r.map(|(p, o)| (o, p)))
        .collect::<Result<Vec<_>>>()?;
    Ok(ReportSource { project, lang, clock: Clock::new(zone, utc_offset_min), plans, observations })
}

// ---- what the template sees (`sys.inputs.data`, JSON) ----

#[derive(Debug, Clone, Serialize)]
pub struct ReportData {
    pub lang: &'static str,
    pub region: Option<&'static str>,
    pub title: String,
    pub date: String,
    pub project: ProjectData,
    pub labels: Labels,
    pub observations: Vec<ObservationData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectData {
    pub name: String,
    pub address: String,
    pub code: String,
}

/// Translated in Rust (`i18n::t`), so the template never reads the string files.
#[derive(Debug, Clone, Serialize)]
pub struct Labels {
    pub date: String,
    pub id: String,
    pub plan: String,
    pub description: String,
    pub images: String,
    pub no_snapshot: String,
    pub no_observations: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservationData {
    #[serde(rename = "ref")]
    pub display_ref: String,
    pub plan_title: String,
    pub description: String,
    pub crop: Option<CropData>,
    pub overview: Option<OverviewData>,
    pub photos: Vec<PhotoData>,
}

/// The crop image and where the pin sits in it (fractions of its width/height).
#[derive(Debug, Clone, Serialize)]
pub struct CropData {
    pub file: String,
    pub aspect: f64,
    pub pin_x: f64,
    pub pin_y: f64,
}

/// The plan's overview image and the crop window to outline on it (fractions).
#[derive(Debug, Clone, Serialize)]
pub struct OverviewData {
    pub file: String,
    pub aspect: f64,
    pub box_x: f64,
    pub box_y: f64,
    pub box_w: f64,
    pub box_h: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotoData {
    pub file: String,
    /// `n. dd.mm.yyyy hh:mm`, local time.
    pub caption: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub observations: usize,
    pub photos: usize,
    /// Photos whose file could not be read or decoded; the report leaves them out.
    pub skipped_photos: usize,
    /// Observations without a plan snapshot (plan not prepared on this device).
    pub without_snapshot: usize,
    pub snapshot_ms: u64,
    pub photo_ms: u64,
}

/// Template data plus the virtual files it references (`img/…`).
pub struct ReportInput {
    pub data: ReportData,
    pub files: Vec<(String, Vec<u8>)>,
    pub stats: Stats,
}

fn labels(lang: Lang) -> Labels {
    let s = |k: &str| t(lang, k, &[]);
    Labels {
        date: s("report.date"),
        id: s("report.id"),
        plan: s("report.plan"),
        description: s("report.description"),
        images: s("report.images"),
        no_snapshot: s("report.no_snapshot"),
        no_observations: s("report.no_observations"),
    }
}

/// Image work, no database. Overviews are made once per plan and shared by its observations.
pub fn prepare(data_dir: &Path, src: ReportSource) -> Result<ReportInput> {
    let ReportSource { project, lang, clock, plans, observations } = src;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stats = Stats { observations: observations.len(), ..Stats::default() };
    let plans: HashMap<&str, &PlanSource> = plans.iter().map(|p| (p.plan.id.as_str(), p)).collect();
    let mut overviews: HashMap<String, Option<(String, f64)>> = HashMap::new();

    let mut out = Vec::with_capacity(observations.len());
    for (i, (obs, photos)) in observations.iter().enumerate() {
        let plan = plans.get(obs.plan_id.as_str());
        let plan_title = plan.map(|p| p.plan.title.clone()).unwrap_or_default();

        let started = Instant::now();
        let (crop, overview) = match plan.and_then(|p| p.manifest.as_ref().map(|m| (p, m))) {
            Some((p, manifest)) => {
                let win = snapshot::window(obs.x_norm, obs.y_norm, manifest.width_pt, manifest.height_pt);
                let crop = snapshot::crop_level(manifest)
                    .map(|level| snapshot::crop(&p.tiles_dir, level, &win))
                    .transpose()
                    .unwrap_or_else(|e| {
                        log::warn!("observation {}: crop failed ({e}); shown without", obs.display_ref);
                        None
                    })
                    .map(|s| {
                        let (pin_x, pin_y) = snapshot::pin_in_window(&win, obs.x_norm, obs.y_norm);
                        let file = format!("img/crop-{i}.png");
                        let data = CropData { file: file.clone(), aspect: s.aspect(), pin_x, pin_y };
                        files.push((file, s.png));
                        data
                    });
                let overview = overviews
                    .entry(p.plan.id.clone())
                    .or_insert_with(|| match snapshot::overview(&p.tiles_dir, manifest) {
                        Ok(s) => {
                            let file = format!("img/overview-{}.png", files.len());
                            let aspect = s.aspect();
                            files.push((file.clone(), s.png));
                            Some((file, aspect))
                        }
                        Err(e) => {
                            log::warn!("plan {}: overview failed ({e})", p.plan.title);
                            None
                        }
                    })
                    .clone()
                    .map(|(file, aspect)| OverviewData { file, aspect, box_x: win.x, box_y: win.y, box_w: win.w, box_h: win.h });
                (crop, overview)
            }
            None => (None, None),
        };
        if crop.is_none() {
            stats.without_snapshot += 1;
        }
        stats.snapshot_ms += started.elapsed().as_millis() as u64;

        let started = Instant::now();
        let mut photo_data = Vec::with_capacity(photos.len());
        for (j, photo) in photos.iter().enumerate() {
            let prepared = std::fs::read(photo.file_path.resolve(data_dir))
                .map_err(crate::ReportError::Io)
                .and_then(|bytes| Ok(annotate::flatten(&bytes, photo.annotation_json.as_deref())?.into_owned()))
                .and_then(|bytes| annotate::report_photo(&bytes));
            match prepared {
                Ok(jpeg) => {
                    let file = format!("img/photo-{i}-{j}.jpg");
                    photo_data.push(PhotoData { file: file.clone(), caption: format!("{}. {}", j + 1, clock.date_time(&photo.taken_at)) });
                    files.push((file, jpeg));
                    stats.photos += 1;
                }
                Err(e) => {
                    log::warn!("observation {}: photo {} skipped ({e})", obs.display_ref, photo.id);
                    stats.skipped_photos += 1;
                }
            }
        }
        stats.photo_ms += started.elapsed().as_millis() as u64;

        out.push(ObservationData {
            display_ref: obs.display_ref.clone(),
            plan_title,
            description: obs.description.clone(),
            crop,
            overview,
            photos: photo_data,
        });
    }

    let data = ReportData {
        lang: lang.code(),
        region: match lang {
            Lang::Pt => Some("PT"),
            Lang::En => None,
        },
        title: t(lang, "report.title", &[]),
        date: clock.today(),
        project: ProjectData { name: project.name, address: project.address, code: project.code },
        labels: labels(lang),
        observations: out,
    };
    Ok(ReportInput { data, files, stats })
}
