use tokio::sync::mpsc;

use crate::client::{Notification, WeakConnection};
use crate::listing::Lister;
use crate::protocol::method;
use crate::watch::Watch;

pub(crate) struct Watcher {
    lister: Lister,
    watch: Watch,
}

impl Watcher {
    pub(crate) fn new(lister: Lister, watch: &Watch) -> Watcher {
        Watcher {
            lister,
            watch: watch.clone(),
        }
    }

    pub(crate) async fn follow(
        self,
        connection: WeakConnection,
        mut notes: mpsc::UnboundedReceiver<Notification>,
    ) {
        while let Some(note) = notes.recv().await {
            let Some(connection) = connection.upgrade() else {
                return;
            };
            match note.method.as_str() {
                method::TOOLS_LIST_CHANGED => {
                    self.relisted("tool(s)", self.lister.tools(&connection).await);
                }
                method::PROMPTS_LIST_CHANGED => {
                    self.relisted("prompt(s)", self.lister.prompts(&connection).await);
                }
                method::RESOURCES_LIST_CHANGED => {
                    self.relisted("resource(s)", self.lister.resources(&connection).await);
                }
                method::PROGRESS | method::MESSAGE => {
                    tracing::debug!(
                        "MCP server \"{}\" reports {}",
                        self.lister.label(),
                        note.params.unwrap_or_default()
                    );
                }
                other => {
                    tracing::debug!(
                        "MCP server \"{}\" notifies of {other}, which is not followed",
                        self.lister.label()
                    );
                }
            }
        }
    }

    fn relisted(&self, what: &str, relisted: Result<usize, String>) {
        let label = self.lister.label();
        match relisted {
            Ok(count) => {
                tracing::info!("MCP server \"{label}\" now offers {count} {what}");
                self.watch.say(label);
            }
            Err(reason) => {
                tracing::warn!("failed to refresh {what} of MCP server \"{label}\": {reason}");
            }
        }
    }
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watcher")
            .field("label", &self.lister.label())
            .finish_non_exhaustive()
    }
}
