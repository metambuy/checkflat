//! Checkflat report (Sprint 5). Three steps, so the database lock is held only for the first:
//! [`collect`] reads the project (rows + tile manifests), [`prepare`] does the image work (plan
//! snapshots from the tile cache, photos through the annotation hook and downscaled), and
//! [`render`] compiles the Typst template to a PDF. Pure Rust: no WebView, no PDF.js (D-014).

pub mod annotate;
pub mod data;
pub mod engine;
pub mod snapshot;
pub mod time;

use checkflat_core::CoreError;
use thiserror::Error;

pub use data::{collect, prepare, ReportInput, ReportSource, Stats};
pub use engine::{render, Rendered};

#[derive(Debug, Error)]
pub enum ReportError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(String),
    #[error("report data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("typst: {0}")]
    Typst(String),
}

pub type Result<T> = std::result::Result<T, ReportError>;
