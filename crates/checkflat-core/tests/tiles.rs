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

#[test]
fn tiles_and_manifest_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let info = tiles::info(&conn, data, &plan.id).unwrap();
    assert!(info.manifest.is_none());
    let tiles_dir = paths::plan_tiles_dir(&plan.project_id, &plan.id).resolve(data);
    assert_eq!(Path::new(&info.dir), tiles_dir);

    tiles::write_tile(&conn, data, &plan.id, 1024, 1, 0, &webp()).unwrap();
    assert!(tiles_dir.join("1024/1_0.webp").is_file());
    let m = manifest(&[(1024, 1024, 724)]);
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
