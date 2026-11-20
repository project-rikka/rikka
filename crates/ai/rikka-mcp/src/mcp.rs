use std::collections::BTreeSet;
use std::sync::Arc;

use futures_util::future::{BoxFuture, join_all};
use rikka_agent::ToolKit;

use crate::catalog::Catalog;
use crate::naming;
use crate::server::Server;
use crate::server_status::{Condition, ServerStatus};
use crate::session::{Ground, Session};
use crate::watch::Watch;

#[derive(Debug, Default)]
pub(crate) struct Mcp {
    links: Vec<Link>,
}

#[derive(Debug)]
struct Link {
    declared: Server,
    label: String,
    standing: ServerStatus,
    session: Option<Arc<Session>>,
}

impl Mcp {
    pub(crate) async fn connect(
        catalog: &Catalog,
        ground: &Ground<'_>,
        watch: &Watch,
        mcp: &Mcp,
        again: &BTreeSet<String>,
    ) -> Mcp {
        let opening = labelled(catalog).into_iter().map(|(server, label)| {
            let reused = mcp
                .links
                .iter()
                .find(|link| {
                    link.declared == *server
                        && link.label == label
                        && link.session.is_some()
                        && !again.contains(server.name())
                })
                .map(|link| (link.standing.clone(), link.session.clone()));
            async move {
                let (standing, session) = match reused {
                    Some(reused) => reused,
                    None => {
                        let opening: BoxFuture<'_, _> =
                            Box::pin(open(server, &label, ground, watch));
                        opening.await
                    }
                };
                Link {
                    declared: server.clone(),
                    label,
                    standing,
                    session,
                }
            }
        });

        Mcp {
            links: join_all(opening).await,
        }
    }

    pub(crate) fn statuses(&self) -> Vec<ServerStatus> {
        self.links
            .iter()
            .map(|link| link.standing.clone())
            .collect()
    }

    pub(crate) fn kits(&self) -> Vec<ToolKit> {
        self.links
            .iter()
            .filter_map(|link| {
                let session = link.session.as_ref()?;
                let peer = session.peer();
                let mut kit =
                    ToolKit::new(link.declared.name()).with_functions(session.functions());
                if let Some(title) = &peer.title {
                    kit = kit.with_description(title.clone());
                }
                if let Some(instructions) = &peer.instructions {
                    kit = kit.with_instructions(instructions.to_string());
                }
                Some(kit)
            })
            .collect()
    }

    pub(crate) fn sessions(&self) -> impl Iterator<Item = &Arc<Session>> {
        self.links.iter().filter_map(|link| link.session.as_ref())
    }

    pub(crate) fn labelled(&self, label: &str) -> Option<&Arc<Session>> {
        self.links
            .iter()
            .find(|link| link.label == label)
            .and_then(|link| link.session.as_ref())
    }

    pub(crate) fn named(&self, server: &str) -> Option<&Arc<Session>> {
        self.links
            .iter()
            .find(|link| link.declared.name() == server)
            .and_then(|link| link.session.as_ref())
    }
}

fn labelled(catalog: &Catalog) -> Vec<(&Server, String)> {
    let mut used_names = BTreeSet::new();
    catalog
        .servers()
        .iter()
        .map(|server| {
            let label = naming::unique(naming::label(server.name()), &mut used_names);
            (server, label)
        })
        .collect()
}

async fn open(
    server: &Server,
    label: &str,
    ground: &Ground<'_>,
    watch: &Watch,
) -> (ServerStatus, Option<Arc<Session>>) {
    if server.is_disabled() {
        return (ServerStatus::new(server.name(), Condition::Unused), None);
    }
    if let Some(error) = server.config_error() {
        return (ServerStatus::unreached(server.name(), error.as_str()), None);
    }
    let expanded = match server.expanded(|name| std::env::var(name).ok()) {
        Ok(expanded) => expanded,
        Err(unset) => {
            return (
                ServerStatus::unreached(server.name(), unset.to_string()),
                None,
            );
        }
    };

    match Session::open(&expanded, label, ground, watch).await {
        Ok(session) => {
            let listing = session.listing();
            tracing::info!(
                "MCP server \"{}\" connected with {} tool(s), {} prompt(s) and {} resource(s)",
                server.name(),
                listing.tools.len(),
                listing.prompts.len(),
                listing.resources.len() + listing.templates.len(),
            );
            let standing = ServerStatus::new(
                server.name(),
                Condition::Reached {
                    offered: session.listed(),
                    about: session.peer().clone(),
                },
            );
            (standing, Some(Arc::new(session)))
        }
        Err(condition) => {
            match &condition {
                Condition::Locked { .. } => {
                    tracing::info!("MCP server \"{}\" requires sign-in", server.name());
                }
                Condition::Unreached { trouble } => {
                    tracing::warn!(
                        "failed to connect MCP server \"{}\": {trouble}",
                        server.name()
                    );
                }
                _ => {}
            }
            (ServerStatus::new(server.name(), condition), None)
        }
    }
}
