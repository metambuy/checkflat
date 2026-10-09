#![allow(dead_code)]
use std::path::{Path, PathBuf};

use checkflat_core::photos::{self, PhotoInput};
use rusqlite::{params, Connection};

/// The data dir of a connection opened with `db::open(<data>/checkflat.db)`.
pub fn data_dir_of(conn: &Connection) -> PathBuf {
    Path::new(conn.path().expect("file database")).parent().unwrap().to_path_buf()
}

/// A plain gradient JPEG of the given size (no EXIF).
pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .unwrap();
    out
}

/// Stage a small photo in `data` and return what the observation save takes.
pub fn staged_photo(data: &Path) -> PhotoInput {
    let s = photos::stage(data, std::io::Cursor::new(jpeg(64, 48)), 0).unwrap();
    PhotoInput { token: s.token, taken_at: s.taken_at }
}

/// One freshly staged photo for the database in `conn`.
pub fn one_photo(conn: &Connection) -> Vec<PhotoInput> {
    vec![staged_photo(&data_dir_of(conn))]
}

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

/// Direct insert with the v2 columns (`seq_key` = `fraction` when given, as the per-fraction
/// scope stores it; `''` otherwise).
pub fn insert_observation(
    conn: &Connection,
    id: &str,
    project_id: &str,
    plan_id: &str,
    seq: i64,
    visit_id: Option<&str>,
) -> rusqlite::Result<usize> {
    insert_observation_in(conn, id, project_id, plan_id, "", seq, visit_id)
}

pub fn insert_observation_in(
    conn: &Connection,
    id: &str,
    project_id: &str,
    plan_id: &str,
    fraction: &str,
    seq: i64,
    visit_id: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO observation(id, project_id, plan_id, fraction, seq, seq_key, x_norm, y_norm, created_visit_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?4, 0.5, 0.5, ?6, '2026-09-25T00:00:00.000Z', '2026-09-25T00:00:00.000Z')",
        params![id, project_id, plan_id, fraction, seq, visit_id],
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
