//! MCP tool upserting a project fact into memory.

use kameo::actor::ActorRef;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};

/// Upsert a discrete project fact into project memory.
#[mcp_tool(name = "upsert_project_info", description = "Upsert project info")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct UpsertProjectInfo {
    /// Record ID, unique key (for example, path to file or task ID).
    pub info_id: String,
    /// Discrete facts or short content text.
    /// For `codestyle` records: one focused rule
    /// per record — what to do, what to avoid,
    /// minimal good/bad example, plus searchable
    /// keywords (e.g. naming, clippy/rustfmt,
    /// `?`/`if let`/iterator chains,
    /// `thiserror`/`anyhow`, import grouping,
    /// `///` docs, QA workflow).
    pub content: String,
    /// Record category: architecture (design/ADRs: recall before refactoring),
    /// codestyle (one rule per record: recall before writing code),
    /// file (per-file: find which file implements a feature),
    /// function (per-function: find callers/params),
    /// code_contract (interfaces/types: recall before integration),
    /// todo (debt/bugs: plan work), changelog (history: recall what changed).
    /// Aliases like 'api' are normalized.
    pub category: String,
    /// Project scope; defaults to the server's
    /// PROJECT_NAME when omitted.
    pub project: Option<String>,
}

/// Executes the upsert tool by forwarding the
/// arguments to the project info actor.
pub async fn execute(
    actor: ActorRef<crate::actors::project_info::ProjectInfoActor>,
    args: UpsertProjectInfo,
) -> String {
    let msg = crate::actors::UpsertMessage {
        id: args.info_id,
        content: args.content,
        category: args.category,
        project: args.project.unwrap_or_default(),
    };
    match actor.ask(msg).await {
        Ok(text) => text,
        Err(e) => format!("❌ Failed to send message to Kameo actor: {e}"),
    }
}
