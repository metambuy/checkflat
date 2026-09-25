mod common;

use checkflat_core::repo::{projects, settings};
use checkflat_core::{db, paths, CoreError};
use common::*;

#[test]
fn crud_and_disk_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;

    let p = projects::create(&conn, "  Obra X  ", " Rua A, 1 ").unwrap();
    assert_eq!(p.name, "Obra X");
    assert_eq!(p.address, "Rua A, 1");
    assert_eq!(p.next_ref_no, 1);
    assert!(paths::RelPath::new(&p.id).is_ok());

    assert!(matches!(projects::create(&conn, "   ", ""), Err(CoreError::Validation(_))));

    let renamed = projects::rename(&conn, &p.id, "Obra Y").unwrap();
    assert_eq!(renamed.name, "Obra Y");
    assert!(renamed.updated_at >= p.updated_at);
    assert!(matches!(projects::rename(&conn, "missing", "x"), Err(CoreError::NotFound)));

    insert_plan(&conn, "plan1", &p.id);
    let list = projects::list(&conn).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].plan_count, 1);

    let pdir = paths::project_dir(&p.id).resolve(data);
    std::fs::create_dir_all(pdir.join("plans")).unwrap();
    std::fs::write(pdir.join("plans/plan1.pdf"), b"%PDF").unwrap();

    projects::delete(&conn, data, &p.id).unwrap();
    assert!(!pdir.exists(), "project dir removed");
    assert_eq!(count(&conn, "project"), 0);
    assert_eq!(count(&conn, "plan"), 0);
    assert!(matches!(projects::delete(&conn, data, &p.id), Err(CoreError::NotFound)));
    // Deleting a project that never had a directory is fine.
    let q = projects::create(&conn, "Q", "").unwrap();
    projects::delete(&conn, data, &q.id).unwrap();
}

#[test]
fn settings_round_trip() {
    let conn = db::open_in_memory().unwrap();
    assert_eq!(settings::get(&conn, settings::LANGUAGE).unwrap(), None);
    settings::set(&conn, settings::LANGUAGE, "pt").unwrap();
    settings::set(&conn, settings::LANGUAGE, "en").unwrap();
    assert_eq!(settings::get(&conn, settings::LANGUAGE).unwrap().as_deref(), Some("en"));
}

#[test]
fn cascades_set_null_and_unique() {
    let conn = db::open_in_memory().unwrap();
    let p = projects::create(&conn, "P", "").unwrap();
    insert_plan(&conn, "plan1", &p.id);
    insert_visit(&conn, "v1", &p.id);
    insert_observation(&conn, "o1", &p.id, "plan1", 1, Some("v1")).unwrap();
    insert_photo(&conn, "ph1", "o1", Some("v1"), &p.id);

    // UNIQUE(project_id, ref_no)
    assert!(insert_observation(&conn, "o2", &p.id, "plan1", 1, None).is_err());
    // CHECK on x/y
    assert!(conn
        .execute(
            "INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, created_at, updated_at)
             VALUES ('o3', ?1, 'plan1', 3, 1.5, 0.5, 'a', 'a')",
            [&p.id]
        )
        .is_err());

    // Deleting a visit keeps the observation and photo, nulls the references.
    conn.execute("DELETE FROM visit WHERE id = 'v1'", []).unwrap();
    let cv: Option<String> = conn
        .query_row("SELECT created_visit_id FROM observation WHERE id='o1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cv, None);
    assert_eq!(count(&conn, "photo"), 1);

    // Project delete cascades through plans and observations (plan_id FK is NO ACTION,
    // checked at statement end, so cascade order does not matter).
    let dir = tempfile::tempdir().unwrap();
    projects::delete(&conn, dir.path(), &p.id).unwrap();
    for t in ["plan", "visit", "observation", "photo"] {
        assert_eq!(count(&conn, t), 0, "{t} emptied by cascade");
    }
}

#[test]
fn project_delete_with_observations_on_two_plans_succeeds() {
    let conn = db::open_in_memory().unwrap();
    let p = projects::create(&conn, "P", "").unwrap();
    insert_plan(&conn, "planA", &p.id);
    insert_plan(&conn, "planB", &p.id);
    insert_observation(&conn, "o1", &p.id, "planA", 1, None).unwrap();
    insert_observation(&conn, "o2", &p.id, "planB", 2, None).unwrap();
    insert_observation(&conn, "o3", &p.id, "planA", 3, None).unwrap();
    let dir = tempfile::tempdir().unwrap();
    projects::delete(&conn, dir.path(), &p.id).unwrap();
    assert_eq!(count(&conn, "observation"), 0);
    assert_eq!(count(&conn, "plan"), 0);
}

#[test]
fn raw_plan_delete_with_observations_fails_and_cross_project_is_rejected() {
    let conn = db::open_in_memory().unwrap();
    let p = projects::create(&conn, "P", "").unwrap();
    let q = projects::create(&conn, "Q", "").unwrap();
    insert_plan(&conn, "planP", &p.id);
    insert_observation(&conn, "o1", &p.id, "planP", 1, None).unwrap();
    // NO ACTION FK: direct plan delete fails while an observation references it.
    assert!(conn.execute("DELETE FROM plan WHERE id = 'planP'", []).is_err());
    // Composite FK: an observation in project Q cannot point at P's plan.
    assert!(insert_observation(&conn, "o2", &q.id, "planP", 1, None).is_err());
    conn.execute("DELETE FROM observation WHERE id = 'o1'", []).unwrap();
    assert_eq!(conn.execute("DELETE FROM plan WHERE id = 'planP'", []).unwrap(), 1);
}
