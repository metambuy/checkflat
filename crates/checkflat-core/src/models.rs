use serde::{Deserialize, Serialize};

use crate::paths::RelPath;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub address: String,
    pub logo_path: Option<RelPath>,
    /// Next observation ref number, continuous per project (pending client confirmation).
    pub next_ref_no: i64,
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

/// An observation. In Sprint 2 only the pin part is edited (position, ref number); description,
/// photos and visits follow in Sprints 3 and 7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub id: String,
    pub project_id: String,
    pub plan_id: String,
    pub ref_no: i64,
    pub x_norm: f64,
    pub y_norm: f64,
    pub description: String,
    pub created_visit_id: Option<String>,
    pub resolved_visit_id: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
