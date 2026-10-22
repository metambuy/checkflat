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

    /// A capture that finished while the app was not running (see the Kotlin side); `None` path when there is none.
    pub fn take_recovered_capture(&self) -> crate::Result<CaptureResponse> {
        self.0.run_mobile_plugin("takeRecoveredCapture", ()).map_err(Into::into)
    }

    /// Opens the system Photo Picker and blocks until the user picks an image or cancels.
    /// Must not be called on the main thread (use an async command).
    pub fn pick_image(&self) -> crate::Result<PickResponse> {
        self.0.run_mobile_plugin("pickImage", ()).map_err(Into::into)
    }

    /// Display name of a `content://` or `file://` URI (ContentResolver query on a background thread).
    pub fn display_name(&self, uri: String) -> crate::Result<Option<String>> {
        let r: DisplayNameResponse = self.0.run_mobile_plugin("displayName", DisplayNameArgs { uri })?;
        Ok(r.name)
    }

    /// Opens a report copy (a file under the app cache's `reports/`) in a PDF viewer. When no app
    /// can open PDFs the error's [`crate::Error::rejected_code`] is `no_app`.
    pub fn view_pdf(&self, path: String) -> crate::Result<()> {
        self.0.run_mobile_plugin("viewPdf", PdfArgs { path, title: None }).map_err(Into::into)
    }

    /// Shares a report copy through the system share sheet (`no_app` as for [`Self::view_pdf`]).
    pub fn share_pdf(&self, path: String, title: String) -> crate::Result<()> {
        self.0.run_mobile_plugin("sharePdf", PdfArgs { path, title: Some(title) }).map_err(Into::into)
    }

    /// The installed versionName and versionCode (PackageManager).
    pub fn app_version(&self) -> crate::Result<AppVersion> {
        self.0.run_mobile_plugin("appVersion", ()).map_err(Into::into)
    }
}
