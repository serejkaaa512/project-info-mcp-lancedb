//! Request payloads sent to the embeddings HTTP endpoint.

use serde::Serialize;

/// Provider-specific pooling options for an embedding request.
#[derive(Serialize)]
pub struct EmbeddingParams {
    /// Pooling strategy: `"LAST"` or `"MEAN"`.
    pub pooling: String,
}

/// Request payload sent to the embeddings HTTP endpoint.
#[derive(Serialize)]
pub struct EmbeddingRequest<'a> {
    /// Text to embed.
    pub input: &'a str,
    /// Embedding model name.
    pub model: &'a str,
    /// Output encoding format (for example, `float`).
    pub encoding_format: &'a str,
    /// Optional provider-specific parameters (for example pooling).
    pub params: Option<EmbeddingParams>,
}
