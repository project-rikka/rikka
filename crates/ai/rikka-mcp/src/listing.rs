use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use crate::client::{Connection, Failure};
use crate::declaration::{Declared, declared};
use crate::prompt::{self, Prompt};
use crate::protocol::{self, ServerCapabilities};
use crate::resource::{self, Resource, ResourceTemplate};

const LISTING: Duration = Duration::from_secs(60);

const RESOURCES_KEPT: usize = 10_000;

#[derive(Clone, Debug, Default)]
pub(crate) struct Listing {
    pub(crate) tools: Arc<[Declared]>,
    pub(crate) prompts: Arc<[Prompt]>,
    pub(crate) resources: Arc<[Resource]>,
    pub(crate) templates: Arc<[ResourceTemplate]>,
}

pub(crate) type Listed = Arc<Mutex<Listing>>;

#[derive(Clone, Debug)]
pub(crate) struct Lister {
    server: String,
    label: String,
    listed: Listed,
}

impl Lister {
    pub(crate) fn new(server: &str, label: &str) -> Lister {
        Lister {
            server: server.to_owned(),
            label: label.to_owned(),
            listed: Listed::default(),
        }
    }

    pub(crate) fn server(&self) -> &str {
        &self.server
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn listed(&self) -> &Listed {
        &self.listed
    }

    pub(crate) async fn everything(
        &self,
        connection: &Connection,
        offers: &ServerCapabilities,
    ) -> Result<(), String> {
        let (tools, prompts, resources) = tokio::join!(
            async {
                if offers.offers_tools() {
                    self.tools(connection).await
                } else {
                    Ok(0)
                }
            },
            async {
                if offers.offers_prompts() {
                    self.prompts(connection).await
                } else {
                    Ok(0)
                }
            },
            async {
                if offers.offers_resources() {
                    self.resources(connection).await
                } else {
                    Ok(0)
                }
            },
        );
        tools?;
        if let Err(reason) = prompts {
            tracing::warn!("MCP server \"{}\" offers no prompts: {reason}", self.label);
        }
        if let Err(reason) = resources {
            tracing::warn!(
                "MCP server \"{}\" offers no resources: {reason}",
                self.label
            );
        }
        Ok(())
    }

    pub(crate) async fn tools(&self, connection: &Connection) -> Result<usize, String> {
        let tools = bounded("tools", connection.list_all::<protocol::Tool>()).await?;
        let tools = declared(&self.label, tools);
        let count = tools.len();
        self.listed.lock().tools = Arc::from(tools);
        Ok(count)
    }

    pub(crate) async fn prompts(&self, connection: &Connection) -> Result<usize, String> {
        let prompts = bounded("prompts", connection.list_all::<protocol::Prompt>()).await?;
        let prompts = prompt::listed(&self.server, &self.label, prompts);
        let count = prompts.len();
        self.listed.lock().prompts = Arc::from(prompts);
        Ok(count)
    }

    pub(crate) async fn resources(&self, connection: &Connection) -> Result<usize, String> {
        let (resources, templates) = tokio::join!(
            bounded("resources", capped(connection, &self.label)),
            bounded(
                "resource templates",
                connection.list_all::<protocol::ResourceTemplate>()
            ),
        );
        let resources = resource::listed(&self.server, &self.label, resources?);
        let templates = match templates {
            Ok(templates) => resource::templated(&self.server, &self.label, templates),
            Err(reason) => {
                tracing::debug!(
                    "MCP server \"{}\" lists no resource templates: {reason}",
                    self.label
                );
                Vec::new()
            }
        };
        let count = resources.len() + templates.len();
        let mut listed = self.listed.lock();
        listed.resources = Arc::from(resources);
        listed.templates = Arc::from(templates);
        Ok(count)
    }
}

async fn capped(connection: &Connection, label: &str) -> Result<Vec<protocol::Resource>, Failure> {
    let mut resources = Vec::new();
    let mut cursor = None;
    loop {
        let page = connection
            .page::<protocol::Resource>(cursor.as_deref())
            .await?;
        resources.extend(page.items);
        cursor = page.next_cursor;
        if resources.len() >= RESOURCES_KEPT {
            if cursor.is_some() || resources.len() > RESOURCES_KEPT {
                tracing::debug!(
                    "MCP server \"{label}\" offers more than {RESOURCES_KEPT} resources"
                );
            }
            resources.truncate(RESOURCES_KEPT);
            return Ok(resources);
        }
        if cursor.is_none() {
            return Ok(resources);
        }
    }
}

async fn bounded<T>(
    what: &str,
    listing: impl Future<Output = Result<Vec<T>, Failure>>,
) -> Result<Vec<T>, String> {
    match tokio::time::timeout(LISTING, listing).await {
        Err(_) => Err(format!(
            "it did not say what {what} it has within {LISTING:?}"
        )),
        Ok(Err(Failure::Refused(error))) if error.is_method_not_found() => Ok(Vec::new()),
        Ok(Err(reason)) => Err(format!("it would not say what {what} it has: {reason}")),
        Ok(Ok(listed)) => Ok(listed),
    }
}
