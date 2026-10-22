//! Pins (observations placed on a plan). `create_observation` is the save of the observation
//! sheet; the ref is computed in the core and returned as `ref` / `marker`.
use std::collections::HashMap;

use checkflat_core::annotation::{self, Annotation};
use checkflat_core::models::Observation;
use checkflat_core::photos::{self, PhotoAnnotation, PhotoInput};
use checkflat_core::repo::{observations, photos as photo_repo, plans, projects};
use serde::Serialize;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

/// One row of the observation list: the observation, its plan's title and its first photo.
/// `photo_id` is that photo (`None` for an observation migrated from Sprint 3a that has none);
/// `thumb_path` is its small list thumbnail (absolute, for `convertFileSrc`) **if it has been made**
/// — otherwise `None` and the page asks for it with `ensure_thumbnails`, so the list never waits
/// for image work — and `thumb_annotation` the shapes to draw over it (never baked into the file).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationRow {
    #[serde(flatten)]
    pub observation: Observation,
    pub plan_title: String,
    pub photo_id: Option<String>,
    pub thumb_path: Option<String>,
    pub thumb_annotation: Option<Annotation>,
}

/// The project's open observations, newest first (the report's order), for the list screen.
#[tauri::command]
pub async fn list_observations(app: AppHandle, project_id: String) -> AppResult<Vec<ObservationRow>> {
    run_db(app, move |c, dir| {
        let titles: HashMap<String, String> = plans::list_for_project(c, &project_id)?.into_iter().map(|p| (p.id, p.title)).collect();
        let mut first = photo_repo::first_for_project(c, &project_id)?;
        Ok(observations::list_open_for_project(c, &project_id)?
            .into_iter()
            .map(|o| {
                let photo = first.remove(&o.id);
                ObservationRow {
                    plan_title: titles.get(&o.plan_id).cloned().unwrap_or_default(),
                    photo_id: photo.as_ref().map(|p| p.id.clone()),
                    thumb_path: photo
                        .as_ref()
                        .and_then(|p| photos::thumb_path(dir, &o.project_id, &p.id))
                        .map(|t| t.to_string_lossy().into_owned()),
                    thumb_annotation: photo.and_then(|p| annotation::parse_stored(p.annotation_json.as_deref())),
                    observation: o,
                }
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn list_pins(app: AppHandle, plan_id: String) -> AppResult<Vec<Observation>> {
    run_db(app, move |c, _| observations::list_for_plan(c, &plan_id)).await
}

#[tauri::command]
pub async fn create_observation(
    app: AppHandle,
    plan_id: String,
    x: f64,
    y: f64,
    fraction: String,
    description: String,
    photos: Vec<PhotoInput>,
) -> AppResult<Observation> {
    run_db(app, move |c, dir| observations::create_observation(c, dir, &plan_id, x, y, &fraction, &description, &photos)).await
}

/// Edit mode of the sheet: description, photos and the kept photos' annotations (fraction and ref
/// are fixed).
#[tauri::command]
pub async fn update_observation(
    app: AppHandle,
    id: String,
    description: String,
    photos: Vec<PhotoInput>,
    remove_photo_ids: Vec<String>,
    annotations: Vec<PhotoAnnotation>,
) -> AppResult<Observation> {
    run_db(app, move |c, dir| observations::update_observation(c, dir, &id, &description, &photos, &remove_photo_ids, &annotations)).await
}

/// The ref the next observation in `fraction` would get with the project's stored settings.
#[tauri::command]
pub async fn preview_ref(app: AppHandle, project_id: String, fraction: String) -> AppResult<String> {
    run_db(app, move |c, _| {
        let p = projects::get(c, &project_id)?;
        projects::preview_ref(c, &project_id, &p.code, &p.ref_template, p.seq_scope, &fraction)
    })
    .await
}

#[tauri::command]
pub async fn move_pin(app: AppHandle, id: String, x: f64, y: f64) -> AppResult<Observation> {
    run_db(app, move |c, _| observations::move_pin(c, &id, x, y)).await
}

#[tauri::command]
pub async fn delete_pin(app: AppHandle, id: String) -> AppResult<()> {
    run_db(app, move |c, dir| observations::delete_pin(c, dir, &id)).await
}
