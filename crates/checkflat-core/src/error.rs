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
    #[error("plan has {count} observation(s); delete them first")]
    PlanHasObservations { count: i64 },
    #[error("invalid ref template: {0}")]
    InvalidTemplate(TemplateProblem),
    #[error("invalid code: {0}")]
    InvalidCode(CodeProblem),
    #[error("the sequence scope cannot change once a number has been issued")]
    ScopeLocked,
    #[error("fraction has issued numbers ({count} observation(s) use it); it cannot be deleted")]
    FractionInUse { count: i64 },
    #[error("a fraction is required when numbers run per fraction")]
    FractionRequired,
    #[error("an observation needs at least one photo")]
    PhotoRequired,
    #[error("an observation can have at most {max} photos")]
    PhotoLimit { max: usize },
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
            CoreError::InvalidTemplate(p) => p.code(),
            CoreError::InvalidCode(p) => p.code(),
            CoreError::ScopeLocked => "scope_locked",
            CoreError::FractionInUse { .. } => "fraction_in_use",
            CoreError::FractionRequired => "fraction_required",
            CoreError::PhotoRequired => "photo_required",
            CoreError::PhotoLimit { .. } => "photo_limit",
            CoreError::UnreadableImage(_) => "unreadable_image",
            CoreError::UnsupportedImage => "image_unsupported",
        }
    }
}

/// What is wrong with a project code or a fraction code (`refs::validate_code`). Each kind has its own
/// code, so the UI shows the rule that was broken in the user's language instead of the core's text.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CodeProblem {
    #[error("longer than {max} characters")]
    TooLong { max: usize },
    #[error("contains whitespace, braces or a control character")]
    InvalidChar,
}

impl CodeProblem {
    pub fn code(&self) -> &'static str {
        match self {
            CodeProblem::TooLong { .. } => "code_too_long",
            CodeProblem::InvalidChar => "code_invalid_char",
        }
    }
}

/// What is wrong with a ref template (`refs::Template::parse` / `validate_for`); one code each.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemplateProblem {
    #[error("template is empty")]
    Empty,
    #[error("template is longer than {max} characters")]
    TooLong { max: usize },
    #[error("unexpected '}}'")]
    StrayClose,
    #[error("unclosed '{{'")]
    Unclosed,
    #[error("SEQ width must be a number")]
    SeqWidthNotNumber,
    #[error("SEQ width must be between 1 and 6")]
    SeqWidthRange,
    #[error("unknown token {{{token}}}")]
    UnknownToken { token: String },
    #[error("template needs exactly one {{SEQ:n}}")]
    NeedsOneSeq,
    #[error("{{PROJ}} and {{FRAC}} may appear only once")]
    RepeatedToken,
    #[error("{{FRAC}} is required when numbers run per fraction")]
    FracRequired,
}

impl TemplateProblem {
    pub fn code(&self) -> &'static str {
        match self {
            TemplateProblem::Empty => "template_empty",
            TemplateProblem::TooLong { .. } => "template_too_long",
            TemplateProblem::StrayClose => "template_stray_close",
            TemplateProblem::Unclosed => "template_unclosed",
            TemplateProblem::SeqWidthNotNumber => "template_seq_width_nan",
            TemplateProblem::SeqWidthRange => "template_seq_width_range",
            TemplateProblem::UnknownToken { .. } => "template_unknown_token",
            TemplateProblem::NeedsOneSeq => "template_needs_one_seq",
            TemplateProblem::RepeatedToken => "template_repeated_token",
            TemplateProblem::FracRequired => "template_frac_required",
        }
    }
}
