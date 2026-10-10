//! Message handler compacting / optimizing storage.

use kameo::message::{Context, Message};

use crate::actors::OptimizeMessage;
use crate::actors::project_info::ProjectInfoActor;

impl Message<OptimizeMessage> for ProjectInfoActor {
    type Reply = Result<String, String>;

    /// Optimizes the backend storage and replies with a
    /// confirmation message.
    async fn handle(
        &mut self,
        _msg: OptimizeMessage,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.store.optimize().await
    }
}
