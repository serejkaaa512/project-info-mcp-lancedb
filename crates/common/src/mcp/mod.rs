//! MCP server plumbing: tool registry and request dispatch.
#![allow(clippy::enum_variant_names)]

pub mod optimize;
pub mod save_file;
pub mod save_function;
pub mod search;
pub mod stats;
pub mod upsert;

use async_trait::async_trait;

use kameo::actor::ActorRef;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, CompleteRequestParams, CompleteResult,
    CompleteResultCompletion, GenericResult, GetPromptRequestParams, ListPromptsResult,
    ListPromptsResultCacheScope, ListResourceTemplatesResult,
    ListResourceTemplatesResultCacheScope, ListResourcesResult, ListResourcesResultCacheScope,
    ListToolsResult, ListToolsResultCacheScope, PaginatedRequestParams, ReadResourceRequestParams,
    RpcError, ServerResult, TextContent,
    schema_utils::{CallToolError, CustomRequest},
};
use rust_mcp_sdk::{McpServer, RequestContext, tool_box};

use crate::actors::project_info::ProjectInfoActor;
use crate::mcp::optimize::OptimizeProjectInfo;
use crate::mcp::save_file::SaveFileDescription;
use crate::mcp::save_function::SaveFunctionDescription;
use crate::mcp::search::SearchProjectInfo;
use crate::mcp::stats::MemoryStats;
use crate::mcp::upsert::UpsertProjectInfo;

// Generates `MemoryTools` enum with `tools()` and `TryFrom<CallToolRequestParams>`.
tool_box!(
    MemoryTools,
    [
        UpsertProjectInfo,
        SearchProjectInfo,
        OptimizeProjectInfo,
        SaveFileDescription,
        SaveFunctionDescription,
        MemoryStats
    ]
);

/// MCP server handler that dispatches tool calls to
/// the `ProjectInfoActor`.
pub struct MemoryToolHandler {
    actor: ActorRef<ProjectInfoActor>,
    version: &'static str,
}

impl MemoryToolHandler {
    /// Creates a handler that dispatches tool calls to
    /// the given project info actor.
    ///
    /// `version` is the server package version reported
    /// in the MCP handshake.
    pub fn new(actor: ActorRef<ProjectInfoActor>, version: &'static str) -> Self {
        Self { actor, version }
    }
}

#[async_trait]
impl ServerHandler for MemoryToolHandler {
    /// Lists the available memory tools.
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: MemoryTools::tools(),
            cache_scope: ListToolsResultCacheScope::Public,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
        })
    }

    /// Dispatches a tool call to the matching `execute`
    /// function.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        let tool = MemoryTools::try_from(params).map_err(CallToolError::new)?;
        let text = match tool {
            MemoryTools::UpsertProjectInfo(args) => upsert::execute(self.actor.clone(), args).await,
            MemoryTools::SaveFileDescription(args) => {
                save_file::execute(self.actor.clone(), args).await
            }
            MemoryTools::SaveFunctionDescription(args) => {
                save_function::execute(self.actor.clone(), args).await
            }
            MemoryTools::SearchProjectInfo(args) => search::execute(self.actor.clone(), args).await,
            MemoryTools::OptimizeProjectInfo(_) => optimize::execute(self.actor.clone()).await,
            MemoryTools::MemoryStats(args) => stats::execute(self.actor.clone(), args).await,
        };
        Ok(ServerResult::from(CallToolResult::text_content(vec![
            TextContent::from(text),
        ])))
    }

    /// This is a tools-only server: answer optional probes with empty
    /// success instead of `method_not_found` so strict clients (OpenCode)
    /// do not mark the server as failed during startup.
    async fn handle_list_resources_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            resources: vec![],
            cache_scope: ListResourcesResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
        })
    }

    /// Returns an empty resource-template list (tools-only server).
    async fn handle_list_resource_templates_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            cache_scope: ListResourceTemplatesResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
        })
    }

    /// Rejects resource reads: this server exposes no resources.
    async fn handle_read_resource_request(
        &self,
        _params: ReadResourceRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No resources are exposed by this server.".to_string()))
    }

    /// Returns an empty prompt list (tools-only server).
    async fn handle_list_prompts_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        Ok(ListPromptsResult {
            prompts: vec![],
            cache_scope: ListPromptsResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
            meta: None,
            next_cursor: None,
        })
    }

    /// Rejects prompt reads: this server exposes no prompts.
    async fn handle_get_prompt_request(
        &self,
        _params: GetPromptRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No prompts are exposed by this server.".to_string()))
    }

    /// Returns an empty completion result.
    async fn handle_complete_request(
        &self,
        _params: CompleteRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CompleteResult, RpcError> {
        Ok(CompleteResult {
            completion: CompleteResultCompletion {
                values: vec![],
                has_more: Some(false),
                total: Some(0),
            },
            result_type: "complete".to_string(),
            meta: None,
        })
    }

    /// Handles requests that missed standard
    /// deserialization (classic-client compatibility
    /// shim).
    async fn handle_custom_request(
        &self,
        request: CustomRequest,
        _context: &RequestContext,
        runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<GenericResult, RpcError> {
        // Compatibility shim: rust-mcp-sdk 2.0 targets the 2026-07-28
        // protocol where every request carries a required `_meta`. Classic
        // clients (OpenCode) omit it, so untagged deserialization misses the
        // standard variants and standard methods (`initialize`, `tools/list`,
        // ...) land here as `CustomRequest`. Dispatch them manually.
        match request.method.as_str() {
            "initialize" => self.initialize_result(runtime),
            "tools/list" => Self::tools_list_result(),
            "resources/list" => Self::empty_list_result("resources"),
            "resources/templates/list" => Self::empty_list_result("resourceTemplates"),
            "prompts/list" => Self::empty_list_result("prompts"),
            "completion/complete" => Self::empty_completion_result(),
            "tools/call" => self.call_tool_from_custom(request).await,
            "ping" => Ok(GenericResult {
                result_type: "complete".to_string(),
                meta: None,
                extra: None,
            }),
            // `notifications/initialized` arrives as notification, but handle
            // it here too in case a client sends it as request.
            "notifications/initialized" => Ok(GenericResult {
                result_type: "complete".to_string(),
                meta: None,
                extra: None,
            }),
            other => Err(RpcError::method_not_found()
                .with_message(format!("No handler is implemented for '{other}'."))),
        }
    }
}

impl MemoryToolHandler {
    /// Classic MCP handshake payload (the active 2026 schema dropped
    /// `InitializeResult`, so it is built manually as JSON).
    fn initialize_result(
        &self,
        runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<GenericResult, RpcError> {
        let details = runtime.server_details();
        let mut extra = serde_json::Map::new();
        extra.insert(
            "protocolVersion".to_string(),
            serde_json::Value::String("2025-11-25".to_string()),
        );
        extra.insert(
            "capabilities".to_string(),
            serde_json::json!({"tools": {"listChanged": true}}),
        );
        extra.insert(
            "serverInfo".to_string(),
            serde_json::json!({
                "name": details.server_info.name,
                "version": self.version.to_string(),
                "title": details.server_info.title,
                "description": details.server_info.description,
            }),
        );
        if let Some(instructions) = details.instructions.clone() {
            extra.insert(
                "instructions".to_string(),
                serde_json::Value::String(instructions),
            );
        }
        Ok(GenericResult {
            result_type: "complete".to_string(),
            meta: None,
            extra: Some(extra),
        })
    }

    /// Serialized `ListToolsResult` with the real tool definitions, so
    /// classic clients that omit `_meta` still discover our tools.
    fn tools_list_result() -> std::result::Result<GenericResult, RpcError> {
        let tools = serde_json::to_value(MemoryTools::tools()).map_err(|e| {
            RpcError::internal_error().with_message(format!("failed to serialize tools: {e}"))
        })?;
        let mut extra = serde_json::Map::new();
        extra.insert("tools".to_string(), tools);
        Ok(GenericResult {
            result_type: "complete".to_string(),
            meta: None,
            extra: Some(extra),
        })
    }

    /// Empty paginated-list payload (`resources`, `prompts`, ...).
    fn empty_list_result(key: &str) -> std::result::Result<GenericResult, RpcError> {
        let mut extra = serde_json::Map::new();
        extra.insert(key.to_string(), serde_json::Value::Array(vec![]));
        Ok(GenericResult {
            result_type: "complete".to_string(),
            meta: None,
            extra: Some(extra),
        })
    }

    /// Empty `completion/complete` payload.
    fn empty_completion_result() -> std::result::Result<GenericResult, RpcError> {
        let mut extra = serde_json::Map::new();
        extra.insert(
            "completion".to_string(),
            serde_json::json!({"values": [], "hasMore": false, "total": 0}),
        );
        Ok(GenericResult {
            result_type: "complete".to_string(),
            meta: None,
            extra: Some(extra),
        })
    }

    /// `tools/call` arriving as `CustomRequest` (classic
    /// client omitted the required `_meta`, so untagged
    /// deserialization missed the standard variant).
    /// Parses `name`/`arguments` manually and runs the
    /// tool.
    async fn call_tool_from_custom(
        &self,
        request: CustomRequest,
    ) -> std::result::Result<GenericResult, RpcError> {
        let params = request.params.clone().unwrap_or_default();
        let name = params
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                RpcError::invalid_params().with_message("missing tool 'name'".to_string())
            })?;
        let arguments = params
            .get("arguments")
            .and_then(serde_json::Value::as_object)
            .cloned();
        let tool_params = CallToolRequestParams {
            name: name.to_string(),
            arguments,
            meta: Default::default(),
            input_responses: None,
            request_state: None,
        };
        let tool = MemoryTools::try_from(tool_params).map_err(|e| {
            RpcError::invalid_params().with_message(format!("unknown tool '{name}': {e:?}"))
        })?;
        let text = match tool {
            MemoryTools::UpsertProjectInfo(args) => upsert::execute(self.actor.clone(), args).await,
            MemoryTools::SaveFileDescription(args) => {
                save_file::execute(self.actor.clone(), args).await
            }
            MemoryTools::SaveFunctionDescription(args) => {
                save_function::execute(self.actor.clone(), args).await
            }
            MemoryTools::SearchProjectInfo(args) => search::execute(self.actor.clone(), args).await,
            MemoryTools::OptimizeProjectInfo(_) => optimize::execute(self.actor.clone()).await,
            MemoryTools::MemoryStats(args) => stats::execute(self.actor.clone(), args).await,
        };
        let mut extra = serde_json::Map::new();
        extra.insert(
            "content".to_string(),
            serde_json::json!([{"type": "text", "text": text}]),
        );
        extra.insert("isError".to_string(), serde_json::Value::Bool(false));
        Ok(GenericResult {
            result_type: "complete".to_string(),
            meta: None,
            extra: Some(extra),
        })
    }
}
