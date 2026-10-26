//! Generated reports: the latest PDF per project, its metadata and the copies handed to other apps
//! (Sprint 5; metadata, readable names and share copies since Sprint 6a, D-028).
//!
//! Layout under the app data dir: `reports/<project_id>-<stamp>.pdf` (one per project) and its
//! metadata `reports/<project_id>.json` ([`ReportMeta`]), outside `projects/` and `tmp/`. The
//! startup sweep removes the files of projects that no longer exist ([`sweep_orphans`]).
//!
//! Open/Share hand another app a **copy** under the app cache dir,
//! `<cache>/reports/<project_id>/<stamp>/<readable name>` ([`share_copy`]): Tauri's data dir on
//! Android is the package root, which a FileProvider cannot serve, and the plugin's provider exposes
//! only the cache's `reports/` (and the camera's `captures/`) folder (D-028). The copy also carries
//! the name the recipient sees ([`display_name`]). Windows saves a copy where the user chooses
//! ([`save_copy`], D-029).
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::paths::{RelPath, SweepReport, REPORTS_DIR};
use crate::{ids, CoreError, Result};

pub const META_VERSION: u32 = 1;

/// What is known about a project's latest report (`reports/<project_id>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMeta {
    pub v: u32,
    /// The stored PDF's file name in `reports/` (`<project_id>-<stamp>.pdf`).
    pub file: String,
    /// The name other apps show ([`display_name`]), fixed at generation: a later project rename
    /// does not change it under the PDF's own cover.
    pub name: String,
    /// `en` | `pt`.
    pub lang: String,
    pub pages: usize,
    pub bytes: u64,
    /// UTC RFC 3339: when the report was generated; its cover date is this instant's local date.
    pub created_at: String,
}

/// What the caller knows about a new report; [`write_report`] adds the file name and size.
#[derive(Debug, Clone)]
pub struct NewReport {
    pub name: String,
    pub lang: String,
    pub pages: usize,
    pub created_at: String,
}

const PDF_PART: &str = ".pdf.part";
const META_EXT: &str = ".json";
const META_PART: &str = ".json.part";
/// Share copies older than this are removed before the next copy is made (the plugin's camera and
/// picker cache copies follow the same rule).
const SHARE_COPY_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// Characters Windows refuses in file names, which some receiving apps also mangle.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
/// The project-name part of a readable name, in bytes: file names are limited to 255 bytes, and
/// Portuguese letters take two.
const NAME_MAX_BYTES: usize = 100;
const NAME_FALLBACK: &str = "Checkflat";

fn check_id(project_id: &str) -> Result<()> {
    if ids::is_id(project_id) {
        Ok(())
    } else {
        Err(CoreError::Validation("invalid project id".into()))
    }
}

/// The stored PDF: `reports/<project_id>-<stamp>.pdf`; `stamp` is a [`crate::clock::now_compact`]
/// value.
pub fn report_file(project_id: &str, stamp: &str) -> RelPath {
    RelPath::new(&format!("{REPORTS_DIR}/{project_id}-{stamp}.pdf")).expect("valid")
}

/// The project's report metadata: `reports/<project_id>.json`.
pub fn meta_file(project_id: &str) -> RelPath {
    RelPath::new(&format!("{REPORTS_DIR}/{project_id}{META_EXT}")).expect("valid")
}

/// The name a recipient sees: `"<project name> - <date> - <LANG>.pdf"`, `date` as `yyyy-mm-dd`,
/// `lang` the report's language code (`pt` → `PT`), so the EN and PT reports of one day differ
/// (Martín's P30 check, D-028). Control characters and `/ \ : * ? " < > |` become spaces, runs of
/// whitespace one space; the project name is cut to [`NAME_MAX_BYTES`] on a character boundary
/// and then loses spaces, dots and hyphens at both ends (no hidden file, no trailing space from
/// the cut). Accents are kept. An empty result is [`NAME_FALLBACK`].
pub fn display_name(project_name: &str, date: &str, lang: &str) -> String {
    let cleaned: String = project_name.chars().map(|c| if c.is_control() || FORBIDDEN.contains(&c) { ' ' } else { c }).collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = collapsed.len().min(NAME_MAX_BYTES);
    while !collapsed.is_char_boundary(end) {
        end -= 1;
    }
    let trimmed = collapsed[..end].trim_matches(|c: char| c == ' ' || c == '.' || c == '-');
    let base = if trimmed.is_empty() { NAME_FALLBACK } else { trimmed };
    format!("{base} - {date} - {}.pdf", lang.to_ascii_uppercase())
}

/// A readable name as [`display_name`] makes it; anything else in the metadata is not used as a
/// file name.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.ends_with(".pdf")
        && !name.starts_with('.')
        && !name.chars().any(|c| c.is_control() || FORBIDDEN.contains(&c))
}

/// The stored file a metadata entry may point at: `<project_id>-<stamp>.pdf`, nothing else.
fn is_own_file(project_id: &str, file: &str) -> bool {
    file.strip_prefix(project_id)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| rest.strip_suffix(".pdf"))
        .is_some_and(|stamp| !stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Writes `bytes` to `part`, fsyncs and renames it to `dest`; the part file is removed on failure.
fn write_via_part(dest: &Path, part: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let written = std::fs::File::create(part).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(part, dest)
    });
    if written.is_err() {
        if let Err(rm) = std::fs::remove_file(part) {
            if rm.kind() != std::io::ErrorKind::NotFound {
                log::warn!("could not remove {}: {rm}", part.display());
            }
        }
    }
    written
}

/// Writes a generated report and its metadata; returns the PDF's absolute path and the metadata.
/// The PDF and then the JSON each go through a `.part` file, fsync and rename; **only then** are
/// the project's earlier PDFs and leftover part files removed, so a failed generation leaves the
/// previous report usable (its metadata still points at it). If the metadata cannot be written,
/// the new PDF is removed again and the error returned. A `name` that is not a safe file name
/// ([`display_name`] always makes one) is refused (`Validation`) before anything is written:
/// [`latest`] would not show such a report, and [`share_copy`] would not hand it out.
pub fn write_report(data_dir: &Path, project_id: &str, stamp: &str, pdf: &[u8], new: NewReport) -> Result<(PathBuf, ReportMeta)> {
    check_id(project_id)?;
    if !is_safe_name(&new.name) {
        return Err(CoreError::Validation(format!("report name not safe: {:?}", new.name)));
    }
    let dest = report_file(project_id, stamp).resolve(data_dir);
    let dir = dest.parent().expect("report_file has a parent").to_path_buf();
    std::fs::create_dir_all(&dir)?;
    let file = dest.file_name().expect("report_file has a name").to_string_lossy().into_owned();
    write_via_part(&dest, &dir.join(format!("{file}.part")), pdf)?;
    let meta = ReportMeta {
        v: META_VERSION,
        file,
        name: new.name,
        lang: new.lang,
        pages: new.pages,
        bytes: pdf.len() as u64,
        created_at: new.created_at,
    };
    let json = serde_json::to_vec_pretty(&meta).map_err(|e| CoreError::Validation(format!("report metadata: {e}")))?;
    let meta_path = meta_file(project_id).resolve(data_dir);
    if let Err(e) = write_via_part(&meta_path, &dir.join(format!("{project_id}{META_PART}")), &json) {
        if let Err(rm) = std::fs::remove_file(&dest) {
            log::warn!("could not remove {} after its metadata failed: {rm}", dest.display());
        }
        return Err(e.into());
    }
    prune(&dir, project_id, &dest);
    Ok((dest, meta))
}

/// Removes the project's `<project_id>-*.pdf` and part files other than `keep`. Regular files only
/// (symlinks and directories are left alone); a failure is logged, not fatal.
fn prune(dir: &Path, project_id: &str, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let prefix = format!("{project_id}-");
    let meta_part = format!("{project_id}{META_PART}");
    for f in entries.flatten() {
        let p = f.path();
        let name = f.file_name().to_string_lossy().into_owned();
        let ours = (name.starts_with(&prefix) && (name.ends_with(".pdf") || name.ends_with(PDF_PART))) || name == meta_part;
        if !ours || p == keep || !is_real_file(&p) {
            continue;
        }
        match std::fs::remove_file(&p) {
            Ok(()) => log::info!("removed earlier report file {name}"),
            Err(e) => log::warn!("could not remove earlier report file {name}: {e}"),
        }
    }
}

/// The project's latest report, if its metadata is readable and the PDF it names exists with the
/// recorded size. Reports from before Sprint 6a have no metadata and read as `None` (they are
/// removed by the next [`write_report`]). Only an invalid project id is an error.
pub fn latest(data_dir: &Path, project_id: &str) -> Result<Option<ReportMeta>> {
    check_id(project_id)?;
    let Ok(raw) = std::fs::read(meta_file(project_id).resolve(data_dir)) else { return Ok(None) };
    let Ok(meta) = serde_json::from_slice::<ReportMeta>(&raw) else {
        log::warn!("report metadata of {project_id} unreadable");
        return Ok(None);
    };
    if meta.v != META_VERSION || !is_own_file(project_id, &meta.file) || !is_safe_name(&meta.name) {
        log::warn!("report metadata of {project_id} not usable: {meta:?}");
        return Ok(None);
    }
    let pdf = data_dir.join(REPORTS_DIR).join(&meta.file);
    let ok = std::fs::symlink_metadata(&pdf).map(|m| m.file_type().is_file() && m.len() == meta.bytes).unwrap_or(false);
    Ok(ok.then_some(meta))
}

/// The `<stamp>` of a stored report's file name `<project_id>-<stamp>.pdf` ([`latest`] has checked
/// the form).
fn stamp_of<'a>(project_id: &str, file: &'a str) -> &'a str {
    file.strip_prefix(project_id).and_then(|r| r.strip_prefix('-')).and_then(|r| r.strip_suffix(".pdf")).unwrap_or(file)
}

/// Copies the project's latest report to `<share_root>/<project_id>/<stamp>/<readable name>` for
/// another app to read, and returns the copy's path; `None` when there is no report. A folder per
/// report (D-029): **a copy already made for this report is reused**, never rewritten. On Windows a
/// PDF viewer such as Acrobat keeps the file it shows locked, so replacing it would fail when Open
/// is pressed again. A new report gets a new folder, so a copy still held open never blocks it. A
/// new copy goes through a part file and a rename, so a copy under the final name is always whole.
/// Copies older than a day are removed first (a held one stays until it is released). Each project
/// has its own folder, so two projects with the same name on the same day never share a copy.
pub fn share_copy(data_dir: &Path, project_id: &str, share_root: &Path) -> Result<Option<PathBuf>> {
    let Some(meta) = latest(data_dir, project_id)? else { return Ok(None) };
    purge_share_copies(share_root, SystemTime::now());
    let dir = share_root.join(project_id).join(stamp_of(project_id, &meta.file));
    let dest = dir.join(&meta.name);
    if std::fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_file() && m.len() == meta.bytes) {
        return Ok(Some(dest));
    }
    std::fs::create_dir_all(&dir)?;
    let part = dir.join(format!("{}.part", meta.name));
    // The copy's age is its own (a clone may carry the source's mtime), so the purge counts from now.
    let copied = std::fs::copy(data_dir.join(REPORTS_DIR).join(&meta.file), &part)
        .and_then(|_| std::fs::File::options().write(true).open(&part)?.set_modified(SystemTime::now()))
        .and_then(|_| std::fs::rename(&part, &dest));
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&part);
        return Err(e.into());
    }
    Ok(Some(dest))
}

/// Copies the project's latest report to `dest`, a path the user chose in a save dialog (Windows'
/// "Save a copy…", D-029), and returns it; `None` when there is no report. The copy goes through
/// `<dest>.part`, fsync and a rename, so `dest` is never half written and an existing file there
/// is replaced only by a whole copy. On failure the part file is removed, for example when a PDF
/// viewer holds `dest` open on Windows.
pub fn save_copy(data_dir: &Path, project_id: &str, dest: &Path) -> Result<Option<PathBuf>> {
    let Some(meta) = latest(data_dir, project_id)? else { return Ok(None) };
    let Some(file_name) = dest.file_name() else { return Err(CoreError::Validation("save path without a file name".into())) };
    let mut part_name = file_name.to_os_string();
    part_name.push(".part");
    let part = dest.with_file_name(part_name);
    let copied = std::fs::copy(data_dir.join(REPORTS_DIR).join(&meta.file), &part)
        .and_then(|_| std::fs::File::options().write(true).open(&part)?.sync_all())
        .and_then(|_| std::fs::rename(&part, dest));
    if let Err(e) = copied {
        if let Err(rm) = std::fs::remove_file(&part) {
            if rm.kind() != std::io::ErrorKind::NotFound {
                log::warn!("could not remove {}: {rm}", part.display());
            }
        }
        return Err(e.into());
    }
    Ok(Some(dest.to_path_buf()))
}

/// Removes files older than [`SHARE_COPY_MAX_AGE`] under `share_root`: in the project folders (the
/// flat copies of Sprint 6a) and in their per-report folders, then the folders left empty. A file
/// that cannot be removed (held open on Windows) is logged and stays. Never follows symlinks.
fn purge_share_copies(share_root: &Path, now: SystemTime) {
    let Ok(dirs) = std::fs::read_dir(share_root) else { return };
    for d in dirs.flatten() {
        let dir = d.path();
        if is_real_dir(&dir) {
            purge_old_files(&dir, now, 1);
            let _ = std::fs::remove_dir(&dir); // only succeeds when empty
        }
    }
}

/// [`purge_share_copies`] for one folder, `depth` levels of subfolders down.
fn purge_old_files(dir: &Path, now: SystemTime, depth: u32) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        if m.file_type().is_dir() && depth > 0 {
            purge_old_files(&p, now, depth - 1);
            let _ = std::fs::remove_dir(&p);
        } else if m.file_type().is_file() {
            let old = m.modified().ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age > SHARE_COPY_MAX_AGE);
            if old {
                if let Err(e) = std::fs::remove_file(&p) {
                    log::warn!("could not remove share copy {}: {e}", p.display());
                }
            }
        }
    }
}

/// Removes the project's share copies (`<share_root>/<project_id>/`), when the project is deleted.
pub fn remove_share_copies(share_root: &Path, project_id: &str) -> Result<()> {
    check_id(project_id)?;
    let dir = share_root.join(project_id);
    if is_real_dir(&dir) {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Removes the project's report, its metadata and any part files (when the project is deleted;
/// the report is client data). Failures are logged: a leftover is removed by the next sweep.
pub fn remove_for_project(data_dir: &Path, project_id: &str) -> Result<()> {
    check_id(project_id)?;
    let Ok(entries) = std::fs::read_dir(data_dir.join(REPORTS_DIR)) else { return Ok(()) };
    for f in entries.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        if owner_of(&name) == Some(project_id) && is_real_file(&f.path()) {
            if let Err(e) = std::fs::remove_file(f.path()) {
                log::warn!("could not remove report file {name}: {e}");
            }
        }
    }
    Ok(())
}

/// The project a file in `reports/` belongs to: `<uuid>-….pdf`, `<uuid>-….pdf.part`,
/// `<uuid>.json` or `<uuid>.json.part`; `None` for any other name.
fn owner_of(name: &str) -> Option<&str> {
    const ID_LEN: usize = 36;
    if name.len() <= ID_LEN || !name.is_char_boundary(ID_LEN) {
        return None;
    }
    let (id, rest) = name.split_at(ID_LEN);
    let report = rest.starts_with('-') && (rest.ends_with(".pdf") || rest.ends_with(PDF_PART));
    let meta = rest == META_EXT || rest == META_PART;
    (ids::is_id(id) && (report || meta)).then_some(id)
}

/// Startup sweep step (called by [`crate::paths::sweep_orphans`]). It removes:
/// - the report files of projects that no longer exist;
/// - for live projects, the leftovers of an interrupted generation (D-029): part files, and PDFs
///   the project's metadata does not name. A kill between the PDF's rename and the metadata's, or
///   before the prune, leaves those behind.
///
/// No report can be in progress meanwhile: the sweep runs at startup, before any command is served.
/// Reports are derived and can be generated again, so they are removed rather than quarantined.
/// Regular files only, inside the data dir (`root` is the canonical data dir).
pub(crate) fn sweep_orphans(root: &Path, live: &HashSet<String>, report: &mut SweepReport) {
    let Ok(entries) = std::fs::read_dir(root.join(REPORTS_DIR)) else { return };
    // Live project → the stored PDF its metadata names (if the metadata is usable).
    let mut named: HashMap<String, Option<String>> = HashMap::new();
    for f in entries.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        let Some(owner) = owner_of(&name) else { continue };
        let p = f.path();
        let contained = p.canonicalize().map(|c| c.starts_with(root)).unwrap_or(false);
        if !is_real_file(&p) || !contained {
            continue;
        }
        if live.contains(owner) {
            let keep = named.entry(owner.to_string()).or_insert_with(|| latest(root, owner).ok().flatten().map(|m| m.file));
            let leftover = name.ends_with(PDF_PART) || name.ends_with(META_PART) || (name.ends_with(".pdf") && keep.as_deref() != Some(name.as_str()));
            if !leftover {
                continue;
            }
        }
        let rel = format!("{REPORTS_DIR}/{name}");
        match std::fs::remove_file(&p) {
            Ok(()) => report.removed.push(rel),
            Err(e) => report.warnings.push(format!("could not remove {rel}: {e}")),
        }
    }
}

fn is_real_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_file()).unwrap_or(false)
}
fn is_real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.file_type().is_dir()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATE: &str = "2026-10-22";

    fn new_report(name: &str) -> NewReport {
        NewReport { name: name.into(), lang: "pt".into(), pages: 3, created_at: "2026-10-22T10:00:00.000Z".into() }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn set_age(p: &Path, hours: u64) {
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(hours * 3600)).unwrap();
    }

    #[test]
    fn paths() {
        assert_eq!(report_file("p", "20261012T100000000Z").as_str(), "reports/p-20261012T100000000Z.pdf");
        assert_eq!(meta_file("p").as_str(), "reports/p.json");
    }

    #[test]
    fn display_name_cleans_cuts_and_keeps_accents() {
        assert_eq!(display_name("Perf A1 4a", DATE, "pt"), "Perf A1 4a - 2026-10-22 - PT.pdf");
        assert_eq!(display_name("Perf A1 4a", DATE, "en"), "Perf A1 4a - 2026-10-22 - EN.pdf", "the two languages of one day differ");
        assert_eq!(display_name("Rua 5/7: \"Bloco A\"", DATE, "pt"), "Rua 5 7 Bloco A - 2026-10-22 - PT.pdf");
        assert_eq!(display_name("Edifício São João", DATE, "pt"), "Edifício São João - 2026-10-22 - PT.pdf");
        assert_eq!(display_name("  ..Obra.  ", DATE, "pt"), "Obra - 2026-10-22 - PT.pdf");
        assert_eq!(display_name("a\u{0}b\tc\nd*e?f<g>h|i\\j", DATE, "en"), "a b c d e f g h i j - 2026-10-22 - EN.pdf");
        assert_eq!(display_name("///", DATE, "pt"), "Checkflat - 2026-10-22 - PT.pdf");
        assert_eq!(display_name("", DATE, "pt"), "Checkflat - 2026-10-22 - PT.pdf");
        assert_eq!(display_name(" - . - ", DATE, "pt"), "Checkflat - 2026-10-22 - PT.pdf");
        // 99 ASCII bytes + "ç" (2 bytes) crosses 100: the cut falls before the "ç".
        let long = format!("{}ç tail", "a".repeat(99));
        assert_eq!(display_name(&long, DATE, "pt"), format!("{} - {DATE} - PT.pdf", "a".repeat(99)));
        // A cut that ends on a space loses it.
        let spaced = format!("{} b{}", "a".repeat(99), "c".repeat(20));
        assert_eq!(display_name(&spaced, DATE, "pt"), format!("{} - {DATE} - PT.pdf", "a".repeat(99)));
        for n in ["Perf A1 4a", "Rua 5/7: \"Bloco A\"", "///", &long] {
            assert!(is_safe_name(&display_name(n, DATE, "pt")), "{n:?}");
        }
        assert!(display_name(&"ã".repeat(200), DATE, "pt").len() <= 100 + " - 2026-10-22 - PT.pdf".len());
    }

    #[test]
    fn write_keeps_one_report_per_project_and_its_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let (p1, p2) = (ids::new_id(), ids::new_id());
        let (first, meta) = write_report(data, &p1, "20261012T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-12.pdf")).unwrap();
        assert_eq!(first, data.join(format!("reports/{p1}-20261012T100000000Z.pdf")));
        assert_eq!(meta.file, format!("{p1}-20261012T100000000Z.pdf"));
        assert_eq!((meta.bytes, meta.pages, meta.lang.as_str()), (6, 3, "pt"));
        assert_eq!(latest(data, &p1).unwrap(), Some(meta));
        write_report(data, &p2, "20261012T100000000Z", b"%PDF-2", new_report("Outra - 2026-10-12.pdf")).unwrap();
        std::fs::write(data.join(format!("reports/{p1}-20261012T090000000Z.pdf.part")), b"interrupted").unwrap();
        std::fs::write(data.join(format!("reports/{p1}-notes.txt")), b"not a report").unwrap();

        // A new report for p1 replaces its earlier one and the leftover part file; p2's stays.
        let (second, meta2) = write_report(data, &p1, "20261012T110000000Z", b"%PDF-33", new_report("Obra - 2026-10-12.pdf")).unwrap();
        assert_eq!(std::fs::read(&second).unwrap(), b"%PDF-33");
        assert_eq!(latest(data, &p1).unwrap(), Some(meta2));
        let mut expected = vec![
            format!("{p1}-20261012T110000000Z.pdf"),
            format!("{p1}-notes.txt"),
            format!("{p1}.json"),
            format!("{p2}-20261012T100000000Z.pdf"),
            format!("{p2}.json"),
        ];
        expected.sort();
        assert_eq!(names(&data.join("reports")), expected);
    }

    #[test]
    fn write_refuses_a_name_that_is_not_a_safe_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let p = ids::new_id();
        let too_long = format!("{}.pdf", "a".repeat(252));
        // `display_name` does not clean `lang`, so a hostile one shows up here, not in a file name.
        let hostile_lang = display_name("Obra", DATE, "p/t");
        let unsafe_names = ["", "no-suffix", "../x.pdf", "a/b.pdf", "a\\b.pdf", ".hidden.pdf", "a\nb.pdf", "a:b.pdf", &too_long, &hostile_lang];

        // Nothing is written: not the PDF, not the metadata, not even the reports dir.
        for bad in unsafe_names {
            let err = write_report(data, &p, "20261012T100000000Z", b"%PDF-1", new_report(bad));
            assert!(matches!(err, Err(CoreError::Validation(_))), "{bad:?}");
        }
        assert!(!data.join("reports").exists());

        // A refused name leaves the previous report as it was.
        let (_, before) = write_report(data, &p, "20261012T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-12.pdf")).unwrap();
        let listing = names(&data.join("reports"));
        assert!(write_report(data, &p, "20261012T110000000Z", b"%PDF-2", new_report("../x.pdf")).is_err());
        assert_eq!(names(&data.join("reports")), listing);
        assert_eq!(latest(data, &p).unwrap(), Some(before));
    }

    #[test]
    fn a_failed_write_keeps_the_previous_report() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let p = ids::new_id();
        let (_, before) = write_report(data, &p, "20261012T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-12.pdf")).unwrap();

        // The PDF's destination is blocked by a directory: an error, no part file, the old report kept.
        std::fs::create_dir(data.join(format!("reports/{p}-20261012T120000000Z.pdf"))).unwrap();
        assert!(matches!(write_report(data, &p, "20261012T120000000Z", b"%PDF-2", new_report("x.pdf")), Err(CoreError::Io(_))));
        assert!(!data.join(format!("reports/{p}-20261012T120000000Z.pdf.part")).exists());
        assert_eq!(latest(data, &p).unwrap(), Some(before.clone()));

        // The metadata cannot be replaced (its name is a directory now): the new PDF is removed again.
        let meta_path = meta_file(&p).resolve(data);
        let saved = std::fs::read(&meta_path).unwrap();
        std::fs::remove_file(&meta_path).unwrap();
        std::fs::create_dir(&meta_path).unwrap();
        assert!(write_report(data, &p, "20261012T130000000Z", b"%PDF-3", new_report("x.pdf")).is_err());
        assert!(!data.join(format!("reports/{p}-20261012T130000000Z.pdf")).exists());
        assert!(!data.join(format!("reports/{p}.json.part")).exists());
        assert!(data.join(format!("reports/{}", before.file)).exists(), "the previous PDF is not pruned");
        std::fs::remove_dir(&meta_path).unwrap();
        std::fs::write(&meta_path, saved).unwrap();
        assert_eq!(latest(data, &p).unwrap(), Some(before));
    }

    #[test]
    fn latest_is_none_unless_the_metadata_matches_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let p = ids::new_id();
        assert_eq!(latest(data, &p).unwrap(), None, "no reports dir");
        let (pdf, meta) = write_report(data, &p, "20261012T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-12.pdf")).unwrap();
        let meta_path = meta_file(&p).resolve(data);
        let write_meta = |m: &ReportMeta| std::fs::write(&meta_path, serde_json::to_vec(m).unwrap()).unwrap();

        write_meta(&ReportMeta { bytes: 7, ..meta.clone() });
        assert_eq!(latest(data, &p).unwrap(), None, "size mismatch");
        write_meta(&ReportMeta { v: 2, ..meta.clone() });
        assert_eq!(latest(data, &p).unwrap(), None, "unknown version");
        write_meta(&ReportMeta { file: "../checkflat.db".into(), ..meta.clone() });
        assert_eq!(latest(data, &p).unwrap(), None, "a file that is not this project's report");
        write_meta(&ReportMeta { name: "../../x.pdf".into(), ..meta.clone() });
        assert_eq!(latest(data, &p).unwrap(), None, "an unsafe name");
        std::fs::write(&meta_path, b"{ not json").unwrap();
        assert_eq!(latest(data, &p).unwrap(), None, "garbled");
        write_meta(&meta);
        assert_eq!(latest(data, &p).unwrap(), Some(meta));
        std::fs::remove_file(&pdf).unwrap();
        assert_eq!(latest(data, &p).unwrap(), None, "PDF gone");

        // A pre-6a report (no metadata) reads as none.
        let q = ids::new_id();
        std::fs::write(data.join(format!("reports/{q}-20261012T100000000Z.pdf")), b"%PDF").unwrap();
        assert_eq!(latest(data, &q).unwrap(), None);
    }

    #[test]
    fn ids_that_are_not_uuids_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        for bad in ["..", "../x", "p1", ""] {
            assert!(matches!(latest(data, bad), Err(CoreError::Validation(_))), "{bad:?}");
            assert!(matches!(share_copy(data, bad, &data.join("cache")), Err(CoreError::Validation(_))));
            assert!(matches!(remove_for_project(data, bad), Err(CoreError::Validation(_))));
            assert!(matches!(remove_share_copies(&data.join("cache"), bad), Err(CoreError::Validation(_))));
            assert!(matches!(write_report(data, bad, "20261012T100000000Z", b"%PDF", new_report("x.pdf")), Err(CoreError::Validation(_))));
        }
    }

    /// Holds `p` open the way a PDF viewer such as Acrobat does on Windows: others may read it but
    /// not delete, rename or replace it. std's own `File::open` shares delete on Windows, so it
    /// would hold nothing there. On Unix an open file never blocks a rename or a delete; there the
    /// same tests check that nothing changes.
    fn hold(p: &Path) -> std::fs::File {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 0x1;
            std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(p).unwrap()
        }
        #[cfg(not(windows))]
        std::fs::File::open(p).unwrap()
    }
    /// Whether a held file blocks its removal and replacement (Windows only).
    const HELD_BLOCKS: bool = cfg!(windows);

    #[test]
    fn share_copy_has_a_folder_per_report_and_purges_old_copies() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let share = dir.path().join("cache/reports");
        let (p, q) = (ids::new_id(), ids::new_id());
        assert_eq!(share_copy(&data, &p, &share).unwrap(), None, "no report yet");

        write_report(&data, &p, "20261022T100000000Z", b"%PDF-p", new_report("Obra - 2026-10-22.pdf")).unwrap();
        write_report(&data, &q, "20261022T100000000Z", b"%PDF-q", new_report("Obra - 2026-10-22.pdf")).unwrap();
        let cp = share_copy(&data, &p, &share).unwrap().unwrap();
        let cq = share_copy(&data, &q, &share).unwrap().unwrap();
        assert_eq!(cp, share.join(&p).join("20261022T100000000Z").join("Obra - 2026-10-22.pdf"));
        assert_eq!(cq, share.join(&q).join("20261022T100000000Z").join("Obra - 2026-10-22.pdf"), "same name, own folder");
        assert_eq!(std::fs::read(&cp).unwrap(), b"%PDF-p");
        assert_eq!(std::fs::read(&cq).unwrap(), b"%PDF-q");

        // The same report again: the copy is reused, not rewritten.
        set_age(&cp, 2);
        let before = std::fs::metadata(&cp).unwrap().modified().unwrap();
        assert_eq!(share_copy(&data, &p, &share).unwrap().unwrap(), cp);
        assert_eq!(std::fs::metadata(&cp).unwrap().modified().unwrap(), before);
        assert_eq!(names(cp.parent().unwrap()), ["Obra - 2026-10-22.pdf"], "no part file left");

        // A new report of the same name gets its own folder; the earlier copy stays until it is old.
        write_report(&data, &p, "20261022T110000000Z", b"%PDF-p2", new_report("Obra - 2026-10-22.pdf")).unwrap();
        let cp2 = share_copy(&data, &p, &share).unwrap().unwrap();
        assert_eq!(cp2, share.join(&p).join("20261022T110000000Z").join("Obra - 2026-10-22.pdf"));
        assert_eq!(std::fs::read(&cp2).unwrap(), b"%PDF-p2");
        assert_eq!(std::fs::read(&cp).unwrap(), b"%PDF-p");

        // Copies older than a day go before the next copy, then their folders; younger ones stay, as
        // do flat copies from Sprint 6a until they are old.
        set_age(&cp, 25);
        let stale_6a = share.join(&p).join("Obra - 2026-10-20.pdf");
        std::fs::write(&stale_6a, b"%PDF-old").unwrap();
        set_age(&stale_6a, 25);
        let young_6a = share.join(&p).join("Obra - 2026-10-21.pdf");
        std::fs::write(&young_6a, b"%PDF-young").unwrap();
        set_age(&young_6a, 23);
        set_age(&cq, 25);
        share_copy(&data, &p, &share).unwrap().unwrap();
        assert!(!cp.exists() && !cp.parent().unwrap().exists(), "the old report's copy and its folder");
        assert!(!stale_6a.exists());
        assert!(young_6a.exists());
        assert!(cp2.exists());
        assert!(!share.join(&q).exists(), "q's only copy was old: its folders are gone too");

        remove_share_copies(&share, &p).unwrap();
        assert!(!share.join(&p).exists());
        remove_share_copies(&share, &p).unwrap();
    }

    /// Windows: a PDF viewer keeps the copy it shows locked (D-029). Open again must not fail, a new
    /// report must not be blocked, and the purge must not fail over a held copy.
    #[test]
    fn share_copy_with_an_earlier_copy_held_open() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let share = dir.path().join("cache/reports");
        let p = ids::new_id();
        write_report(&data, &p, "20261022T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        let first = share_copy(&data, &p, &share).unwrap().unwrap();

        // Open again while the viewer holds the copy: the same file, untouched.
        let held = hold(&first);
        assert_eq!(share_copy(&data, &p, &share).unwrap().unwrap(), first);
        assert_eq!(std::fs::read(&first).unwrap(), b"%PDF-1");
        // A new report while the old copy is held: its own folder.
        write_report(&data, &p, "20261022T110000000Z", b"%PDF-22", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        let second = share_copy(&data, &p, &share).unwrap().unwrap();
        assert_ne!(second, first);
        assert_eq!(std::fs::read(&second).unwrap(), b"%PDF-22");
        drop(held);

        // A held copy past a day: the purge logs and goes on; the copy goes once it is released.
        set_age(&first, 25);
        let held = hold(&first);
        assert_eq!(share_copy(&data, &p, &share).unwrap().unwrap(), second);
        assert_eq!(first.exists(), HELD_BLOCKS);
        drop(held);
        share_copy(&data, &p, &share).unwrap().unwrap();
        assert!(!first.exists());
        assert!(second.exists());
    }

    /// Windows: regenerating while the viewer still shows the stored report or a share copy.
    #[test]
    fn regenerate_while_the_stored_report_or_a_copy_is_held_open() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let share = dir.path().join("cache/reports");
        let p = ids::new_id();
        let (pdf1, _) = write_report(&data, &p, "20261022T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        let copy1 = share_copy(&data, &p, &share).unwrap().unwrap();
        let (held_pdf, held_copy) = (hold(&pdf1), hold(&copy1));

        let (pdf2, meta2) = write_report(&data, &p, "20261022T110000000Z", b"%PDF-22", new_report("Obra - 2026-10-22 - EN.pdf")).unwrap();
        assert_eq!(latest(&data, &p).unwrap(), Some(meta2), "the new report is the latest");
        assert_eq!(std::fs::read(&pdf2).unwrap(), b"%PDF-22");
        assert_eq!(pdf1.exists(), HELD_BLOCKS, "a held old PDF survives the prune (logged)");
        assert!(copy1.exists(), "a held share copy is not touched by generation");
        drop((held_pdf, held_copy));

        // Released: the next generation prunes it.
        let (_, meta3) = write_report(&data, &p, "20261022T120000000Z", b"%PDF-333", new_report("Obra - 2026-10-22 - EN.pdf")).unwrap();
        assert!(!pdf1.exists() && !pdf2.exists());
        assert_eq!(latest(&data, &p).unwrap(), Some(meta3));
        assert_eq!(names(&data.join("reports")), [format!("{p}-20261022T120000000Z.pdf"), format!("{p}.json")]);
    }

    #[test]
    fn save_copy_writes_a_whole_copy_and_replaces_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let out = dir.path().join("Desktop");
        std::fs::create_dir_all(&out).unwrap();
        let dest = out.join("Obra - 2026-10-22 - PT.pdf");
        let p = ids::new_id();
        assert_eq!(save_copy(&data, &p, &dest).unwrap(), None, "no report yet");
        assert!(!dest.exists());

        write_report(&data, &p, "20261022T100000000Z", b"%PDF-1", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        assert_eq!(save_copy(&data, &p, &dest).unwrap(), Some(dest.clone()));
        assert_eq!(std::fs::read(&dest).unwrap(), b"%PDF-1");
        assert_eq!(names(&out), ["Obra - 2026-10-22 - PT.pdf"], "no part file left");

        // Over an existing file (the dialog asked before replacing it): replaced whole.
        write_report(&data, &p, "20261022T110000000Z", b"%PDF-22", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        save_copy(&data, &p, &dest).unwrap().unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"%PDF-22");
        assert_eq!(names(&out), ["Obra - 2026-10-22 - PT.pdf"]);

        // A destination held open by a viewer: an error on Windows with the file as it was; no part
        // file left either way.
        write_report(&data, &p, "20261022T120000000Z", b"%PDF-333", new_report("Obra - 2026-10-22 - PT.pdf")).unwrap();
        let held = hold(&dest);
        let saved = save_copy(&data, &p, &dest);
        assert_eq!(saved.is_err(), HELD_BLOCKS, "{saved:?}");
        drop(held);
        assert_eq!(std::fs::read(&dest).unwrap(), if HELD_BLOCKS { &b"%PDF-22"[..] } else { &b"%PDF-333"[..] });
        assert_eq!(names(&out), ["Obra - 2026-10-22 - PT.pdf"]);

        // A folder that does not exist: an error, nothing written.
        assert!(matches!(save_copy(&data, &p, &dir.path().join("gone/x.pdf")), Err(CoreError::Io(_))));
        assert!(!dir.path().join("gone").exists());
    }

    #[test]
    fn remove_for_project_takes_only_that_projects_files() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        let (p, q) = (ids::new_id(), ids::new_id());
        write_report(data, &p, "20261022T100000000Z", b"%PDF-p", new_report("P - 2026-10-22.pdf")).unwrap();
        write_report(data, &q, "20261022T100000000Z", b"%PDF-q", new_report("Q - 2026-10-22.pdf")).unwrap();
        std::fs::write(data.join(format!("reports/{p}-20261022T090000000Z.pdf.part")), b"x").unwrap();
        std::fs::write(data.join(format!("reports/{p}.json.part")), b"x").unwrap();
        std::fs::write(data.join(format!("reports/{p}-notes.txt")), b"x").unwrap();
        remove_for_project(data, &p).unwrap();
        let mut expected = vec![format!("{p}-notes.txt"), format!("{q}-20261022T100000000Z.pdf"), format!("{q}.json")];
        expected.sort();
        assert_eq!(names(&data.join("reports")), expected);
        assert_eq!(latest(data, &p).unwrap(), None);
        let empty = tempfile::tempdir().unwrap();
        remove_for_project(empty.path(), &p).unwrap();
    }

    #[test]
    fn owner_of_recognises_report_files_only() {
        let p = ids::new_id();
        assert_eq!(owner_of(&format!("{p}-20261022T100000000Z.pdf")), Some(p.as_str()));
        assert_eq!(owner_of(&format!("{p}-20261022T100000000Z.pdf.part")), Some(p.as_str()));
        assert_eq!(owner_of(&format!("{p}.json")), Some(p.as_str()));
        assert_eq!(owner_of(&format!("{p}.json.part")), Some(p.as_str()));
        assert_eq!(owner_of(&format!("{p}-notes.txt")), None);
        assert_eq!(owner_of(&format!("{p}.pdf")), None);
        assert_eq!(owner_of("p1-20261022T100000000Z.pdf"), None);
        assert_eq!(owner_of(&format!("{}-x.pdf", p.to_uppercase())), None);
        assert_eq!(owner_of("ã"), None);
    }
}
