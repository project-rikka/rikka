use rikka_proton_api::{Page, Paging};

use crate::api::{Notification, NotificationPage, NotificationState, NotificationStateRequest};
use crate::client::{PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::NotificationId;

impl PassClient {
    pub async fn notifications(&self) -> Result<Vec<Notification>> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/notification"))
            .pages(Paging::since())
            .field("Notifications")
            .collect(|page: NotificationPage| Page::new(page.notifications).next(page.last_id))
            .await?)
    }

    pub async fn set_notification_state(
        &self,
        notification: &NotificationId,
        state: NotificationState,
    ) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/notification/{notification}"))
            .json(&NotificationStateRequest { state })
            .send_unit()
            .await?;

        Ok(())
    }
}
