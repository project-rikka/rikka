use std::sync::Arc;

use crate::listing::{Listed, Listing};
use crate::offer::Offer;
use crate::prompt::Prompt;
use crate::resource::{Resource, ResourceTemplate};

#[derive(Clone, Debug)]
pub struct ServerStatus {
    server: String,
    place: Option<String>,
    condition: Condition,
}

#[derive(Clone, Debug)]
pub(crate) enum Condition {
    Unused,
    Reached { offered: Listed, about: Peer },
    Locked { challenge: Option<String> },
    Unreached { trouble: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Peer {
    pub(crate) title: Option<String>,
    pub(crate) instructions: Option<Arc<str>>,
}

impl ServerStatus {
    pub(crate) fn new(server: impl Into<String>, condition: Condition) -> ServerStatus {
        ServerStatus {
            server: server.into(),
            place: None,
            condition,
        }
    }

    pub(crate) fn unreached(server: impl Into<String>, trouble: impl Into<String>) -> ServerStatus {
        ServerStatus::new(
            server,
            Condition::Unreached {
                trouble: trouble.into(),
            },
        )
    }

    pub(crate) fn with_place(mut self, place: Option<&str>) -> ServerStatus {
        self.place = place.map(str::to_owned);
        self
    }

    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    #[must_use]
    pub fn place(&self) -> Option<&str> {
        self.place.as_deref()
    }

    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.listing().map_or(0, |listing| listing.tools.len())
    }

    #[must_use]
    pub fn tools(&self) -> Vec<Offer> {
        self.listing().map_or_else(Vec::new, |listing| {
            listing
                .tools
                .iter()
                .map(|declared| declared.offer().clone())
                .collect()
        })
    }

    #[must_use]
    pub fn prompts(&self) -> Arc<[Prompt]> {
        self.listing()
            .map_or_else(|| Arc::from([]), |listing| listing.prompts)
    }

    #[must_use]
    pub fn resources(&self) -> Arc<[Resource]> {
        self.listing()
            .map_or_else(|| Arc::from([]), |listing| listing.resources)
    }

    #[must_use]
    pub fn resource_templates(&self) -> Arc<[ResourceTemplate]> {
        self.listing()
            .map_or_else(|| Arc::from([]), |listing| listing.templates)
    }

    #[must_use]
    pub fn prompt_count(&self) -> usize {
        self.listing().map_or(0, |listing| listing.prompts.len())
    }

    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.listing().map_or(0, |listing| {
            listing.resources.len() + listing.templates.len()
        })
    }

    #[must_use]
    pub fn offers_resources(&self) -> bool {
        self.resource_count() > 0
    }

    fn listing(&self) -> Option<Listing> {
        match &self.condition {
            Condition::Reached { offered, .. } => Some(offered.lock().clone()),
            _ => None,
        }
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        match &self.condition {
            Condition::Unreached { trouble } => Some(trouble),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_started(&self) -> bool {
        !matches!(self.condition, Condition::Unused)
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.condition, Condition::Reached { .. })
    }

    #[must_use]
    pub fn needs_sign_in(&self) -> bool {
        matches!(self.condition, Condition::Locked { .. })
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        match &self.condition {
            Condition::Reached { about, .. } => about.title.as_deref(),
            _ => None,
        }
    }

    #[must_use]
    pub fn instructions(&self) -> Option<&str> {
        match &self.condition {
            Condition::Reached { about, .. } => about.instructions.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn challenge(&self) -> Option<&str> {
        match &self.condition {
            Condition::Locked { challenge } => challenge.as_deref(),
            _ => None,
        }
    }
}
