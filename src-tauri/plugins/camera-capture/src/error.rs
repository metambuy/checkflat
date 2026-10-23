use serde::{ser::Serializer, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("only available on Android")]
    Unsupported,
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Error {
    /// The code a Kotlin `invoke.reject(message, code)` gave, if this error is such a rejection
    /// (e.g. `no_app` from `viewPdf`/`sharePdf`).
    pub fn rejected_code(&self) -> Option<&str> {
        match self {
            #[cfg(mobile)]
            Error::PluginInvoke(tauri::plugin::mobile::PluginInvokeError::InvokeRejected(r)) => r.code.as_deref(),
            _ => None,
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
