pub mod config;
pub mod fastembed;
pub mod http;

use async_trait::async_trait;

#[async_trait]
pub trait Embedder: Send + Sync + 'static {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}
