//! MCP tool reporting memory usage statistics.

use kameo::actor::ActorRef;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};

/// Report memory usage statistics (record counts, per-category breakdown).
#[mcp_tool(name = "memory_stats", description = "Show memory usage statistics")]
#[derive(Debug, Default, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct MemoryStats {
    /// Project scope; defaults to the server's
    /// PROJECT_NAME. Use "*" for all projects.
    pub project: Option<String>,
}

/// Executes the stats tool by sending a stats request
/// to the project info actor.
pub async fn execute(
    actor: ActorRef<crate::actors::project_info::ProjectInfoActor>,
    args: MemoryStats,
) -> String {
    let msg = crate::actors::StatsMessage {
        project: args.project.unwrap_or_default(),
    };
    match actor.ask(msg).await {
        Ok(text) => text,
        Err(e) => format!("❌ Failed to send message to Kameo actor: {e}"),
    }
}
