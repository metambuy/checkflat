//! Schema migrations tracked with `PRAGMA user_version` via rusqlite_migration.
//! Migration 1 creates every v1 table (plan §3 data model), including the ones later sprints use.
use std::path::Path;

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M, SchemaVersion};

use crate::Result;

/// All v1 tables. IDs are UUID v7 strings, timestamps are UTC RFC 3339 strings.
///
/// FK policy: `project` -> children ON DELETE CASCADE. `observation.plan_id` deliberately has no
/// ON DELETE clause (default NO ACTION, checked at end of statement) so that deleting a project
/// succeeds whatever order the cascade removes plans and observations in, while a direct plan
/// delete with observations still fails (the app checks first and reports `plan_has_observations`).
/// The two visit back-references on `observation` are SET NULL: deleting a visit must never delete
/// observations that carried over from it.
pub const SQL_V1: &str = r#"
CREATE TABLE project (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  address TEXT NOT NULL DEFAULT '',
  logo_path TEXT,
  next_ref_no INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE plan (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
  file_path TEXT NOT NULL,
  title TEXT NOT NULL,
  width_pt REAL NOT NULL,
  height_pt REAL NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (id, project_id)
);
CREATE TABLE visit (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  attendees TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  report_lang TEXT NOT NULL DEFAULT 'en' CHECK (report_lang IN ('en','pt')),
  created_at TEXT NOT NULL
);
CREATE TABLE observation (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
  plan_id TEXT NOT NULL,
  ref_no INTEGER NOT NULL,
  x_norm REAL NOT NULL CHECK (x_norm BETWEEN 0 AND 1),
  y_norm REAL NOT NULL CHECK (y_norm BETWEEN 0 AND 1),
  description TEXT NOT NULL DEFAULT '',
  created_visit_id TEXT REFERENCES visit(id) ON DELETE SET NULL,
  resolved_visit_id TEXT REFERENCES visit(id) ON DELETE SET NULL,
  resolved_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (project_id, ref_no),
  FOREIGN KEY (plan_id, project_id) REFERENCES plan(id, project_id)
);
CREATE TABLE photo (
  id TEXT PRIMARY KEY,
  observation_id TEXT NOT NULL REFERENCES observation(id) ON DELETE CASCADE,
  visit_id TEXT REFERENCES visit(id) ON DELETE SET NULL,
  file_path TEXT NOT NULL,
  taken_at TEXT NOT NULL,
  annotation_json TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE INDEX idx_plan_project ON plan(project_id);
CREATE INDEX idx_visit_project ON visit(project_id, date);
CREATE INDEX idx_observation_plan ON observation(plan_id);
CREATE INDEX idx_photo_observation ON photo(observation_id);
"#;

/// The production migration list. Append, never edit, released entries.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(SQL_V1)])
}

pub const LATEST_VERSION: usize = 1;

/// Current `user_version` of the connection (0 for a fresh database).
pub fn current_version(conn: &Connection) -> Result<usize> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(usize::try_from(v).unwrap_or(0))
}

/// Apply the production migrations; see [`apply_with`].
pub fn apply(conn: &mut Connection, db_path: Option<&Path>) -> Result<()> {
    apply_with(conn, db_path, &migrations())
}

/// Migrate `conn` to the latest version of `migrations`.
///
/// If the database already has a schema (`0 < user_version < latest`) and lives in a file,
/// a consistent copy is written next to it first as `<db>.bak-v<old>` (WAL checkpoint, then
/// `VACUUM INTO`), overwriting an older backup of the same version. Fresh and up-to-date
/// databases are never copied.
pub fn apply_with(conn: &mut Connection, db_path: Option<&Path>, migrations: &Migrations<'_>) -> Result<()> {
    let current = current_version(conn)?;
    let latest = migrations_len(migrations);
    if current > 0 && current < latest {
        if let Some(path) = db_path {
            backup(conn, path, current)?;
        }
    }
    migrations.to_latest(conn)?;
    Ok(())
}

fn migrations_len(migrations: &Migrations<'_>) -> usize {
    // `Migrations` does not expose its length directly; `pending_migrations` on a scratch
    // in-memory connection at version 0 equals the total count.
    let scratch = Connection::open_in_memory().expect("in-memory connection");
    migrations.pending_migrations(&scratch).map(|n| usize::try_from(n).unwrap_or(0)).unwrap_or(0)
}

fn backup(conn: &Connection, db_path: &Path, version: usize) -> Result<()> {
    let mut target = db_path.as_os_str().to_owned();
    target.push(format!(".bak-v{version}"));
    let target = std::path::PathBuf::from(target);
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])?;
    log::info!("database backup written to {}", target.display());
    Ok(())
}

/// Whether the schema is at the latest version of the production list.
pub fn is_latest(conn: &Connection) -> Result<bool> {
    Ok(matches!(migrations().current_version(conn)?, SchemaVersion::Inside(_)) && current_version(conn)? == LATEST_VERSION)
}
