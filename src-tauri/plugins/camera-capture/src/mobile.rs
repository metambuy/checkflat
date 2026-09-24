use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<CameraCapture<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.checkflat.cameracapture", "CameraCapturePlugin")?;
    #[cfg(not(target_os = "android"))]
    compile_error!("camera-capture: only Android is implemented");
    Ok(CameraCapture(handle))
}

/// Access to the camera-capture APIs.
pub struct CameraCapture<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> CameraCapture<R> {
    /// Opens the system camera and blocks until the user takes a photo or cancels.
    /// Must not be called on the main thread (use an async command).
    pub fn capture(&self) -> crate::Result<CaptureResponse> {
        self.0.run_mobile_plugin("capture", ()).map_err(Into::into)
    }
}
