//! Sprint 0 Spike B fallback: `<input capture>` opens the Photo Picker on Android WebView,
//! so the camera is launched natively via `MediaStore.ACTION_IMAGE_CAPTURE`.
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::CameraCapture;
#[cfg(mobile)]
use mobile::CameraCapture;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the camera-capture APIs.
pub trait CameraCaptureExt<R: Runtime> {
    fn camera_capture(&self) -> &CameraCapture<R>;
}

impl<R: Runtime, T: Manager<R>> crate::CameraCaptureExt<R> for T {
    fn camera_capture(&self) -> &CameraCapture<R> {
        self.state::<CameraCapture<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("camera-capture")
        .invoke_handler(tauri::generate_handler![commands::capture])
        .setup(|app, api| {
            #[cfg(mobile)]
            let camera_capture = mobile::init(app, api)?;
            #[cfg(desktop)]
            let camera_capture = desktop::init(app, api)?;
            app.manage(camera_capture);
            Ok(())
        })
        .build()
}
