mod common;

use checkflat_core::repo::{plans, projects};
use checkflat_core::{db, paths, pdf, CoreError};
use common::*;
use std::io::Cursor;

fn a1() -> Vec<u8> {
    pdf::make_pdf(&[(2384.0, 1684.0)], 0)
}

fn tmp_is_empty(data: &std::path::Path) -> bool {
    std::fs::read_dir(data.join("tmp")).map(|d| d.count() == 0).unwrap_or(true)
}

#[test]
fn stage_then_import_single_page() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();

    let staged = plans::stage(data, Cursor::new(a1())).unwrap();
    assert_eq!(staged.info.page_count, 1);
    assert!(paths::staging_file(&staged.token).resolve(data).is_file());

    let title = plans::default_title("Piso 3.PDF");
    assert_eq!(title, "Piso 3");
    let plan = plans::import(&conn, data, &p.id, &staged.token, &title).unwrap();
    assert_eq!(plan.title, "Piso 3");
    assert_eq!((plan.width_pt, plan.height_pt), (2384.0, 1684.0));
    assert_eq!(plan.file_path.as_str(), format!("projects/{}/plans/{}.pdf", p.id, plan.id));
    assert!(plan.file_path.resolve(data).is_file());
    assert!(tmp_is_empty(data), "staging file moved, not copied");
    assert_eq!(plans::list_for_project(&conn, &p.id).unwrap().len(), 1);
    let summary = projects::list(&conn).unwrap();
    assert_eq!(summary[0].plan_count, 1);

    // Import with the same token again: staging file is gone.
    assert!(matches!(plans::import(&conn, data, &p.id, &staged.token, "x"), Err(CoreError::NotFound)));
    // Unknown project.
    let s2 = plans::stage(data, Cursor::new(a1())).unwrap();
    assert!(matches!(plans::import(&conn, data, "missing", &s2.token, "x"), Err(CoreError::NotFound)));
    // Empty title rejected, staging file kept for retry.
    assert!(matches!(plans::import(&conn, data, &p.id, &s2.token, "  "), Err(CoreError::Validation(_))));
    assert!(paths::staging_file(&s2.token).resolve(data).is_file());
    plans::discard_staged(data, &s2.token).unwrap();
    assert!(tmp_is_empty(data));

    let renamed = plans::rename(&conn, &plan.id, " Piso 3 (rev B) ").unwrap();
    assert_eq!(renamed.title, "Piso 3 (rev B)");
    plans::delete(&conn, data, &plan.id).unwrap();
    assert!(!plan.file_path.resolve(data).exists());
    assert!(matches!(plans::delete(&conn, data, &plan.id), Err(CoreError::NotFound)));
}

#[test]
fn multi_page_and_garbage_are_rejected_without_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let two = pdf::make_pdf(&[(595.0, 842.0), (595.0, 842.0)], 0);
    assert!(matches!(plans::stage(data, Cursor::new(two)), Err(CoreError::MultiPage { pages: 2 })));
    assert!(matches!(plans::stage(data, Cursor::new(b"nope".to_vec())), Err(CoreError::Unreadable(_))));
    assert!(tmp_is_empty(data));
}

#[test]
fn rotated_page_swaps_dimensions_on_import() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[(2384.0, 1684.0)], 270))).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "rot").unwrap();
    assert_eq!((plan.width_pt, plan.height_pt), (1684.0, 2384.0));
}

#[test]
fn failing_insert_leaves_no_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    // Make the insert fail: drop the plan table's project after staging but keep the id check
    // passing is not possible, so simulate with a trigger that aborts inserts.
    conn.execute_batch("CREATE TRIGGER fail_insert BEFORE INSERT ON plan BEGIN SELECT RAISE(ABORT, 'boom'); END;")
        .unwrap();
    let staged = plans::stage(data, Cursor::new(a1())).unwrap();
    assert!(matches!(plans::import(&conn, data, &p.id, &staged.token, "t"), Err(CoreError::Db(_))));
    let plans_dir = paths::project_dir(&p.id).resolve(data).join("plans");
    let leftovers = std::fs::read_dir(&plans_dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(leftovers, 0, "renamed file removed after failed insert");
    assert_eq!(count(&conn, "plan"), 0);
}

#[test]
fn delete_blocked_while_observations_exist() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(a1())).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "t").unwrap();
    insert_observation(&conn, "o1", &p.id, &plan.id, 1, None).unwrap();
    let err = plans::delete(&conn, data, &plan.id).unwrap_err();
    assert!(matches!(err, CoreError::PlanHasObservations { count: 1 }));
    assert_eq!(err.code(), "plan_has_observations");
    assert!(plan.file_path.resolve(data).is_file(), "file untouched");
    assert_eq!(count(&conn, "plan"), 1);
    conn.execute("DELETE FROM observation WHERE id = 'o1'", []).unwrap();
    plans::delete(&conn, data, &plan.id).unwrap();
    assert!(!plan.file_path.resolve(data).exists());
}

#[test]
fn default_title_rules() {
    assert_eq!(plans::default_title("plan.pdf"), "plan");
    assert_eq!(plans::default_title("PLAN.PDF"), "PLAN");
    assert_eq!(plans::default_title("archive.tar.pdf"), "archive.tar");
    assert_eq!(plans::default_title(".pdf"), ".pdf");
    assert_eq!(plans::default_title("  notes.txt "), "notes.txt");
}

/// Local-only sanity check against the client's real plans (Examples/ is gitignored; skipped when absent).
#[test]
fn real_example_plans_if_present() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Examples");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("pdf")) != Some(true) {
            continue;
        }
        let info = pdf::inspect_file(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        eprintln!("[real] {} -> {:?}", path.file_name().unwrap().to_string_lossy(), info);
        assert_eq!(info.page_count, 1);
        assert!(info.width_pt > 100.0 && info.height_pt > 100.0);
    }
}
