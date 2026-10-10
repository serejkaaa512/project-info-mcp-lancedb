//! Runtime configuration loaded from environment variables.

use std::env;

use piim_common::{actors::embedding::config::EmbeddingBackend, config::CommonConfig};

/// Runtime configuration loaded from environment
/// variables, with fallback defaults.
pub struct Config {
    /// Path to the LanceDB database directory (`LANCEDB_PATH`).
    pub db_dir: String,
    /// URL of the OpenAI-compatible embeddings endpoint (`EMBEDDINGS_URL`).
    pub embeddings_url: String,
    /// Embedding model name (`EMBEDDINGS_MODEL`).
    pub model: String,
    /// Expected embedding vector dimension (`VECTOR_DIMENSION`).
    pub vector_dimension: usize,
    /// Embedding backend selection.
    pub embedding_backend: EmbeddingBackend,
    /// Default project name used to scope records (`PROJECT_NAME`).
    ///
    /// Multiple projects can share one LanceDB database: every record carries
    /// a `project` column and all reads/writes are filtered by it. The value
    /// can be overridden per tool call via the optional `project` argument.
    pub project: String,
    /// HTTP port for the REST API + dashboard
    /// (`HTTP_PORT`, default `6333` Qdrant-style).
    /// Set to `0` or empty to disable the HTTP server (stdio MCP only).
    pub http_port: u16,
    /// Directory where `.tar.gz` DB snapshots are stored (`SNAPSHOT_DIR`).
    pub snapshot_dir: String,
}

impl Config {
    /// Loads configuration from environment variables,
    /// falling back to defaults when unset or invalid.
    pub fn get_from_env() -> Config {
        let db_dir =
            env::var("LANCEDB_PATH").unwrap_or_else(|_| "./.opencode_memory/lance_db".to_string());

        let common = CommonConfig::from_env();

        let embedding_backend = EmbeddingBackend::from_env();
        let vector_dimension = embedding_backend.dimension();

        Config {
            db_dir,
            embeddings_url: common.embeddings_url,
            model: common.model,
            vector_dimension,
            embedding_backend,
            project: common.project,
            http_port: env::var("HTTP_PORT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(6333),
            snapshot_dir: env::var("SNAPSHOT_DIR")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "./.opencode_memory/snapshots".to_string()),
        }
    }
}
