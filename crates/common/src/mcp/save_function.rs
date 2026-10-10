//! MCP tool saving a function description into memory.

use kameo::actor::ActorRef;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};

use crate::actors::{UpsertMessage, project_info::ProjectInfoActor};

/// Category for per-function content descriptions
/// (unique id = file path + scope + function).
pub const FUNCTION_CATEGORY: &str = "function";

/// Builds the unique record id for a function description.
///
/// Format: `<file_path>::<function_name>` for free
/// functions, or
/// `<file_path>::<struct_name>::<function_name>` for
/// struct/impl-associated functions.
///
/// Example: `src/auth.rs::validate_jwt` or
/// `src/auth.rs::AuthService::validate`.
pub fn build_function_id(
    file_path: &str,
    struct_name: Option<&str>,
    function_name: &str,
) -> String {
    let file = file_path.trim();
    let func = function_name.trim();
    if let Some(scope) = struct_name.map(str::trim).filter(|s| !s.is_empty()) {
        format!("{file}::{scope}::{func}")
    } else {
        format!("{file}::{func}")
    }
}

/// Save a description of a single function/method into project memory.
#[mcp_tool(
    name = "save_function_description",
    description = "Save description of a function's content"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SaveFunctionDescription {
    /// Relative path to the file containing the
    /// function (for example, `src/auth.rs`).
    pub file_path: String,
    /// Function or method name (for example, `validate_jwt`).
    pub function_name: String,
    /// Struct/impl name for associated
    /// functions/methods (omit for free functions).
    pub struct_name: Option<String>,
    /// Short description of the function: purpose,
    /// parameters/return, side effects, how it is
    /// used.
    pub description: String,
    /// Project scope; defaults to the server's
    /// PROJECT_NAME when omitted.
    pub project: Option<String>,
}

/// Executes the save-function tool by upserting the
/// description under the `function` category.
pub async fn execute(actor: ActorRef<ProjectInfoActor>, args: SaveFunctionDescription) -> String {
    let id = build_function_id(
        &args.file_path,
        args.struct_name.as_deref(),
        &args.function_name,
    );
    let msg = UpsertMessage {
        id,
        content: args.description,
        category: FUNCTION_CATEGORY.to_string(),
        project: args.project.unwrap_or_default(),
    };
    match actor.ask(msg).await {
        Ok(text) => text,
        Err(e) => format!("❌ Failed to send message to Kameo actor: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_function_id_omits_struct() {
        assert_eq!(
            build_function_id("src/auth.rs", None, "validate_jwt"),
            "src/auth.rs::validate_jwt"
        );
    }

    #[test]
    fn associated_function_id_includes_struct() {
        assert_eq!(
            build_function_id("src/auth.rs", Some("AuthService"), "validate"),
            "src/auth.rs::AuthService::validate"
        );
    }

    #[test]
    fn blank_struct_treated_as_free_function() {
        assert_eq!(
            build_function_id("src/auth.rs", Some("  "), "validate_jwt"),
            "src/auth.rs::validate_jwt"
        );
    }
}
