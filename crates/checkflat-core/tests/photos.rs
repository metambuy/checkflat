mod common;

use std::io::Cursor;
use std::path::Path;

use checkflat_core::annotation::{self, Annotation, Shape};
use checkflat_core::models::Plan;
use checkflat_core::paths::{self, staging_photo};
use checkflat_core::photos::{self, PhotoAnnotation, PhotoInput};
use checkflat_core::repo::{observations, photos as photo_repo, plans, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_core::{clock, db, pdf, CoreError};
use image::GenericImageView;

/// A JPEG whose left half is red and right half blue, with an EXIF block spliced in after SOI.
fn jpeg_with_exif(w: u32, h: u32, orientation: Option<u16>, original: Option<&str>, offset: Option<&str>) -> Vec<u8> {
    use exif::experimental::Writer;
    use exif::{Field, In, Tag, Value};
    let img = image::RgbImage::from_fn(w, h, |x, _| if x < w / 2 { image::Rgb([255, 0, 0]) } else { image::Rgb([0, 0, 255]) });
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95).encode_image(&img).unwrap();

    let mut fields = Vec::new();
    if let Some(o) = orientation {
        fields.push(Field { tag: Tag::Orientation, ifd_num: In::PRIMARY, value: Value::Short(vec![o]) });
    }
    if let Some(t) = original {
        fields.push(Field { tag: Tag::DateTimeOriginal, ifd_num: In::PRIMARY, value: Value::Ascii(vec![t.as_bytes().to_vec()]) });
    }
    if let Some(o) = offset {
        fields.push(Field { tag: Tag::OffsetTimeOriginal, ifd_num: In::PRIMARY, value: Value::Ascii(vec![o.as_bytes().to_vec()]) });
    }
    if fields.is_empty() {
        return jpeg;
    }
    let mut writer = Writer::new();
    for f in &fields {
        writer.push_field(f);
    }
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).unwrap();
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff.into_inner());
    let len = (payload.len() + 2) as u16;
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn stage(data: &Path, bytes: Vec<u8>, offset_min: i32) -> checkflat_core::Result<photos::StagedPhoto> {
    photos::stage(data, Cursor::new(bytes), offset_min)
}

fn staged_image(data: &Path, token: &str) -> image::DynamicImage {
    image::open(staging_photo(token).resolve(data)).unwrap()
}

fn setup(data: &Path) -> (Connection, Plan) {
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[(2384.0, 1684.0)], 0))).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "A1").unwrap();
    (conn, plan)
}

fn save(conn: &Connection, data: &Path, plan: &Plan, photos: &[PhotoInput]) -> checkflat_core::Result<checkflat_core::models::Observation> {
    observations::create_observation(conn, data, &plan.id, 0.5, 0.5, "", "d", photos)
}

fn tmp_files(data: &Path) -> Vec<String> {
    std::fs::read_dir(data.join("tmp")).map(|d| d.flatten().map(|f| f.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}

#[test]
fn long_side_is_capped_at_1600_keeping_the_aspect_ratio() {
    let dir = tempfile::tempdir().unwrap();
    for ((w, h), want) in [((3200, 2400), (1600, 1200)), ((2400, 3200), (1200, 1600)), ((640, 480), (640, 480)), ((1600, 900), (1600, 900))] {
        let s = stage(dir.path(), common::jpeg(w, h), 0).unwrap();
        assert_eq!(staged_image(dir.path(), &s.token).dimensions(), want, "{w}x{h}");
    }
}

#[test]
fn exif_orientation_is_applied_and_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    // Orientation 6: the stored pixels must be turned 90° clockwise to display upright, so the
    // left (red) half ends up on top of a portrait image.
    let s = stage(dir.path(), jpeg_with_exif(80, 40, Some(6), None, None), 0).unwrap();
    let img = staged_image(dir.path(), &s.token);
    assert_eq!(img.dimensions(), (40, 80));
    let top = img.get_pixel(20, 5);
    let bottom = img.get_pixel(20, 75);
    assert!(top[0] > 200 && top[2] < 60, "top is red: {top:?}");
    assert!(bottom[2] > 200 && bottom[0] < 60, "bottom is blue: {bottom:?}");
    // The output has no EXIF at all: nothing can rotate it a second time, and GPS is gone.
    let bytes = std::fs::read(staging_photo(&s.token).resolve(dir.path())).unwrap();
    assert!(exif::Reader::new().read_from_container(&mut Cursor::new(&bytes)).is_err());
    // No orientation tag: unchanged.
    let s = stage(dir.path(), jpeg_with_exif(80, 40, None, None, None), 0).unwrap();
    assert_eq!(staged_image(dir.path(), &s.token).dimensions(), (80, 40));
}

#[test]
fn taken_at_comes_from_exif_in_utc() {
    let dir = tempfile::tempdir().unwrap();
    // Local time + the device's offset (minutes east of UTC).
    let s = stage(dir.path(), jpeg_with_exif(32, 32, None, Some("2026:10:05 14:03:09"), None), 120).unwrap();
    assert_eq!(s.taken_at, "2026-10-05T12:03:09.000Z");
    // A zone stated in the file wins over the device's.
    let s = stage(dir.path(), jpeg_with_exif(32, 32, None, Some("2026:10:05 14:03:09"), Some("-03:00")), 120).unwrap();
    assert_eq!(s.taken_at, "2026-10-05T17:03:09.000Z");
    // No EXIF, or an unusable date: the import time.
    for bytes in [common::jpeg(32, 32), jpeg_with_exif(32, 32, None, Some("0000:00:00 00:00:00"), None)] {
        let s = stage(dir.path(), bytes, 0).unwrap();
        let at = clock::parse_iso(&s.taken_at).expect("iso");
        assert!((clock::parse_iso(&clock::now_iso()).unwrap() - at).whole_seconds().abs() < 5);
    }
}

#[test]
fn png_with_alpha_and_garbage_and_heic() {
    let dir = tempfile::tempdir().unwrap();
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(50, 30, image::Rgba([10, 20, 30, 128]))
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let s = stage(dir.path(), png, 0).unwrap();
    assert_eq!(staged_image(dir.path(), &s.token).dimensions(), (50, 30));

    assert!(matches!(stage(dir.path(), b"not an image at all".to_vec(), 0), Err(CoreError::UnreadableImage(_))));
    assert!(matches!(stage(dir.path(), Vec::new(), 0), Err(CoreError::UnreadableImage(_))));
    let mut heic = vec![0, 0, 0, 24];
    heic.extend_from_slice(b"ftypheic");
    heic.extend_from_slice(&[0; 32]);
    assert!(matches!(stage(dir.path(), heic, 0), Err(CoreError::UnsupportedImage)));
    // Failures leave nothing behind: only the one PNG result is in tmp.
    assert_eq!(tmp_files(dir.path()).len(), 1);
}

#[test]
fn save_moves_the_photo_into_the_project_and_records_it() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let a = stage(dir.path(), jpeg_with_exif(64, 48, None, Some("2026:10:05 14:03:09"), Some("+00:00")), 0).unwrap();
    let b = stage(dir.path(), common::jpeg(64, 48), 0).unwrap();
    let input = [
        PhotoInput { token: a.token.clone(), taken_at: a.taken_at.clone(), annotation: None },
        PhotoInput { token: b.token.clone(), taken_at: "not a date".into(), annotation: None },
    ];
    let obs = save(&conn, dir.path(), &plan, &input).unwrap();

    let rows = photo_repo::list_for_observation(&conn, &obs.id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].taken_at, "2026-10-05T14:03:09.000Z");
    assert!(clock::parse_iso(&rows[1].taken_at).is_some(), "an unparseable time falls back to now");
    for r in &rows {
        assert_eq!(r.file_path, paths::photo_file(&plan.project_id, &r.id));
        assert!(r.file_path.resolve(dir.path()).is_file());
        assert!(r.visit_id.is_none());
        assert!(r.annotation_json.is_none(), "nothing drawn: NULL");
    }
    assert!(tmp_files(dir.path()).is_empty(), "staged files were moved, not copied");
}

#[test]
fn a_photo_is_required_and_a_failed_save_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    assert!(matches!(save(&conn, dir.path(), &plan, &[]), Err(CoreError::PhotoRequired)));

    // The second token has no staged file: the first photo goes back to staging, no row, no number used.
    let good = common::staged_photo(dir.path());
    let missing = PhotoInput { token: checkflat_core::ids::new_id(), taken_at: clock::now_iso(), annotation: None };
    let err = save(&conn, dir.path(), &plan, &[good.clone(), missing]).unwrap_err();
    assert!(matches!(err, CoreError::NotFound), "{err:?}");
    assert!(staging_photo(&good.token).resolve(dir.path()).is_file(), "photo returned to staging");
    assert_eq!(common::count(&conn, "observation"), 0);
    assert_eq!(common::count(&conn, "photo"), 0);
    assert_eq!(projects::get(&conn, &plan.project_id).unwrap().next_seq, 1, "no number consumed");
    let photos_dir = paths::project_dir(&plan.project_id).resolve(dir.path()).join("photos");
    assert!(std::fs::read_dir(photos_dir).map(|d| d.count()).unwrap_or(0) == 0);

    // A token that is not a UUID never touches the file system.
    let bad = PhotoInput { token: "../../etc/passwd".into(), taken_at: clock::now_iso(), annotation: None };
    assert!(matches!(save(&conn, dir.path(), &plan, &[bad]), Err(CoreError::Validation(_))));

    // The same staged photo can then be saved.
    assert!(save(&conn, dir.path(), &plan, &[good]).is_ok());
}

/// Client 2026-10-13 (D-025): one to three photos, on create and on edit.
#[test]
fn at_most_three_photos_on_create_and_edit() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let four: Vec<PhotoInput> = (0..4).map(|_| common::staged_photo(dir.path())).collect();

    // Four on create: refused before anything is touched.
    let err = save(&conn, dir.path(), &plan, &four).unwrap_err();
    assert!(matches!(err, CoreError::PhotoLimit { max: 3 }), "{err:?}");
    assert_eq!(common::count(&conn, "observation"), 0);
    assert_eq!(projects::get(&conn, &plan.project_id).unwrap().next_seq, 1, "no number consumed");
    assert_eq!(tmp_files(dir.path()).len(), 4, "staged files untouched");

    // Three is the maximum.
    let obs = save(&conn, dir.path(), &plan, &four[..3]).unwrap();
    let ids = photo_ids(&conn, &obs.id);
    assert_eq!(ids.len(), 3);
    assert_eq!(tmp_files(dir.path()).len(), 1);

    // Edit: a fourth is refused, rows and files unchanged, the staged file still in tmp/.
    let err = observations::update_observation(&conn, dir.path(), &obs.id, "more", &four[3..], &[], &[]).unwrap_err();
    assert!(matches!(err, CoreError::PhotoLimit { max: 3 }), "{err:?}");
    assert_eq!(observations::get(&conn, &obs.id).unwrap().description, "d");
    assert_eq!(photo_ids(&conn, &obs.id), ids);
    assert!(staging_photo(&four[3].token).resolve(dir.path()).is_file());

    // Edit: one out, one in keeps three.
    let edited = observations::update_observation(&conn, dir.path(), &obs.id, "swapped", &four[3..], &ids[..1], &[]).unwrap();
    assert_eq!(edited.description, "swapped");
    let now = photo_ids(&conn, &obs.id);
    assert_eq!(now.len(), 3);
    assert!(!now.contains(&ids[0]));
    assert!(tmp_files(dir.path()).is_empty());
}

#[test]
fn deleting_a_pin_removes_its_photo_files() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let obs = save(&conn, dir.path(), &plan, &[common::staged_photo(dir.path()), common::staged_photo(dir.path())]).unwrap();
    let files: Vec<_> = photo_repo::list_for_observation(&conn, &obs.id).unwrap().into_iter().map(|p| p.file_path.resolve(dir.path())).collect();
    assert!(files.iter().all(|f| f.is_file()));
    observations::delete_pin(&conn, dir.path(), &obs.id).unwrap();
    assert_eq!(common::count(&conn, "photo"), 0);
    assert!(files.iter().all(|f| !f.exists()));
}

#[test]
fn discard_removes_a_staged_photo_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let p = common::staged_photo(dir.path());
    photos::discard_staged(dir.path(), &p.token).unwrap();
    photos::discard_staged(dir.path(), &p.token).unwrap();
    assert!(tmp_files(dir.path()).is_empty());
    assert!(matches!(photos::discard_staged(dir.path(), "x/../y"), Err(CoreError::Validation(_))));
}

#[test]
fn sweep_keeps_saved_photos_and_fresh_staged_ones() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let obs = save(&conn, dir.path(), &plan, &common::one_photo(&conn)).unwrap();
    let kept = photo_repo::list_for_observation(&conn, &obs.id).unwrap()[0].file_path.resolve(dir.path());
    let staged = common::staged_photo(dir.path());
    let report = paths::sweep_orphans(&conn, dir.path(), false).unwrap();
    assert!(kept.is_file(), "referenced photo kept: {report:?}");
    assert!(staging_photo(&staged.token).resolve(dir.path()).is_file(), "a staged photo survives startup (recovery after a kill)");
    assert!(report.quarantined.is_empty(), "{report:?}");
}

fn photo_ids(conn: &Connection, obs: &str) -> Vec<String> {
    photo_repo::list_for_observation(conn, obs).unwrap().into_iter().map(|p| p.id).collect()
}

#[test]
fn edit_changes_description_and_photos_but_never_drops_the_last_photo() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let obs = save(&conn, dir.path(), &plan, &[common::staged_photo(dir.path())]).unwrap();
    let first = photo_ids(&conn, &obs.id);
    let first_file = photo_repo::list_for_observation(&conn, &obs.id).unwrap()[0].file_path.resolve(dir.path());

    // Removing the only photo without adding one is refused and changes nothing.
    let err = observations::update_observation(&conn, dir.path(), &obs.id, "new text", &[], &first, &[]).unwrap_err();
    assert!(matches!(err, CoreError::PhotoRequired), "{err:?}");
    assert_eq!(observations::get(&conn, &obs.id).unwrap().description, "d");
    assert!(first_file.is_file());

    // Swap: add one, remove the old one, new description (trimmed); ref and position untouched.
    let added = common::staged_photo(dir.path());
    let edited = observations::update_observation(&conn, dir.path(), &obs.id, "  new text ", &[added], &first, &[]).unwrap();
    assert_eq!(edited.description, "new text");
    assert_eq!((edited.display_ref.as_str(), edited.seq, edited.x_norm), (obs.display_ref.as_str(), obs.seq, obs.x_norm));
    assert!(edited.updated_at > obs.updated_at);
    let now_ids = photo_ids(&conn, &obs.id);
    assert_eq!(now_ids.len(), 1);
    assert_ne!(now_ids, first);
    assert!(!first_file.exists(), "removed photo's file deleted after the commit");
    assert!(tmp_files(dir.path()).is_empty());
}

#[test]
fn edit_rejects_foreign_photos_and_rolls_back_on_a_missing_token() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let a = save(&conn, dir.path(), &plan, &common::one_photo(&conn)).unwrap();
    let b = save(&conn, dir.path(), &plan, &common::one_photo(&conn)).unwrap();
    let b_photo = photo_ids(&conn, &b.id);
    assert!(matches!(
        observations::update_observation(&conn, dir.path(), &a.id, "x", &[], &b_photo, &[]),
        Err(CoreError::Validation(_))
    ));

    // One good staged photo + one missing token: description, rows and files unchanged, the good
    // photo back in staging.
    let a_photo = photo_ids(&conn, &a.id);
    let good = common::staged_photo(dir.path());
    let missing = PhotoInput { token: checkflat_core::ids::new_id(), taken_at: clock::now_iso(), annotation: None };
    let err = observations::update_observation(&conn, dir.path(), &a.id, "changed", &[good.clone(), missing], &a_photo, &[]).unwrap_err();
    assert!(matches!(err, CoreError::NotFound), "{err:?}");
    assert_eq!(photo_ids(&conn, &a.id), a_photo, "removal rolled back");
    assert_eq!(observations::get(&conn, &a.id).unwrap().description, "d");
    assert!(photo_repo::list_for_observation(&conn, &a.id).unwrap()[0].file_path.resolve(dir.path()).is_file());
    assert!(staging_photo(&good.token).resolve(dir.path()).is_file());
    assert!(matches!(
        observations::update_observation(&conn, dir.path(), "missing", "x", &[], &[], &[]),
        Err(CoreError::NotFound)
    ));
}

#[test]
fn an_observation_migrated_without_photos_opens_and_gets_its_first_photo_on_edit() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    common::insert_observation(&conn, "legacy", &plan.project_id, &plan.id, 1, None).unwrap();
    // Reads work with no photos at all.
    assert_eq!(observations::get(&conn, "legacy").unwrap().seq, 1);
    assert_eq!(observations::list_for_plan(&conn, &plan.id).unwrap().len(), 1);
    assert!(photo_repo::list_for_observation(&conn, "legacy").unwrap().is_empty());
    // Saving without a photo is refused; with one it works.
    assert!(matches!(observations::update_observation(&conn, dir.path(), "legacy", "t", &[], &[], &[]), Err(CoreError::PhotoRequired)));
    let o = observations::update_observation(&conn, dir.path(), "legacy", "t", &common::one_photo(&conn), &[], &[]).unwrap();
    assert_eq!(o.description, "t");
    assert_eq!(photo_ids(&conn, "legacy").len(), 1);
}

fn ellipse() -> Annotation {
    Annotation::new(vec![Shape::Ellipse { cx: 0.5, cy: 0.5, rx: 0.2, ry: 0.1 }])
}

/// Sprint 4 (M6): the shapes drawn on a photo are saved with the sheet — on create through
/// `PhotoInput`, on edit through `PhotoAnnotation` — and read back as the same shapes.
#[test]
fn annotations_are_stored_on_create_and_changed_on_edit() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let mut a = common::staged_photo(dir.path());
    a.annotation = Some(ellipse());
    let b = common::staged_photo(dir.path());
    let obs = save(&conn, dir.path(), &plan, &[a, b]).unwrap();
    let rows = photo_repo::list_for_observation(&conn, &obs.id).unwrap();
    assert_eq!(rows[0].annotation_json.as_deref(), Some(r#"{"v":1,"shapes":[{"t":"ellipse","cx":0.5,"cy":0.5,"rx":0.2,"ry":0.1}]}"#));
    assert_eq!(annotation::parse_stored(rows[0].annotation_json.as_deref()), Some(ellipse()));
    assert!(rows[1].annotation_json.is_none(), "nothing drawn: NULL");

    // Edit: draw on the second, clear the first; description and photo set untouched otherwise.
    let arrow = Annotation::new(vec![Shape::Arrow { x1: 0.1, y1: 0.1, x2: 0.9, y2: 0.9 }]);
    let changes = [
        PhotoAnnotation { photo_id: rows[1].id.clone(), annotation: Some(arrow.clone()) },
        PhotoAnnotation { photo_id: rows[0].id.clone(), annotation: None },
    ];
    let edited = observations::update_observation(&conn, dir.path(), &obs.id, "d", &[], &[], &changes).unwrap();
    assert!(edited.updated_at >= obs.updated_at, "same millisecond on a fast machine is fine");
    let rows = photo_repo::list_for_observation(&conn, &obs.id).unwrap();
    assert!(rows[0].annotation_json.is_none());
    assert_eq!(annotation::parse_stored(rows[1].annotation_json.as_deref()), Some(arrow));
    // An annotation without shapes clears as well.
    let empty = [PhotoAnnotation { photo_id: rows[1].id.clone(), annotation: Some(Annotation::new(vec![])) }];
    observations::update_observation(&conn, dir.path(), &obs.id, "d", &[], &[], &empty).unwrap();
    assert!(photo_repo::list_for_observation(&conn, &obs.id).unwrap()[1].annotation_json.is_none());
}

#[test]
fn invalid_or_foreign_annotations_are_refused_and_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let bad = Annotation::new(vec![Shape::Ellipse { cx: 1.5, cy: 0.5, rx: 0.2, ry: 0.1 }]);

    // Create: the good first photo goes back to staging, no row, no number used.
    let good = common::staged_photo(dir.path());
    let mut wrong = common::staged_photo(dir.path());
    wrong.annotation = Some(bad.clone());
    let err = save(&conn, dir.path(), &plan, &[good.clone(), wrong]).unwrap_err();
    assert!(matches!(err, CoreError::Validation(ref m) if m.starts_with("annotation: ")), "{err:?}");
    assert_eq!(common::count(&conn, "observation"), 0);
    assert_eq!(projects::get(&conn, &plan.project_id).unwrap().next_seq, 1, "no number consumed");
    assert_eq!(tmp_files(dir.path()).len(), 2, "both staged files back in tmp/");

    // Edit: an invalid annotation, or another observation's photo id → refused before anything is touched.
    let obs = save(&conn, dir.path(), &plan, &[good]).unwrap();
    let other = save(&conn, dir.path(), &plan, &common::one_photo(&conn)).unwrap();
    let mine = photo_ids(&conn, &obs.id);
    let theirs = photo_ids(&conn, &other.id);
    let added = common::staged_photo(dir.path());
    for changes in [
        vec![PhotoAnnotation { photo_id: mine[0].clone(), annotation: Some(bad.clone()) }],
        vec![PhotoAnnotation { photo_id: theirs[0].clone(), annotation: Some(ellipse()) }],
    ] {
        let err = observations::update_observation(&conn, dir.path(), &obs.id, "changed", std::slice::from_ref(&added), &[], &changes).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)), "{err:?}");
        assert_eq!(observations::get(&conn, &obs.id).unwrap().description, "d");
        assert_eq!(photo_ids(&conn, &obs.id), mine);
        assert!(staging_photo(&added.token).resolve(dir.path()).is_file(), "staged file untouched");
    }
    assert!(photo_repo::list_for_observation(&conn, &other.id).unwrap()[0].annotation_json.is_none());

    // A change for a photo that is removed in the same save is dropped, not a foreign id.
    let changes = [PhotoAnnotation { photo_id: mine[0].clone(), annotation: Some(ellipse()) }];
    let edited = observations::update_observation(&conn, dir.path(), &obs.id, "swapped", &[added], &mine, &changes).unwrap();
    assert_eq!(edited.description, "swapped");
    let rows = photo_repo::list_for_observation(&conn, &obs.id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].id, mine[0]);
    assert!(rows[0].annotation_json.is_none());
}

#[test]
fn staged_present_reports_only_files_that_exist() {
    let dir = tempfile::tempdir().unwrap();
    let a = common::staged_photo(dir.path());
    let b = common::staged_photo(dir.path());
    photos::discard_staged(dir.path(), &b.token).unwrap();
    let asked = vec![a.token.clone(), b.token.clone(), "../x".to_string(), checkflat_core::ids::new_id()];
    assert_eq!(photos::staged_present(dir.path(), &asked), vec![a.token]);
}
