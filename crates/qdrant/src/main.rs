//! `piim`: MCP server exposing persistent project memory over Qdrant with vector search.
//!
//! Wires up the embeddings and project-info actors, then serves the memory tools over stdio.

mod config;
mod helpers;
mod store;

use std::collections::BTreeMap;

use kameo::actor::Spawn;
use qdrant_client::Qdrant;
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::mcp_server::{McpServerOptions, server_runtime};
use rust_mcp_sdk::schema::{Implementation, ServerCapabilities, ServerCapabilitiesTools};
use rust_mcp_sdk::{
    McpServer, ServerDetails, StdioTransport, ToMcpServerHandler, TransportOptions,
};

use piim_common::actors::embedding::config::EmbeddingBackend;
use piim_common::actors::embedding::{self, fastembed, http};
use piim_common::actors::project_info::ProjectInfoActor;
use piim_common::mcp::MemoryToolHandler;

use crate::config::Config;
use crate::helpers::ensure_collection;

/// Main entry point for the `piim` MCP server.
#[tokio::main]
async fn main() -> SdkResult<()> {
    let config = Config::get_from_env();

    // Initialize Qdrant client
    let mut client_builder = Qdrant::from_url(&config.qdrant_url);
    if let Some(ref api_key) = config.qdrant_api_key {
        client_builder = client_builder.api_key(api_key.clone());
    }
    let client =
        client_builder
            .build()
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?;

    // Ensure the collection exists with proper vector config
    ensure_collection(
        &client,
        &config.collection_name,
        &config.model,
        config.vector_dimension,
    )
    .await
    .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
        description: e.to_string(),
    })?;

    // Create the appropriate embedding actor based on the selected backend.
    let embed_actor = match &config.embedding_backend {
        EmbeddingBackend::Http => Box::new(http::EmbeddingActor::new(
            config.embeddings_url.clone(),
            config.model.clone(),
            None,
        )) as Box<dyn embedding::Embedder>,
        EmbeddingBackend::Fastembed(model) => {
            let actor = fastembed::EmbeddingActor::new_fastembed(model.clone()).map_err(|e| {
                rust_mcp_sdk::error::McpSdkError::Internal {
                    description: format!("Failed to initialize local embedding model: {e}"),
                }
            })?;

            Box::new(actor) as Box<dyn embedding::Embedder>
        }
    };

    let store = store::QdrantStore::new(client, config.collection_name, config.model.clone());

    let memory_actor_ref = ProjectInfoActor::spawn(ProjectInfoActor::new(
        Box::new(store),
        embed_actor,
        config.project,
    ));

    let server_details = ServerDetails {
        server_info: Implementation {
            name: "piim".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("Project Info in MCP".into()),
            description: Some("Persistent project memory over Qdrant with vector search".into()),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools {
                list_changed: Some(true),
            }),
            experimental: Some({
                let mut exp = BTreeMap::new();
                exp.insert(
                    "customRequests".to_string(),
                    rust_mcp_sdk::schema::JsonObject(BTreeMap::new()),
                );
                exp
            }),
            ..Default::default()
        },
        instructions: None,
        meta: None,
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler =
        MemoryToolHandler::new(memory_actor_ref, env!("CARGO_PKG_VERSION")).to_mcp_server_handler();
    let server = server_runtime::create_server(McpServerOptions {
        transport,
        handler,
        server_details,
        message_observer: None,
    });
    server.start().await
}
