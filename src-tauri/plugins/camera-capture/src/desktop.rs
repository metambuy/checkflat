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

    /// Nothing is ever lost behind a camera on desktop.
    pub fn take_recovered_capture(&self) -> crate::Result<CaptureResponse> {
        Ok(CaptureResponse::default())
    }

    pub fn pick_image(&self) -> crate::Result<PickResponse> {
        Err(crate::Error::Unsupported)
    }

    /// Desktop paths carry their own file name; nothing to resolve.
    pub fn display_name(&self, _uri: String) -> crate::Result<Option<String>> {
        Ok(None)
    }

    /// Android only (Windows saves the report instead, Sprint 6b).
    pub fn view_pdf(&self, _path: String) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    pub fn share_pdf(&self, _path: String, _title: String) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// The package version; desktop builds carry no build number.
    pub fn app_version(&self) -> crate::Result<AppVersion> {
        Ok(AppVersion { version_name: self.0.package_info().version.to_string(), version_code: None })
    }
}
