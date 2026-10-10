//! Message handler listing stored points with filters (REST API / dashboard).

use kameo::message::{Context, Message};

use crate::actors::project_info::ProjectInfoActor;
use crate::actors::{ListPointsMessage, PointRecord};
use crate::categories;

impl Message<ListPointsMessage> for ProjectInfoActor {
    type Reply = Result<(Vec<PointRecord>, usize), String>;

    /// Counts and scans via the backend store (project/category predicate),
    /// then applies the substring filter and offset/limit pagination.
    async fn handle(
        &mut self,
        msg: ListPointsMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        let category = msg.category.as_deref().map(categories::normalize);
        let category = category.as_deref();
        let total = self.store.count(&project, category).await?;
        let all = self.store.list(&project, category, msg.limit).await?;

        // Substring filter on id + content (case-insensitive).
        let has_filter = msg.query.as_deref().is_some_and(|q| !q.trim().is_empty());
        let filtered: Vec<PointRecord> = match msg.query {
            Some(ref q) if !q.trim().is_empty() => {
                let needle = q.to_lowercase();
                all.into_iter()
                    .filter(|p| {
                        p.id.to_lowercase().contains(&needle)
                            || p.content.to_lowercase().contains(&needle)
                    })
                    .collect()
            }
            _ => all,
        };
        let filtered_total = if has_filter { filtered.len() } else { total };
        let page: Vec<PointRecord> = filtered
            .into_iter()
            .skip(msg.offset)
            .take(msg.limit.max(1))
            .collect();
        Ok((page, filtered_total))
    }
}
