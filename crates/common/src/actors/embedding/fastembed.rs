use std::sync::Mutex;

use crate::actors::embedding;
use async_trait::async_trait;
use kameo::actor::{Actor, ActorRef};

/// Model name used in the HTTP config that maps to a local fastembed model.
#[derive(Debug, Clone, PartialEq)]
pub enum FastembedModel {
    /// BAAI/bge-small-en-v1.5 — 130M params, 384 dims
    /// (default fastembed model).
    BgeSmallEnV15,
    /// BAAI/bge-base-en-v1.5 — 110M params, 768 dims.
    BgeBaseEnV15,
    /// BAAI/bge-large-en-v1.5 — 335M params, 1024 dims.
    BgeLargeEnV15,
    /// nomic-ai/nomic-embed-text-v1 — 27M params, 768 dims.
    NomicEmbedTextV1,
    /// nomic-ai/nomic-embed-text-v1.5 — 27M params, 768 dims.
    NomicEmbedTextV15,
    /// Qwen/Qwen3-Embedding-0.6B — requires `qwen3` feature.
    Qwen3Embedding0_6B,
    /// Qwen/Qwen3-Embedding-4B — requires `qwen3` feature.
    Qwen3Embedding4B,
    /// Qwen/Qwen3-Embedding-8B — requires `qwen3` feature.
    Qwen3Embedding8B,
    /// Sentence-transformers all-MiniLM-L6-v2 — 22M params, 384 dims.
    AllMiniLML6V2,
}

impl FastembedModel {
    /// Dimensionality of embeddings produced by this model.
    pub const fn dimension(&self) -> usize {
        match self {
            Self::BgeSmallEnV15 => 384,
            Self::BgeBaseEnV15 => 768,
            Self::BgeLargeEnV15 => 1024,
            Self::NomicEmbedTextV1 => 768,
            Self::NomicEmbedTextV15 => 768,
            Self::Qwen3Embedding0_6B => 1024,
            Self::Qwen3Embedding4B => 4096,
            Self::Qwen3Embedding8B => 4096,
            Self::AllMiniLML6V2 => 384,
        }
    }

    /// Map an HTTP model name (from `EMBEDDINGS_MODEL`) to a local model.
    ///
    /// Unknown names fall back to [`FastembedModel::BgeSmallEnV15`].
    pub fn from_http_name(name: &str) -> Self {
        match name {
            "qwen3-embed" | "qwen3-embedding-0.6b" => Self::Qwen3Embedding0_6B,
            "qwen3-embedding-4b" => Self::Qwen3Embedding4B,
            "qwen3-embedding-8b" => Self::Qwen3Embedding8B,
            "bge-small-en" | "bge-small-en-v1.5" => Self::BgeSmallEnV15,
            "bge-base-en" | "bge-base-en-v1.5" => Self::BgeBaseEnV15,
            "bge-large-en" | "bge-large-en-v1.5" => Self::BgeLargeEnV15,
            "nomic-embed-text" | "nomic-embed-text-v1" => Self::NomicEmbedTextV1,
            "nomic-embed-text-v1.5" => Self::NomicEmbedTextV15,
            "all-MiniLM-L6-v2" | "minilm-l6" => Self::AllMiniLML6V2,
            _ => Self::BgeSmallEnV15,
        }
    }
}

/// Backend for local embedding inference.
pub enum FastembedBackend {
    /// ONNX-based models (BGE, MiniLM, etc.) via [`fastembed::TextEmbedding`].
    Onnx(Mutex<fastembed::TextEmbedding>),
    /// Candle-based Qwen3 models.
    Qwen3(Mutex<fastembed::Qwen3TextEmbedding>),
}

impl FastembedBackend {
    /// Create a new backend from the chosen model.
    ///
    /// Downloads the model on first call if not yet cached.
    pub fn new(model: FastembedModel) -> anyhow::Result<Self> {
        Ok(match model {
            FastembedModel::Qwen3Embedding0_6B => {
                let device = candle_core::Device::Cpu;
                let m = fastembed::Qwen3TextEmbedding::from_hf(
                    "Qwen/Qwen3-Embedding-0.6B",
                    &device,
                    candle_core::DType::F32,
                    512,
                )?;
                Self::Qwen3(std::sync::Mutex::new(m))
            }
            FastembedModel::Qwen3Embedding4B => {
                let device = candle_core::Device::Cpu;
                let m = fastembed::Qwen3TextEmbedding::from_hf(
                    "Qwen/Qwen3-Embedding-4B",
                    &device,
                    candle_core::DType::F32,
                    512,
                )?;
                Self::Qwen3(std::sync::Mutex::new(m))
            }
            FastembedModel::Qwen3Embedding8B => {
                let device = candle_core::Device::Cpu;
                let m = fastembed::Qwen3TextEmbedding::from_hf(
                    "Qwen/Qwen3-Embedding-8B",
                    &device,
                    candle_core::DType::F32,
                    512,
                )?;
                Self::Qwen3(std::sync::Mutex::new(m))
            }
            _ => {
                let fe_model = match model {
                    FastembedModel::BgeSmallEnV15 => fastembed::EmbeddingModel::BGESmallENV15,
                    FastembedModel::BgeBaseEnV15 => fastembed::EmbeddingModel::BGEBaseENV15,
                    FastembedModel::BgeLargeEnV15 => fastembed::EmbeddingModel::BGELargeENV15,
                    FastembedModel::NomicEmbedTextV1 => fastembed::EmbeddingModel::NomicEmbedTextV1,
                    FastembedModel::NomicEmbedTextV15 => {
                        fastembed::EmbeddingModel::NomicEmbedTextV15
                    }
                    FastembedModel::AllMiniLML6V2 => fastembed::EmbeddingModel::AllMiniLML6V2,
                    FastembedModel::Qwen3Embedding0_6B
                    | FastembedModel::Qwen3Embedding4B
                    | FastembedModel::Qwen3Embedding8B => {
                        return Err(anyhow::anyhow!(
                            "Qwen3 models require explicit Qwen3 backend path"
                        ));
                    }
                };
                let m =
                    fastembed::TextEmbedding::try_new(fastembed::TextInitOptions::new(fe_model))?;
                Self::Onnx(std::sync::Mutex::new(m))
            }
        })
    }

    /// Generate embeddings for the given texts.
    pub fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        match self {
            Self::Onnx(lock) => {
                let mut m = lock
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                let strings: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
                let strings_ref: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
                let results = m.embed(strings_ref, None)?;
                Ok(results.into_iter().map(|e| e.to_vec()).collect())
            }
            Self::Qwen3(lock) => {
                let m = lock
                    .lock()
                    .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
                let results = m.embed(texts)?;
                Ok(results)
            }
        }
    }
}

/// Kameo actor that requests text embeddings.
///
/// When the `fastembed` feature is enabled, the actor can operate in
/// local mode using fastembed (no external service needed).
/// Otherwise it always calls the HTTP embeddings endpoint.
pub struct EmbeddingActor {
    backend: FastembedBackend,
}

impl EmbeddingActor {
    /// Creates the actor for local embedding inference using fastembed.
    ///
    /// The model weights are downloaded on first call if not yet cached.
    pub fn new_fastembed(model: FastembedModel) -> anyhow::Result<Self> {
        Ok(EmbeddingActor {
            backend: FastembedBackend::new(model)?,
        })
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
impl embedding::Embedder for EmbeddingActor {
    async fn embed(&self, query: &str) -> Result<Vec<f32>, String> {
        let texts: Vec<&str> = vec![query];

        self.backend
            .embed(&texts)
            .map(|mut v| v.pop().unwrap_or_default())
            .map_err(|e| e.to_string())
    }
}
