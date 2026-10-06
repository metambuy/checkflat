//! Per-project fraction list (D-020). Codes are unique per project case-insensitively; the row's
//! `code` is the canonical spelling every observation stores. A fraction that has issued a number
//! (`next_seq > 1`) or is used by an observation can never be deleted, so numbers are not reused.
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::Fraction;
use crate::refs;
use crate::{clock, ids, CoreError, Result};

fn row_to_fraction(r: &Row<'_>) -> rusqlite::Result<Fraction> {
    Ok(Fraction {
        id: r.get("id")?,
        project_id: r.get("project_id")?,
        code: r.get("code")?,
        next_seq: r.get("next_seq")?,
        created_at: r.get("created_at")?,
    })
}

pub fn list(conn: &Connection, project_id: &str) -> Result<Vec<Fraction>> {
    let mut stmt = conn.prepare("SELECT * FROM fraction WHERE project_id = ?1 ORDER BY code")?;
    let rows = stmt.query_map([project_id], row_to_fraction)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Fraction> {
    conn.query_row("SELECT * FROM fraction WHERE id = ?1", [id], row_to_fraction)
        .optional()?
        .ok_or(CoreError::NotFound)
}

/// The project's fraction with this code, compared case-insensitively in Rust (full Unicode:
/// `Ático` = `ático`; SQLite's NOCASE only folds ASCII, so the DB constraint alone is not enough).
/// The list is per project and short, so it is scanned rather than queried.
pub fn find(conn: &Connection, project_id: &str, code: &str) -> Result<Option<Fraction>> {
    let wanted = code.to_lowercase();
    Ok(list(conn, project_id)?.into_iter().find(|f| f.code.to_lowercase() == wanted))
}

/// Insert-or-get: typing a new fraction adds it. Returns the stored row, so callers use its
/// canonical `code`. Runs inside the caller's transaction when part of an observation save.
pub fn ensure(conn: &Connection, project_id: &str, code: &str) -> Result<Fraction> {
    let code = refs::validate_code(code)?;
    if code.is_empty() {
        return Err(CoreError::Validation("fraction code is empty".into()));
    }
    if let Some(f) = find(conn, project_id, &code)? {
        return Ok(f);
    }
    let id = ids::new_id();
    conn.execute(
        "INSERT INTO fraction(id, project_id, code, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, project_id, code, clock::now_iso()],
    )?;
    get(conn, &id)
}

/// Add from the settings screen (same as [`ensure`]; an existing code returns that row).
pub fn add(conn: &Connection, project_id: &str, code: &str) -> Result<Fraction> {
    let tx = conn.unchecked_transaction()?;
    if !tx.query_row("SELECT 1 FROM project WHERE id = ?1", [project_id], |_| Ok(())).optional()?.is_some() {
        return Err(CoreError::NotFound);
    }
    let f = ensure(&tx, project_id, code)?;
    tx.commit()?;
    Ok(f)
}

/// Deletes a fraction that never issued a number and no observation uses; otherwise
/// `FractionInUse` (D-017: numbers are never reused, so neither is the fraction's counter).
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let f = get(&tx, id)?;
    let used: i64 = tx.query_row(
        "SELECT count(*) FROM observation WHERE project_id = ?1 AND fraction = ?2",
        params![f.project_id, f.code],
        |r| r.get(0),
    )?;
    if used > 0 || f.next_seq > 1 {
        return Err(CoreError::FractionInUse { count: used });
    }
    tx.execute("DELETE FROM fraction WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}
