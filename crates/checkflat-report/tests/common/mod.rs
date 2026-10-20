#![allow(dead_code)]
//! Fixtures: a project with plans, a synthetic tile pyramid (one flat colour per tile, so a
//! stitched pixel tells which tile it came from) and staged photos.
use std::io::Cursor;
use std::path::Path;

use checkflat_core::models::Plan;
use checkflat_core::photos::{self, PhotoInput};
use checkflat_core::repo::tiles::{self, TileLevel, TileManifest, TILE};
use checkflat_core::repo::{observations, plans, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_core::{db, pdf};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};

pub const A1: (f32, f32) = (2384.0, 1684.0);
pub const A4: (f32, f32) = (595.28, 841.89);

pub fn open(data: &Path) -> Connection {
    db::open(&data.join("checkflat.db")).unwrap().conn
}

pub fn project(conn: &Connection, name: &str, address: &str) -> String {
    projects::create(conn, name, address).unwrap().id
}

pub fn plan(conn: &Connection, data: &Path, project_id: &str, title: &str, size: (f32, f32)) -> Plan {
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[size], 0))).unwrap();
    plans::import(conn, data, project_id, &staged.token, title).unwrap()
}

/// The colour the pyramid gives tile `(tx, ty)` of level `size`.
pub fn tile_colour(size: u32, tx: u32, ty: u32) -> [u8; 3] {
    [(tx * 37 % 200 + 20) as u8, (ty * 53 % 200 + 20) as u8, (size / 64 % 256) as u8]
}

/// A grey per tile, for a pyramid without colour (black-on-white plans).
pub fn tile_grey(size: u32, tx: u32, ty: u32) -> [u8; 3] {
    let v = ((tx * 37 + ty * 53 + size / 64) % 200 + 20) as u8;
    [v, v, v]
}

fn webp(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb));
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out).write_image(img.as_raw(), w, h, ExtendedColorType::Rgb8).unwrap();
    out
}

/// Level geometry as the generator computes it (`src/lib/viewer/tiles.ts`).
pub fn level(size: u32, width_pt: f64, height_pt: f64) -> TileLevel {
    let s = f64::from(size) / width_pt.max(height_pt);
    let width = (width_pt * s).ceil() as u32;
    let height = (height_pt * s).ceil() as u32;
    TileLevel { size, width, height, cols: width.div_ceil(TILE), rows: height.div_ceil(TILE) }
}

/// Writes the levels in `sizes` (a prefix of `tiles::LEVELS`) and the manifest, tiles coloured
/// by [`tile_colour`].
pub fn pyramid(conn: &Connection, data: &Path, plan: &Plan, sizes: &[u32]) -> TileManifest {
    pyramid_with(conn, data, plan, sizes, tile_colour)
}

pub fn pyramid_with(conn: &Connection, data: &Path, plan: &Plan, sizes: &[u32], colour: fn(u32, u32, u32) -> [u8; 3]) -> TileManifest {
    let levels: Vec<TileLevel> = sizes.iter().map(|&s| level(s, plan.width_pt, plan.height_pt)).collect();
    for l in &levels {
        for ty in 0..l.rows {
            for tx in 0..l.cols {
                let w = (l.width - tx * TILE).min(TILE);
                let h = (l.height - ty * TILE).min(TILE);
                tiles::write_tile(conn, data, &plan.id, l.size, tx, ty, &webp(w, h, colour(l.size, tx, ty))).unwrap();
            }
        }
    }
    let m = TileManifest { version: tiles::VERSION, tile: TILE, width_pt: plan.width_pt, height_pt: plan.height_pt, levels };
    tiles::write_manifest(conn, data, &plan.id, &m).unwrap();
    m
}

/// A gradient JPEG; `seed` varies the content so no two photos are byte-identical (Typst
/// memoizes decoded images by content, which would hide the real memory cost).
pub fn jpeg_seeded(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let n = (x.wrapping_mul(31) ^ y.wrapping_mul(17) ^ seed.wrapping_mul(2654435761)) % 97;
        image::Rgb([((x * 255 / w) as u8).wrapping_add(n as u8), (y * 255 / h) as u8, (90 + seed % 100) as u8])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img).unwrap();
    out
}

pub fn jpeg(w: u32, h: u32) -> Vec<u8> {
    jpeg_seeded(w, h, 0)
}

/// Stages a photo and overrides its capture time (UTC ISO).
pub fn photo(data: &Path, w: u32, h: u32, taken_at: &str) -> PhotoInput {
    photo_seeded(data, w, h, taken_at, 0)
}

pub fn photo_seeded(data: &Path, w: u32, h: u32, taken_at: &str, seed: u32) -> PhotoInput {
    let s = photos::stage(data, Cursor::new(jpeg_seeded(w, h, seed)), 0).unwrap();
    PhotoInput { token: s.token, taken_at: taken_at.to_string(), annotation: None }
}

pub fn observation(conn: &Connection, data: &Path, plan: &Plan, x: f64, y: f64, description: &str, photos: &[PhotoInput]) -> String {
    observation_in(conn, data, plan, x, y, "", description, photos)
}

#[allow(clippy::too_many_arguments)]
pub fn observation_in(conn: &Connection, data: &Path, plan: &Plan, x: f64, y: f64, fraction: &str, description: &str, photos: &[PhotoInput]) -> String {
    observations::create_observation(conn, data, &plan.id, x, y, fraction, description, photos).unwrap().id
}
