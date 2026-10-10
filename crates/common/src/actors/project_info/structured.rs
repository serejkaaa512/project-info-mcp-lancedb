//! Structured stats / search handlers for the REST API + dashboard.

use kameo::message::{Context, Message};

use crate::actors::project_info::ProjectInfoActor;
use crate::actors::{SearchHit, StatsData, StatsStructuredMessage, StructuredSearchMessage};

impl Message<StatsStructuredMessage> for ProjectInfoActor {
    type Reply = Result<StatsData, String>;

    /// Aggregates per-category / per-project stats via the backend store.
    async fn handle(
        &mut self,
        msg: StatsStructuredMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        self.store.stats(&project).await
    }
}

impl Message<StructuredSearchMessage> for ProjectInfoActor {
    type Reply = Result<Vec<SearchHit>, String>;

    /// Embeds the query and runs the backend structured
    /// search, replying with structured hits.
    async fn handle(
        &mut self,
        msg: StructuredSearchMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        let query_vector = self
            .embedder
            .embed(&msg.query)
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .search_structured(query_vector, &project, msg.category.as_deref(), msg.limit)
            .await
    }
}
