//! Backend-agnostic storage trait shared by the LanceDB and Qdrant servers.

use async_trait::async_trait;

use crate::actors::{PointRecord, SearchHit, StatsData};

/// Record payload passed to [`MemoryStore::write_record`].
#[derive(Debug, Clone, PartialEq)]
pub struct Record<'a> {
    /// Unique record key.
    pub id: &'a str,
    /// Project scope.
    pub project: &'a str,
    /// Content text to store.
    pub content: &'a str,
    /// Record category.
    pub category: &'a str,
    /// SHA-256 of the content.
    pub file_hash: &'a str,
    /// Unix timestamp of the write.
    pub timestamp: i64,
}

/// Storage operations required by the `ProjectInfoActor`.
///
/// Each server binary implements this trait for its backend; the actor, the
/// MCP tool dispatch, and the message types live in this crate and stay
/// backend-agnostic.
#[async_trait]
pub trait MemoryStore: Send + Sync + 'static {
    /// Prepares an upsert for `(id, project)`.
    ///
    /// Returns `false` when an identical record (matching `file_hash`) is
    /// already stored, so the caller can skip embedding and writing. Backends
    /// may also remove the stale row they are about to replace.
    ///
    /// Errors during the check fall through to `true` (write proceeds), which
    /// matches the pre-refactor behavior of both backends.
    async fn prepare_upsert(&self, id: &str, project: &str, file_hash: &str) -> bool;

    /// Writes the record, replacing any prior row per backend semantics.
    async fn write_record(&self, record: Record<'_>, vector: Vec<f32>) -> Result<(), String>;

    /// Runs the text-search tool query and returns hits in backend order.
    async fn search_text(
        &self,
        query_vector: Vec<f32>,
        project: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>, String>;

    /// Runs the structured (REST API / dashboard) search and returns hits.
    async fn search_structured(
        &self,
        query_vector: Vec<f32>,
        project: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>, String>;

    /// Counts records for the `(project, category)`
    /// scope, ignoring any text query.
    async fn count(&self, project: &str, category: Option<&str>) -> Result<usize, String>;

    /// Lists all records for the `(project, category)` scope, before the
    /// caller applies substring filtering and pagination. `limit` is passed
    /// through for backends that page while scanning.
    async fn list(
        &self,
        project: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PointRecord>, String>;

    /// Aggregates statistics for the project scope. An empty scope returns
    /// [`StatsData::default()`] (i.e. `total == 0`) without scanning.
    async fn stats(&self, project: &str) -> Result<StatsData, String>;

    /// Deletes the record matching `(id, project)`; `true` when one existed.
    async fn delete(&self, id: &str, project: &str) -> Result<bool, String>;

    /// Compacts / optimizes storage; returns the backend confirmation message.
    async fn optimize(&self) -> Result<String, String>;

    /// Re-opens the storage after a snapshot restore; returns the backend
    /// confirmation message.
    async fn reopen(&self) -> Result<String, String>;

    /// Noun used in the empty-stats reply (`"table"` / `"collection"`).
    fn stats_empty_noun(&self) -> &'static str;
}
