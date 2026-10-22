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

/// A report copy under the app cache's `reports/` (the Rust side makes it); `title` is the share
/// sheet's title where the chooser shows one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfArgs {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// The installed app version: Android's versionName and versionCode (the CI run number, D-021);
/// on desktop the package version and no build number.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppVersion {
    pub version_name: String,
    #[serde(default)]
    pub version_code: Option<i64>,
}
