use rikka_notifications::Notification;

use crate::look::Look;

#[derive(Debug)]
pub struct StoredNotification {
    pub notification: Notification,
    pub look: Look,
}

impl StoredNotification {
    #[must_use]
    pub(crate) fn new(notification: Notification, look: Look) -> StoredNotification {
        StoredNotification { notification, look }
    }

    #[must_use]
    pub fn id(&self) -> u32 {
        self.notification.id
    }
}
