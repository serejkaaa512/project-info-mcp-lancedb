//! `piim-lance`: MCP server exposing persistent project memory over LanceDB with hybrid search.
//!
//! Wires up the embeddings and project-info actors, then serves the memory tools over stdio.

mod config;
mod helpers;
mod http;
mod store;

use std::collections::BTreeMap;

use kameo::actor::Spawn;
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::mcp_server::{McpServerOptions, server_runtime};
use rust_mcp_sdk::schema::{Implementation, ServerCapabilities, ServerCapabilitiesTools};
use rust_mcp_sdk::{
    McpServer, ServerDetails, StdioTransport, ToMcpServerHandler, TransportOptions,
};

use piim_common::actors;
use piim_common::actors::embedding;
use piim_common::actors::embedding::config::EmbeddingBackend;
use piim_common::actors::embedding::{fastembed, http as embedding_http};
use piim_common::actors::project_info::ProjectInfoActor;
use piim_common::mcp::MemoryToolHandler;

use crate::config::Config;

/// Main entry point for the `piim-lance` MCP server.
#[tokio::main]
async fn main() -> SdkResult<()> {
    let config = Config::get_from_env();
    let store = store::LanceStore::new(config.db_dir.clone(), config.vector_dimension)
        .await
        .map_err(|description| rust_mcp_sdk::error::McpSdkError::Internal { description })?;

    // Create the appropriate embedding actor based on the selected backend.
    let embed_actor = match &config.embedding_backend {
        EmbeddingBackend::Http => Box::new(embedding_http::EmbeddingActor::new(
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

    let memory_actor_ref = ProjectInfoActor::spawn(ProjectInfoActor::new(
        Box::new(store),
        embed_actor,
        config.project.clone(),
    ));

    // Qdrant-style HTTP API + dashboard on :HTTP_PORT (runs alongside MCP stdio).
    if config.http_port != 0 {
        let state = http::AppState {
            actor: memory_actor_ref.clone(),
            db_dir: config.db_dir.clone(),
            snapshot_dir: config.snapshot_dir.clone(),
            default_project: config.project.clone(),
            model: config.model.clone(),
            vector_dimension: config.vector_dimension,
            started_unix: chrono::Utc::now().timestamp(),
        };
        let _ = http::snapshots::ensure_dir(&config.snapshot_dir);
        let app = http::router(state);
        let port = config.http_port;
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await;
            match listener {
                Ok(l) => {
                    eprintln!("piim-lance dashboard: http://localhost:{port}/dashboard");
                    if let Err(e) = axum::serve(l, app).await {
                        eprintln!("HTTP server error: {e}");
                    }
                }
                Err(e) => eprintln!("HTTP server bind error on port {port}: {e}"),
            }
        });
    }

    let server_details = ServerDetails {
        server_info: Implementation {
            name: "piim".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("Project Info in MCP".into()),
            description: Some("Persistent project memory over LanceDB with hybrid search".into()),
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
