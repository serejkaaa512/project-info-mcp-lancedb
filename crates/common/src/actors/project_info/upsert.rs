//! Message handler inserting or replacing a project info record.

use std::time::{SystemTime, UNIX_EPOCH};

use kameo::message::{Context, Message};
use sha2::{Digest, Sha256};

use crate::actors::UpsertMessage;
use crate::actors::project_info::ProjectInfoActor;
use crate::categories;
use crate::store::Record;

impl Message<UpsertMessage> for ProjectInfoActor {
    type Reply = Result<String, String>;

    /// Hashes the content, skips writes when unchanged,
    /// then embeds and upserts the record via the
    /// backend store.
    async fn handle(
        &mut self,
        msg: UpsertMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        let category = categories::normalize(&msg.category);
        let enriched = categories::enrich_content(&category, &msg.content);
        let mut hasher = Sha256::new();
        hasher.update(enriched.as_bytes());
        let current_hash = hex::encode(hasher.finalize());

        if !self
            .store
            .prepare_upsert(&msg.id, &project, &current_hash)
            .await
        {
            return Ok(format!(
                "ℹ️ [Kameo] Data for id '{}' didn't change (hash matches). Model inference skipped.",
                msg.id
            ));
        }

        let vector = self
            .embedder
            .embed(&enriched)
            .await
            .map_err(|e| e.to_string())?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.store
            .write_record(
                Record {
                    id: &msg.id,
                    project: &project,
                    content: &enriched,
                    category: &category,
                    file_hash: &current_hash,
                    timestamp,
                },
                vector,
            )
            .await?;

        Ok(format!(
            "✅ [Kameo] Data '{}' successfully updated in project memory (project '{}').",
            msg.id, project
        ))
    }
}
