mod common;

use std::path::Path;

use checkflat_core::annotation::{Annotation, Shape};
use checkflat_core::i18n::Lang;
use checkflat_core::refs::{Scope, DEFAULT_TEMPLATE};
use checkflat_core::repo::{photos, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_report::{collect, prepare, render, ReportInput};

const PT_TEXT: &str = "Fissura horizontal na parede da sala, junto ao rodapé, com cerca de 1,2 m de comprimento e abertura visível a olho nu. \
A superfície apresenta manchas de humidade na zona inferior, o que indica possível infiltração pela junta da caixilharia. \
Recomenda-se abrir a zona afetada, verificar a impermeabilização e refazer o acabamento após a correção; reavaliar na próxima visita.";

/// Project numbering per fraction (long pin labels such as `PC-01`) with an A1 plan (pyramid to
/// 4096) and an A4 plan (never prepared); two observations.
fn fixture(data: &Path) -> (Connection, String) {
    let conn = common::open(data);
    let pid = common::project(&conn, "Edifício LAMAS", "Rua das Flores 12, Porto");
    projects::update_ref_settings(&conn, &pid, "LAM", DEFAULT_TEMPLATE, Scope::Fraction).unwrap();
    let a1 = common::plan(&conn, data, &pid, "Piso 1", common::A1);
    common::pyramid(&conn, data, &a1, &[1024, 2048, 4096]);
    let a4 = common::plan(&conn, data, &pid, "Piso 2", common::A4);
    common::observation_in(&conn, data, &a1, 0.3, 0.6, "A", PT_TEXT, &[common::photo(data, 1600, 1200, "2026-10-20T10:00:00.000Z")]);
    common::observation_in(
        &conn,
        data,
        &a4,
        0.98,
        0.02,
        "PC",
        "",
        &[common::photo(data, 1200, 1600, "2026-10-30T10:00:00.000Z"), common::photo(data, 600, 400, "2026-10-30T11:00:00.000Z")],
    );
    (conn, pid)
}

fn save(name: &str, pdf: &[u8]) {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/report-tests");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join(name), pdf).unwrap();
}

fn make(conn: &Connection, data: &Path, pid: &str, lang: Lang) -> ReportInput {
    let src = collect(conn, data, pid, lang, "Europe/Lisbon", 60).unwrap();
    assert_eq!(src.observations.len(), 2);
    prepare(data, src).unwrap()
}

#[test]
fn the_cover_date_is_the_reports_own_instant() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, pid) = fixture(dir.path());
    let mut src = collect(&conn, dir.path(), &pid, Lang::En, "Europe/Lisbon", 60).unwrap();
    assert!(checkflat_core::clock::parse_iso(&src.created_at).is_some(), "{}", src.created_at);
    // 23:30 UTC on the 24th is already the 25th in Lisbon (UTC+1 until the DST change).
    src.created_at = "2026-10-24T23:30:00.000Z".into();
    assert_eq!(src.clock.iso_date(&src.created_at), "2026-10-25");
    assert_eq!(prepare(dir.path(), src).unwrap().data.date, "25.10.2026");
}

#[test]
fn report_has_a_cover_and_the_observations_in_a_continuous_list_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, pid) = fixture(dir.path());
    let input = make(&conn, dir.path(), &pid, Lang::Pt);
    let d = &input.data;
    assert_eq!((d.lang, d.region), ("pt", Some("PT")));
    assert_eq!(d.title, "Relatório de visita à obra");
    assert_eq!(d.labels.description, "Descrição");
    assert_eq!(d.project.name, "Edifício LAMAS");

    // Newest first: the A4 observation (no pyramid) comes before the A1 one.
    let (a4, a1) = (&d.observations[0], &d.observations[1]);
    assert_eq!((a4.plan_title.as_str(), a1.plan_title.as_str()), ("Piso 2", "Piso 1"));
    assert_eq!((a4.display_ref.as_str(), a1.display_ref.as_str()), ("LAM-PC-01", "LAM-A-01"));
    assert_eq!((a4.marker.as_str(), a1.marker.as_str()), ("PC-01", "A-01"), "the app's pin label");
    assert!(a4.crop.is_none() && a4.overview.is_none(), "plan never prepared on this device");
    assert_eq!(a4.photos.len(), 2);
    assert_eq!(a4.photos[0].caption, "1. 30.10.2026 10:00", "after the DST change: UTC+0");
    assert_eq!(a4.photos[1].caption, "2. 30.10.2026 11:00");
    let crop = a1.crop.as_ref().unwrap();
    let ov = a1.overview.as_ref().unwrap();
    assert!((crop.pin_x - 0.5).abs() < 1e-9 && (crop.pin_y - 0.5).abs() < 1e-9, "pin centred: {crop:?}");
    assert!((ov.box_x - 0.2).abs() < 1e-9 && (ov.box_w - 0.2).abs() < 1e-9, "box: {ov:?}");
    assert!((ov.aspect - 1684.0 / 2384.0).abs() < 0.01);
    assert_eq!(a1.photos[0].caption, "1. 20.10.2026 11:00", "before the DST change: UTC+1");
    assert_eq!(a1.description, PT_TEXT);
    assert_eq!(input.stats.photos, 3);
    assert_eq!(input.stats.skipped_photos, 0);
    assert_eq!(input.stats.without_snapshot, 1);
    assert_eq!(input.files.iter().filter(|(n, _)| n.starts_with("img/overview")).count(), 1);
    // Report photos are ≤ 640 px (REPORT_PHOTO_PX); a small one is embedded as stored.
    let big = input.files.iter().find(|(n, _)| n == "img/photo-1-0.jpg").unwrap();
    let img = image::load_from_memory(&big.1).unwrap();
    assert_eq!((img.width(), img.height()), (640, 480));

    let r = render(&input).unwrap();
    assert!(r.pdf.starts_with(b"%PDF"));
    save("report-pt.pdf", &r.pdf);
    // Cover; the A4 observation (no snapshot) with both photos; the A1 observation (D-025).
    assert_eq!(r.pages, 3);
    eprintln!("[measure] pt: {} bytes, compile {} ms, pdf {} ms, stats {:?}", r.pdf.len(), r.compile_ms, r.pdf_ms, input.stats);

    let en = make(&conn, dir.path(), &pid, Lang::En);
    assert_eq!((en.data.lang, en.data.region), ("en", None));
    assert_eq!(en.data.labels.images, "Images");
    let r = render(&en).unwrap();
    save("report-en.pdf", &r.pdf);
    assert_eq!(r.pages, 3);
}

/// Sprint 4 (M6): a photo's stored shapes are drawn on its report copy; a photo without shapes
/// that is already small is embedded byte for byte as stored.
#[test]
fn annotated_photos_reach_the_report_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "P", "");
    let a1 = common::plan(&conn, data, &pid, "Piso 1", common::A1);
    let mut drawn = common::photo(data, 1600, 1200, "2026-10-20T10:00:00.000Z");
    drawn.annotation = Some(Annotation::new(vec![Shape::Ellipse { cx: 0.5, cy: 0.5, rx: 0.25, ry: 0.25 }]));
    let plain = common::photo(data, 640, 480, "2026-10-20T11:00:00.000Z");
    let obs = common::observation(&conn, data, &a1, 0.3, 0.6, "d", &[drawn, plain]);
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "Europe/Lisbon", 60).unwrap()).unwrap();
    let file = |name: &str| input.files.iter().find(|(f, _)| f == name).map(|(_, b)| b.clone()).unwrap_or_else(|| panic!("{name} missing"));

    let img = image::load_from_memory(&file("img/photo-0-0.jpg")).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (640, 480));
    let ring = img.get_pixel(160, 240);
    assert!(ring[0] > 180 && ring[1] < 60 && ring[2] < 60, "red on the ring: {ring:?}");
    let centre = img.get_pixel(320, 240);
    assert!(centre[1] > 90, "the fixture's own colour inside the ring: {centre:?}");

    let stored = photos::list_for_observation(&conn, &obs).unwrap()[1].file_path.resolve(data);
    assert_eq!(file("img/photo-0-1.jpg"), std::fs::read(stored).unwrap(), "no shapes, ≤ 640 px: embedded as stored");
    assert_eq!(input.stats.photos, 2);
    assert_eq!(input.stats.skipped_photos, 0);
}

#[test]
fn empty_project_and_missing_photo_file() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Empty", "");
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    assert!(input.data.observations.is_empty() && input.files.is_empty());
    let r = render(&input).unwrap();
    assert_eq!(r.pages, 1, "cover only");

    // A photo whose file vanished is skipped, not fatal, and the captions stay consecutive.
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    let photos = [
        common::photo_seeded(data, 100, 100, "2026-01-01T00:00:00.000Z", 1),
        common::photo_seeded(data, 100, 100, "2026-01-01T01:00:00.000Z", 2),
        common::photo_seeded(data, 100, 100, "2026-01-01T02:00:00.000Z", 3),
    ];
    let id = common::observation(&conn, data, &plan, 0.5, 0.5, "x", &photos);
    let middle = checkflat_core::repo::photos::list_for_observation(&conn, &id).unwrap().remove(1);
    std::fs::remove_file(middle.file_path.resolve(data)).unwrap();
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    assert_eq!((input.stats.photos, input.stats.skipped_photos, input.stats.without_snapshot), (2, 1, 1));
    let captions: Vec<&str> = input.data.observations[0].photos.iter().map(|p| p.caption.as_str()).collect();
    assert_eq!(captions, ["1. 01.01.2026 00:00", "2. 01.01.2026 02:00"], "the third photo is numbered 2");
    assert_eq!(render(&input).unwrap().pages, 2, "cover; the head (no snapshot) and two square photos fit one page");
    assert!(matches!(collect(&conn, data, "nope", Lang::En, "UTC", 0), Err(checkflat_report::ReportError::Core(checkflat_core::CoreError::NotFound))));
}

/// Three photos one below the other take more than a page with the head: the list flows on,
/// and a description longer than a page is not kept together (it would be clipped) but printed.
#[test]
fn long_blocks_flow_over_pages() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Flow", "");
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    common::pyramid(&conn, data, &plan, &[1024, 2048, 4096]);
    let three = |n: u32| {
        [
            common::photo_seeded(data, 1600, 1200, "2026-01-01T00:00:00.000Z", n),
            common::photo_seeded(data, 1200, 1600, "2026-01-01T01:00:00.000Z", n + 1),
            common::photo_seeded(data, 1600, 1200, "2026-01-01T02:00:00.000Z", n + 2),
        ]
    };
    common::observation(&conn, data, &plan, 0.5, 0.5, "short", &three(0));
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    assert_eq!(render(&input).unwrap().pages, 3, "cover + head/description/first photos + the rest");

    common::observation(&conn, data, &plan, 0.5, 0.5, &PT_TEXT.repeat(40), &three(3));
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    let pages = render(&input).unwrap().pages;
    assert!(pages >= 7, "the long description spreads over pages instead of being clipped: {pages}");
}

/// The text of each page of a rendered report, as lopdf extracts it.
fn page_texts(pdf: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(pdf).unwrap();
    doc.get_pages().keys().map(|&n| doc.extract_text(&[n]).unwrap()).collect()
}

/// An observation without photos (all its files gone) gets no "Images:" label, so none sticks
/// to the next observation's head (D-025).
#[test]
fn no_images_label_without_photos() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Labels", "");
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    common::observation(&conn, data, &plan, 0.4, 0.4, "with photo", &[common::photo_seeded(data, 100, 100, "2026-01-01T00:00:00.000Z", 1)]);
    let id = common::observation(&conn, data, &plan, 0.6, 0.6, "without photo", &[common::photo_seeded(data, 100, 100, "2026-01-01T01:00:00.000Z", 2)]);
    for p in checkflat_core::repo::photos::list_for_observation(&conn, &id).unwrap() {
        std::fs::remove_file(p.file_path.resolve(data)).unwrap();
    }
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    // Newest first: the photo-less observation comes first, directly above the other one.
    let obs = &input.data.observations;
    assert_eq!((obs[0].description.as_str(), obs[0].photos.len(), obs[1].photos.len()), ("without photo", 0, 1));
    let text = page_texts(&render(&input).unwrap().pdf).concat();
    assert_eq!(text.matches("Images").count(), 1, "one label, for the observation with a photo: {text}");
    let (label, second) = (text.find("Images").unwrap(), text.find("with photo").unwrap());
    assert!(label > second, "the label follows the second observation's description: {text}");
}

/// Line breaks and blank lines in a description stay: the zeroed paragraph spacing of the list
/// does not run the lines together.
#[test]
fn description_keeps_its_lines() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Lines", "");
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    let description = "Fissura na parede.\nManchas de humidade.\n\nRecomenda-se reparar.";
    common::observation(&conn, data, &plan, 0.5, 0.5, description, &[common::photo(data, 100, 100, "2026-01-01T00:00:00.000Z")]);
    let input = prepare(data, collect(&conn, data, &pid, Lang::En, "UTC", 0).unwrap()).unwrap();
    let text = page_texts(&render(&input).unwrap().pdf).concat();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for line in ["Fissura na parede.", "Manchas de humidade.", "Recomenda-se reparar."] {
        assert!(lines.contains(&line), "{line:?} is a line of its own: {lines:?}");
    }
}

/// The Sprint 5 load case (D-024): 50 observations × 3 photos on an A1 + an A4 plan, both with
/// a full synthetic pyramid.
fn fixture_50(data: &Path) -> (Connection, String) {
    let conn = common::open(data);
    let pid = common::project(&conn, "Edifício LAMAS", "Rua das Flores 12, Porto");
    let a1 = common::plan(&conn, data, &pid, "Piso 1", common::A1);
    common::pyramid(&conn, data, &a1, &[1024, 2048, 4096, 8192]);
    let a4 = common::plan(&conn, data, &pid, "Piso 2", common::A4);
    common::pyramid(&conn, data, &a4, &[1024, 2048, 4096, 8192]);
    for i in 0..50u32 {
        let plan = if i % 3 == 0 { &a4 } else { &a1 };
        let (x, y) = (f64::from(i % 7) / 6.0, f64::from(i % 5) / 4.0);
        // Every photo unique: Typst memoizes decoded images by content.
        let photos = [
            common::photo_seeded(data, 1600, 1200, "2026-10-20T10:00:00.000Z", i * 3),
            common::photo_seeded(data, 1200, 1600, "2026-10-30T10:00:00.000Z", i * 3 + 1),
            common::photo_seeded(data, 1600, 1200, "2026-10-30T11:00:00.000Z", i * 3 + 2),
        ];
        common::observation(&conn, data, plan, x, y, &format!("{i}. {PT_TEXT}"), &photos);
    }
    (conn, pid)
}

/// Writes the load fixture as an app data dir at `$CHECKFLAT_FIXTURE_DIR` (to push into the
/// emulator's app data with `adb` and press Generate report there, D-024).
/// `CHECKFLAT_FIXTURE_DIR=/tmp/fixture cargo test -p checkflat-report --test report -- --ignored write_fixture`
#[test]
#[ignore]
fn write_fixture() {
    let Ok(dir) = std::env::var("CHECKFLAT_FIXTURE_DIR") else {
        eprintln!("CHECKFLAT_FIXTURE_DIR not set; nothing written");
        return;
    };
    let data = Path::new(&dir);
    std::fs::create_dir_all(data).unwrap();
    let (conn, _) = fixture_50(data);
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    eprintln!("fixture written to {dir}");
}

/// `cargo test --release -p checkflat-report --test report -- --ignored --nocapture measure`
/// With `CHECKFLAT_DATA_DIR=<an app data dir>` (e.g. pulled from the emulator after seeding),
/// measures that directory's first project instead of the synthetic fixture.
#[test]
#[ignore]
fn measure_50_observations() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::env::var("CHECKFLAT_DATA_DIR").ok();
    let data = real.as_deref().map(Path::new).unwrap_or(dir.path());
    let (conn, pid) = match &real {
        Some(_) => {
            let conn = common::open(data);
            let pid: String = conn.query_row("SELECT id FROM project ORDER BY updated_at DESC LIMIT 1", [], |r| r.get(0)).unwrap();
            (conn, pid)
        }
        None => fixture_50(data),
    };
    let started = std::time::Instant::now();
    let src = collect(&conn, data, &pid, Lang::Pt, "Europe/Lisbon", 60).unwrap();
    let collect_ms = started.elapsed().as_millis();
    let input = prepare(data, src).unwrap();
    let prepare_ms = started.elapsed().as_millis() - collect_ms;
    let r = render(&input).unwrap();
    eprintln!(
        "[measure] 50 × 3: {} pages, {} bytes; collect {collect_ms} ms, prepare {prepare_ms} ms (snapshots {} ms, photos {} ms), compile {} ms, pdf {} ms, total {} ms",
        r.pages,
        r.pdf.len(),
        input.stats.snapshot_ms,
        input.stats.photo_ms,
        r.compile_ms,
        r.pdf_ms,
        started.elapsed().as_millis()
    );
    save("report-50.pdf", &r.pdf);
    assert!(r.pages >= 51);
}
