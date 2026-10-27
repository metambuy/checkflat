//! A start that cannot finish: the data folder or the database cannot be opened, or a migration
//! fails (D-029, v0.1 review). Without this the window never appears and the app just goes away — on
//! Windows not even a console (`windows_subsystem = "windows"`), on Android a "keeps stopping" dialog
//! — and a tester cannot tell what happened. The failure is written to `startup-error.log`, shown in
//! a native message with the error and the data folder, and the process exits.
//!
//! The message is in English and Portuguese at once: the language setting lives in the database that
//! failed to open.
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// What the message says: what failed, where the data is, where the details were saved, what to try.
pub(crate) fn message(error: &str, data_dir: Option<&Path>, log: Option<&Path>) -> String {
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
    out.push_str(
        "\nClose other Checkflat windows and any program that may be using these files (backup, antivirus), then start the app again.\n\
         Feche outras janelas do Checkflat e programas que possam estar a usar estes ficheiros (cópias de segurança, antivírus) e abra a aplicação de novo.",
    );
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
    show(app, &message(&text, data_dir, log.as_deref()));
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
        let m = message("migration error: foreign key mismatch", Some(dir), Some(&log));
        for needle in [
            "could not open its data",
            "não conseguiu abrir os seus dados",
            "migration error: foreign key mismatch",
            "C:\\Users\\João\\AppData\\Roaming\\com.checkflat.app",
            "startup-error.log",
            "antivirus",
            "antivírus",
        ] {
            assert!(m.contains(needle), "{needle} in {m}");
        }
        let unknown = message("x", None, None);
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
}
