//! Runtime configuration shared by all PIIM server backends.

use std::env;

/// Backend-independent configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct CommonConfig {
    /// URL of the OpenAI-compatible embeddings endpoint (`EMBEDDINGS_URL`).
    pub embeddings_url: String,
    /// Embedding model name (`EMBEDDINGS_MODEL`).
    pub model: String,
    /// Expected embedding vector dimension (`VECTOR_DIMENSION`).
    pub vector_dimension: usize,
    /// Default project name used to scope records (`PROJECT_NAME`).
    ///
    /// Multiple projects can share one database: every record carries a
    /// `project` key and all reads/writes are filtered by it. The value can be
    /// overridden per tool call via the optional `project` argument.
    pub project: String,
}

impl CommonConfig {
    /// Loads the shared configuration from environment
    /// variables, falling back to defaults when unset
    /// or invalid.
    pub fn from_env() -> CommonConfig {
        let embeddings_url = env::var("EMBEDDINGS_URL")
            .unwrap_or_else(|_| "http://localhost:8002/v1/embeddings".to_string());

        let model = env::var("EMBEDDINGS_MODEL").unwrap_or_else(|_| "qwen3-embed".to_string());

        let vector_dimension = env::var("VECTOR_DIMENSION")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1024);

        let project = env::var("PROJECT_NAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| crate::DEFAULT_PROJECT.to_string());

        CommonConfig {
            embeddings_url,
            model,
            vector_dimension,
            project,
        }
    }
}
