//! Error shape sent to the WebView: a stable `code` the UI translates, plus a developer message.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        AppError { code: code.into(), message: message.into(), pages: None, count: None }
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        AppError::new("internal", e.to_string())
    }
}

impl From<checkflat_core::CoreError> for AppError {
    fn from(e: checkflat_core::CoreError) -> Self {
        use checkflat_core::CoreError as C;
        let mut out = AppError::new(e.code(), e.to_string());
        match &e {
            C::MultiPage { pages } => out.pages = Some(*pages),
            C::PlanHasObservations { count } => out.count = Some(*count),
            _ => {}
        }
        out
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("io", e.to_string())
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
