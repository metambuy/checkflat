mod common;

use checkflat_core::refs::Scope;
use checkflat_core::repo::{observations, projects};
use checkflat_core::{db, migrations, CoreError};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::path::Path;

fn table_names(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap();
    stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
}

#[test]
fn fresh_db_gets_all_v1_tables_and_pragmas() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    let opened = db::open(&path).unwrap();
    assert!(opened.freshly_created);
    let conn = opened.conn;
    assert_eq!(migrations::current_version(&conn).unwrap(), migrations::LATEST_VERSION);
    assert_eq!(
        table_names(&conn),
        ["fraction", "observation", "photo", "plan", "project", "setting", "visit"]
    );
    let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
    let jm: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(jm.to_lowercase(), "wal");
    assert!(!path.with_extension("db.bak-v0").exists(), "fresh db must not be backed up");
}

#[test]
fn reopening_is_idempotent_and_not_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    drop(db::open(&path).unwrap());
    let opened = db::open(&path).unwrap();
    assert!(!opened.freshly_created);
    assert_eq!(migrations::current_version(&opened.conn).unwrap(), migrations::LATEST_VERSION);
    assert!(migrations::is_latest(&opened.conn).unwrap());
    let baks: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .collect();
    assert!(baks.is_empty(), "up-to-date db must not be backed up");
}

#[test]
fn production_migrations_validate() {
    migrations::migrations().validate().unwrap();
}

#[test]
fn backup_is_written_before_migrating_an_existing_db() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    // A v1 database with data (the production list would take it straight to v2).
    {
        let mut conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;").unwrap();
        migrations::apply_with(&mut conn, Some(&path), &Migrations::new(vec![M::up(migrations::SQL_V1)])).unwrap();
        conn.execute_batch("INSERT INTO setting(key, value) VALUES ('language', 'pt');").unwrap();
    }
    // A stale backup file from an earlier attempt must be overwritten, not block the migration.
    let bak = dir.path().join("checkflat.db.bak-v1");
    std::fs::write(&bak, b"not a database").unwrap();
    // Pretend a v2 exists.
    let list = Migrations::new(vec![
        M::up(migrations::SQL_V1),
        M::up("CREATE TABLE v2_marker (id INTEGER PRIMARY KEY);"),
    ]);
    let mut conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;").unwrap();
    migrations::apply_with(&mut conn, Some(&path), &list).unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 2);

    assert!(bak.exists(), "backup of the v1 database must exist");
    let bconn = Connection::open(&bak).unwrap();
    let v: i64 = bconn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 1, "backup is the pre-migration schema");
    let lang: String = bconn
        .query_row("SELECT value FROM setting WHERE key='language'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(lang, "pt");
    assert!(table_names(&bconn).iter().all(|t| t != "v2_marker"));
    assert!(!dir.path().join("checkflat.db.bak-v2").exists(), "no backup when already at latest");
    // Applying again at the latest version is a no-op and creates no further backup.
    migrations::apply_with(&mut conn, Some(&path), &list).unwrap();
    assert!(!dir.path().join("checkflat.db.bak-v2").exists());
}

#[test]
fn in_memory_db_has_schema() {
    let conn = db::open_in_memory().unwrap();
    assert_eq!(table_names(&conn).len(), 7);
}

#[test]
fn zero_byte_or_schemaless_db_counts_as_fresh_and_skips_the_sweep() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    std::fs::write(&path, b"").unwrap();
    let dead = dir.path().join(format!("projects/{}/plans/x.pdf", checkflat_core::ids::new_id()));
    std::fs::create_dir_all(dead.parent().unwrap()).unwrap();
    std::fs::write(&dead, b"%PDF").unwrap();

    let opened = db::open(&path).unwrap();
    assert!(opened.freshly_created, "an existing but empty file is a fresh database");
    let report = checkflat_core::paths::sweep_orphans(&opened.conn, dir.path(), opened.freshly_created).unwrap();
    assert!(report.skipped_fresh_db);
    assert!(dead.exists());
    // Once migrated, reopening is not fresh anymore.
    drop(opened);
    assert!(!db::open(&path).unwrap().freshly_created);
}

/// A Sprint 2 (v1) database file with a project (`next_ref_no` 5), a plan, a visit, pins 1, 2
/// and 4 (3 was deleted), a photo and a setting — written through `apply_with` so it is a real
/// v1 file, not a v2 one with columns renamed.
fn write_v1_db(path: &Path) {
    let mut conn = Connection::open(path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;").unwrap();
    migrations::apply_with(&mut conn, Some(path), &Migrations::new(vec![M::up(migrations::SQL_V1)])).unwrap();
    conn.execute_batch(
        "INSERT INTO project(id, name, address, next_ref_no, created_at, updated_at) VALUES ('p1', 'Obra', 'Rua', 5, 't', 't');
         INSERT INTO plan(id, project_id, file_path, title, width_pt, height_pt, created_at) VALUES ('pl1', 'p1', 'projects/p1/plans/pl1.pdf', 'A1', 2384, 1684, 't');
         INSERT INTO visit(id, project_id, date, created_at) VALUES ('v1', 'p1', '2026-09-26', 't');
         INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, description, created_visit_id, created_at, updated_at)
           VALUES ('o1', 'p1', 'pl1', 1, 0.1, 0.1, 'one', 'v1', 't', 't'),
                  ('o2', 'p1', 'pl1', 2, 0.2, 0.2, '', NULL, 't', 't'),
                  ('o4', 'p1', 'pl1', 4, 0.4, 0.4, 'four', NULL, 't', 't');
         INSERT INTO photo(id, observation_id, visit_id, file_path, taken_at, created_at) VALUES ('ph1', 'o1', 'v1', 'projects/p1/photos/ph1.jpg', 't', 't');
         INSERT INTO setting(key, value) VALUES ('language', 'pt');",
    )
    .unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), 1);
}

#[test]
fn v1_database_with_pins_migrates_to_v2_keeping_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    write_v1_db(&path);

    let opened = db::open(&path).unwrap();
    assert!(!opened.freshly_created);
    let conn = opened.conn;
    assert_eq!(migrations::current_version(&conn).unwrap(), 2);
    assert!(migrations::is_latest(&conn).unwrap());
    let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1, "foreign keys back on after the rebuild");
    assert!(conn.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(ok, "ok");

    // The backup is the untouched v1 file.
    let bak = dir.path().join("checkflat.db.bak-v1");
    let bconn = Connection::open(&bak).unwrap();
    let v: i64 = bconn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 1);
    let n: i64 = bconn.query_row("SELECT count(*) FROM observation WHERE ref_no = 4", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);

    // Project: counter carried over, defaults for the new settings, locked (numbers were issued).
    let p = projects::get(&conn, "p1").unwrap();
    assert_eq!((p.name.as_str(), p.address.as_str(), p.next_seq), ("Obra", "Rua", 5));
    assert_eq!((p.code.as_str(), p.ref_template.as_str(), p.seq_scope), ("", "{PROJ}-{FRAC}-{SEQ:2}", Scope::Project));
    assert!(p.scope_locked);
    assert_eq!(p.observation_count, 3);
    // Observations: ref_no became seq, no fraction, refs render, descriptions and links kept.
    let pins = observations::list_for_plan(&conn, "pl1").unwrap();
    assert_eq!(pins.iter().map(|o| (o.seq, o.fraction.as_str(), o.display_ref.as_str())).collect::<Vec<_>>(),
        [(1, "", "01"), (2, "", "02"), (4, "", "04")]);
    assert_eq!((pins[0].description.as_str(), pins[0].created_visit_id.as_deref(), pins[0].x_norm), ("one", Some("v1"), 0.1));
    let seq_keys: Vec<String> = conn.prepare("SELECT seq_key FROM observation").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    assert!(seq_keys.iter().all(|k| k.is_empty()));
    for (table, n) in [("plan", 1), ("visit", 1), ("photo", 1), ("setting", 1), ("fraction", 0)] {
        let c: i64 = conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(c, n, "{table}");
    }
    // Numbering continues from the old counter, never from the gap.
    let next = observations::create_observation(&conn, dir.path(), "pl1", 0.5, 0.5, "", "", &[common::staged_photo(dir.path())]).unwrap();
    assert_eq!((next.seq, next.display_ref.as_str()), (5, "05"));
    // The FKs survived the rebuild: deleting the project cascades through everything.
    projects::delete(&conn, dir.path(), "p1").unwrap();
    for table in ["plan", "visit", "observation", "photo"] {
        let c: i64 = conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(c, 0, "{table} emptied by cascade");
    }
}

#[test]
fn failed_migration_leaves_v1_intact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("checkflat.db");
    write_v1_db(&path);
    // Corrupt the v1 data: an observation whose plan does not exist (only possible with FKs off).
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, created_at, updated_at)
               VALUES ('bad', 'p1', 'nope', 9, 0.5, 0.5, 't', 't');",
        )
        .unwrap();
    }
    let Err(err) = db::open(&path) else { panic!("migration must fail") };
    assert!(matches!(err, CoreError::Migration(_)), "{err}");

    let conn = Connection::open(&path).unwrap();
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 1, "still v1");
    assert_eq!(table_names(&conn), ["observation", "photo", "plan", "project", "setting", "visit"]);
    let n: i64 = conn.query_row("SELECT count(*) FROM observation WHERE ref_no IN (1, 2, 4, 9)", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 4, "rows intact");
    let next: i64 = conn.query_row("SELECT next_ref_no FROM project", [], |r| r.get(0)).unwrap();
    assert_eq!(next, 5);
    assert!(dir.path().join("checkflat.db.bak-v1").exists());
    // The connection that failed had foreign keys switched back on.
    let mut failed = Connection::open(&path).unwrap();
    failed.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    assert!(migrations::apply(&mut failed, Some(&path)).is_err());
    let fk: i64 = failed.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
}
