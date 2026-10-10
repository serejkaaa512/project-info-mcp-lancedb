//! Message handler reporting usage statistics about the project memory.

use kameo::message::{Context, Message};

use crate::actors::StatsMessage;
use crate::actors::project_info::{ProjectInfoActor, format::format_stats};

impl Message<StatsMessage> for ProjectInfoActor {
    type Reply = Result<String, String>;

    /// Aggregates stats via the backend store and replies
    /// with a formatted summary.
    async fn handle(
        &mut self,
        msg: StatsMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        let data = self.store.stats(&project).await?;

        if data.total == 0 {
            return Ok(format!(
                "📊 Memory stats for project '{project}': {} is empty (0 records).",
                self.store.stats_empty_noun()
            ));
        }

        Ok(format_stats(&project, &data, project == "*"))
    }
}
