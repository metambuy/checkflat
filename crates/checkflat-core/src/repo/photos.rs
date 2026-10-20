//! Photo rows and files. A photo enters through `photos::stage` (a JPEG in `tmp/`) and becomes a
//! row + `projects/<pid>/photos/<photo_id>.jpg` in [`attach`], inside the transaction that saves
//! its observation. Rows go before files on delete (D-010): a failed removal leaves an orphan the
//! sweep quarantines, never a row pointing at nothing.
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, Row};

use crate::annotation::to_column;
use crate::models::Photo;
use crate::paths::{photo_file, RelPath};
use crate::photos::{staged_path, PhotoAnnotation, PhotoInput};
use crate::{clock, ids, CoreError, Result};

fn row_to_photo(r: &Row<'_>) -> rusqlite::Result<Photo> {
    let fp: String = r.get("file_path")?;
    Ok(Photo {
        id: r.get("id")?,
        observation_id: r.get("observation_id")?,
        visit_id: r.get("visit_id")?,
        file_path: RelPath::new(&fp).map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        taken_at: r.get("taken_at")?,
        annotation_json: r.get("annotation_json")?,
        created_at: r.get("created_at")?,
    })
}

/// Photos of an observation in the order they were added.
pub fn list_for_observation(conn: &Connection, observation_id: &str) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare("SELECT * FROM photo WHERE observation_id = ?1 ORDER BY created_at, id")?;
    let rows = stmt.query_map([observation_id], row_to_photo)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The first photo (the one [`list_for_observation`] returns first) of every observation of the
/// project that has one, by observation id: the observation list's thumbnails. One query, no
/// per-observation round trips.
pub fn first_for_project(conn: &Connection, project_id: &str) -> Result<HashMap<String, Photo>> {
    let mut stmt = conn.prepare(
        "SELECT p.* FROM photo p JOIN observation o ON o.id = p.observation_id
         WHERE o.project_id = ?1
           AND p.id = (SELECT q.id FROM photo q WHERE q.observation_id = p.observation_id ORDER BY q.created_at, q.id LIMIT 1)",
    )?;
    let rows = stmt.query_map([project_id], row_to_photo)?;
    let mut first = HashMap::new();
    for photo in rows {
        let photo = photo?;
        first.insert(photo.observation_id.clone(), photo);
    }
    Ok(first)
}

/// A staged file moved into the project: `(destination, where it came from)`.
pub type Moved = Vec<(PathBuf, PathBuf)>;

/// Move each staged photo into `projects/<project_id>/photos/` and insert its row (with its
/// annotation, validated here). Must run inside the transaction that saves the observation. On
/// error every file already moved goes back to staging; if the caller's commit fails afterwards it
/// must call [`revert`] with the result.
pub fn attach(
    tx: &Connection,
    data_dir: &Path,
    project_id: &str,
    observation_id: &str,
    photos: &[PhotoInput],
    now: &str,
) -> Result<Moved> {
    let mut moved: Moved = Vec::new();
    let result = (|| -> Result<()> {
        for p in photos {
            let staged = staged_path(data_dir, &p.token)?;
            if !staged.is_file() {
                return Err(CoreError::NotFound);
            }
            let annotation_json = to_column(p.annotation.as_ref())?;
            let id = ids::new_id();
            let rel = photo_file(project_id, &id);
            let dest = rel.resolve(data_dir);
            std::fs::create_dir_all(dest.parent().expect("photos dir"))?;
            std::fs::rename(&staged, &dest)?;
            moved.push((dest, staged));
            let taken_at = clock::parse_iso(&p.taken_at).map(clock::format_iso).unwrap_or_else(|| now.to_string());
            tx.execute(
                "INSERT INTO photo(id, observation_id, file_path, taken_at, annotation_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, observation_id, rel.as_str(), taken_at, annotation_json, now],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(moved),
        Err(e) => {
            revert(&moved);
            Err(e)
        }
    }
}

/// The edit sheet's annotation changes, validated and serialised for the column before anything is
/// touched: every id must be one of `existing` (else `Validation`, like a foreign removal); ids
/// in `removed` are dropped (the photo goes anyway).
pub fn prepare_annotations(
    annotations: &[PhotoAnnotation],
    existing: &[Photo],
    removed: &[&Photo],
) -> Result<Vec<(String, Option<String>)>> {
    annotations
        .iter()
        .filter(|a| !removed.iter().any(|r| r.id == a.photo_id))
        .map(|a| {
            if !existing.iter().any(|p| p.id == a.photo_id) {
                return Err(CoreError::Validation(format!("photo {} does not belong to this observation", a.photo_id)));
            }
            Ok((a.photo_id.clone(), to_column(a.annotation.as_ref())?))
        })
        .collect()
}

/// Writes what [`prepare_annotations`] produced, inside the save transaction.
pub fn set_annotations(tx: &Connection, observation_id: &str, prepared: &[(String, Option<String>)]) -> Result<()> {
    for (id, json) in prepared {
        tx.execute("UPDATE photo SET annotation_json = ?3 WHERE id = ?1 AND observation_id = ?2", params![id, observation_id, json])?;
    }
    Ok(())
}

/// Put moved files back into staging (best effort; a failure leaves an orphan for the sweep).
pub fn revert(moved: &Moved) {
    for (dest, staged) in moved {
        if let Err(e) = std::fs::rename(dest, staged) {
            log::warn!("photo {} could not be returned to staging: {e}", dest.display());
        }
    }
}

/// Remove photo files after their rows are gone. A missing file is fine; other failures are logged.
pub fn remove_files(data_dir: &Path, paths: &[RelPath]) {
    for p in paths {
        match std::fs::remove_file(p.resolve(data_dir)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => log::warn!("photo {p} deleted but its file could not be removed: {e}"),
        }
    }
}
