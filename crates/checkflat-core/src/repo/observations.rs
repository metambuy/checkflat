//! Observations as pins. A draft pin exists only in the UI; [`create_observation`] is the save of
//! the observation sheet and the only place a sequence number is taken, so a cancelled draft never
//! leaves a gap. How the number is chosen lives in [`assign_ref`] alone (D-020): per project or
//! per fraction, from a counter that only ever grows, so deleted numbers are never reused.
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Observation, Project};
use crate::paths::RelPath;
use crate::photos::PhotoInput;
use crate::refs::{self, Scope, Template};
use crate::repo::{fractions, photos, plans, projects};
use crate::{clock, ids, CoreError, Result};

fn check_pos(x: f64, y: f64) -> Result<()> {
    let ok = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
    if ok(x) && ok(y) {
        Ok(())
    } else {
        Err(CoreError::Validation(format!("pin position out of range: ({x}, {y})")))
    }
}

/// Observation columns joined with the project's ref settings, so every row renders its ref.
const SELECT: &str = "SELECT o.*, p.code AS p_code, p.ref_template AS p_template, p.seq_scope AS p_scope
    FROM observation o JOIN project p ON p.id = o.project_id";

fn row_to_observation(r: &Row<'_>) -> rusqlite::Result<Observation> {
    let code: String = r.get("p_code")?;
    let template: String = r.get("p_template")?;
    let scope: String = r.get("p_scope")?;
    let fraction: String = r.get("fraction")?;
    let seq: i64 = r.get("seq")?;
    // Stored templates are validated on save; a bad one must still never break a read.
    let template = Template::parse(&template).unwrap_or_else(|e| {
        log::warn!("project {}: stored ref template rejected ({e}); using the default", code);
        Template::parse(refs::DEFAULT_TEMPLATE).expect("default template parses")
    });
    let scope = Scope::parse(&scope).unwrap_or(Scope::Project);
    Ok(Observation {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        plan_id: r.get("plan_id")?,
        display_ref: template.render(&code, &fraction, seq),
        marker: refs::marker(scope, &template, &fraction, seq),
        fraction,
        seq,
        x_norm: r.get("x_norm")?,
        y_norm: r.get("y_norm")?,
        description: r.get("description")?,
        created_visit_id: r.get("created_visit_id")?,
        resolved_visit_id: r.get("resolved_visit_id")?,
        resolved_at: r.get("resolved_at")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<Observation> {
    conn.query_row(&format!("{SELECT} WHERE o.id = ?1"), [id], row_to_observation)
        .optional()?
        .ok_or(CoreError::NotFound)
}

/// Pins of a plan in display order: by fraction (when numbers run per fraction), then sequence.
pub fn list_for_plan(conn: &Connection, plan_id: &str) -> Result<Vec<Observation>> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE o.plan_id = ?1 ORDER BY o.seq_key, o.seq"))?;
    let rows = stmt.query_map([plan_id], row_to_observation)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The number [`assign_ref`] would give next for `fraction` under `scope` (read-only; the
/// fraction need not exist yet). The number is never below an existing seq + 1 in the same scope,
/// so a seq edited upwards or an imported row cannot collide.
pub fn peek_seq(conn: &Connection, project: &Project, scope: Scope, fraction: &str) -> Result<i64> {
    let seq_key = scope.seq_key(fraction);
    let counter = match scope {
        Scope::Project => project.next_seq,
        Scope::Fraction => {
            if fraction.is_empty() {
                return Err(CoreError::FractionRequired);
            }
            fractions::find(conn, &project.id, fraction)?.map_or(1, |f| f.next_seq)
        }
    };
    let highest: i64 = conn.query_row(
        "SELECT COALESCE(max(seq) + 1, 1) FROM observation WHERE project_id = ?1 AND seq_key = ?2",
        params![project.id, seq_key],
        |r| r.get(0),
    )?;
    Ok(counter.max(highest))
}

/// Takes the next sequence number for `fraction` under the project's scope and advances that
/// scope's counter (`project.next_seq` or `fraction.next_seq`). The only writer of `seq`,
/// `seq_key` and the counters; must run inside the transaction that inserts the observation.
/// Issuing a number touches `project.updated_at` in both scopes (the projects list is ordered
/// by it). Returns `(seq, seq_key)`.
fn assign_ref(tx: &Connection, project: &Project, fraction: &str, now: &str) -> Result<(i64, String)> {
    let scope = project.seq_scope;
    let seq = peek_seq(tx, project, scope, fraction)?;
    match scope {
        Scope::Project => {
            tx.execute(
                "UPDATE project SET next_seq = ?2, updated_at = ?3 WHERE id = ?1",
                params![project.id, seq + 1, now],
            )?;
        }
        Scope::Fraction => {
            let n = tx.execute(
                "UPDATE fraction SET next_seq = ?3 WHERE project_id = ?1 AND code = ?2",
                params![project.id, fraction, seq + 1],
            )?;
            if n != 1 {
                return Err(CoreError::Validation(format!("fraction {fraction} does not exist")));
            }
            tx.execute("UPDATE project SET updated_at = ?2 WHERE id = ?1", params![project.id, now])?;
        }
    }
    Ok((seq, scope.seq_key(fraction).to_string()))
}

/// Save of the observation sheet: in one transaction add the fraction if it is new
/// ([`fractions::ensure`], whose canonical code is what gets stored), assign the ref
/// ([`assign_ref`]), insert the observation and move its staged photos in
/// ([`photos::attach`]); at least one photo is required. On any error nothing changes (staged
/// files are back in `tmp/`).
#[allow(clippy::too_many_arguments)]
pub fn create_observation(
    conn: &Connection,
    data_dir: &Path,
    plan_id: &str,
    x: f64,
    y: f64,
    fraction: &str,
    description: &str,
    new_photos: &[PhotoInput],
) -> Result<Observation> {
    if new_photos.is_empty() {
        return Err(CoreError::PhotoRequired);
    }
    check_pos(x, y)?;
    let fraction = refs::validate_code(fraction)?;
    let plan = plans::get(conn, plan_id)?;
    let id = ids::new_id();
    let now = clock::now_iso();
    let tx = conn.unchecked_transaction()?;
    let project = projects::get(&tx, &plan.project_id)?;
    if project.seq_scope == Scope::Fraction && fraction.is_empty() {
        return Err(CoreError::FractionRequired);
    }
    let fraction = if fraction.is_empty() { fraction } else { fractions::ensure(&tx, &project.id, &fraction)?.code };
    let (seq, seq_key) = assign_ref(&tx, &project, &fraction, &now)?;
    tx.execute(
        "INSERT INTO observation(id, project_id, plan_id, fraction, seq, seq_key, x_norm, y_norm, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![id, project.id, plan_id, fraction, seq, seq_key, x, y, description.trim(), now],
    )?;
    let moved = photos::attach(&tx, data_dir, &project.id, &id, new_photos, &now)?;
    if let Err(e) = tx.commit() {
        photos::revert(&moved);
        return Err(e.into());
    }
    get(conn, &id)
}

pub fn move_pin(conn: &Connection, id: &str, x: f64, y: f64) -> Result<Observation> {
    check_pos(x, y)?;
    let n = conn.execute(
        "UPDATE observation SET x_norm = ?2, y_norm = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, x, y, clock::now_iso()],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

/// Deletes a confirmed pin: the row first (its photo rows cascade), then the photo files. Its
/// number is not reused.
pub fn delete_pin(conn: &Connection, data_dir: &Path, id: &str) -> Result<()> {
    let files: Vec<RelPath> = photos::list_for_observation(conn, id)?.into_iter().map(|p| p.file_path).collect();
    let n = conn.execute("DELETE FROM observation WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    photos::remove_files(data_dir, &files);
    Ok(())
}
