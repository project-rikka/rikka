use futures_util::future::join_all;
use rikka_proton_account::{CoreEvent, EventPage, Feed, FeedError, FeedFuture, Poll};
use rikka_proton_api::offload;

use crate::changes::{CoreEvents, ModelEvents};
use crate::client::CalendarClient;
use crate::error::Error;
use crate::identifier::{CalendarId, ModelEventId};

#[derive(Debug)]
pub enum CalendarUpdate {
    Calendars(Box<CoreEvents>),
    Model {
        calendar: CalendarId,
        events: Box<ModelEvents>,
    },
}

impl Feed for CalendarClient {
    type Change = CalendarUpdate;

    fn apply_core(&self, event: &CoreEvent) -> Result<Option<CalendarUpdate>, FeedError> {
        let calendars: CoreEvents = event.project()?;
        self.apply_core_events(&calendars);
        let changed = calendars.needs_reload()
            || !calendars.calendars().is_empty()
            || !calendars.members().is_empty()
            || calendars.user_settings().is_some();
        Ok(changed.then(|| CalendarUpdate::Calendars(Box::new(calendars))))
    }

    fn poll<'a>(&'a self, poll: &'a Poll<'a, CalendarUpdate>) -> FeedFuture<'a> {
        Box::pin(async move {
            let recalling = self.clone();
            let calendars = match offload(move || recalling.cached_calendars()).await {
                Some(calendars) => calendars,
                None => self.calendars().await?,
            };
            let walks = calendars
                .iter()
                .map(|calendar| self.follow_model(poll, calendar.id()));
            join_all(walks).await.into_iter().collect()
        })
    }
}

impl CalendarClient {
    async fn follow_model(
        &self,
        poll: &Poll<'_, CalendarUpdate>,
        calendar: &CalendarId,
    ) -> Result<(), FeedError> {
        poll.walk(
            &format!("calendar:{calendar}"),
            async { Ok(self.latest_model_event(calendar).await?.as_str().to_owned()) },
            |since| async move {
                Ok(self
                    .model_events(calendar, &ModelEventId::from(since.as_str()))
                    .await?)
            },
            |_, events: ModelEvents| async move {
                self.apply_model_events(calendar, &events);
                let changed = events.needs_reload()
                    || !events.events().is_empty()
                    || !events.alarms().is_empty()
                    || !events.subscriptions().is_empty();
                if changed {
                    poll.emit(CalendarUpdate::Model {
                        calendar: calendar.clone(),
                        events: Box::new(events),
                    });
                }
                Ok(())
            },
        )
        .await
    }
}

impl EventPage for ModelEvents {
    fn next_id(&self) -> &str {
        self.id().as_str()
    }

    fn has_more(&self) -> bool {
        self.has_more()
    }
}

impl From<Error> for FeedError {
    fn from(error: Error) -> FeedError {
        FeedError::new(error, Error::api)
    }
}
