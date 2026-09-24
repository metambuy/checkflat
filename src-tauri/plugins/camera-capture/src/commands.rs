use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::CameraCaptureExt;
use crate::Result;

/// Async so the blocking mobile call runs off the main thread.
#[command]
pub(crate) async fn capture<R: Runtime>(app: AppHandle<R>) -> Result<CaptureResponse> {
    app.camera_capture().capture()
}
