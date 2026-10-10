//! Actor managing project info records through a backend [`MemoryStore`].

pub mod delete;
pub mod format;
pub mod list;
pub mod optimize;
pub mod reopen;
pub mod search;
pub mod stats;
pub mod structured;
pub mod upsert;

use kameo::actor::{Actor, ActorRef};

use crate::actors::embedding;
use crate::store::MemoryStore;

/// Kameo actor managing project info records via a
/// backend storage implementation.
pub struct ProjectInfoActor {
    store: Box<dyn MemoryStore>,
    embedder: Box<dyn embedding::Embedder>,
    default_project: String,
}

impl ProjectInfoActor {
    /// Creates the actor with the given backend store, embedding actor
    /// reference, and default project name.
    pub fn new(
        store: Box<dyn MemoryStore>,
        embedder: Box<dyn embedding::Embedder>,
        default_project: String,
    ) -> Self {
        let default_project = normalize_project(&default_project);
        ProjectInfoActor {
            store,
            embedder,
            default_project,
        }
    }

    /// Resolves the effective project scope: explicit
    /// value wins, otherwise the actor default.
    pub fn resolve_project(&self, project: &str) -> String {
        let trimmed = project.trim();
        if trimmed.is_empty() {
            self.default_project.clone()
        } else {
            trimmed.to_string()
        }
    }
}

/// Normalizes a project name, falling back to the default when blank.
pub fn normalize_project(project: &str) -> String {
    let trimmed = project.trim();
    if trimmed.is_empty() {
        crate::DEFAULT_PROJECT.to_string()
    } else {
        trimmed.to_string()
    }
}

impl Actor for ProjectInfoActor {
    type Args = ProjectInfoActor;

    type Error = anyhow::Error;

    /// Creates the actor from the arguments provided at
    /// spawn time.
    async fn on_start(args: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_blank_project() {
        assert_eq!(normalize_project(""), crate::DEFAULT_PROJECT);
        assert_eq!(normalize_project("  "), crate::DEFAULT_PROJECT);
        assert_eq!(normalize_project(" proj-a "), "proj-a");
    }
}
