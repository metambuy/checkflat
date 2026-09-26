mod common;

use checkflat_core::models::Plan;
use checkflat_core::repo::{observations, plans, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_core::{db, pdf, CoreError};
use std::io::Cursor;
use std::path::Path;

fn setup(data: &Path) -> (Connection, Plan) {
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[(2384.0, 1684.0)], 0))).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "A1").unwrap();
    (conn, plan)
}

fn next_ref(conn: &Connection, project_id: &str) -> i64 {
    projects::get(conn, project_id).unwrap().next_ref_no
}

#[test]
fn confirm_takes_consecutive_ref_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    assert_eq!(next_ref(&conn, &plan.project_id), 1);
    let refs: Vec<i64> = [(0.1, 0.2), (0.5, 0.5), (1.0, 0.0)]
        .iter()
        .map(|&(x, y)| observations::create_pin(&conn, &plan.id, x, y).unwrap().ref_no)
        .collect();
    assert_eq!(refs, vec![1, 2, 3]);
    assert_eq!(next_ref(&conn, &plan.project_id), 4);
    let pins = observations::list_for_plan(&conn, &plan.id).unwrap();
    assert_eq!(pins.iter().map(|o| o.ref_no).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!((pins[0].x_norm, pins[0].y_norm), (0.1, 0.2));
    assert_eq!(pins[0].project_id, plan.project_id, "project taken from the plan");
}

#[test]
fn cancelled_draft_consumes_no_number() {
    // A draft is UI state only: cancelling it never reaches the core. The next confirm after a
    // cancelled draft therefore continues without a gap.
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    assert_eq!(observations::create_pin(&conn, &plan.id, 0.2, 0.2).unwrap().ref_no, 1);
    // [draft placed, then cancelled: no core call]
    assert_eq!(next_ref(&conn, &plan.project_id), 2);
    assert_eq!(observations::create_pin(&conn, &plan.id, 0.3, 0.3).unwrap().ref_no, 2);
}

#[test]
fn invalid_positions_are_rejected_without_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    for (x, y) in [(-0.01, 0.5), (0.5, 1.01), (f64::NAN, 0.5), (f64::INFINITY, 0.0)] {
        let err = observations::create_pin(&conn, &plan.id, x, y).unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)), "{x},{y}: {err}");
    }
    assert_eq!(next_ref(&conn, &plan.project_id), 1);
    assert!(matches!(observations::create_pin(&conn, "missing", 0.5, 0.5), Err(CoreError::NotFound)));
    let pin = observations::create_pin(&conn, &plan.id, 0.5, 0.5).unwrap();
    assert!(matches!(observations::move_pin(&conn, &pin.id, 2.0, 0.5), Err(CoreError::Validation(_))));
    assert_eq!(observations::get(&conn, &pin.id).unwrap().x_norm, 0.5);
}

#[test]
fn failed_insert_rolls_back_the_counter() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    conn.execute_batch(
        "CREATE TEMP TRIGGER fail_insert BEFORE INSERT ON observation BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    )
    .unwrap();
    assert!(matches!(observations::create_pin(&conn, &plan.id, 0.5, 0.5), Err(CoreError::Db(_))));
    assert_eq!(next_ref(&conn, &plan.project_id), 1, "counter unchanged");
    conn.execute_batch("DROP TRIGGER fail_insert;").unwrap();
    assert_eq!(observations::create_pin(&conn, &plan.id, 0.5, 0.5).unwrap().ref_no, 1);
}

#[test]
fn ref_numbers_never_collide_with_existing_ones() {
    // A ref edited upwards (M5, Sprint 3) or an imported row must not make the next confirm fail
    // on UNIQUE(project_id, ref_no).
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let pin = observations::create_pin(&conn, &plan.id, 0.5, 0.5).unwrap();
    conn.execute("UPDATE observation SET ref_no = 7 WHERE id = ?1", [&pin.id]).unwrap();
    assert_eq!(observations::create_pin(&conn, &plan.id, 0.5, 0.5).unwrap().ref_no, 8);
    assert_eq!(next_ref(&conn, &plan.project_id), 9);
}

#[test]
fn move_and_delete_pins() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let a = observations::create_pin(&conn, &plan.id, 0.1, 0.1).unwrap();
    let b = observations::create_pin(&conn, &plan.id, 0.2, 0.2).unwrap();
    let moved = observations::move_pin(&conn, &a.id, 0.9, 0.8).unwrap();
    assert_eq!((moved.x_norm, moved.y_norm, moved.ref_no), (0.9, 0.8, 1));
    assert!(moved.updated_at >= a.updated_at);
    // The plan cannot be deleted while it has pins.
    assert!(matches!(plans::delete(&conn, data, &plan.id), Err(CoreError::PlanHasObservations { count: 2 })));
    observations::delete_pin(&conn, &b.id).unwrap();
    assert!(matches!(observations::delete_pin(&conn, &b.id), Err(CoreError::NotFound)));
    assert!(matches!(observations::move_pin(&conn, &b.id, 0.5, 0.5), Err(CoreError::NotFound)));
    // Deleted numbers are not reused.
    assert_eq!(observations::create_pin(&conn, &plan.id, 0.3, 0.3).unwrap().ref_no, 3);
}
