use crate::api::{ShareEventList, UserEvents};
use crate::client::{PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::{EventId, ShareId};
use crate::share_events::ShareEvents;

impl PassClient {
    pub async fn latest_event(&self, share: &ShareId) -> Result<EventId> {
        let envelope: EventId = self
            .api()
            .get(format!("{PREFIX}/share/{share}/event"))
            .send_field_or_default("EventID")
            .await?;

        Ok(envelope)
    }

    pub async fn events(&self, share: &ShareId, since: &EventId) -> Result<ShareEvents> {
        let events = self.share_event_list(share, since).await?;
        self.apply_events(share, since, &events);

        let latest_event = events.latest_event().clone();
        let pending = events.is_pending();
        let new_key_rotation = events.new_key_rotation();
        let deleted = events.deleted_items().to_vec();

        let (updated_share, revisions) = events.into_parts();
        if let Some(rotation) = new_key_rotation {
            self.share_keys_for_rotations(share, &[rotation]).await?;
        }
        let updated = self.decrypt_items(share, revisions).await?;

        let share = match updated_share {
            Some(response) => Some(self.decrypt_share(response).await?),
            None => None,
        };

        Ok(ShareEvents {
            share,
            updated,
            deleted,
            new_key_rotation,
            latest_event,
            pending,
        })
    }

    pub(crate) async fn share_event_list(
        &self,
        share: &ShareId,
        since: &EventId,
    ) -> Result<ShareEventList> {
        let envelope: ShareEventList = self
            .api()
            .get(format!("{PREFIX}/share/{share}/event/{since}"))
            .send_field("Events")
            .await?;

        Ok(envelope)
    }

    pub async fn latest_user_event(&self) -> Result<EventId> {
        let envelope: EventId = self
            .api()
            .get(format!("{PREFIX}/user/sync_event"))
            .send_field_or_default("EventID")
            .await?;

        Ok(envelope)
    }

    pub async fn user_events(&self, since: &EventId) -> Result<UserEvents> {
        let envelope: UserEvents = self
            .api()
            .get(format!("{PREFIX}/user/sync_event/{since}"))
            .send_field("Events")
            .await?;

        Ok(envelope)
    }
}
