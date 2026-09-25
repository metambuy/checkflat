//! Checkflat storage core. Pure Rust: SQLite (rusqlite), migrations, repositories,
//! relative-path policy, PDF metadata. The Tauri layer in `src-tauri` is a thin shell over this.

pub mod clock;
pub mod db;
pub mod error;
pub mod ids;
pub mod migrations;

pub use error::{CoreError, Result};
