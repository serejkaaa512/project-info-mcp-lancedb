//! Kameo actors and the messages exchanged between them.

pub mod embedding;
pub mod project_info;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Message requesting insertion or replacement of a project info record.
#[derive(Serialize, Deserialize)]
pub struct UpsertMessage {
    /// Unique record key (for example, file path or task ID).
    pub id: String,
    /// Content text to store.
    pub content: String,
    /// Record category (for example, `file`, `todo`, `architecture`).
    pub category: String,
    /// Project scope; empty string means the actor's default project.
    #[serde(default)]
    pub project: String,
}

/// Message requesting a hybrid vector + full-text search over project info.
#[derive(Serialize, Deserialize)]
pub struct SearchMessage {
    /// Search query text.
    pub query: String,
    /// Optional record category filter.
    pub category: Option<String>,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Project scope; empty string means the actor's default project.
    /// Use `"*"` to search across all projects.
    #[serde(default)]
    pub project: String,
}

/// Message requesting an embedding vector for the given query text.
#[derive(Serialize, Deserialize)]
pub struct EmbeddingMessage {
    /// Text to embed.
    pub query: String,
}

/// Message requesting compaction / optimization of the LanceDB table.
#[derive(Serialize, Deserialize)]
pub struct OptimizeMessage {}

/// Message requesting usage statistics about the project memory table.
#[derive(Serialize, Deserialize)]
pub struct StatsMessage {
    /// Project scope; empty string means the actor's default project.
    /// Use `"*"` to aggregate across all projects.
    #[serde(default)]
    pub project: String,
}

/// Single stored point, JSON-serializable for the REST API / dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointRecord {
    /// Unique record key.
    pub id: String,
    /// Project scope.
    pub project: String,
    /// Stored content text.
    pub content: String,
    /// Record category.
    pub category: String,
    /// SHA-256 of content.
    pub file_hash: String,
    /// Unix timestamp of last write.
    pub timestamp: i64,
}

/// Single structured search hit for the REST API / dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Unique record key.
    pub id: String,
    /// Project scope.
    pub project: String,
    /// Record category.
    pub category: String,
    /// Stored content text.
    pub content: String,
    /// Vector distance (lower is closer); `None` when unavailable.
    pub distance: Option<f32>,
}

/// Structured collection statistics for the REST API / dashboard.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsData {
    /// Total record count.
    pub total: usize,
    /// Records scanned for the aggregation.
    pub scanned: usize,
    /// Total content characters.
    pub content_chars: usize,
    /// Average content characters per record.
    pub avg_chars: usize,
    /// Per-category counts.
    pub per_category: BTreeMap<String, usize>,
    /// Per-project counts (populated when scope is `*`).
    pub per_project: BTreeMap<String, usize>,
}

/// Message listing stored points with optional
/// filters (REST API / dashboard).
#[derive(Serialize, Deserialize)]
pub struct ListPointsMessage {
    /// Project scope; empty means default, `"*"` means all projects.
    #[serde(default)]
    pub project: String,
    /// Optional category filter.
    pub category: Option<String>,
    /// Optional substring filter matched against `id` + `content`.
    pub query: Option<String>,
    /// Maximum rows to return.
    pub limit: usize,
    /// Rows to skip (offset pagination).
    pub offset: usize,
}

/// Message deleting a single point by `(id, project)`.
#[derive(Serialize, Deserialize)]
pub struct DeletePointMessage {
    /// Unique record key.
    pub id: String,
    /// Project scope; empty means default.
    #[serde(default)]
    pub project: String,
}

/// Message requesting structured stats (REST API / dashboard).
#[derive(Serialize, Deserialize)]
pub struct StatsStructuredMessage {
    /// Project scope; empty means default, `"*"`
    /// aggregates all projects.
    #[serde(default)]
    pub project: String,
}

/// Message requesting a structured hybrid search (REST API / dashboard).
#[derive(Serialize, Deserialize)]
pub struct StructuredSearchMessage {
    /// Search query text.
    pub query: String,
    /// Optional record category filter.
    pub category: Option<String>,
    /// Maximum number of results to return.
    pub limit: usize,
    /// Project scope; empty means default, `"*"` searches all projects.
    #[serde(default)]
    pub project: String,
}

/// Message asking the actor to re-open its LanceDB
/// table (used after snapshot restore).
#[derive(Serialize, Deserialize)]
pub struct ReopenMessage {}
