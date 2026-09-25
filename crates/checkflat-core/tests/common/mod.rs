#![allow(dead_code)]
use rusqlite::{params, Connection};

/// Insert minimal rows for later-sprint tables so cascade tests can exercise them.
pub fn insert_plan(conn: &Connection, id: &str, project_id: &str) {
    conn.execute(
        "INSERT INTO plan(id, project_id, file_path, title, width_pt, height_pt, created_at)
         VALUES (?1, ?2, ?3, 'Plan', 2384, 1684, '2026-09-25T00:00:00.000Z')",
        params![id, project_id, format!("projects/{project_id}/plans/{id}.pdf")],
    )
    .unwrap();
}

pub fn insert_visit(conn: &Connection, id: &str, project_id: &str) {
    conn.execute(
        "INSERT INTO visit(id, project_id, date, created_at) VALUES (?1, ?2, '2026-09-25', '2026-09-25T00:00:00.000Z')",
        params![id, project_id],
    )
    .unwrap();
}

pub fn insert_observation(
    conn: &Connection,
    id: &str,
    project_id: &str,
    plan_id: &str,
    ref_no: i64,
    visit_id: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, created_visit_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0.5, 0.5, ?5, '2026-09-25T00:00:00.000Z', '2026-09-25T00:00:00.000Z')",
        params![id, project_id, plan_id, ref_no, visit_id],
    )
}

pub fn insert_photo(conn: &Connection, id: &str, observation_id: &str, visit_id: Option<&str>, project_id: &str) {
    conn.execute(
        "INSERT INTO photo(id, observation_id, visit_id, file_path, taken_at, created_at)
         VALUES (?1, ?2, ?3, ?4, '2026-09-25T00:00:00.000Z', '2026-09-25T00:00:00.000Z')",
        params![id, observation_id, visit_id, format!("projects/{project_id}/photos/{id}.jpg")],
    )
    .unwrap();
}

pub fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}
