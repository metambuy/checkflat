use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    /// Absolute path of the JPEG written by the camera app (in the app cache dir); `None` when
    /// the user cancelled.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickResponse {
    /// Absolute path of the picked image in the app cache dir (HEIC already converted to JPEG);
    /// `None` when the user cancelled.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayNameArgs {
    pub uri: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayNameResponse {
    /// Sanitized display name, or None if the provider does not expose one.
    #[serde(default)]
    pub name: Option<String>,
}
