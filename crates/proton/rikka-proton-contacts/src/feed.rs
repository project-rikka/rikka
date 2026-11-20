use rikka_proton_account::{CoreEvent, Feed, FeedError};

use crate::client::ContactsClient;
use crate::event::ContactEvents;

impl Feed for ContactsClient {
    type Change = Box<ContactEvents>;

    fn apply_core(&self, event: &CoreEvent) -> Result<Option<Box<ContactEvents>>, FeedError> {
        let contacts: ContactEvents = event.project()?;
        self.apply_events(&contacts);
        let changed = contacts.needs_reload()
            || !contacts.contacts().is_empty()
            || !contacts.contact_emails().is_empty()
            || contacts.groups().next().is_some();
        Ok(changed.then(|| Box::new(contacts)))
    }
}
