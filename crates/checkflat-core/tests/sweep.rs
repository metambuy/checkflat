mod common;

use checkflat_core::paths::{self, sweep_orphans};
use checkflat_core::repo::projects;
use checkflat_core::reports::{self, NewReport};
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
    let mut options = fs::File::options();
    options.write(true);
    // Windows opens a directory only with FILE_FLAG_BACKUP_SEMANTICS (found by the core tests' first
    // Windows run, D-029); Unix opens it read-only below, which is enough to set its times.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    }
    let f = options.open(p).or_else(|_| fs::File::open(p)).unwrap();
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
    let old_stamp = checkflat_core::clock::format_compact(time::OffsetDateTime::now_utc() - time::Duration::days(8));
    let expired_trash = data.join(format!("projects/.trash/{}-{old_stamp}", ids::new_id()));
    touch(&expired_trash.join("f"), b"x");

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
    let stamped: Vec<String> = fs::read_dir(data.join("projects/.trash"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&live.id))
        .collect();
    assert_eq!(stamped.len(), 1, "strays of one project share one stamped dir: {stamped:?}");
    assert!(data.join("projects/.trash").join(&stamped[0]).join("plans/stray.pdf").exists());
    assert!(data.join("projects/.trash").join(&stamped[0]).join("photos/stray.jpg").exists());
    assert!(!stale.exists(), "stale staging removed");
    assert!(fresh_tmp.exists(), "fresh staging kept");
    assert!(!expired_trash.exists(), "expired quarantine purged");
    let mut r = report.removed.clone();
    r.sort();
    let mut expected_removed = vec![
        format!("projects/.trash/{}", expired_trash.file_name().unwrap().to_string_lossy()),
        "tmp/old.pdf".to_string(),
    ];
    expected_removed.sort();
    assert_eq!(r, expected_removed);
    #[cfg(unix)]
    {
        assert!(outside.path().join("victim.pdf").exists(), "symlink target never touched");
        assert!(report.warnings.iter().any(|w| w.contains("symlink or outside")));
    }
}

#[test]
fn plan_cache_dirs_kept_for_live_plans_only() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let live = projects::create(&conn, "Live", "").unwrap();
    let plan_id = ids::new_id();
    insert_plan(&conn, &plan_id, &live.id);
    let plans_dir = paths::project_dir(&live.id).resolve(data).join("plans");
    touch(&plans_dir.join(format!("{plan_id}.pdf")), b"%PDF");
    let kept = plans_dir.join(format!("{plan_id}/tiles/1024/0_0.webp"));
    touch(&kept, b"RIFF");
    let dead_plan = ids::new_id();
    touch(&plans_dir.join(format!("{dead_plan}/tiles/1024/0_0.webp")), b"RIFF");
    touch(&plans_dir.join("not-a-uuid/x"), b"x");

    let report = sweep_orphans(&conn, data, false).unwrap();
    assert_eq!(report.quarantined, vec![format!("projects/{}/plans/{dead_plan}", live.id)]);
    assert!(kept.exists(), "live plan cache kept");
    assert!(!plans_dir.join(&dead_plan).exists());
    assert!(plans_dir.join("not-a-uuid/x").exists(), "non-uuid dir left alone");
    assert!(report.warnings.is_empty(), "no warnings for plan dirs: {:?}", report.warnings);
    let trash: Vec<_> = fs::read_dir(data.join("projects/.trash")).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(trash.len(), 1);
    assert!(trash[0].join("plans").join(&dead_plan).join("tiles/1024/0_0.webp").exists());
}

#[test]
fn fresh_db_never_sweeps() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let dead = data.join(format!("projects/{}/plans/x.pdf", ids::new_id()));
    touch(&dead, b"%PDF");
    let dead_report = data.join(format!("reports/{}-20261022T100000000Z.pdf", ids::new_id()));
    touch(&dead_report, b"%PDF");
    let opened = db::open(&data.join("checkflat.db")).unwrap();
    assert!(opened.freshly_created);
    let report = sweep_orphans(&opened.conn, data, opened.freshly_created).unwrap();
    assert!(report.skipped_fresh_db);
    assert!(report.quarantined.is_empty());
    assert!(dead.exists());
    assert!(dead_report.exists(), "reports are not swept on a fresh database either");
}

#[test]
fn reports_of_deleted_projects_are_removed_and_live_ones_kept() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let live = projects::create(&conn, "Live", "").unwrap();
    reports::write_report(data, &live.id, "20261022T100000000Z", b"%PDF", new_report()).unwrap();
    let dead = ids::new_id();
    let reports_dir = data.join("reports");
    let kept = [
        format!("{}-20261022T100000000Z.pdf", live.id),
        format!("{}.json", live.id),
        format!("{dead}-notes.txt"),
        "p1-20261012T100000000Z.pdf".to_string(),
        "readme.txt".to_string(),
    ];
    let removed = [
        format!("{dead}-20261022T100000000Z.pdf"),
        format!("{dead}-20261022T090000000Z.pdf.part"),
        format!("{dead}.json"),
        format!("{dead}.json.part"),
    ];
    for name in kept.iter().skip(2).chain(removed.iter()) {
        touch(&reports_dir.join(name), b"x");
    }
    // A directory with a report-like name is not a file: left alone.
    let dead_dir = reports_dir.join(format!("{}-20261022T100000000Z.pdf", ids::new_id()));
    fs::create_dir_all(&dead_dir).unwrap();

    let report = sweep_orphans(&conn, data, false).unwrap();
    let mut got = report.removed.clone();
    got.sort();
    let mut expected: Vec<String> = removed.iter().map(|n| format!("reports/{n}")).collect();
    expected.sort();
    assert_eq!(got, expected);
    for name in &kept {
        assert!(reports_dir.join(name).exists(), "{name} kept");
    }
    assert!(dead_dir.is_dir());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

fn new_report() -> NewReport {
    NewReport { name: "Live - 2026-10-22 - PT.pdf".into(), lang: "pt".into(), pages: 1, created_at: "2026-10-22T10:00:00.000Z".into() }
}

/// D-029: a kill during a generation can leave part files, or a new PDF that the metadata does not
/// name yet (between the two renames), or the earlier PDF (before the prune). The next start
/// removes them and keeps the report the metadata names.
#[test]
fn leftovers_of_an_interrupted_generation_are_removed_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let live = projects::create(&conn, "Live", "").unwrap();
    let (named, meta) = reports::write_report(data, &live.id, "20261022T100000000Z", b"%PDF-named", new_report()).unwrap();
    let id = &live.id;
    // A project with a report from before Sprint 6a (no metadata): its PDF is never shown.
    let old = projects::create(&conn, "Old", "").unwrap();
    let reports_dir = data.join("reports");
    let leftovers = [
        format!("{id}-20261022T110000000Z.pdf.part"),
        format!("{id}.json.part"),
        format!("{id}-20261022T090000000Z.pdf"),
        format!("{id}-20261022T120000000Z.pdf"),
        format!("{}-20261012T100000000Z.pdf", old.id),
    ];
    for name in &leftovers {
        touch(&reports_dir.join(name), b"x");
    }
    touch(&reports_dir.join(format!("{id}-notes.txt")), b"not a report");

    let report = sweep_orphans(&conn, data, false).unwrap();
    let mut got = report.removed.clone();
    got.sort();
    let mut expected: Vec<String> = leftovers.iter().map(|n| format!("reports/{n}")).collect();
    expected.sort();
    assert_eq!(got, expected);
    assert!(named.exists());
    assert!(reports_dir.join(format!("{id}.json")).exists());
    assert!(reports_dir.join(format!("{id}-notes.txt")).exists());
    assert_eq!(reports::latest(data, id).unwrap(), Some(meta), "the named report is still the latest");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
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

#[test]
fn quarantined_entries_survive_the_pass_that_quarantines_them_and_purge_by_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = db::open(&data.join("checkflat.db")).unwrap().conn;
    let live = projects::create(&conn, "Live", "").unwrap();
    // Orphan project dir and stray file whose content is far older than the 7-day window.
    let dead_id = ids::new_id();
    let dead_file = data.join(format!("projects/{dead_id}/plans/x.pdf"));
    touch(&dead_file, b"%PDF");
    set_old(&dead_file, 30);
    set_old(&data.join(format!("projects/{dead_id}")), 30);
    let stray = paths::project_dir(&live.id).resolve(data).join("plans/old-stray.pdf");
    touch(&stray, b"%PDF");
    set_old(&stray, 30);
    // A quarantine entry stamped 8 days ago (name-based), and one stamped just now.
    let old_stamp = checkflat_core::clock::format_compact(time::OffsetDateTime::now_utc() - time::Duration::days(8));
    let expired = data.join(format!("projects/.trash/{}-{old_stamp}", ids::new_id()));
    touch(&expired.join("plans/e.pdf"), b"x");
    let recent_stamp = checkflat_core::clock::now_compact();
    let recent = data.join(format!("projects/.trash/{}-{recent_stamp}", ids::new_id()));
    touch(&recent.join("plans/r.pdf"), b"x");
    set_old(&recent.join("plans/r.pdf"), 30); // stale mtime must not matter
    let unknown = data.join("projects/.trash/legacy-name");
    touch(&unknown.join("f"), b"x");
    set_old(&unknown, 30);

    let report = sweep_orphans(&conn, data, false).unwrap();
    assert_eq!(report.quarantined.len(), 2, "{report:?}");
    // Both orphans are in timestamped quarantine dirs and still exist after the pass.
    let trash: Vec<String> = fs::read_dir(data.join("projects/.trash"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let dead_entry = trash.iter().find(|n| n.starts_with(&dead_id)).expect("dead project quarantined");
    assert!(paths::trash_entry_time(dead_entry).is_some(), "stamped name: {dead_entry}");
    assert!(data.join("projects/.trash").join(dead_entry).join("plans/x.pdf").exists());
    let live_entry = trash.iter().find(|n| n.starts_with(&live.id)).expect("stray quarantined under project stamp dir");
    assert!(data.join("projects/.trash").join(live_entry).join("plans/old-stray.pdf").exists());
    // Purge decisions come from the stamp in the name only.
    assert!(!expired.exists(), "8-day-old stamp purged");
    assert!(recent.join("plans/r.pdf").exists(), "recent stamp kept despite old mtime");
    assert!(unknown.exists(), "unrecognized names are left alone");
    assert!(report.warnings.iter().any(|w| w.contains("legacy-name")));
    assert_eq!(report.removed, vec![format!("projects/.trash/{}", expired.file_name().unwrap().to_string_lossy())]);
}
