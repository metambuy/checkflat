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

    /// Opens a report copy in the default PDF app (`ShellExecuteExW` on Windows, `open` on macOS,
    /// D-029). Windows' "no association" becomes [`crate::Error::NoApp`]; Windows usually asks
    /// "How do you want to open this file?" itself instead.
    pub fn view_pdf(&self, path: String) -> crate::Result<()> {
        /// `ERROR_NO_ASSOCIATION`: no app is registered for the file type.
        const NO_ASSOCIATION: i32 = 1155;
        open::that_detached(&path).map_err(|e| if e.raw_os_error() == Some(NO_ASSOCIATION) { crate::Error::NoApp } else { e.into() })
    }

    /// Android only; Windows saves a copy instead (`save_report_copy`, D-029).
    pub fn share_pdf(&self, _path: String, _title: String) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }

    /// Android only: a desktop window stays where it is.
    pub fn move_task_to_back(&self) -> crate::Result<()> {
        Ok(())
    }

    /// The package version, and the build number when the build was given one: CI passes its run
    /// number as `CHECKFLAT_BUILD` (the Windows installer, D-029), as Android's versionCode is.
    /// Local builds have none.
    pub fn app_version(&self) -> crate::Result<AppVersion> {
        let version_code = option_env!("CHECKFLAT_BUILD").and_then(|b| b.trim().parse().ok());
        Ok(AppVersion { version_name: self.0.package_info().version.to_string(), version_code })
    }
}
