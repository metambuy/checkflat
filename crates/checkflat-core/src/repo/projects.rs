use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Project, ProjectSummary};
use crate::paths::{project_dir, RelPath};
use crate::{clock, ids, CoreError, Result};

fn clean_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(CoreError::Validation("name is empty".into()));
    }
    Ok(n.to_string())
}

fn row_to_project(r: &Row<'_>) -> rusqlite::Result<Project> {
    let logo: Option<String> = r.get("logo_path")?;
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        address: r.get("address")?,
        logo_path: logo.and_then(|s| RelPath::new(&s).ok()),
        next_ref_no: r.get("next_ref_no")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<ProjectSummary>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.address, p.created_at, p.updated_at,
                (SELECT count(*) FROM plan WHERE plan.project_id = p.id) AS plan_count
         FROM project p ORDER BY p.updated_at DESC, p.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProjectSummary {
            id: r.get("id")?,
            name: r.get("name")?,
            address: r.get("address")?,
            plan_count: r.get("plan_count")?,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Project> {
    conn.query_row("SELECT * FROM project WHERE id = ?1", [id], row_to_project)
        .optional()?
        .ok_or(CoreError::NotFound)
}

pub fn create(conn: &Connection, name: &str, address: &str) -> Result<Project> {
    let name = clean_name(name)?;
    let now = clock::now_iso();
    let id = ids::new_id();
    conn.execute(
        "INSERT INTO project(id, name, address, next_ref_no, created_at, updated_at)
         VALUES (?1, ?2, ?3, 1, ?4, ?4)",
        params![id, name, address.trim(), now],
    )?;
    get(conn, &id)
}

pub fn rename(conn: &Connection, id: &str, name: &str) -> Result<Project> {
    let name = clean_name(name)?;
    let n = conn.execute(
        "UPDATE project SET name = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, name, clock::now_iso()],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

pub fn update_address(conn: &Connection, id: &str, address: &str) -> Result<Project> {
    let n = conn.execute(
        "UPDATE project SET address = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, address.trim(), clock::now_iso()],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

/// Deletes the project row (cascading to plans, visits, observations, photos) and then its
/// directory on disk. DB first: a failed disk removal can never leave rows pointing at nothing;
/// leftovers are quarantined by the next startup sweep.
pub fn delete(conn: &Connection, data_dir: &Path, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM project WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    let dir = project_dir(id).resolve(data_dir);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            log::warn!("project {id} deleted but its directory could not be removed: {e}");
        }
    }
    Ok(())
}
