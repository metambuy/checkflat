//! Checkflat storage core. Pure Rust: SQLite (rusqlite), migrations, repositories,
//! relative-path policy, PDF metadata. The Tauri layer in `src-tauri` is a thin shell over this.

pub mod clock;
pub mod db;
pub mod error;
pub mod i18n;
pub mod ids;
pub mod migrations;
pub mod models;
pub mod paths;
pub mod pdf;
pub mod repo;

pub use error::{CoreError, Result};
/// Re-exported so the Tauri layer uses the same SQLite build without a second dependency line.
pub use rusqlite;
