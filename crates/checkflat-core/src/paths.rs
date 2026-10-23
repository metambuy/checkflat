//! Relative-path policy and on-disk layout under the app data dir.
//!
//! Layout: `projects/<project_id>/plans/<plan_id>.pdf`, its tile cache
//! `projects/<project_id>/plans/<plan_id>/tiles/` (D-014), `projects/<project_id>/photos/<photo_id>.jpg`
//! with its list thumbnail `<photo_id>.thumb.jpg` (D-027),
//! `tmp/<token>.pdf` (plan import staging), `tmp/<token>.jpg` (processed photo waiting for its observation), `projects/.trash/` (quarantine),
//! `reports/` (the latest report per project, [`crate::reports`]),
//! `checkflat.db` (+ `-wal`, `-shm`, `.bak-v<n>`) at the root. The database only ever stores [`RelPath`]s.
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::Connection;
use time::OffsetDateTime;
use serde::{Deserialize, Serialize};

use crate::{clock, ids, CoreError, Result};

pub const PROJECTS_DIR: &str = "projects";
pub const TRASH_DIR: &str = ".trash";
pub const TMP_DIR: &str = "tmp";
pub const PLANS_DIR: &str = "plans";
pub const PHOTOS_DIR: &str = "photos";
pub const TILES_DIR: &str = "tiles";
pub const REPORTS_DIR: &str = "reports";

/// A validated path relative to the app data dir: `/`-separated, no absolute form, no `..`,
/// no `\\`, `:` or control characters (string rules, identical on every OS).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    pub fn new(s: &str) -> Result<RelPath> {
        let bad = |why: &str| CoreError::InvalidPath(format!("{s:?}: {why}"));
        if s.is_empty() {
            return Err(bad("empty"));
        }
        if s.starts_with('/') {
            return Err(bad("absolute path"));
        }
        if s.chars().any(|c| c == '\\' || c == ':' || c.is_control()) {
            return Err(bad("contains '\\\\', ':' or a control character"));
        }
        let mut parts = Vec::new();
        for comp in s.split('/') {
            match comp {
                "" | "." => return Err(bad("empty or '.' component")),
                ".." => return Err(bad("'..' component")),
                _ => parts.push(comp),
            }
        }
        Ok(RelPath(parts.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn join(&self, segment: &str) -> Result<RelPath> {
        RelPath::new(&format!("{}/{}", self.0, segment))
    }

    /// Absolute location under `data_dir`. Guaranteed to stay inside it.
    pub fn resolve(&self, data_dir: &Path) -> PathBuf {
        let mut p = data_dir.to_path_buf();
        for c in self.0.split('/') {
            p.push(c);
        }
        debug_assert!(p.starts_with(data_dir));
        p
    }

    /// Comparison key: case-folded on Windows (NTFS is case-insensitive), exact elsewhere.
    pub fn key(&self) -> String {
        if cfg!(windows) {
            self.0.to_lowercase()
        } else {
            self.0.clone()
        }
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RelPath {
    type Error = CoreError;
    fn try_from(s: String) -> Result<RelPath> {
        RelPath::new(&s)
    }
}

impl From<RelPath> for String {
    fn from(p: RelPath) -> String {
        p.0
    }
}

pub fn project_dir(project_id: &str) -> RelPath {
    RelPath::new(&format!("{PROJECTS_DIR}/{project_id}")).expect("uuid is a valid component")
}

pub fn plan_file(project_id: &str, plan_id: &str) -> RelPath {
    RelPath::new(&format!("{PROJECTS_DIR}/{project_id}/{PLANS_DIR}/{plan_id}.pdf")).expect("valid")
}

/// Per-plan directory for derived files (the tile cache); deleted with the plan.
pub fn plan_dir(project_id: &str, plan_id: &str) -> RelPath {
    RelPath::new(&format!("{PROJECTS_DIR}/{project_id}/{PLANS_DIR}/{plan_id}")).expect("valid")
}

pub fn plan_tiles_dir(project_id: &str, plan_id: &str) -> RelPath {
    plan_dir(project_id, plan_id).join(TILES_DIR).expect("valid")
}

pub fn staging_file(token: &str) -> RelPath {
    RelPath::new(&format!("{TMP_DIR}/{token}.pdf")).expect("valid")
}

/// A processed photo waiting for its observation to be saved (`tmp/` so the startup sweep, which
/// quarantines unreferenced files under `photos/`, cannot take it before the UI recovers it).
pub fn staging_photo(token: &str) -> RelPath {
    RelPath::new(&format!("{TMP_DIR}/{token}.jpg")).expect("valid")
}

pub fn photo_file(project_id: &str, photo_id: &str) -> RelPath {
    RelPath::new(&format!("{PROJECTS_DIR}/{project_id}/{PHOTOS_DIR}/{photo_id}.jpg")).expect("valid")
}

/// Suffix of a photo's list thumbnail: `<photo_id>.thumb.jpg` next to `<photo_id>.jpg`.
pub const THUMB_SUFFIX: &str = ".thumb.jpg";

/// The small derived copy of a stored photo that the observation list shows (Sprint 4b, D-027).
/// A cache, like the tile pyramid: no database column, made on demand, deleted with the photo (and
/// with the project directory), kept by the startup sweep while the photo is referenced, and left
/// out of the Sprint 8 archive export.
pub fn photo_thumb_file(project_id: &str, photo_id: &str) -> RelPath {
    RelPath::new(&format!("{PROJECTS_DIR}/{project_id}/{PHOTOS_DIR}/{photo_id}{THUMB_SUFFIX}")).expect("valid")
}

/// The thumbnail that belongs to a photo file (`…/<id>.jpg` → `…/<id>.thumb.jpg`); `None` for a
/// path that is not a photo file or already is a thumbnail.
pub fn thumb_of(photo: &RelPath) -> Option<RelPath> {
    let stem = photo.as_str().strip_suffix(".jpg")?;
    if stem.ends_with(".thumb") {
        return None;
    }
    RelPath::new(&format!("{stem}{THUMB_SUFFIX}")).ok()
}

/// The photo file a thumbnail belongs to (`…/<id>.thumb.jpg` → `…/<id>.jpg`), as a relative path
/// string; `None` for any other name.
pub fn thumb_owner(rel: &str) -> Option<String> {
    Some(format!("{}.jpg", rel.strip_suffix(THUMB_SUFFIX)?))
}

/// What the startup sweep did. Paths are relative to the data dir.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct SweepReport {
    pub skipped_fresh_db: bool,
    pub quarantined: Vec<String>,
    pub removed: Vec<String>,
    pub warnings: Vec<String>,
}

const STAGING_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
const TRASH_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

/// Startup maintenance. Quarantines (moves to `projects/.trash/`) rather than deletes:
/// (a) project dirs with no `project` row, (b) files under live projects' `plans/` and `photos/`
/// that no row references, and UUID-named `plans/<plan_id>/` cache dirs whose plan
/// row is gone; removes (c) staging files older than 24 h,
/// (d) quarantine entries older than 7 days and (e) the report files of projects that no longer
/// exist ([`crate::reports`]; derived files of a project deleted on purpose). Skipped entirely when
/// the database was created by this process (`freshly_created`): an empty DB must never wipe user files.
///
/// Every quarantined entry lives under `.trash/<id>-<stamp>/` where `<stamp>` is the time of
/// quarantine ([`clock::now_compact`]); purge (d) is decided by that stamp, never by mtime
/// (a `rename` keeps the original mtime, which could be years old), and runs *before* (a)/(b)
/// so nothing quarantined in this pass can be purged in the same pass.
///
/// Containment: only UUID-named entries are considered, symlinks are never followed, and every
/// touched path must canonicalize inside the canonical data dir.
pub fn sweep_orphans(conn: &Connection, data_dir: &Path, freshly_created: bool) -> Result<SweepReport> {
    let mut report = SweepReport::default();
    if freshly_created {
        report.skipped_fresh_db = true;
        log::info!("sweep skipped: fresh database");
        return Ok(report);
    }
    let root = match data_dir.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            report.warnings.push(format!("data dir not canonicalizable: {e}"));
            return Ok(report);
        }
    };
    let live: HashSet<String> = query_strings(conn, "SELECT id FROM project")?.into_iter().collect();
    // Per-plan cache dirs `plans/<plan_id>/` are kept while the plan row exists.
    let live_plan_dirs: HashSet<String> = query_strings(conn, "SELECT project_id || '/' || id FROM plan")?
        .into_iter()
        .filter_map(|s| {
            let (project, plan) = s.split_once('/')?;
            RelPath::new(&format!("{PROJECTS_DIR}/{project}/{PLANS_DIR}/{plan}")).ok().map(|p| p.key())
        })
        .collect();
    let mut referenced: HashSet<String> = HashSet::new();
    for sql in ["SELECT file_path FROM plan", "SELECT file_path FROM photo"] {
        for s in query_strings(conn, sql)? {
            match RelPath::new(&s) {
                Ok(p) => {
                    referenced.insert(p.key());
                }
                Err(e) => report.warnings.push(format!("unparseable path in db ignored: {e}")),
            }
        }
    }

    let projects_root = root.join(PROJECTS_DIR);
    let trash_root = projects_root.join(TRASH_DIR);
    let now = SystemTime::now();
    let stamp = clock::now_compact();

    // (d) expired quarantine entries — decided by the stamp in the directory name, before any
    // quarantining in this pass.
    purge_expired_trash(&trash_root, &root, OffsetDateTime::now_utc(), &mut report);

    if let Ok(entries) = std::fs::read_dir(&projects_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == TRASH_DIR || !ids::is_id(&name) {
                continue;
            }
            let path = entry.path();
            if !is_real_dir(&path) || !contained(&path, &root) {
                report.warnings.push(format!("skipped {name}: symlink or outside data dir"));
                continue;
            }
            let rel_project = format!("{PROJECTS_DIR}/{name}");
            if !live.contains(&name) {
                let dest = trash_root.join(format!("{name}-{stamp}"));
                quarantine(&path, &dest, &rel_project, &mut report);
                continue;
            }
            // Live project: unreferenced files in plans/ and photos/.
            let mut candidates: Vec<(PathBuf, String)> = Vec::new();
            for sub in [PLANS_DIR, PHOTOS_DIR] {
                if let Ok(files) = std::fs::read_dir(path.join(sub)) {
                    for f in files.flatten() {
                        let fname = f.file_name().to_string_lossy().into_owned();
                        let rel = format!("{rel_project}/{sub}/{fname}");
                        // Plan cache dirs: only UUID-named real dirs are considered; kept while
                        // the plan exists, quarantined otherwise. Other dirs are left alone.
                        if sub == PLANS_DIR && is_real_dir(&f.path()) {
                            let Ok(key) = RelPath::new(&rel).map(|p| p.key()) else { continue };
                            if ids::is_id(&fname) && !live_plan_dirs.contains(&key) && contained(&f.path(), &root) {
                                let dest = trash_root.join(format!("{name}-{stamp}")).join(sub).join(&fname);
                                quarantine(&f.path(), &dest, &rel, &mut report);
                            }
                            continue;
                        }
                        candidates.push((f.path(), rel));
                    }
                }
            }
            for (file, rel) in candidates {
                if !is_real_file(&file) || !contained(&file, &root) {
                    report.warnings.push(format!("skipped {rel}: symlink or outside data dir"));
                    continue;
                }
                let key = match RelPath::new(&rel) {
                    Ok(p) => p.key(),
                    Err(_) => {
                        report.warnings.push(format!("skipped {rel}: invalid name"));
                        continue;
                    }
                };
                if referenced.contains(&key) {
                    continue;
                }
                // A thumbnail is a derived file: it stays while its photo is referenced. A photo
                // that no row references takes its thumbnail with it, and so does a leftover
                // `….tmp` of a kill in the middle of writing one (it names no photo).
                if let Some(owner) = thumb_owner(&rel) {
                    if RelPath::new(&owner).map(|o| referenced.contains(&o.key())).unwrap_or(false) {
                        continue;
                    }
                }
                let dest = trash_root
                    .join(format!("{name}-{stamp}"))
                    .join(rel.trim_start_matches(&format!("{rel_project}/")));
                quarantine(&file, &dest, &rel, &mut report);
            }
        }
    }

    // (c) stale staging files
    if let Ok(entries) = std::fs::read_dir(root.join(TMP_DIR)) {
        for f in entries.flatten() {
            let p = f.path();
            let rel = format!("{TMP_DIR}/{}", f.file_name().to_string_lossy());
            if is_real_file(&p) && contained(&p, &root) && older_than(&p, now, STAGING_MAX_AGE) {
                match std::fs::remove_file(&p) {
                    Ok(()) => report.removed.push(rel),
                    Err(e) => report.warnings.push(format!("could not remove {rel}: {e}")),
                }
            }
        }
    }

    // (e) reports of deleted projects
    crate::reports::sweep_orphans(&root, &live, &mut report);
    log::info!(
        "sweep: {} quarantined, {} removed, {} warnings",
        report.quarantined.len(),
        report.removed.len(),
        report.warnings.len()
    );
    Ok(report)
}

/// Stamp of a quarantine entry name `<uuid>-<stamp>`, if well-formed.
pub fn trash_entry_time(name: &str) -> Option<OffsetDateTime> {
    let (id, stamp) = name.rsplit_once('-')?;
    if !ids::is_id(id) {
        return None;
    }
    clock::parse_compact(stamp)
}

fn purge_expired_trash(trash_root: &Path, root: &Path, now: OffsetDateTime, report: &mut SweepReport) {
    let Ok(entries) = std::fs::read_dir(trash_root) else { return };
    let max_age = time::Duration::seconds(TRASH_MAX_AGE.as_secs() as i64);
    for f in entries.flatten() {
        let p = f.path();
        let name = f.file_name().to_string_lossy().into_owned();
        let rel = format!("{PROJECTS_DIR}/{TRASH_DIR}/{name}");
        if !contained(&p, root) || is_symlink(&p) {
            continue;
        }
        let Some(at) = trash_entry_time(&name) else {
            report.warnings.push(format!("unrecognized quarantine entry left alone: {rel}"));
            continue;
        };
        if now - at <= max_age {
            continue;
        }
        let res = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        match res {
            Ok(()) => report.removed.push(rel),
            Err(e) => report.warnings.push(format!("could not remove {rel}: {e}")),
        }
    }
}

fn quarantine(from: &Path, dest: &Path, rel: &str, report: &mut SweepReport) {
    let res = dest
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| std::fs::rename(from, dest));
    match res {
        Ok(()) => report.quarantined.push(rel.to_string()),
        Err(e) => report.warnings.push(format!("could not quarantine {rel}: {e}")),
    }
}

fn query_strings(conn: &Connection, sql: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(true)
}
fn is_real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_dir()).unwrap_or(false)
}
fn is_real_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_file()).unwrap_or(false)
}
fn contained(p: &Path, root: &Path) -> bool {
    p.canonicalize().map(|c| c.starts_with(root)).unwrap_or(false)
}
fn older_than(p: &Path, now: SystemTime, age: Duration) -> bool {
    std::fs::symlink_metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| now.duration_since(m).ok())
        .map(|d| d > age)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relpath_rules() {
        for bad in ["", "/abs", "a/../b", "..", "./a", "a//b", "a/", "C:\\x", "a\\b", "a:b", "a\u{0}b", "a\nb"] {
            assert!(RelPath::new(bad).is_err(), "{bad:?} must be rejected");
        }
        let ok = RelPath::new("projects/x/plans/y.pdf").unwrap();
        assert_eq!(ok.as_str(), "projects/x/plans/y.pdf");
        assert_eq!(ok.resolve(Path::new("/data")), PathBuf::from("/data/projects/x/plans/y.pdf"));
        assert_eq!(plan_file("p", "q").as_str(), "projects/p/plans/q.pdf");
        assert_eq!(staging_file("t").as_str(), "tmp/t.pdf");
        let json = serde_json::to_string(&ok).unwrap();
        assert_eq!(json, "\"projects/x/plans/y.pdf\"");
        assert!(serde_json::from_str::<RelPath>("\"../x\"").is_err());
    }
}
