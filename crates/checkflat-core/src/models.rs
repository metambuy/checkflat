use serde::{Deserialize, Serialize};

use crate::paths::RelPath;
use crate::refs::Scope;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub address: String,
    /// Project code shown by `{PROJ}` in the ref template (may be empty).
    pub code: String,
    /// Ref template (`refs::Template`), e.g. `{PROJ}-{FRAC}-{SEQ:2}`.
    pub ref_template: String,
    /// Whether sequence numbers run per project or per fraction (D-020).
    pub seq_scope: Scope,
    /// Next sequence number when `seq_scope` is `project`.
    pub next_seq: i64,
    /// `seq_scope` can no longer change: a number has been issued (`next_seq > 1` here or on any
    /// fraction), even if every observation was deleted since.
    pub scope_locked: bool,
    pub observation_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub address: String,
    pub plan_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub project_id: String,
    pub file_path: RelPath,
    pub title: String,
    pub width_pt: f64,
    pub height_pt: f64,
    pub created_at: String,
}

/// A fraction (unit) of a project, e.g. `A`, `1D`, or `PC` for common areas. Observations store
/// its `code`; `next_seq` is the counter used when numbers run per fraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fraction {
    pub id: String,
    pub project_id: String,
    pub code: String,
    pub next_seq: i64,
    pub created_at: String,
}

/// An observation. The displayed ref is computed from the project's template and never stored
/// (`refs`). Photos are in [`Photo`]; visits follow in Sprint 7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub id: String,
    pub project_id: String,
    pub plan_id: String,
    /// Fraction code (canonical case from the `fraction` table); empty when none.
    pub fraction: String,
    pub seq: i64,
    /// Full displayed ref, e.g. `LAM-A-03`.
    #[serde(rename = "ref")]
    pub display_ref: String,
    /// Short label for the pin marker: `3` (numbers per project) or `A-03` (per fraction).
    pub marker: String,
    pub x_norm: f64,
    pub y_norm: f64,
    pub description: String,
    pub created_visit_id: Option<String>,
    pub resolved_visit_id: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A stored photo: `projects/<project_id>/photos/<id>.jpg`, long side ≤ 1600 px, orientation applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: String,
    pub observation_id: String,
    pub visit_id: Option<String>,
    pub file_path: RelPath,
    /// UTC, RFC 3339 (EXIF capture time, or the import time when the file has none).
    pub taken_at: String,
    pub created_at: String,
}
