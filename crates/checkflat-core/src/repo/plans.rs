//! Plan import in two steps so the UI can confirm a title before a row exists, and the source
//! is read exactly once: `stage` streams the source into `tmp/<token>.pdf` and inspects it;
//! `import` renames the staged file into the project and inserts the row.
use std::io::{self, Read, Write};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::models::Plan;
use crate::paths::{plan_dir, plan_file, staging_file, RelPath, PLANS_DIR, TMP_DIR};
use crate::pdf::{self, PdfInfo};
use crate::{clock, ids, CoreError, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Staged {
    pub token: String,
    pub info: PdfInfo,
}

/// Stream `source` into the staging area and inspect it. On any error the staging file is
/// removed. Rejects PDFs with more than one page (one plan per file).
pub fn stage(data_dir: &Path, mut source: impl Read) -> Result<Staged> {
    std::fs::create_dir_all(data_dir.join(TMP_DIR))?;
    let token = ids::new_id();
    let path = staging_file(&token).resolve(data_dir);
    let result = (|| {
        let mut file = std::fs::File::create(&path)?;
        io::copy(&mut source, &mut file)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        let info = pdf::inspect_file(&path)?;
        if info.page_count != 1 {
            return Err(CoreError::MultiPage { pages: info.page_count });
        }
        Ok(Staged { token, info })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

/// Discard a staged file (user cancelled the title step).
pub fn discard_staged(data_dir: &Path, token: &str) -> Result<()> {
    if !ids::is_id(token) {
        return Err(CoreError::Validation("invalid token".into()));
    }
    match std::fs::remove_file(staging_file(token).resolve(data_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Default title for a display name: extension stripped (case-insensitively), trimmed.
pub fn default_title(display_name: &str) -> String {
    let name = display_name.trim();
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("pdf") && !stem.is_empty() => stem,
        _ => name,
    };
    stem.trim().to_string()
}

fn clean_title(title: &str) -> Result<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(CoreError::Validation("title is empty".into()));
    }
    Ok(t.to_string())
}

/// Move the staged file into `projects/<project_id>/plans/<plan_id>.pdf` and insert the row.
/// The rename is atomic (same filesystem); if the insert fails the file is removed. A crash
/// between rename and commit leaves an orphan that the next startup sweep quarantines.
pub fn import(conn: &Connection, data_dir: &Path, project_id: &str, token: &str, title: &str) -> Result<Plan> {
    let title = clean_title(title)?;
    if !ids::is_id(token) {
        return Err(CoreError::Validation("invalid token".into()));
    }
    let staged = staging_file(token).resolve(data_dir);
    if !staged.is_file() {
        return Err(CoreError::NotFound);
    }
    let exists: bool = conn
        .query_row("SELECT 1 FROM project WHERE id = ?1", [project_id], |_| Ok(true))
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Err(CoreError::NotFound);
    }
    // Re-inspect the staged file: it is the source of truth for the stored dimensions.
    let info = pdf::inspect_file(&staged)?;
    if info.page_count != 1 {
        return Err(CoreError::MultiPage { pages: info.page_count });
    }

    let plan_id = ids::new_id();
    let rel = plan_file(project_id, &plan_id);
    let dest = rel.resolve(data_dir);
    std::fs::create_dir_all(dest.parent().expect("plans dir"))?;
    std::fs::rename(&staged, &dest)?;

    let inserted = insert_row(conn, &plan_id, project_id, &rel, &title, &info);
    if let Err(e) = inserted {
        let _ = std::fs::remove_file(&dest);
        return Err(e);
    }
    get(conn, &plan_id)
}

fn insert_row(conn: &Connection, plan_id: &str, project_id: &str, rel: &RelPath, title: &str, info: &PdfInfo) -> Result<()> {
    let now = clock::now_iso();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO plan(id, project_id, file_path, title, width_pt, height_pt, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![plan_id, project_id, rel.as_str(), title, info.width_pt, info.height_pt, now],
    )?;
    tx.execute(
        "UPDATE project SET updated_at = ?2 WHERE id = ?1",
        params![project_id, now],
    )?;
    tx.commit()?;
    Ok(())
}

fn row_to_plan(r: &Row<'_>) -> rusqlite::Result<Plan> {
    let fp: String = r.get("file_path")?;
    Ok(Plan {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        file_path: RelPath::new(&fp).map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        title: r.get("title")?,
        width_pt: r.get("width_pt")?,
        height_pt: r.get("height_pt")?,
        created_at: r.get("created_at")?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<Plan> {
    conn.query_row("SELECT * FROM plan WHERE id = ?1", [id], row_to_plan)
        .optional()?
        .ok_or(CoreError::NotFound)
}

pub fn list_for_project(conn: &Connection, project_id: &str) -> Result<Vec<Plan>> {
    let mut stmt = conn.prepare("SELECT * FROM plan WHERE project_id = ?1 ORDER BY created_at, title")?;
    let rows = stmt.query_map([project_id], row_to_plan)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn rename(conn: &Connection, id: &str, title: &str) -> Result<Plan> {
    let title = clean_title(title)?;
    let n = conn.execute("UPDATE plan SET title = ?2 WHERE id = ?1", params![id, title])?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

/// Delete a plan, its file and its tile cache. Refused while observations reference it (the NO ACTION FK is the
/// safety net behind this explicit check).
pub fn delete(conn: &Connection, data_dir: &Path, id: &str) -> Result<()> {
    let plan = get(conn, id)?;
    let count: i64 = conn.query_row("SELECT count(*) FROM observation WHERE plan_id = ?1", [id], |r| r.get(0))?;
    if count > 0 {
        return Err(CoreError::PlanHasObservations { count });
    }
    conn.execute("DELETE FROM plan WHERE id = ?1", [id])?;
    match std::fs::remove_file(plan.file_path.resolve(data_dir)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("plan {id} deleted but its file could not be removed: {e}"),
    }
    // Tile cache (D-014); a leftover is quarantined by the next sweep.
    match std::fs::remove_dir_all(plan_dir(&plan.project_id, id).resolve(data_dir)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("plan {id} deleted but its tile cache could not be removed: {e}"),
    }
    Ok(())
}

pub fn plans_dir_name() -> &'static str {
    PLANS_DIR
}
