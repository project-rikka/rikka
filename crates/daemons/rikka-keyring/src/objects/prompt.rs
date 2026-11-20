use std::sync::Arc;

use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, Value};

use crate::fault::Fault;
use crate::shared::Shared;

pub(crate) struct PromptObject {
    shared: Arc<Shared>,
    path: OwnedObjectPath,
}

impl PromptObject {
    pub(crate) fn new(shared: Arc<Shared>, path: OwnedObjectPath) -> PromptObject {
        PromptObject { shared, path }
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Prompt")]
impl PromptObject {
    async fn prompt(&self, window_id: String) -> Result<(), Fault> {
        tracing::debug!(prompt = %self.path.as_str(), window = %window_id, "prompt shown");
        let path = self.path.clone();
        self.shared.run(move |state| state.show_prompt(&path)).await
    }

    async fn dismiss(&self) -> Result<(), Fault> {
        let path = self.path.clone();
        self.shared
            .run(move |state| state.dismiss_prompt(&path))
            .await
    }

    #[zbus(signal)]
    pub(crate) async fn completed(
        emitter: &SignalEmitter<'_>,
        dismissed: bool,
        result: Value<'_>,
    ) -> zbus::Result<()>;
}
