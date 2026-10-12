mod common;

use checkflat_core::repo::tiles::{self, TILE};
use checkflat_report::snapshot::{self, CROP_PX};
use image::GenericImageView;

/// Decodes a snapshot and returns the pixel at `(fx, fy)` of its size.
fn pixel_at(png: &[u8], fx: f64, fy: f64) -> [u8; 3] {
    let img = image::load_from_memory(png).unwrap();
    let x = ((f64::from(img.width()) - 1.0) * fx).round() as u32;
    let y = ((f64::from(img.height()) - 1.0) * fy).round() as u32;
    let p = img.get_pixel(x, y);
    [p[0], p[1], p[2]]
}

fn colour_type(png: &[u8]) -> image::ColorType {
    image::load_from_memory(png).unwrap().color()
}

#[test]
fn crop_comes_from_the_4096_level_and_shows_the_tile_under_the_pin() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let plan = common::plan(&conn, data, &pid, "A1", common::A1);
    let m = common::pyramid(&conn, data, &plan, &[1024, 2048, 4096]);
    let tiles_dir = checkflat_core::paths::plan_tiles_dir(&pid, &plan.id).resolve(data);

    let (xn, yn) = (0.51, 0.47); // away from tile boundaries at every level
    let win = snapshot::window(xn, yn, plan.width_pt, plan.height_pt);
    let level = snapshot::crop_level(&m).unwrap();
    assert_eq!(level.size, 4096);
    let crop = snapshot::crop(&tiles_dir, level, &win).unwrap();
    assert_eq!(crop.width.max(crop.height), CROP_PX, "819 px window resized to 800");
    assert!((crop.aspect() - 1.0).abs() < 0.01, "square window");
    assert!(!crop.grey && colour_type(&crop.png) == image::ColorType::Rgb8, "coloured tiles stay RGB");

    // The pin's pixel is the colour of the 4096-level tile under it.
    let (px, py) = snapshot::pin_in_window(&win, xn, yn);
    let lx = (xn * f64::from(level.width)) as u32;
    let ly = (yn * f64::from(level.height)) as u32;
    assert_eq!(pixel_at(&crop.png, px, py), common::tile_colour(4096, lx / TILE, ly / TILE));
    // Its top-left corner is the tile at the window's origin.
    let (ox, oy) = ((win.x * f64::from(level.width)) as u32, (win.y * f64::from(level.height)) as u32);
    assert_eq!(pixel_at(&crop.png, 0.0, 0.0), common::tile_colour(4096, ox / TILE, oy / TILE));
}

#[test]
fn overview_is_the_whole_1024_level() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let plan = common::plan(&conn, data, &pid, "A1", common::A1);
    let m = common::pyramid(&conn, data, &plan, &[1024]);
    let tiles_dir = checkflat_core::paths::plan_tiles_dir(&pid, &plan.id).resolve(data);
    let o = snapshot::overview(&tiles_dir, &m).unwrap();
    assert_eq!((o.width, o.height), (m.levels[0].width, m.levels[0].height));
    assert!(!o.grey && colour_type(&o.png) == image::ColorType::Rgb8);
    assert_eq!(pixel_at(&o.png, 0.0, 0.0), common::tile_colour(1024, 0, 0));
    assert_eq!(pixel_at(&o.png, 1.0, 1.0), common::tile_colour(1024, 1, 1));
    assert_eq!(pixel_at(&o.png, 0.9, 0.1), common::tile_colour(1024, 1, 0));
}

#[test]
fn a_plan_without_colour_gives_greyscale_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let plan = common::plan(&conn, data, &pid, "A1", common::A1);
    let m = common::pyramid_with(&conn, data, &plan, &[1024, 2048, 4096], common::tile_grey);
    let tiles_dir = checkflat_core::paths::plan_tiles_dir(&pid, &plan.id).resolve(data);

    let o = snapshot::overview(&tiles_dir, &m).unwrap();
    assert!(o.grey && colour_type(&o.png) == image::ColorType::L8);
    assert_eq!(pixel_at(&o.png, 0.0, 0.0), common::tile_grey(1024, 0, 0));
    assert_eq!(pixel_at(&o.png, 1.0, 1.0), common::tile_grey(1024, 1, 1));

    let (xn, yn) = (0.51, 0.47);
    let win = snapshot::window(xn, yn, plan.width_pt, plan.height_pt);
    let level = snapshot::crop_level(&m).unwrap();
    let crop = snapshot::crop(&tiles_dir, level, &win).unwrap();
    assert!(crop.grey && colour_type(&crop.png) == image::ColorType::L8);
    assert_eq!(crop.width.max(crop.height), CROP_PX);
    let (px, py) = snapshot::pin_in_window(&win, xn, yn);
    let (lx, ly) = ((xn * f64::from(level.width)) as u32, (yn * f64::from(level.height)) as u32);
    assert_eq!(pixel_at(&crop.png, px, py), common::tile_grey(4096, lx / TILE, ly / TILE));
}

#[test]
fn a_partial_pyramid_still_gives_a_crop_from_its_top_level() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let plan = common::plan(&conn, data, &pid, "A4", common::A4);
    let m = common::pyramid(&conn, data, &plan, &[1024, 2048]);
    let tiles_dir = checkflat_core::paths::plan_tiles_dir(&pid, &plan.id).resolve(data);
    let win = snapshot::window(0.02, 0.98, plan.width_pt, plan.height_pt);
    let level = snapshot::crop_level(&m).unwrap();
    assert_eq!(level.size, 2048);
    let crop = snapshot::crop(&tiles_dir, level, &win).unwrap();
    assert!(crop.width.max(crop.height) < CROP_PX, "no upscaling: {} px", crop.width.max(crop.height));
    // Clamped to the bottom-left corner: the crop's bottom-left pixel is the level's last row, first column.
    assert_eq!(pixel_at(&crop.png, 0.0, 1.0), common::tile_colour(2048, 0, level.rows - 1));
}

#[test]
fn a_damaged_tile_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let plan = common::plan(&conn, data, &pid, "A1", common::A1);
    let m = common::pyramid(&conn, data, &plan, &[1024]);
    let tiles_dir = checkflat_core::paths::plan_tiles_dir(&pid, &plan.id).resolve(data);
    std::fs::write(tiles_dir.join("1024/1_1.webp"), b"RIFF....WEBPgarbage").unwrap();
    assert!(matches!(snapshot::overview(&tiles_dir, &m), Err(checkflat_report::ReportError::Image(_))));
    // tiles::info still lists the level (the file exists and is non-empty); the report degrades per observation.
    assert_eq!(tiles::info(&conn, data, &plan.id).unwrap().manifest.unwrap().levels.len(), 1);
}
