use rusqlite::{params, Connection, OptionalExtension};

use crate::Result;

pub const LANGUAGE: &str = "language";

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
