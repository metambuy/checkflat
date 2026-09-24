use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<CameraCapture<R>> {
    Ok(CameraCapture(app.clone()))
}

/// Access to the camera-capture APIs.
pub struct CameraCapture<R: Runtime>(AppHandle<R>);

impl<R: Runtime> CameraCapture<R> {
    pub fn capture(&self) -> crate::Result<CaptureResponse> {
        Err(crate::Error::Unsupported)
    }
}
