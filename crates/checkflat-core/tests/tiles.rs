mod common;

use checkflat_core::models::Plan;
use checkflat_core::repo::tiles::{self, TileLevel, TileManifest};
use checkflat_core::repo::{plans, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_core::{db, paths, pdf, CoreError};
use std::io::Cursor;
use std::path::Path;

fn setup(data: &Path) -> (Connection, Plan) {
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[(2384.0, 1684.0)], 0))).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "A1").unwrap();
    (conn, plan)
}

fn webp() -> Vec<u8> {
    let mut b = b"RIFF\x10\x00\x00\x00WEBPVP8L".to_vec();
    b.extend_from_slice(&[0; 8]);
    b
}

fn manifest(levels: &[(u32, u32, u32)]) -> TileManifest {
    TileManifest {
        version: 1,
        tile: 512,
        width_pt: 2384.0,
        height_pt: 1684.0,
        levels: levels
            .iter()
            .map(|&(size, width, height)| TileLevel { size, width, height, cols: width.div_ceil(512), rows: height.div_ceil(512) })
            .collect(),
    }
}

/// Writes every tile of a level, as the generator does before listing it.
fn write_level(conn: &Connection, data: &Path, plan_id: &str, m: &TileManifest, idx: usize) {
    let l = &m.levels[idx];
    for y in 0..l.rows {
        for x in 0..l.cols {
            tiles::write_tile(conn, data, plan_id, l.size, x, y, &webp()).unwrap();
        }
    }
}

#[test]
fn tiles_and_manifest_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let info = tiles::info(&conn, data, &plan.id).unwrap();
    assert!(info.manifest.is_none());
    let tiles_dir = paths::plan_tiles_dir(&plan.project_id, &plan.id).resolve(data);
    assert_eq!(Path::new(&info.dir), tiles_dir);
    assert_eq!(Path::new(&info.pdf), plan.file_path.resolve(data));

    let m = manifest(&[(1024, 1024, 724)]);
    write_level(&conn, data, &plan.id, &m, 0);
    assert!(tiles_dir.join("1024/1_0.webp").is_file());
    tiles::write_manifest(&conn, data, &plan.id, &m).unwrap();
    assert_eq!(tiles::info(&conn, data, &plan.id).unwrap().manifest, Some(m));
    assert!(!tiles_dir.join("manifest.json.tmp").exists());

    tiles::clear(&conn, data, &plan.id).unwrap();
    assert!(!tiles_dir.exists());
    tiles::clear(&conn, data, &plan.id).unwrap();
}

#[test]
fn rejects_bad_tiles_and_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    assert!(matches!(tiles::write_tile(&conn, data, &plan.id, 1024, 0, 0, b"%PDF-1.7 not webp"), Err(CoreError::Validation(_))));
    assert!(matches!(tiles::write_tile(&conn, data, &plan.id, 99999, 0, 0, &webp()), Err(CoreError::Validation(_))));
    assert!(matches!(tiles::write_tile(&conn, data, &plan.id, 1024, 999, 0, &webp()), Err(CoreError::Validation(_))));
    assert!(matches!(tiles::write_tile(&conn, data, "missing", 1024, 0, 0, &webp()), Err(CoreError::NotFound)));
    // Levels out of order, and cols inconsistent with width.
    assert!(tiles::write_manifest(&conn, data, &plan.id, &manifest(&[(2048, 2048, 1447), (1024, 1024, 724)])).is_err());
    let mut m = manifest(&[(1024, 1024, 724)]);
    m.levels[0].cols = 5;
    assert!(tiles::write_manifest(&conn, data, &plan.id, &m).is_err());
    // A corrupt manifest on disk reads as "no manifest" (regenerate), not as an error.
    let tiles_dir = paths::plan_tiles_dir(&plan.project_id, &plan.id).resolve(data);
    std::fs::create_dir_all(&tiles_dir).unwrap();
    std::fs::write(tiles_dir.join("manifest.json"), "{ truncated").unwrap();
    assert_eq!(tiles::info(&conn, data, &plan.id).unwrap().manifest, None);
}

#[test]
fn a_listed_level_with_missing_or_empty_tiles_is_dropped_on_read() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let tiles_dir = paths::plan_tiles_dir(&plan.project_id, &plan.id).resolve(data);
    let m = manifest(&[(1024, 1024, 724), (2048, 2048, 1447), (4096, 4096, 2894)]);
    for i in 0..3 {
        write_level(&conn, data, &plan.id, &m, i);
    }
    tiles::write_manifest(&conn, data, &plan.id, &m).unwrap();
    let levels = |conn: &Connection| -> Vec<u32> {
        tiles::info(conn, data, &plan.id).unwrap().manifest.unwrap().levels.iter().map(|l| l.size).collect()
    };
    assert_eq!(levels(&conn), [1024, 2048, 4096]);

    // Zero-length tile (power loss before the data reached the disk): that level is dropped.
    std::fs::write(tiles_dir.join("4096/7_5.webp"), b"").unwrap();
    assert_eq!(levels(&conn), [1024, 2048]);
    // Missing tile in a lower level: it and every level above it are dropped (levels stay a prefix).
    std::fs::remove_file(tiles_dir.join("2048/3_2.webp")).unwrap();
    assert_eq!(levels(&conn), [1024]);
    // A manifest left behind without any tiles reads as "no finished level".
    std::fs::remove_dir_all(tiles_dir.join("1024")).unwrap();
    assert_eq!(levels(&conn), [] as [u32; 0]);
}

#[test]
fn manifest_validation_rejects_zero_sizes_bad_page_size_and_unknown_settings() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let good = manifest(&[(1024, 1024, 724)]);
    write_level(&conn, data, &plan.id, &good, 0);
    let rejected = |m: &TileManifest| matches!(tiles::write_manifest(&conn, data, &plan.id, m), Err(CoreError::Validation(_)));
    let with = |f: &dyn Fn(&mut TileManifest)| {
        let mut m = good.clone();
        f(&mut m);
        m
    };
    assert!(!rejected(&good));
    // A zero-sized level (cols/rows consistent with it) and a non-positive or non-finite page size.
    assert!(rejected(&with(&|m| m.levels[0] = TileLevel { size: 1024, width: 0, height: 724, cols: 0, rows: 2 })));
    assert!(rejected(&with(&|m| m.levels[0] = TileLevel { size: 1024, width: 1024, height: 0, cols: 2, rows: 0 })));
    assert!(rejected(&with(&|m| m.width_pt = 0.0)));
    assert!(rejected(&with(&|m| m.height_pt = -1684.0)));
    assert!(rejected(&with(&|m| m.width_pt = f64::NAN)));
    assert!(rejected(&with(&|m| m.height_pt = f64::INFINITY)));
    // Settings other than the configured ones: version, tile size, a level that is not next in LEVELS.
    assert!(rejected(&with(&|m| m.version = tiles::VERSION + 1)));
    assert!(rejected(&with(&|m| m.tile = 256)));
    assert!(rejected(&manifest(&[(2048, 2048, 1447)])));
    assert!(rejected(&manifest(&[(1024, 1024, 724), (2048, 2048, 1447), (4096, 4096, 2894), (8192, 8192, 5787), (16384, 16384, 11574)])));

    // The same on disk reads as "no manifest", so the UI rebuilds instead of showing NaN geometry.
    let tiles_dir = paths::plan_tiles_dir(&plan.project_id, &plan.id).resolve(data);
    let json = serde_json::to_string(&with(&|m| m.width_pt = 0.0)).unwrap();
    std::fs::write(tiles_dir.join("manifest.json"), json).unwrap();
    assert_eq!(tiles::info(&conn, data, &plan.id).unwrap().manifest, None);
}

#[test]
fn write_tile_accepts_only_configured_levels_and_their_grid() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let rejected = |size, x, y| matches!(tiles::write_tile(&conn, data, &plan.id, size, x, y, &webp()), Err(CoreError::Validation(_)));
    assert!(rejected(512, 0, 0), "not a level");
    assert!(rejected(1000, 0, 0), "not a level");
    assert!(rejected(16384, 0, 0), "not a level");
    assert!(rejected(1024, 2, 0), "1024 has a 2 × 2 grid at most");
    assert!(rejected(1024, 0, 2));
    assert!(!rejected(1024, 1, 1));
    assert!(!rejected(8192, 15, 15));
    assert!(rejected(8192, 16, 0));
}

/// `tiles::{VERSION, TILE, LEVELS}` mirror the generator's `TILES` in the frontend.
#[test]
fn core_tile_settings_match_the_frontend_config() {
    let config = include_str!("../../../src/lib/viewer/config.ts");
    let expected = format!("version: {}, levels: {:?}, tile: {},", tiles::VERSION, tiles::LEVELS, tiles::TILE);
    assert!(config.contains(&expected), "config.ts TILES does not contain `{expected}`");
}

#[test]
fn deleting_the_plan_removes_its_tile_cache() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    tiles::write_tile(&conn, data, &plan.id, 1024, 0, 0, &webp()).unwrap();
    let plan_dir = paths::plan_dir(&plan.project_id, &plan.id).resolve(data);
    assert!(plan_dir.is_dir());
    plans::delete(&conn, data, &plan.id).unwrap();
    assert!(!plan_dir.exists());
    assert!(!plan.file_path.resolve(data).exists());
}
