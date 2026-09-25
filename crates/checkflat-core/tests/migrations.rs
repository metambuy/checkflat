use checkflat_core::{db, migrations};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

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
        ["observation", "photo", "plan", "project", "setting", "visit"]
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
    // A v1 database with data, created through the real open() path.
    {
        let opened = db::open(&path).unwrap();
        opened
            .conn
            .execute_batch("INSERT INTO setting(key, value) VALUES ('language', 'pt');")
            .unwrap();
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
    assert_eq!(table_names(&conn).len(), 6);
}
