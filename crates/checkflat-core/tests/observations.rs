mod common;

use checkflat_core::models::Plan;
use checkflat_core::refs::Scope;
use checkflat_core::repo::{fractions, observations, plans, projects};
use checkflat_core::rusqlite::Connection;
use checkflat_core::{db, pdf, CoreError};
use std::io::Cursor;
use std::path::Path;

const TEMPLATE: &str = "{PROJ}-{FRAC}-{SEQ:2}";

fn setup(data: &Path) -> (Connection, Plan) {
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    let staged = plans::stage(data, Cursor::new(pdf::make_pdf(&[(2384.0, 1684.0)], 0))).unwrap();
    let plan = plans::import(&conn, data, &p.id, &staged.token, "A1").unwrap();
    (conn, plan)
}

/// A project numbered per fraction with code `LAM`.
fn setup_fraction_scope(data: &Path) -> (Connection, Plan) {
    let (conn, plan) = setup(data);
    projects::update_ref_settings(&conn, &plan.project_id, "LAM", TEMPLATE, Scope::Fraction).unwrap();
    (conn, plan)
}

fn next_seq(conn: &Connection, project_id: &str) -> i64 {
    projects::get(conn, project_id).unwrap().next_seq
}

fn create(conn: &Connection, plan: &Plan, fraction: &str) -> checkflat_core::Result<checkflat_core::models::Observation> {
    observations::create_observation(conn, &plan.id, 0.5, 0.5, fraction, "")
}

#[test]
fn project_scope_takes_consecutive_numbers_and_renders_refs() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    assert_eq!(next_seq(&conn, &plan.project_id), 1);
    let pins: Vec<_> = [(0.1, 0.2), (0.5, 0.5), (1.0, 0.0)]
        .iter()
        .map(|&(x, y)| observations::create_observation(&conn, &plan.id, x, y, "", " first ").unwrap())
        .collect();
    assert_eq!(pins.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(pins[0].description, "first");
    assert_eq!((pins[0].display_ref.as_str(), pins[0].marker.as_str()), ("01", "1"), "no code, no fraction");
    assert_eq!(next_seq(&conn, &plan.project_id), 4);
    let listed = observations::list_for_plan(&conn, &plan.id).unwrap();
    assert_eq!(listed.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!((listed[0].x_norm, listed[0].y_norm), (0.1, 0.2));
    assert_eq!(listed[0].project_id, plan.project_id, "project taken from the plan");

    // A fraction is optional in this scope and does not start its own sequence.
    let a = create(&conn, &plan, "A").unwrap();
    assert_eq!((a.seq, a.fraction.as_str(), a.display_ref.as_str(), a.marker.as_str()), (4, "A", "A-04", "4"));
    // Settings change how every ref renders, retroactively.
    projects::update_ref_settings(&conn, &plan.project_id, "LAM", "{PROJ}/{FRAC}/{SEQ:3}", Scope::Project).unwrap();
    let refs: Vec<String> = observations::list_for_plan(&conn, &plan.id).unwrap().into_iter().map(|o| o.display_ref).collect();
    assert_eq!(refs, ["LAM/001", "LAM/002", "LAM/003", "LAM/A/004"]);
}

#[test]
fn fraction_scope_numbers_each_fraction_independently() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir.path());
    let a1 = create(&conn, &plan, "A").unwrap();
    let a2 = create(&conn, &plan, "A").unwrap();
    let pc = create(&conn, &plan, "PC").unwrap();
    assert_eq!((a1.display_ref.as_str(), a1.marker.as_str()), ("LAM-A-01", "A-01"));
    assert_eq!((a2.display_ref.as_str(), a2.marker.as_str()), ("LAM-A-02", "A-02"));
    assert_eq!((pc.display_ref.as_str(), pc.marker.as_str()), ("LAM-PC-01", "PC-01"));
    assert_eq!(next_seq(&conn, &plan.project_id), 1, "project counter untouched");
    let fr = fractions::list(&conn, &plan.project_id).unwrap();
    assert_eq!(fr.iter().map(|f| (f.code.as_str(), f.next_seq)).collect::<Vec<_>>(), [("A", 3), ("PC", 2)]);
    // Listed by fraction, then sequence.
    let order: Vec<String> = observations::list_for_plan(&conn, &plan.id).unwrap().into_iter().map(|o| o.display_ref).collect();
    assert_eq!(order, ["LAM-A-01", "LAM-A-02", "LAM-PC-01"]);
    // No fraction is an error here, with nothing written.
    assert!(matches!(create(&conn, &plan, ""), Err(CoreError::FractionRequired)));
    assert!(matches!(create(&conn, &plan, "  "), Err(CoreError::FractionRequired)));
    assert_eq!(observations::list_for_plan(&conn, &plan.id).unwrap().len(), 3);
    // Issuing a number still marks the project as worked on (the projects list orders by it),
    // even though the project counter is not the one that moved.
    conn.execute("UPDATE project SET updated_at = '2000-01-01T00:00:00.000Z' WHERE id = ?1", [&plan.project_id]).unwrap();
    let b = create(&conn, &plan, "B").unwrap();
    assert_eq!(projects::get(&conn, &plan.project_id).unwrap().updated_at, b.created_at, "project.updated_at bumped in fraction scope");
}

#[test]
fn fraction_codes_are_case_insensitive_and_stored_canonically() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir.path());
    let first = create(&conn, &plan, "pc").unwrap();
    let second = create(&conn, &plan, "PC").unwrap();
    assert_eq!((first.display_ref.as_str(), second.display_ref.as_str()), ("LAM-pc-01", "LAM-pc-02"));
    assert_eq!((first.fraction.as_str(), second.fraction.as_str()), ("pc", "pc"), "the typed text is never stored");
    assert_eq!(fractions::list(&conn, &plan.project_id).unwrap().len(), 1, "one fraction row");
    // Duplicate numbers in one fraction are refused by the DB whatever the case.
    assert!(common::insert_observation_in(&conn, "dup", &plan.project_id, &plan.id, "Pc", 1, None).is_err());
}

#[test]
fn cancelled_draft_consumes_no_number() {
    // A draft is UI state only: cancelling it never reaches the core. The next save after a
    // cancelled draft therefore continues without a gap.
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    assert_eq!(create(&conn, &plan, "").unwrap().seq, 1);
    // [draft placed, then cancelled: no core call]
    assert_eq!(next_seq(&conn, &plan.project_id), 2);
    assert_eq!(create(&conn, &plan, "").unwrap().seq, 2);
}

#[test]
fn invalid_input_is_rejected_without_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    for (x, y) in [(-0.01, 0.5), (0.5, 1.01), (f64::NAN, 0.5), (f64::INFINITY, 0.0)] {
        let err = observations::create_observation(&conn, &plan.id, x, y, "", "").unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)), "{x},{y}: {err}");
    }
    assert!(matches!(create(&conn, &plan, "a b"), Err(CoreError::Validation(_))), "bad fraction code");
    assert_eq!(next_seq(&conn, &plan.project_id), 1);
    assert!(fractions::list(&conn, &plan.project_id).unwrap().is_empty());
    assert!(matches!(observations::create_observation(&conn, "missing", 0.5, 0.5, "", ""), Err(CoreError::NotFound)));
    let pin = create(&conn, &plan, "").unwrap();
    assert!(matches!(observations::move_pin(&conn, &pin.id, 2.0, 0.5), Err(CoreError::Validation(_))));
    assert_eq!(observations::get(&conn, &pin.id).unwrap().x_norm, 0.5);
}

#[test]
fn failed_insert_rolls_back_counter_and_new_fraction() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir.path());
    conn.execute_batch(
        "CREATE TEMP TRIGGER fail_insert BEFORE INSERT ON observation BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    )
    .unwrap();
    assert!(matches!(create(&conn, &plan, "A"), Err(CoreError::Db(_))));
    assert!(fractions::list(&conn, &plan.project_id).unwrap().is_empty(), "fraction added in the same transaction is gone");
    conn.execute_batch("DROP TRIGGER fail_insert;").unwrap();
    assert_eq!(create(&conn, &plan, "A").unwrap().display_ref, "LAM-A-01");
    assert_eq!(fractions::list(&conn, &plan.project_id).unwrap()[0].next_seq, 2);
}

#[test]
fn numbers_never_collide_with_existing_ones() {
    // A seq edited upwards (M5) or an imported row must not make the next save fail on
    // UNIQUE(project_id, seq_key, seq) — in either scope.
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let pin = create(&conn, &plan, "").unwrap();
    conn.execute("UPDATE observation SET seq = 7 WHERE id = ?1", [&pin.id]).unwrap();
    assert_eq!(create(&conn, &plan, "").unwrap().seq, 8);
    assert_eq!(next_seq(&conn, &plan.project_id), 9);

    let dir2 = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir2.path());
    let a = create(&conn, &plan, "A").unwrap();
    conn.execute("UPDATE observation SET seq = 5 WHERE id = ?1", [&a.id]).unwrap();
    assert_eq!(create(&conn, &plan, "A").unwrap().seq, 6);
    assert_eq!(create(&conn, &plan, "B").unwrap().seq, 1, "other fractions unaffected");
}

#[test]
fn deleted_numbers_are_not_reused_in_either_scope() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let (conn, plan) = setup(data);
    let a = create(&conn, &plan, "").unwrap();
    let b = create(&conn, &plan, "").unwrap();
    let moved = observations::move_pin(&conn, &a.id, 0.9, 0.8).unwrap();
    assert_eq!((moved.x_norm, moved.y_norm, moved.seq), (0.9, 0.8, 1));
    assert!(moved.updated_at >= a.updated_at);
    // The plan cannot be deleted while it has pins.
    assert!(matches!(plans::delete(&conn, data, &plan.id), Err(CoreError::PlanHasObservations { count: 2 })));
    observations::delete_pin(&conn, &b.id).unwrap();
    assert!(matches!(observations::delete_pin(&conn, &b.id), Err(CoreError::NotFound)));
    assert!(matches!(observations::move_pin(&conn, &b.id, 0.5, 0.5), Err(CoreError::NotFound)));
    assert_eq!(create(&conn, &plan, "").unwrap().seq, 3);

    let dir2 = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir2.path());
    let a1 = create(&conn, &plan, "A").unwrap();
    observations::delete_pin(&conn, &a1.id).unwrap();
    assert_eq!(create(&conn, &plan, "A").unwrap().display_ref, "LAM-A-02");
    // Deleting every observation does not reset anything either.
    for o in observations::list_for_plan(&conn, &plan.id).unwrap() {
        observations::delete_pin(&conn, &o.id).unwrap();
    }
    assert_eq!(create(&conn, &plan, "A").unwrap().display_ref, "LAM-A-03");
}

#[test]
fn fraction_delete_only_when_it_never_issued_a_number() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir.path());
    let a1 = create(&conn, &plan, "A").unwrap();
    let a = fractions::find(&conn, &plan.project_id, "a").unwrap().unwrap();
    assert!(matches!(fractions::delete(&conn, &a.id), Err(CoreError::FractionInUse { count: 1 })));
    observations::delete_pin(&conn, &a1.id).unwrap();
    assert!(matches!(fractions::delete(&conn, &a.id), Err(CoreError::FractionInUse { count: 0 })), "issued a number once");
    assert_eq!(create(&conn, &plan, "A").unwrap().display_ref, "LAM-A-02");
    // A fraction added from settings and never used can go.
    let unused = fractions::add(&conn, &plan.project_id, "Z").unwrap();
    fractions::delete(&conn, &unused.id).unwrap();
    assert!(matches!(fractions::delete(&conn, &unused.id), Err(CoreError::NotFound)));
    assert!(matches!(fractions::add(&conn, "missing", "Z"), Err(CoreError::NotFound)));
    assert!(matches!(fractions::add(&conn, &plan.project_id, ""), Err(CoreError::Validation(_))));
    // In project scope an observation that merely uses the fraction also blocks the delete.
    let dir2 = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir2.path());
    create(&conn, &plan, "B").unwrap();
    let b = fractions::find(&conn, &plan.project_id, "B").unwrap().unwrap();
    assert_eq!(b.next_seq, 1);
    assert!(matches!(fractions::delete(&conn, &b.id), Err(CoreError::FractionInUse { count: 1 })));
}

#[test]
fn scope_locks_once_a_number_was_issued() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let pid = plan.project_id.clone();
    // Free to change while nothing was issued, even with fractions in the list.
    fractions::add(&conn, &pid, "A").unwrap();
    let p = projects::update_ref_settings(&conn, &pid, "lam", TEMPLATE, Scope::Fraction).unwrap();
    assert_eq!((p.seq_scope, p.scope_locked, p.code.as_str()), (Scope::Fraction, false, "lam"));
    let p = projects::update_ref_settings(&conn, &pid, "LAM", TEMPLATE, Scope::Project).unwrap();
    assert_eq!((p.seq_scope, p.scope_locked), (Scope::Project, false));
    // The template must fit the scope, and the code is validated.
    assert!(matches!(
        projects::update_ref_settings(&conn, &pid, "LAM", "{PROJ}-{SEQ:2}", Scope::Fraction),
        Err(CoreError::InvalidTemplate(_))
    ));
    assert!(matches!(projects::update_ref_settings(&conn, &pid, "L M", TEMPLATE, Scope::Project), Err(CoreError::Validation(_))));
    assert!(matches!(projects::update_ref_settings(&conn, "missing", "", TEMPLATE, Scope::Project), Err(CoreError::NotFound)));

    let only = create(&conn, &plan, "").unwrap();
    let p = projects::get(&conn, &pid).unwrap();
    assert!(p.scope_locked);
    assert_eq!(p.observation_count, 1);
    assert!(matches!(
        projects::update_ref_settings(&conn, &pid, "LAM", TEMPLATE, Scope::Fraction),
        Err(CoreError::ScopeLocked)
    ));
    // Code and template still change under a lock; only the scope is fixed.
    let p = projects::update_ref_settings(&conn, &pid, "OBR", "{PROJ}#{SEQ:3}", Scope::Project).unwrap();
    assert_eq!(p.code, "OBR");
    assert_eq!(observations::get(&conn, &only.id).unwrap().display_ref, "OBR#001");
    // Deleting the only observation keeps the lock: the number was issued.
    observations::delete_pin(&conn, &only.id).unwrap();
    let p = projects::get(&conn, &pid).unwrap();
    assert!((p.scope_locked, p.observation_count) == (true, 0));
    assert!(matches!(
        projects::update_ref_settings(&conn, &pid, "OBR", TEMPLATE, Scope::Fraction),
        Err(CoreError::ScopeLocked)
    ));

    // Same in fraction scope: the fraction counter locks it.
    let dir2 = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir2.path());
    let a1 = create(&conn, &plan, "A").unwrap();
    observations::delete_pin(&conn, &a1.id).unwrap();
    assert!(projects::get(&conn, &plan.project_id).unwrap().scope_locked);
}

#[test]
fn preview_matches_the_next_assigned_ref() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let pid = plan.project_id.clone();
    let preview = |code: &str, template: &str, scope: Scope, fraction: &str| projects::preview_ref(&conn, &pid, code, template, scope, fraction);
    assert_eq!(preview("LAM", TEMPLATE, Scope::Project, "").unwrap(), "LAM-01");
    assert_eq!(preview("LAM", TEMPLATE, Scope::Project, "A").unwrap(), "LAM-A-01");
    assert_eq!(preview("", "{SEQ:3}", Scope::Project, "").unwrap(), "001");
    assert_eq!(next_seq(&conn, &pid), 1, "nothing written");
    assert!(matches!(preview("LAM", "{PROJ}-{SEQ:2}", Scope::Fraction, "A"), Err(CoreError::InvalidTemplate(_))));
    assert!(matches!(preview("LAM", TEMPLATE, Scope::Fraction, ""), Err(CoreError::FractionRequired)));
    assert_eq!(preview("LAM", TEMPLATE, Scope::Fraction, "A").unwrap(), "LAM-A-01", "fraction need not exist yet");
    create(&conn, &plan, "").unwrap();
    create(&conn, &plan, "").unwrap();
    assert_eq!(preview("LAM", TEMPLATE, Scope::Project, "").unwrap(), "LAM-03");
    assert_eq!(create(&conn, &plan, "").unwrap().seq, 3);

    let dir2 = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir2.path());
    create(&conn, &plan, "A").unwrap();
    assert_eq!(projects::preview_ref(&conn, &plan.project_id, "LAM", TEMPLATE, Scope::Fraction, "a").unwrap(), "LAM-A-02", "stored spelling");
    assert_eq!(projects::preview_ref(&conn, &plan.project_id, "LAM", TEMPLATE, Scope::Fraction, "PC").unwrap(), "LAM-PC-01");
}

#[test]
fn fraction_codes_fold_case_beyond_ascii() {
    // SQLite NOCASE folds ASCII only; the core compares Unicode-case-insensitively in Rust.
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup_fraction_scope(dir.path());
    let first = create(&conn, &plan, "Ático").unwrap();
    let second = create(&conn, &plan, "ático").unwrap();
    assert_eq!((first.display_ref.as_str(), second.display_ref.as_str()), ("LAM-Ático-01", "LAM-Ático-02"));
    assert_eq!(second.fraction, "Ático", "stored canonical spelling");
    assert_eq!(fractions::add(&conn, &plan.project_id, "ÁTICO").unwrap().code, "Ático");
    assert_eq!(fractions::list(&conn, &plan.project_id).unwrap().len(), 1, "one fraction row");
    assert_eq!(
        projects::preview_ref(&conn, &plan.project_id, "LAM", TEMPLATE, Scope::Fraction, "ÁTICO").unwrap(),
        "LAM-Ático-03"
    );
}

#[test]
fn ref_settings_are_trimmed_once_and_stored_as_validated() {
    let dir = tempfile::tempdir().unwrap();
    let (conn, plan) = setup(dir.path());
    let pid = plan.project_id.clone();
    let p = projects::update_ref_settings(&conn, &pid, " LAM ", "  {PROJ}-{SEQ:2}  ", Scope::Project).unwrap();
    assert_eq!((p.code.as_str(), p.ref_template.as_str()), ("LAM", "{PROJ}-{SEQ:2}"));
    let preview = projects::preview_ref(&conn, &pid, " LAM ", "  {PROJ}-{SEQ:2}  ", Scope::Project, "").unwrap();
    assert_eq!(preview, "LAM-01");
    assert_eq!(create(&conn, &plan, "").unwrap().display_ref, "LAM-01", "preview and stored value agree");
    assert!(matches!(projects::update_ref_settings(&conn, &pid, "LAM", "   ", Scope::Project), Err(CoreError::InvalidTemplate(_))));
}
