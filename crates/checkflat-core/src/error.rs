use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid relative path: {0}")]
    InvalidPath(String),
    #[error("not found")]
    NotFound,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("PDF has {pages} pages; one plan per file is required")]
    MultiPage { pages: usize },
    #[error("file is not a readable PDF: {0}")]
    Unreadable(String),
    #[error("plan has {count} observation(s); delete or move them first")]
    PlanHasObservations { count: i64 },
    #[error("invalid ref template: {0}")]
    InvalidTemplate(String),
    #[error("the sequence scope cannot change once a number has been issued")]
    ScopeLocked,
    #[error("fraction has issued numbers ({count} observation(s) use it); it cannot be deleted")]
    FractionInUse { count: i64 },
    #[error("a fraction is required when numbers run per fraction")]
    FractionRequired,
    #[error("an observation needs at least one photo")]
    PhotoRequired,
    #[error("file is not a readable image: {0}")]
    UnreadableImage(String),
    #[error("this image format (HEIC/HEIF) cannot be read on this device")]
    UnsupportedImage,
}

impl CoreError {
    /// Stable machine-readable code the UI maps to a translated message.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Db(_) => "db",
            CoreError::Migration(_) => "migration",
            CoreError::Io(_) => "io",
            CoreError::InvalidPath(_) => "invalid_path",
            CoreError::NotFound => "not_found",
            CoreError::Validation(_) => "validation",
            CoreError::MultiPage { .. } => "multi_page",
            CoreError::Unreadable(_) => "unreadable_pdf",
            CoreError::PlanHasObservations { .. } => "plan_has_observations",
            CoreError::InvalidTemplate(_) => "invalid_template",
            CoreError::ScopeLocked => "scope_locked",
            CoreError::FractionInUse { .. } => "fraction_in_use",
            CoreError::FractionRequired => "fraction_required",
            CoreError::PhotoRequired => "photo_required",
            CoreError::UnreadableImage(_) => "unreadable_image",
            CoreError::UnsupportedImage => "image_unsupported",
        }
    }
}
