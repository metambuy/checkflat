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
    /// The offending token of an unknown-token template error (`template_unknown_token`), for the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        AppError { code: code.into(), message: message.into(), pages: None, count: None, detail: None }
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        AppError::new("internal", e.to_string())
    }
}

impl From<checkflat_core::CoreError> for AppError {
    fn from(e: checkflat_core::CoreError) -> Self {
        use checkflat_core::error::{CodeProblem as K, TemplateProblem as T};
        use checkflat_core::CoreError as C;
        let mut out = AppError::new(e.code(), e.to_string());
        match &e {
            C::MultiPage { pages } => out.pages = Some(*pages),
            C::PlanHasObservations { count } | C::FractionInUse { count } => out.count = Some(*count),
            C::PhotoLimit { max } | C::InvalidCode(K::TooLong { max }) | C::InvalidTemplate(T::TooLong { max }) => out.count = Some(*max as i64),
            C::InvalidTemplate(T::UnknownToken { token }) => out.detail = Some(token.clone()),
            _ => {}
        }
        out
    }
}

impl From<checkflat_report::ReportError> for AppError {
    fn from(e: checkflat_report::ReportError) -> Self {
        match e {
            checkflat_report::ReportError::Core(c) => c.into(),
            other => AppError::new("report", other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("io", e.to_string())
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use checkflat_core::error::{CodeProblem, TemplateProblem};
    use checkflat_core::CoreError;

    #[test]
    fn code_and_template_problems_reach_the_ui_as_codes_with_what_the_message_needs() {
        let json = |e: CoreError| serde_json::to_value(AppError::from(e)).unwrap();
        let bad = json(CoreError::InvalidCode(CodeProblem::InvalidChar));
        assert_eq!(bad["code"], "code_invalid_char");
        let long = json(CoreError::InvalidCode(CodeProblem::TooLong { max: 16 }));
        assert_eq!((long["code"].as_str(), long["count"].as_i64()), (Some("code_too_long"), Some(16)));
        let token = json(CoreError::InvalidTemplate(TemplateProblem::UnknownToken { token: "NOPE".into() }));
        assert_eq!((token["code"].as_str(), token["detail"].as_str()), (Some("template_unknown_token"), Some("NOPE")));
        let frac = json(CoreError::InvalidTemplate(TemplateProblem::FracRequired));
        assert_eq!(frac["code"], "template_frac_required");
        assert!(frac.get("detail").is_none() && frac.get("count").is_none(), "nothing extra is sent");
        // The core's English text is still there for logs; the UI picks its message by code.
        assert_eq!(token["message"], "invalid ref template: unknown token {NOPE}");
    }
}
