//! Actor requesting text embeddings from an OpenAI-compatible HTTP endpoint
//! or locally via fastembed (when the `fastembed` feature is enabled).

mod request;
mod response;

use async_trait::async_trait;
use kameo::actor::{Actor, ActorRef};
use reqwest::Client;

use request::{EmbeddingParams, EmbeddingRequest};
use response::EmbeddingResponse;

use crate::actors::embedding::Embedder;

/// Encoding format for the embeddings request. The OpenAI-compatible endpoint expects "float" for float32 vectors.
const ENCODING_FORMAT: &str = "float";

/// Kameo actor that requests text embeddings.
///
/// When the `fastembed` feature is enabled, the actor can operate in
/// local mode using fastembed (no external service needed).
/// Otherwise it always calls the HTTP embeddings endpoint.
pub struct EmbeddingActor {
    http_client: Client,
    embeddings_url: String,
    model: String,
    pooling: Option<String>,
}

impl EmbeddingActor {
    /// Creates the actor with the given HTTP client, embeddings endpoint URL, model name, and optional pooling strategy.
    pub fn new(embeddings_url: String, model: String, pooling: Option<String>) -> Self {
        EmbeddingActor {
            http_client: Client::new(),
            embeddings_url,
            model,
            pooling,
        }
    }
}

impl Actor for EmbeddingActor {
    type Args = EmbeddingActor;

    type Error = anyhow::Error;

    /// Creates the actor from the arguments provided at spawn time.
    async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        Ok(args)
    }
}

#[async_trait]
impl Embedder for EmbeddingActor {
    async fn embed(&self, query: &str) -> Result<Vec<f32>, String> {
        let response = self
            .http_client
            .post(&self.embeddings_url)
            .json(&EmbeddingRequest {
                input: query,
                model: &self.model,
                encoding_format: ENCODING_FORMAT,
                params: self.pooling.as_ref().map(|p| EmbeddingParams {
                    pooling: p.to_string(),
                }),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let parsed: EmbeddingResponse = response.json().await.map_err(|e| e.to_string())?;

        if let Some(d) = parsed.data.first() {
            Ok(d.embedding.clone())
        } else {
            Err("Model returned empty vectors array".to_string())
        }
    }
}
