use rikka_proton_account::{CoreEvent, Feed, FeedError};

use crate::client::MailClient;
use crate::event::Event;

impl Feed for MailClient {
    type Change = Box<Event>;

    fn apply_core(&self, event: &CoreEvent) -> Result<Option<Box<Event>>, FeedError> {
        let mail: Event = event.project()?;
        self.apply_event(&mail);
        let changed = !mail.is_empty() || event.refresh().includes_mail();
        Ok(changed.then(|| Box::new(mail)))
    }
}
