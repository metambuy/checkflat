//! Tile cache of a plan (D-014): `plans/<plan_id>/tiles/<size>/<x>_<y>.webp` plus
//! `manifest.json`, which the generator rewrites after each finished level (a level is usable
//! once it is listed). Derived data: no DB column, excluded from archives, rebuilt when missing.
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::paths::plan_tiles_dir;
use crate::repo::plans;
use crate::{CoreError, Result};

pub const MANIFEST: &str = "manifest.json";
/// Largest accepted level (long side, px) and tile file; generous upper bounds, not the config.
const MAX_LEVEL: u32 = 16384;
const MAX_TILE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileLevel {
    /// Long side in px; also the level's directory name.
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileManifest {
    /// Generator settings version; the UI regenerates when it differs from its own.
    pub version: u32,
    pub tile: u32,
    /// Page size as PDF.js saw it (cross-checked against the stored plan size, D-011).
    pub width_pt: f64,
    pub height_pt: f64,
    /// Finished levels, smallest first.
    pub levels: Vec<TileLevel>,
}

impl TileManifest {
    fn validate(&self) -> Result<()> {
        let bad = |why: String| Err(CoreError::Validation(format!("tile manifest: {why}")));
        if !(64..=2048).contains(&self.tile) {
            return bad(format!("tile size {}", self.tile));
        }
        let mut prev = 0;
        for l in &self.levels {
            if l.size <= prev || l.size > MAX_LEVEL {
                return bad(format!("level {} out of order or too large", l.size));
            }
            prev = l.size;
            if l.width.max(l.height) > l.size + 1 || l.cols != l.width.div_ceil(self.tile) || l.rows != l.height.div_ceil(self.tile) {
                return bad(format!("level {} dimensions inconsistent", l.size));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileInfo {
    /// Absolute tile directory and plan PDF, for the WebView's asset protocol (`convertFileSrc`).
    pub dir: String,
    pub pdf: String,
    pub manifest: Option<TileManifest>,
}

fn tiles_dir(conn: &Connection, data_dir: &Path, plan_id: &str) -> Result<PathBuf> {
    let plan = plans::get(conn, plan_id)?;
    Ok(plan_tiles_dir(&plan.project_id, &plan.id).resolve(data_dir))
}

fn read_manifest(dir: &Path) -> Option<TileManifest> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    match serde_json::from_str::<TileManifest>(&text) {
        Ok(m) if m.validate().is_ok() => Some(m),
        _ => {
            log::warn!("ignoring unreadable tile manifest in {}", dir.display());
            None
        }
    }
}

pub fn info(conn: &Connection, data_dir: &Path, plan_id: &str) -> Result<TileInfo> {
    let plan = plans::get(conn, plan_id)?;
    let dir = plan_tiles_dir(&plan.project_id, &plan.id).resolve(data_dir);
    let manifest = read_manifest(&dir);
    Ok(TileInfo {
        dir: dir.to_string_lossy().into_owned(),
        pdf: plan.file_path.resolve(data_dir).to_string_lossy().into_owned(),
        manifest,
    })
}

/// Writes one WebP tile. Partial writes are harmless: a level only counts once the manifest
/// lists it, and an unfinished level is regenerated.
pub fn write_tile(conn: &Connection, data_dir: &Path, plan_id: &str, size: u32, x: u32, y: u32, bytes: &[u8]) -> Result<()> {
    if size == 0 || size > MAX_LEVEL || x > 255 || y > 255 {
        return Err(CoreError::Validation(format!("tile {size}/{x}_{y} out of range")));
    }
    if bytes.len() > MAX_TILE_BYTES || bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(CoreError::Validation("tile is not a WebP image".into()));
    }
    let dir = tiles_dir(conn, data_dir, plan_id)?.join(size.to_string());
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{x}_{y}.webp")), bytes)?;
    Ok(())
}

/// Validates and atomically replaces the manifest (write to a temp file, then rename).
pub fn write_manifest(conn: &Connection, data_dir: &Path, plan_id: &str, manifest: &TileManifest) -> Result<()> {
    manifest.validate()?;
    let dir = tiles_dir(conn, data_dir, plan_id)?;
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!("{MANIFEST}.tmp"));
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(serde_json::to_string(manifest).expect("serializable").as_bytes())?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, dir.join(MANIFEST))?;
    Ok(())
}

/// Removes the whole cache (generator settings changed, or a forced rebuild).
pub fn clear(conn: &Connection, data_dir: &Path, plan_id: &str) -> Result<()> {
    match std::fs::remove_dir_all(tiles_dir(conn, data_dir, plan_id)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
