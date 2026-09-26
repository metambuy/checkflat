//! Observations as pins (Sprint 2). A draft pin exists only in the UI; [`create_pin`] is the
//! confirm step and the only place a ref number is taken, so a cancelled draft never leaves a gap.
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::Observation;
use crate::repo::plans;
use crate::{clock, ids, CoreError, Result};

fn check_pos(x: f64, y: f64) -> Result<()> {
    let ok = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
    if ok(x) && ok(y) {
        Ok(())
    } else {
        Err(CoreError::Validation(format!("pin position out of range: ({x}, {y})")))
    }
}

fn row_to_observation(r: &Row<'_>) -> rusqlite::Result<Observation> {
    Ok(Observation {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        plan_id: r.get("plan_id")?,
        ref_no: r.get("ref_no")?,
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
    conn.query_row("SELECT * FROM observation WHERE id = ?1", [id], row_to_observation)
        .optional()?
        .ok_or(CoreError::NotFound)
}

pub fn list_for_plan(conn: &Connection, plan_id: &str) -> Result<Vec<Observation>> {
    let mut stmt = conn.prepare("SELECT * FROM observation WHERE plan_id = ?1 ORDER BY ref_no")?;
    let rows = stmt.query_map([plan_id], row_to_observation)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Confirm a draft pin: in one transaction take the project's next ref number, insert the
/// observation and advance the counter. The number is never below an existing ref + 1, so a ref
/// edited upwards (M5, Sprint 3) cannot collide. On any error nothing changes.
pub fn create_pin(conn: &Connection, plan_id: &str, x: f64, y: f64) -> Result<Observation> {
    check_pos(x, y)?;
    let plan = plans::get(conn, plan_id)?;
    let id = ids::new_id();
    let now = clock::now_iso();
    let tx = conn.unchecked_transaction()?;
    let ref_no: i64 = tx.query_row(
        "SELECT max(p.next_ref_no, COALESCE((SELECT max(ref_no) + 1 FROM observation WHERE project_id = p.id), 1))
         FROM project p WHERE p.id = ?1",
        [&plan.project_id],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, plan.project_id, plan_id, ref_no, x, y, now],
    )?;
    tx.execute(
        "UPDATE project SET next_ref_no = ?2, updated_at = ?3 WHERE id = ?1",
        params![plan.project_id, ref_no + 1, now],
    )?;
    tx.commit()?;
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

/// Deletes a confirmed pin (photos cascade in the DB). Its ref number is not reused.
pub fn delete_pin(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute("DELETE FROM observation WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}
