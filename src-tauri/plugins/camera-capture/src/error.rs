use serde::{
    ser::{SerializeStruct, Serializer},
    Serialize,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("only available on Android")]
    Unsupported,
    /// Desktop: no app opens this file type.
    #[error("no app to open the file")]
    NoApp,
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Error {
    /// The code a Kotlin `invoke.reject(message, code)` gave, if this error is such a rejection
    /// (e.g. `no_app` from `viewPdf`/`sharePdf`); `no_app` also for the desktop's [`Error::NoApp`].
    pub fn rejected_code(&self) -> Option<&str> {
        match self {
            Error::NoApp => Some("no_app"),
            #[cfg(mobile)]
            Error::PluginInvoke(tauri::plugin::mobile::PluginInvokeError::InvokeRejected(r)) => r.code.as_deref(),
            _ => None,
        }
    }
}

impl Error {
    /// The code the page translates (`src/lib/errors.ts`): a Kotlin rejection's own code
    /// (`no_camera_app`, `source_unreadable`, …), `io` for a file error, `internal` for the rest.
    pub fn code(&self) -> &str {
        match self {
            Error::Io(_) => "io",
            other => other.rejected_code().unwrap_or("internal"),
        }
    }
}

/// `{ code, message }`, the shape of the app's own errors, so a rejection keeps its code on the way to
/// the page (it used to arrive as a bare string, which the page could only show as "Unexpected
/// error"). `message` is the developer text.
impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("Error", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_reach_the_page_as_code_and_message() {
        let json = |e: Error| serde_json::to_value(e).unwrap();
        let io = json(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied").into());
        assert_eq!(io["code"], "io");
        assert!(io["message"].as_str().unwrap().contains("denied"));
        assert_eq!(json(Error::Unsupported), serde_json::json!({ "code": "internal", "message": "only available on Android" }));
        assert_eq!(json(Error::NoApp)["code"], "no_app");
    }
}
