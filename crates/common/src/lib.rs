//! Shared building blocks for the PIIM memory servers: kameo actors, MCP
//! tool dispatch, message types, and the backend-agnostic [`store::MemoryStore`]
//! trait implemented by each storage backend (LanceDB, Qdrant).

pub mod actors;
pub mod categories;
pub mod config;
pub mod mcp;
pub mod store;

/// Default project name used when the caller does not specify one.
pub const DEFAULT_PROJECT: &str = "default";
