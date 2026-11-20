use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::client::DriveClient;
use crate::error::Result;
use crate::identifier::{EventId, LinkId, VolumeId};

wire_enum! {
    pub enum EventAction: default Update {
        Delete = 0,
        Create = 1,
        Update = 2,
        UpdateMetadata = 3,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventLink {
    #[serde(rename = "LinkID")]
    id: LinkId,
    #[serde(rename = "ParentLinkID", default)]
    parent_id: Option<LinkId>,
    #[serde(rename = "IsTrashed", default)]
    trashed: bool,
    #[serde(rename = "IsShared", default)]
    shared: bool,
}

impl EventLink {
    #[must_use]
    pub fn id(&self) -> &LinkId {
        &self.id
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&LinkId> {
        self.parent_id.as_ref()
    }

    #[must_use]
    pub fn is_trashed(&self) -> bool {
        self.trashed
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.shared
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveEvent {
    #[serde(rename = "EventID")]
    id: EventId,
    #[serde(rename = "EventType", default)]
    action: EventAction,
    #[serde(rename = "Link")]
    link: EventLink,
}

impl DriveEvent {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub fn action(&self) -> EventAction {
        self.action
    }

    #[must_use]
    pub fn link(&self) -> &EventLink {
        &self.link
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventBatch {
    #[serde(rename = "EventID")]
    id: EventId,
    #[serde(rename = "Events", default)]
    events: Vec<DriveEvent>,
    #[serde(rename = "ConvertibleExternalInvitations", default)]
    convertible: Vec<ConvertibleLink>,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
    #[serde(
        rename = "Refresh",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    refresh: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ConvertibleLink {
    #[serde(rename = "LinkID")]
    id: LinkId,
}

impl EventBatch {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub fn events(&self) -> &[DriveEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<DriveEvent> {
        self.events
    }

    #[must_use]
    pub fn convertible_invitations(&self) -> Vec<&LinkId> {
        self.convertible
            .iter()
            .map(|event_link| &event_link.id)
            .collect()
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        self.refresh
    }
}

#[derive(Debug, Clone)]
pub struct EventLog {
    last: EventId,
    events: Vec<DriveEvent>,
    convertible: Vec<LinkId>,
    refresh: bool,
}

impl EventLog {
    #[must_use]
    pub fn last(&self) -> &EventId {
        &self.last
    }

    #[must_use]
    pub fn events(&self) -> &[DriveEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<DriveEvent> {
        self.events
    }

    #[must_use]
    pub fn convertible_invitations(&self) -> &[LinkId] {
        &self.convertible
    }

    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        self.refresh
    }
}

impl DriveClient {
    pub async fn latest_event(&self, volume: &VolumeId) -> Result<EventId> {
        Ok(self
            .api()
            .get(format!("drive/volumes/{volume}/events/latest"))
            .send_field("EventID")
            .await?)
    }

    pub async fn events(&self, volume: &VolumeId, since: &EventId) -> Result<EventBatch> {
        Ok(self
            .api()
            .get(format!("drive/v2/volumes/{volume}/events/{since}"))
            .send()
            .await?)
    }

    pub async fn catch_up(&self, volume: &VolumeId, since: &EventId) -> Result<EventLog> {
        let mut events = Vec::new();
        let mut convertible = Vec::new();
        let mut last = since.clone();
        loop {
            let batch = self.events(volume, &last).await?;
            let more = batch.more && batch.id != last;
            last = batch.id;
            if batch.refresh {
                return Ok(EventLog {
                    last,
                    events: Vec::new(),
                    convertible: Vec::new(),
                    refresh: true,
                });
            }
            events.extend(batch.events);
            convertible.extend(
                batch
                    .convertible
                    .into_iter()
                    .map(|event_link| event_link.id),
            );
            if !more {
                return Ok(EventLog {
                    last,
                    events,
                    convertible,
                    refresh: false,
                });
            }
        }
    }
}
