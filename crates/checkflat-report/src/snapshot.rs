//! Plan snapshots stitched from the tile cache (D-014, D-024): a square crop around the pin and
//! an overview of the whole plan, both PNG. Crops are always greyscale with [`CROP_GREY_LEVELS`]
//! levels (D-029). typst-pdf embeds a Luma8 PNG as DeviceGray at a third of the raw size and Typst
//! holds it at one byte a pixel; the few grey values deflate to a fraction of the bytes. The
//! overview is greyscale when it carries no colour, RGB otherwise. The pin and the red box are
//! *not* drawn here — the Typst template places them as vectors from the fractions this module
//! computes, so they stay red and the overview image is the same bytes for every observation of a
//! plan (embedded once).
use std::path::Path;

use checkflat_core::repo::tiles::{TileLevel, TileManifest, TILE};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{ExtendedColorType, GenericImageView, ImageEncoder, ImageReader, RgbImage};
use serde::Serialize;

use crate::{ReportError, Result};

/// Side of the crop window as a fraction of the plan's long side (~185 mm on an A1 sheet).
pub const CROP_FRACTION: f64 = 0.20;
/// Longest side of the crop image; prints at 80 mm (~250 dpi). Picks the 4096 level.
pub const CROP_PX: u32 = 800;
/// Grey levels of a crop (D-029): 16 = steps of 17, so black and white stay exact and a near-white
/// anti-aliasing fringe becomes white. 256 would keep every level. The PDF stores 8 bits per sample
/// whatever the PNG's bit depth (typst-pdf re-deflates the raw samples), so the saving comes from
/// the few distinct values, not from a 4-bit PNG.
pub const CROP_GREY_LEVELS: u16 = 16;
/// Largest per-pixel chroma (max − min channel) the overview may have and still be written as
/// greyscale. A black-on-white plan through the lossy WebP tiles measures 1; the faintest
/// coloured layer is far above (a client plan's overview measured 189).
pub const GREY_CHROMA_MAX: u8 = 8;

/// A crop window in plan fractions (0–1 of width and height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Window {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

/// Square window of [`CROP_FRACTION`] × the long side, centred on the pin but kept inside the
/// plan (near an edge the pin sits off-centre, as in the client's mockup). A plan narrower than
/// the window (never for A4/A3/A1) shows its full width.
pub fn window(x_norm: f64, y_norm: f64, width_pt: f64, height_pt: f64) -> Window {
    let side = CROP_FRACTION * width_pt.max(height_pt);
    let w = clamp(side / width_pt, 0.0, 1.0);
    let h = clamp(side / height_pt, 0.0, 1.0);
    Window { x: clamp(x_norm - w / 2.0, 0.0, 1.0 - w), y: clamp(y_norm - h / 2.0, 0.0, 1.0 - h), w, h }
}

/// Where the pin lands inside the window (fractions of the crop image).
pub fn pin_in_window(win: &Window, x_norm: f64, y_norm: f64) -> (f64, f64) {
    (clamp((x_norm - win.x) / win.w, 0.0, 1.0), clamp((y_norm - win.y) / win.h, 0.0, 1.0))
}

/// The level the crop is cut from: the smallest finished level whose crop side reaches
/// [`CROP_PX`], else the top finished one (a pyramid interrupted by a kill still gives a crop).
pub fn crop_level(manifest: &TileManifest) -> Option<&TileLevel> {
    manifest
        .levels
        .iter()
        .find(|l| CROP_FRACTION * f64::from(l.size) >= f64::from(CROP_PX))
        .or(manifest.levels.last())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Written as an 8-bit greyscale PNG: always for a crop, for the overview when every pixel is
    /// within [`GREY_CHROMA_MAX`].
    pub grey: bool,
}

impl Snapshot {
    /// Height over width, for the template to size the image box before placing overlays.
    pub fn aspect(&self) -> f64 {
        f64::from(self.height) / f64::from(self.width)
    }
}

/// Crop of `win` from `level` (tiles under `tiles_dir/<size>/`): greyscale, resized to ≤
/// [`CROP_PX`], quantised to [`CROP_GREY_LEVELS`]. Colour on the plan (trees, street fills) prints
/// as grey.
pub fn crop(tiles_dir: &Path, level: &TileLevel, win: &Window) -> Result<Snapshot> {
    let px = |f: f64, size: u32| (f * f64::from(size)).round() as u32;
    let x0 = px(win.x, level.width).min(level.width.saturating_sub(1));
    let y0 = px(win.y, level.height).min(level.height.saturating_sub(1));
    let w = px(win.w, level.width).clamp(1, level.width - x0);
    let h = px(win.h, level.height).clamp(1, level.height - y0);
    // Grey before the resize: a third of the samples to filter.
    let mut luma = image::imageops::grayscale(&stitch(tiles_dir, level, x0, y0, w, h)?);
    if w.max(h) > CROP_PX {
        luma = image::imageops::resize(&luma, scaled(w, w.max(h)), scaled(h, w.max(h)), FilterType::Triangle);
    }
    for p in luma.pixels_mut() {
        p.0[0] = quantise(p.0[0], CROP_GREY_LEVELS);
    }
    let png = png(luma.as_raw(), luma.width(), luma.height(), ExtendedColorType::L8)?;
    Ok(Snapshot { png, width: luma.width(), height: luma.height(), grey: true })
}

/// `v` rounded to the nearest of `levels` evenly spaced values from 0 to 255 (2 ≤ levels ≤ 256).
fn quantise(v: u8, levels: u16) -> u8 {
    let steps = u32::from(levels.clamp(2, 256)) - 1;
    let level = (u32::from(v) * steps + 127) / 255;
    ((level * 255 + steps / 2) / steps) as u8
}

fn scaled(v: u32, long: u32) -> u32 {
    ((u64::from(v) * u64::from(CROP_PX)) / u64::from(long)).max(1) as u32
}

/// The whole plan from its smallest level (1024 px long side).
pub fn overview(tiles_dir: &Path, manifest: &TileManifest) -> Result<Snapshot> {
    let level = manifest.levels.first().ok_or_else(|| ReportError::Image("no finished tile level".into()))?;
    encode(&stitch(tiles_dir, level, 0, 0, level.width, level.height)?)
}

/// Copies the `w × h` region at `(x0, y0)` of a level out of its 512 px tiles. Tiles are opaque
/// (PDF.js renders on white), but any alpha is composited over white rather than trusted.
fn stitch(tiles_dir: &Path, level: &TileLevel, x0: u32, y0: u32, w: u32, h: u32) -> Result<RgbImage> {
    let mut out = RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    let dir = tiles_dir.join(level.size.to_string());
    for ty in y0 / TILE..=(y0 + h - 1) / TILE {
        for tx in x0 / TILE..=(x0 + w - 1) / TILE {
            let path = dir.join(format!("{tx}_{ty}.webp"));
            let tile = ImageReader::open(&path)
                .and_then(|r| r.with_guessed_format())
                .map_err(|e| ReportError::Image(format!("{}: {e}", path.display())))?
                .decode()
                .map_err(|e| ReportError::Image(format!("{}: {e}", path.display())))?;
            // Intersection of this tile with the region, in level px.
            let (tile_x, tile_y) = (tx * TILE, ty * TILE);
            let left = x0.max(tile_x);
            let top = y0.max(tile_y);
            let right = (x0 + w).min(tile_x + tile.width());
            let bottom = (y0 + h).min(tile_y + tile.height());
            if right <= left || bottom <= top {
                continue;
            }
            let view = tile.view(left - tile_x, top - tile_y, right - left, bottom - top).to_image();
            for (x, y, p) in view.enumerate_pixels() {
                let a = u32::from(p[3]);
                let over = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
                out.put_pixel(left - x0 + x, top - y0 + y, image::Rgb([over(p[0]), over(p[1]), over(p[2])]));
            }
        }
    }
    Ok(out)
}

/// True when no pixel's chroma exceeds [`GREY_CHROMA_MAX`].
fn is_grey(img: &RgbImage) -> bool {
    img.pixels().all(|p| {
        let (lo, hi) = p.0.iter().fold((u8::MAX, 0), |(lo, hi), &c| (lo.min(c), hi.max(c)));
        hi - lo <= GREY_CHROMA_MAX
    })
}

/// The overview's encoding: greyscale PNG (L8) when the image has no colour, RGB PNG otherwise.
fn encode(img: &RgbImage) -> Result<Snapshot> {
    let grey = is_grey(img);
    let png = if grey {
        let luma = image::imageops::grayscale(img);
        png(luma.as_raw(), img.width(), img.height(), ExtendedColorType::L8)?
    } else {
        png(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgb8)?
    };
    Ok(Snapshot { png, width: img.width(), height: img.height(), grey })
}

fn png(raw: &[u8], w: u32, h: u32, colour: ExtendedColorType) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Default, PngFilter::Adaptive)
        .write_image(raw, w, h, colour)
        .map_err(|e| ReportError::Image(e.to_string()))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn window_is_square_on_the_sheet_and_centred_on_the_pin() {
        // A1 landscape: side = 0.2 × 2384 = 476.8 pt → 20 % of the width, 28.3 % of the height.
        let w = window(0.5, 0.5, 2384.0, 1684.0);
        assert!(close(w.w, 0.2) && close(w.h, 476.8 / 1684.0));
        assert!(close(w.x, 0.4) && close(w.y, 0.5 - w.h / 2.0));
        let (px, py) = pin_in_window(&w, 0.5, 0.5);
        assert!(close(px, 0.5) && close(py, 0.5));
        // A4 portrait: the long side is the height.
        let p = window(0.5, 0.5, 595.28, 841.89);
        assert!(close(p.h, 0.2) && close(p.w, 0.2 * 841.89 / 595.28));
    }

    #[test]
    fn window_is_clamped_inside_the_plan_so_the_pin_goes_off_centre() {
        let w = window(0.02, 0.97, 2384.0, 1684.0);
        assert!(close(w.x, 0.0) && close(w.y, 1.0 - w.h));
        let (px, py) = pin_in_window(&w, 0.02, 0.97);
        assert!(close(px, 0.1), "pin 2 % in, window 20 % wide → 10 % of the crop");
        assert!(py > 0.85 && py < 1.0);
        let c = window(1.0, 0.0, 2384.0, 1684.0);
        assert!(close(c.x, 0.8) && close(c.y, 0.0));
        let (px, py) = pin_in_window(&c, 1.0, 0.0);
        assert!(close(px, 1.0) && close(py, 0.0));
        // Degenerate: a plan narrower than the window shows all of it.
        let n = window(0.5, 0.5, 100.0, 1000.0);
        assert!(close(n.w, 1.0) && close(n.x, 0.0));
    }

    #[test]
    fn encode_writes_greyscale_only_when_every_pixel_is_within_the_chroma_threshold() {
        let colour_type = |s: &Snapshot| image::load_from_memory(&s.png).unwrap().color();
        // Anti-aliased black on white with the chroma noise lossy tiles leave: greyscale.
        let mut img = RgbImage::from_fn(64, 64, |x, y| if (x + y) % 7 == 0 { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) });
        img.put_pixel(3, 3, image::Rgb([120, 128, 124]));
        let s = encode(&img).unwrap();
        assert!(s.grey);
        assert_eq!(colour_type(&s), image::ColorType::L8);
        assert_eq!((s.width, s.height), (64, 64));
        let decoded = image::load_from_memory(&s.png).unwrap();
        assert_eq!(decoded.get_pixel(0, 0)[0], 0);
        assert_eq!(decoded.get_pixel(1, 0)[0], 255);
        // One pixel just over the threshold keeps the whole image RGB.
        img.put_pixel(3, 3, image::Rgb([120, 129, 120]));
        let s = encode(&img).unwrap();
        assert!(!s.grey);
        assert_eq!(colour_type(&s), image::ColorType::Rgb8);
        assert_eq!(image::load_from_memory(&s.png).unwrap().get_pixel(3, 3).0, [120, 129, 120, 255]);
    }

    #[test]
    fn quantise_keeps_black_and_white_and_rounds_to_the_nearest_level() {
        use std::collections::BTreeSet;
        assert_eq!((quantise(0, 16), quantise(255, 16)), (0, 255));
        assert_eq!((quantise(8, 16), quantise(9, 16)), (0, 17), "half way between 0 and 17");
        assert_eq!((quantise(246, 16), quantise(247, 16)), (238, 255), "a near-white fringe becomes white");
        let sixteen: BTreeSet<u8> = (0..=255u8).map(|v| quantise(v, 16)).collect();
        assert_eq!(sixteen.len(), 16);
        assert!(sixteen.iter().all(|v| v % 17 == 0), "{sixteen:?}");
        assert!((0..=255u8).all(|v| quantise(v, 256) == v), "256 levels keep every value");
        let thirty_two: BTreeSet<u8> = (0..=255u8).map(|v| quantise(v, 32)).collect();
        assert_eq!((thirty_two.len(), thirty_two.first(), thirty_two.last()), (32, Some(&0), Some(&255)));
    }

    #[test]
    fn crop_level_prefers_4096_and_degrades_to_what_is_finished() {
        let level = |size: u32| TileLevel { size, width: size, height: size * 1684 / 2384, cols: size.div_ceil(512), rows: 1 };
        let m = |sizes: &[u32]| TileManifest { version: 1, tile: 512, width_pt: 2384.0, height_pt: 1684.0, levels: sizes.iter().map(|&s| level(s)).collect() };
        assert_eq!(crop_level(&m(&[1024, 2048, 4096, 8192])).unwrap().size, 4096);
        assert_eq!(crop_level(&m(&[1024, 2048])).unwrap().size, 2048);
        assert_eq!(crop_level(&m(&[1024])).unwrap().size, 1024);
        assert!(crop_level(&m(&[])).is_none());
    }
}
