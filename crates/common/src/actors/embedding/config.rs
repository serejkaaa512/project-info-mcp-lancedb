use std::env;

use crate::config::CommonConfig;

/// Embedding backend selection.
#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingBackend {
    /// Use an external OpenAI-compatible HTTP endpoint.
    Http,
    /// Use local inference via fastembed.
    Fastembed(super::fastembed::FastembedModel),
}

impl EmbeddingBackend {
    /// Parse backend from `EMBEDDINGS_BACKEND` env var.
    ///
    /// Supports: `http` (default), `fastembed`.
    /// When `fastembed`, the model is resolved from `EMBEDDINGS_MODEL`.
    pub fn from_env() -> Self {
        match env::var("EMBEDDINGS_BACKEND").ok().as_deref() {
            Some("fastembed") => {
                let model_name =
                    env::var("EMBEDDINGS_MODEL").unwrap_or_else(|_| "qwen3-embed".to_string());
                Self::Fastembed(super::fastembed::FastembedModel::from_http_name(
                    &model_name,
                ))
            }
            Some(_) | None => Self::Http,
        }
    }

    /// Return the vector dimension for this backend.
    pub fn dimension(&self) -> usize {
        match self {
            Self::Http => {
                // For HTTP we fall back to the common config value
                CommonConfig::from_env().vector_dimension
            }
            Self::Fastembed(model) => model.dimension(),
        }
    }
}
