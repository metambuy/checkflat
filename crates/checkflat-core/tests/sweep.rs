mod common;

use checkflat_core::paths::{self, sweep_orphans};
use checkflat_core::repo::projects;
use checkflat_core::{db, ids};
use common::*;
use std::fs;
use std::path::Path;

fn touch(p: &Path, bytes: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn set_old(p: &Path, days: u64) {
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 3600);
    let f = fs::File::options().write(true).open(p).or_else(|_| fs::File::open(p)).unwrap();
    f.set_modified(t).unwrap();
}

#[test]
fn quarantines_orphans_and_leaves_everything_else_alone() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let opened = db::open(&data.join("checkflat.db")).unwrap();
    let conn = opened.conn;
    let live = projects::create(&conn, "Live", "").unwrap();
    insert_plan(&conn, "plan1", &live.id);
    let live_dir = paths::project_dir(&live.id).resolve(data);
    let referenced = live_dir.join("plans/plan1.pdf");
    touch(&referenced, b"%PDF");
    touch(&live_dir.join("plans/stray.pdf"), b"%PDF");
    touch(&live_dir.join("photos/stray.jpg"), b"jpg");
    touch(&live_dir.join("notes/keep.txt"), b"keep");
    let dead_id = ids::new_id();
    touch(&data.join(format!("projects/{dead_id}/plans/x.pdf")), b"%PDF");
    touch(&data.join("projects/not-a-uuid/file.pdf"), b"%PDF");
    let stale = data.join("tmp/old.pdf");
    touch(&stale, b"%PDF");
    set_old(&stale, 2);
    let fresh_tmp = data.join("tmp/fresh.pdf");
    touch(&fresh_tmp, b"%PDF");
    let expired_trash = data.join("projects/.trash/expired");
    touch(&expired_trash.join("f"), b"x");
    set_old(&expired_trash, 8);

    #[cfg(unix)]
    let outside = {
        let outside = tempfile::tempdir().unwrap();
        touch(&outside.path().join("victim.pdf"), b"%PDF");
        std::os::unix::fs::symlink(outside.path(), data.join(format!("projects/{}", ids::new_id()))).unwrap();
        outside
    };

    let report = sweep_orphans(&conn, data, false).unwrap();
    assert!(!report.skipped_fresh_db);
    let mut q = report.quarantined.clone();
    q.sort();
    let mut expected = vec![
        format!("projects/{dead_id}"),
        format!("projects/{}/photos/stray.jpg", live.id),
        format!("projects/{}/plans/stray.pdf", live.id),
    ];
    expected.sort();
    assert_eq!(q, expected);
    assert!(referenced.exists(), "referenced file untouched");
    assert!(live_dir.join("notes/keep.txt").exists(), "unknown subdir untouched");
    assert!(data.join("projects/not-a-uuid/file.pdf").exists(), "non-uuid dir untouched");
    assert!(!live_dir.join("plans/stray.pdf").exists());
    assert!(data.join(format!("projects/.trash/{}/plans/stray.pdf", live.id)).exists());
    assert!(!stale.exists(), "stale staging removed");
    assert!(fresh_tmp.exists(), "fresh staging kept");
    assert!(!expired_trash.exists(), "expired quarantine purged");
    let mut r = report.removed.clone();
    r.sort();
    assert_eq!(r, vec!["projects/.trash/expired".to_string(), "tmp/old.pdf".to_string()]);
    #[cfg(unix)]
    {
        assert!(outside.path().join("victim.pdf").exists(), "symlink target never touched");
        assert!(report.warnings.iter().any(|w| w.contains("symlink or outside")));
    }
}

#[test]
fn fresh_db_never_sweeps() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let dead = data.join(format!("projects/{}/plans/x.pdf", ids::new_id()));
    touch(&dead, b"%PDF");
    let opened = db::open(&data.join("checkflat.db")).unwrap();
    assert!(opened.freshly_created);
    let report = sweep_orphans(&opened.conn, data, opened.freshly_created).unwrap();
    assert!(report.skipped_fresh_db);
    assert!(report.quarantined.is_empty());
    assert!(dead.exists());
}

#[cfg(windows)]
#[test]
fn case_insensitive_reference_match_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let p = projects::create(&conn, "P", "").unwrap();
    insert_plan(&conn, "ABC", &p.id);
    let f = paths::project_dir(&p.id).resolve(data).join("plans/abc.pdf");
    touch(&f, b"%PDF");
    let report = sweep_orphans(&conn, data, false).unwrap();
    assert!(report.quarantined.is_empty());
    assert!(f.exists());
}
