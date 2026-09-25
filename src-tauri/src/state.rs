//! Single database connection + data dir, created in `setup`.
//!
//! Locking rule: commands are `async`, do their `.await`s first, then run DB/disk work inside
//! `spawn_blocking` via [`run_db`], which takes the `std::sync::Mutex` and drops it before
//! returning. A guard can therefore never be held across an `.await` (it would not be `Send`).
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use checkflat_core::rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

pub struct AppState {
    db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(conn: Connection, data_dir: PathBuf) -> Self {
        AppState { db: Mutex::new(conn), data_dir }
    }

    /// Lock, recovering from poisoning (a panic in one command must not brick the rest).
    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Run `f` with the connection on the blocking pool.
pub async fn run_db<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &Path) -> checkflat_core::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = state.lock();
        f(&conn, &state.data_dir)
    })
    .await
    .map_err(AppError::internal)?
    .map_err(Into::into)
}

/// Run `f` with the data dir only (no DB lock) on the blocking pool.
pub async fn run_fs<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Path) -> checkflat_core::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        f(&state.data_dir)
    })
    .await
    .map_err(AppError::internal)?
    .map_err(Into::into)
}
