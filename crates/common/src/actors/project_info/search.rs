//! Message handler searching project memory.

use kameo::message::{Context, Message};

use crate::actors::SearchMessage;
use crate::actors::project_info::{ProjectInfoActor, format::format_matches};
use crate::categories;

impl Message<SearchMessage> for ProjectInfoActor {
    type Reply = Result<String, String>;

    /// Embeds the query, runs the backend search, and
    /// replies with formatted matches.
    async fn handle(
        &mut self,
        msg: SearchMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        let category = msg.category.as_deref().map(categories::normalize);
        let expanded = categories::enrich_query(&msg.query, category.as_deref());
        let query_vector = self
            .embedder
            .embed(&expanded)
            .await
            .map_err(|e| e.to_string())?;

        let found = self
            .store
            .search_text(query_vector, &project, category.as_deref(), msg.limit)
            .await?;

        Ok(format_matches(&found))
    }
}
