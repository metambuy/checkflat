use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    /// Absolute path of the JPEG written by the camera app (in the app cache dir).
    pub path: String,
}
