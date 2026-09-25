//! Connection setup: one connection per app, `foreign_keys = ON`, WAL journal, busy timeout,
//! migrations applied on open.
use std::path::Path;

use rusqlite::Connection;

use crate::{migrations, Result};

/// An opened, migrated connection plus whether the database file was created by this call.
/// `freshly_created` gates destructive maintenance (the orphan sweep) so that an empty
/// database resulting from a lost or replaced file can never wipe user files.
pub struct Opened {
    pub conn: Connection,
    pub freshly_created: bool,
}

pub fn open(path: &Path) -> Result<Opened> {
    let existed = path.exists();
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrations::apply(&mut conn, Some(path))?;
    Ok(Opened { conn, freshly_created: !existed })
}

/// In-memory database with the same configuration and schema (tests).
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrations::apply(&mut conn, None)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}
