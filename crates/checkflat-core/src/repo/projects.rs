use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Project, ProjectSummary};
use crate::paths::project_dir;
use crate::refs::{self, Scope, Template};
use crate::repo::{fractions, observations};
use crate::{clock, ids, CoreError, Result};

fn clean_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(CoreError::Validation("name is empty".into()));
    }
    Ok(n.to_string())
}

const SELECT_PROJECT: &str = "SELECT p.*,
    (p.next_seq > 1 OR EXISTS (SELECT 1 FROM fraction f WHERE f.project_id = p.id AND f.next_seq > 1)) AS scope_locked,
    (SELECT count(*) FROM observation o WHERE o.project_id = p.id) AS observation_count
    FROM project p WHERE p.id = ?1";

fn row_to_project(r: &Row<'_>) -> rusqlite::Result<Project> {
    let scope: String = r.get("seq_scope")?;
    Ok(Project {
        id: r.get("id")?,
        name: r.get("name")?,
        address: r.get("address")?,
        code: r.get("code")?,
        ref_template: r.get("ref_template")?,
        seq_scope: Scope::parse(&scope).unwrap_or(Scope::Project), // CHECK constraint keeps it valid
        next_seq: r.get("next_seq")?,
        scope_locked: r.get("scope_locked")?,
        observation_count: r.get("observation_count")?,
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
    conn.query_row(SELECT_PROJECT, [id], row_to_project)
        .optional()?
        .ok_or(CoreError::NotFound)
}

pub fn create(conn: &Connection, name: &str, address: &str) -> Result<Project> {
    let name = clean_name(name)?;
    let now = clock::now_iso();
    let id = ids::new_id();
    conn.execute(
        "INSERT INTO project(id, name, address, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
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

/// Validated ref settings: the code, the parsed template (and its trimmed source — the string
/// that gets stored, so the preview and the saved value never differ) and the scope.
struct RefSettings {
    code: String,
    template: Template,
    template_src: String,
    scope: Scope,
}

fn check_ref_settings(code: &str, template: &str, scope: Scope) -> Result<RefSettings> {
    let code = refs::validate_code(code)?;
    let template_src = template.trim().to_string();
    let template = Template::parse(&template_src)?;
    template.validate_for(scope)?;
    Ok(RefSettings { code, template, template_src, scope })
}

/// Project code, ref template and sequence scope (project settings screen). The scope cannot
/// change once a number has been issued (`scope_locked`): `observation.seq_key` is derived from
/// it and is never rewritten.
pub fn update_ref_settings(conn: &Connection, id: &str, code: &str, template: &str, scope: Scope) -> Result<Project> {
    let s = check_ref_settings(code, template, scope)?;
    let tx = conn.unchecked_transaction()?;
    let current = get(&tx, id)?;
    if s.scope != current.seq_scope && current.scope_locked {
        return Err(CoreError::ScopeLocked);
    }
    tx.execute(
        "UPDATE project SET code = ?2, ref_template = ?3, seq_scope = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, s.code, s.template_src, s.scope.as_str(), clock::now_iso()],
    )?;
    tx.commit()?;
    get(conn, id)
}

/// The ref the next observation in `fraction` would get under the given (unsaved) settings —
/// validated like [`update_ref_settings`], nothing written. Drives the live preview.
pub fn preview_ref(conn: &Connection, id: &str, code: &str, template: &str, scope: Scope, fraction: &str) -> Result<String> {
    let s = check_ref_settings(code, template, scope)?;
    let typed = refs::validate_code(fraction)?;
    let project = get(conn, id)?;
    // An existing fraction renders with its stored spelling, as the save would store it.
    let fraction = fractions::find(conn, id, &typed)?.map_or(typed, |f| f.code);
    let seq = observations::peek_seq(conn, &project, scope, &fraction)?;
    Ok(s.template.render(&s.code, &fraction, seq))
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
