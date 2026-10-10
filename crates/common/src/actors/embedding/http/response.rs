//! Response payloads returned by the embeddings HTTP endpoint.

use serde::Deserialize;

/// Single embedding entry in the embeddings endpoint response.
#[derive(Deserialize)]
pub struct Data {
    /// Embedding vector for the input.
    pub embedding: Vec<f32>,
}
/// Response payload returned by the embeddings HTTP endpoint.
#[derive(Deserialize)]
pub struct EmbeddingResponse {
    /// Embedding entries returned by the endpoint; the first entry is used.
    pub data: Vec<Data>,
}
