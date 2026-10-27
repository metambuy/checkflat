//! A start that cannot finish: the data folder or the database cannot be opened, or a migration
//! fails (D-029, v0.1 review). Without this the window never appears and the app just goes away — on
//! Windows not even a console (`windows_subsystem = "windows"`), on Android a "keeps stopping" dialog
//! — and a tester cannot tell what happened. The failure is written to `startup-error.log`, shown in
//! a native message with the error and the data folder, and the process exits.
//!
//! The message is in English and Portuguese at once: the language setting lives in the database that
//! failed to open. Its last paragraph depends on the kind of failure ([`Advice`]): something else
//! holding the files is worth closing and trying again; anything else may be damage, and the one thing
//! not to do then is delete the folder.
use std::io::Write;
use std::path::{Path, PathBuf};

use checkflat_core::rusqlite::{self, ErrorCode};

pub(crate) const LOG_NAME: &str = "startup-error.log";
const TITLE: &str = "Checkflat";

/// The error and what caused it, once each.
pub(crate) fn describe(error: &(dyn std::error::Error + 'static)) -> String {
    let mut out = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        let text = c.to_string();
        if !out.contains(&text) {
            out.push_str(&format!(" ({text})"));
        }
        cause = c.source();
    }
    out
}

/// What the message tells the user to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Advice {
    /// Something else has the files for now (another window, a backup or antivirus scan): close it
    /// and start again.
    CloseOthers,
    /// The database is corrupt or unreadable, a migration failed, or the cause is not known: leave
    /// the folder alone and send the message on. Deleting the folder is what a tester would try.
    DontDelete,
}

const CLOSE_OTHERS: &str = "Close other Checkflat windows and any program that may be using these files (backup, antivirus), then start the app again.\n\
     Feche outras janelas do Checkflat e programas que possam estar a usar estes ficheiros (cópias de segurança, antivírus) e abra a aplicação de novo.";
const DONT_DELETE: &str = "Don't delete this folder. Send a screenshot of this message to whoever gave you the app.\n\
     Não apague esta pasta. Envie uma captura deste ecrã a quem lhe deu a aplicação.";

/// SQLite says another connection or process holds the database (`SQLITE_BUSY`, `SQLITE_LOCKED` and
/// their extended codes).
fn sqlite_busy(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked))
}

/// The OS says the file is in use: `EBUSY` / `ERROR_BUSY` (`ResourceBusy`), and on Windows a sharing
/// or lock violation (32, 33) — what a PDF viewer, a backup or an antivirus scan gives.
fn file_in_use(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::ResourceBusy {
        return true;
    }
    #[cfg(windows)]
    if matches!(e.raw_os_error(), Some(32 | 33)) {
        return true;
    }
    false
}

/// Which advice fits this failure: [`Advice::CloseOthers`] when anything in the error's chain is a
/// busy or locked database or a file in use, [`Advice::DontDelete`] otherwise (a database that is
/// not one, corruption, a failed migration, a folder that cannot be created, and what is not known).
pub(crate) fn advice(error: &(dyn std::error::Error + 'static)) -> Advice {
    let mut current = Some(error);
    while let Some(e) = current {
        let locked = e.downcast_ref::<rusqlite::Error>().is_some_and(sqlite_busy) || e.downcast_ref::<std::io::Error>().is_some_and(file_in_use);
        if locked {
            return Advice::CloseOthers;
        }
        current = e.source();
    }
    Advice::DontDelete
}

/// What the message says: what failed, where the data is, where the details were saved, what to try.
pub(crate) fn message(error: &str, data_dir: Option<&Path>, log: Option<&Path>, advice: Advice) -> String {
    let dir = data_dir.map_or_else(|| "(unknown / desconhecida)".to_string(), |d| d.display().to_string());
    let mut out = format!(
        "Checkflat could not open its data and has to close.\n\
         O Checkflat não conseguiu abrir os seus dados e vai fechar.\n\n\
         {error}\n\n\
         Data folder / Pasta de dados:\n{dir}\n"
    );
    if let Some(log) = log {
        out.push_str(&format!("\nDetails saved in / Detalhes guardados em:\n{}\n", log.display()));
    }
    out.push('\n');
    out.push_str(match advice {
        Advice::CloseOthers => CLOSE_OTHERS,
        Advice::DontDelete => DONT_DELETE,
    });
    out
}

/// Appends the failure to `startup-error.log` in the first of `dirs` that accepts it (the data
/// folder, then the temp folder: the data folder is what may be unusable); returns the file.
pub(crate) fn write_log(dirs: &[&Path], when: &str, version: &str, error: &str, data_dir: Option<&Path>) -> Option<PathBuf> {
    let line = format!("{when}\tCheckflat {version}\t{error}\tdata folder: {}\n", data_dir.map_or("unknown".to_string(), |d| d.display().to_string()));
    for dir in dirs {
        let path = dir.join(LOG_NAME);
        let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| f.write_all(line.as_bytes()));
        match written {
            Ok(()) => return Some(path),
            Err(e) => log::warn!("could not write {}: {e}", path.display()),
        }
    }
    None
}

/// Logs the failure, shows it and exits. Called from `setup` when the app cannot start.
pub(crate) fn fail(app: &tauri::App, data_dir: Option<&Path>, error: &(dyn std::error::Error + 'static)) -> ! {
    let text = describe(error);
    log::error!("startup failed: {text} (data folder: {data_dir:?})");
    let temp = std::env::temp_dir();
    let dirs: Vec<&Path> = data_dir.into_iter().chain(std::iter::once(temp.as_path())).collect();
    let log = write_log(&dirs, &checkflat_core::clock::now_iso(), &app.package_info().version.to_string(), &text, data_dir);
    show(app, &message(&text, data_dir, log.as_deref(), advice(error)));
    std::process::exit(1)
}

/// Desktop: rfd's synchronous message box on this thread. The dialog plugin's own blocking call hands
/// the dialog to the main thread and waits, and `setup` runs on the main thread, inside the event
/// loop (on macOS that never returns).
#[cfg(desktop)]
fn show(_app: &tauri::App, text: &str) {
    rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_title(TITLE).set_description(text).set_buttons(rfd::MessageButtons::Ok).show();
}

/// Android: the dialog plugin's message (an `AlertDialog`), waited for before the process exits.
#[cfg(mobile)]
fn show(app: &tauri::App, text: &str) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    app.dialog().message(text).title(TITLE).kind(MessageDialogKind::Error).blocking_show();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Outer(std::io::Error);
    impl std::fmt::Display for Outer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "database error")
        }
    }
    impl std::error::Error for Outer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn the_error_is_described_with_its_cause_once() {
        let e = Outer(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied"));
        assert_eq!(describe(&e), "database error (access denied)");
        let plain = std::io::Error::other("disk full");
        assert_eq!(describe(&plain), "disk full");
    }

    #[test]
    fn the_message_names_the_error_the_data_folder_and_the_log_in_both_languages() {
        let dir = Path::new("C:\\Users\\João\\AppData\\Roaming\\com.checkflat.app");
        let log = dir.join(LOG_NAME);
        let m = message("migration error: foreign key mismatch", Some(dir), Some(&log), Advice::DontDelete);
        for needle in [
            "could not open its data",
            "não conseguiu abrir os seus dados",
            "migration error: foreign key mismatch",
            "C:\\Users\\João\\AppData\\Roaming\\com.checkflat.app",
            "startup-error.log",
        ] {
            assert!(m.contains(needle), "{needle} in {m}");
        }
        let unknown = message("x", None, None, Advice::CloseOthers);
        assert!(unknown.contains("desconhecida") && !unknown.contains("Details saved"), "{unknown}");
    }

    #[test]
    fn the_log_goes_to_the_first_folder_that_accepts_it_and_is_appended_to() {
        let tmp = tempfile::tempdir().unwrap();
        let ok = tmp.path().join("ok");
        std::fs::create_dir(&ok).unwrap();
        let missing = tmp.path().join("missing"); // the data folder that cannot be used
        let first = write_log(&[&missing, &ok], "2026-10-27T10:00:00.000Z", "0.1.0", "no such file", Some(&missing)).unwrap();
        assert_eq!(first, ok.join(LOG_NAME), "fell through to the second folder");
        let second = write_log(&[&ok], "2026-10-27T10:05:00.000Z", "0.1.0", "locked", Some(&ok)).unwrap();
        assert_eq!(second, first);
        let text = std::fs::read_to_string(&first).unwrap();
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(text.contains("no such file") && text.contains("locked") && text.contains("0.1.0"), "{text}");
        assert_eq!(write_log(&[&missing], "t", "v", "e", None), None, "nowhere to write: no file, no panic");
    }

    use checkflat_core::rusqlite::{ffi, Connection};
    use checkflat_core::CoreError;

    fn sqlite(code: i32) -> Box<dyn std::error::Error> {
        Box::new(CoreError::Db(rusqlite::Error::SqliteFailure(ffi::Error::new(code), None)))
    }

    #[test]
    fn a_busy_or_locked_database_or_a_file_in_use_says_close_the_other_windows() {
        // SQLITE_BUSY, SQLITE_LOCKED and the extended codes of both (BUSY_RECOVERY, BUSY_SNAPSHOT, LOCKED_SHAREDCACHE).
        for code in [5, 6, 5 | (1 << 8), 5 | (2 << 8), 6 | (1 << 8)] {
            assert_eq!(advice(sqlite(code).as_ref()), Advice::CloseOthers, "sqlite code {code}");
        }
        assert_eq!(advice(&std::io::Error::from(std::io::ErrorKind::ResourceBusy)), Advice::CloseOthers);
        // Behind another error: a migration that could not start because the file was locked, and a
        // data folder whose creation failed for the same reason.
        let wrapped = Outer(std::io::Error::from(std::io::ErrorKind::ResourceBusy));
        assert_eq!(advice(&wrapped), Advice::CloseOthers);
        #[cfg(windows)]
        for raw in [32, 33] {
            assert_eq!(advice(&std::io::Error::from_raw_os_error(raw)), Advice::CloseOthers, "windows error {raw}: a sharing or lock violation");
        }
    }

    #[test]
    fn a_corrupt_or_unreadable_database_a_failed_migration_or_anything_unknown_says_do_not_delete() {
        // NOTADB, CORRUPT (+ its extended code), CANTOPEN, IOERR, PERM, READONLY, FULL, NOMEM, MISUSE.
        for code in [26, 11, 11 | (1 << 8), 14, 10, 3, 8, 13, 7, 21] {
            assert_eq!(advice(sqlite(code).as_ref()), Advice::DontDelete, "sqlite code {code}");
        }
        for kind in [std::io::ErrorKind::PermissionDenied, std::io::ErrorKind::NotFound, std::io::ErrorKind::StorageFull, std::io::ErrorKind::Other] {
            assert_eq!(advice(&std::io::Error::from(kind)), Advice::DontDelete, "{kind:?}");
        }
        let unknown: Box<dyn std::error::Error> = "the path of the data folder is unknown".into();
        assert_eq!(advice(unknown.as_ref()), Advice::DontDelete);
    }

    /// The mapping on errors `db::open` really returns, not only on constructed ones.
    #[test]
    fn the_real_errors_of_a_garbage_file_a_failed_migration_and_a_locked_database() {
        let dir = tempfile::tempdir().unwrap();

        // A file that is not a database.
        let garbage = dir.path().join("garbage.db");
        std::fs::write(&garbage, vec![0xA5u8; 4096]).unwrap();
        let Err(not_a_db) = checkflat_core::db::open(&garbage) else { panic!("a garbage file must not open") };
        assert!(not_a_db.to_string().contains("not a database"), "{not_a_db}");
        assert_eq!(advice(&not_a_db), Advice::DontDelete);

        // A v1 database with an observation whose plan is gone: the migration's foreign-key check fails.
        let v1 = dir.path().join("v1.db");
        {
            let conn = Connection::open(&v1).unwrap();
            conn.execute_batch(checkflat_core::migrations::SQL_V1).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = OFF;
                 INSERT INTO project(id, name, created_at, updated_at) VALUES ('p1', 'Obra', 't', 't');
                 INSERT INTO observation(id, project_id, plan_id, ref_no, x_norm, y_norm, created_at, updated_at) VALUES ('bad', 'p1', 'nope', 1, 0.5, 0.5, 't', 't');
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let Err(failed) = checkflat_core::db::open(&v1) else { panic!("the migration must fail") };
        assert!(matches!(failed, CoreError::Migration(_)), "{failed:?}");
        assert_eq!(advice(&failed), Advice::DontDelete);

        // A database another connection holds: opening it asks for the WAL journal, which needs the lock.
        let busy = dir.path().join("busy.db");
        let holder = Connection::open(&busy).unwrap();
        holder.execute_batch("CREATE TABLE t(x); BEGIN EXCLUSIVE;").unwrap();
        let Err(locked) = checkflat_core::db::open(&busy) else { panic!("a locked database must not open") };
        assert!(locked.to_string().contains("locked") || locked.to_string().contains("busy"), "{locked}");
        assert_eq!(advice(&locked), Advice::CloseOthers);
        drop(holder);
    }

    #[test]
    fn the_message_ends_with_the_advice_that_fits_in_both_languages() {
        let close = message("database error: database is locked", None, None, Advice::CloseOthers);
        assert!(close.contains("Close other Checkflat windows and any program that may be using these files"), "{close}");
        assert!(close.contains("Feche outras janelas do Checkflat e programas que possam estar a usar estes ficheiros"), "{close}");
        assert!(!close.contains("delete this folder") && !close.contains("Não apague"), "{close}");

        let keep = message("database error: file is not a database", None, None, Advice::DontDelete);
        assert!(keep.ends_with("Don't delete this folder. Send a screenshot of this message to whoever gave you the app.\nNão apague esta pasta. Envie uma captura deste ecrã a quem lhe deu a aplicação."), "{keep}");
        assert!(!keep.contains("Close other Checkflat windows") && !keep.contains("Feche outras"), "{keep}");
    }
}
