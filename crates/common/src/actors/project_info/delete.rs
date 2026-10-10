//! Message handler deleting a single point by `(id, project)`.

use kameo::message::{Context, Message};

use crate::actors::DeletePointMessage;
use crate::actors::project_info::ProjectInfoActor;

impl Message<DeletePointMessage> for ProjectInfoActor {
    type Reply = Result<bool, String>;

    /// Deletes the record matching `(id, project)`;
    /// replies `true` when a record existed.
    async fn handle(
        &mut self,
        msg: DeletePointMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let project = self.resolve_project(&msg.project);
        self.store.delete(&msg.id, &project).await
    }
}
