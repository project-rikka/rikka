use std::collections::{BTreeSet, HashMap};
use std::mem;

use rikka_proton_cache::{Batch, Cache, SINGLETON_ID, Table, Wired};

use crate::bootstrap::{Bootstrap, UserSettings};
use crate::calendar::{Calendar, Member};
use crate::calendar_event::CalendarEvent;
use crate::changes::{CalendarAlarm, CalendarBook, CalendarModel, CoreEvents, ModelEvents};
use crate::client::CalendarClient;
use crate::identifier::{AlarmId, CalendarId, EventId, MemberId};
use crate::subscription::Subscription;

const CALENDARS: Table<Calendar> = Table::new("calendar.calendars");
const BOOTSTRAPS: Table<Wired<Bootstrap>> = Table::new("calendar.bootstraps");
const EVENTS: Table<CalendarEvent> = Table::new("calendar.events");
const EVENT_IDS: Table<BTreeSet<EventId>> = Table::new("calendar.event-ids");
const ALARMS: Table<CalendarAlarm> = Table::new("calendar.alarms");
const SUBSCRIPTIONS: Table<Subscription> = Table::new("calendar.subscriptions");
const SETTINGS: Table<UserSettings> = Table::new("calendar.user-settings");

impl CalendarClient {
    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> CalendarClient {
        self.account = self.account.with_cache(cache.clone());
        self.contacts = self.contacts.with_cache(cache);
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    #[must_use]
    pub fn cached_calendars(&self) -> Option<Vec<Calendar>> {
        let calendars = self.cache()?.values(&CALENDARS);
        (!calendars.is_empty()).then_some(calendars)
    }

    #[must_use]
    pub fn cached_bootstrap(&self, calendar: &CalendarId) -> Option<Bootstrap> {
        self.cache()?
            .get(&BOOTSTRAPS, calendar.as_str())
            .map(Wired::into_value)
    }

    #[must_use]
    pub fn cached_event(&self, calendar: &CalendarId, id: &EventId) -> Option<CalendarEvent> {
        self.cache()?.get(&EVENTS, &event_key(calendar, id))
    }

    #[must_use]
    pub fn cached_events(&self, calendar: &CalendarId) -> Vec<CalendarEvent> {
        let Some(cache) = self.cache() else {
            return Vec::new();
        };
        cache
            .get(&EVENT_IDS, calendar.as_str())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| cache.get(&EVENTS, &event_key(calendar, id)))
            .collect()
    }

    #[must_use]
    pub fn cached_book(&self) -> Option<CalendarBook> {
        let cache = self.cache()?;
        let calendars = cache.values(&CALENDARS);
        if calendars.is_empty() {
            return None;
        }
        let mut book = CalendarBook::new();
        for calendar in calendars {
            book.put_calendar(calendar);
        }
        for event in cache.values(&EVENTS) {
            book.put_event(event);
        }
        for alarm in cache.values(&ALARMS) {
            book.put_alarm(alarm);
        }
        for subscription in cache.values(&SUBSCRIPTIONS) {
            book.put_subscription(subscription);
        }
        if let Some(settings) = cache.get(&SETTINGS, SINGLETON_ID) {
            book.put_user_settings(settings);
        }
        Some(book)
    }

    pub fn apply_core_events(&self, events: &CoreEvents) {
        self.with_writer(|writer| events.apply(writer));
    }

    pub fn apply_model_events(&self, calendar: &CalendarId, events: &ModelEvents) {
        self.with_writer(|writer| events.apply(calendar, writer));
    }

    pub fn forget_cached(&self) {
        self.with_writer(|writer| writer.reload());
    }

    pub(crate) fn cache_calendars(&self, calendars: &[Calendar]) {
        if let Some(cache) = self.cache() {
            cache.replace(
                &CALENDARS,
                calendars
                    .iter()
                    .map(|calendar| (calendar.id().as_str().to_owned(), calendar.clone())),
            );
        }
    }

    pub(crate) fn cache_calendar(&self, calendar: &Calendar) {
        if let Some(cache) = self.cache() {
            cache.put(&CALENDARS, calendar.id().as_str(), calendar.clone());
        }
    }

    pub(crate) fn cache_bootstrap(&self, calendar: &CalendarId, bootstrap: &Wired<Bootstrap>) {
        if let Some(cache) = self.cache() {
            cache.put(&BOOTSTRAPS, calendar.as_str(), bootstrap.clone());
        }
    }

    pub(crate) fn cache_events(&self, events: &[CalendarEvent]) {
        self.with_writer(|writer| {
            for event in events {
                writer.put_event(event.clone());
            }
        });
    }

    pub(crate) fn cache_user_settings(&self, settings: &UserSettings) {
        if let Some(cache) = self.cache() {
            cache.put(&SETTINGS, SINGLETON_ID, settings.clone());
        }
    }

    fn with_writer(&self, apply: impl FnOnce(&mut CacheWriter<'_>)) {
        let Some(cache) = self.cache() else {
            return;
        };
        let _guard = self.cache_writes.lock();
        let mut writer = CacheWriter {
            cache,
            batch: cache.batch(),
            event_ids: HashMap::new(),
            events_cleared: false,
        };
        apply(&mut writer);
        writer.commit();
    }
}

fn event_key(calendar: &CalendarId, id: &EventId) -> String {
    format!("{}/{}", calendar.as_str(), id.as_str())
}

struct CacheWriter<'a> {
    cache: &'a Cache,
    batch: Batch,
    event_ids: HashMap<CalendarId, BTreeSet<EventId>>,
    events_cleared: bool,
}

impl CalendarModel for CacheWriter<'_> {
    fn put_calendar(&mut self, calendar: Calendar) {
        self.batch
            .put(&CALENDARS, calendar.id().as_str(), calendar.clone());
    }

    fn remove_calendar(&mut self, id: &CalendarId) {
        self.batch
            .remove(&CALENDARS, id.as_str())
            .remove(&BOOTSTRAPS, id.as_str())
            .remove(&SUBSCRIPTIONS, id.as_str());
        self.remove_calendar_events(id);
    }

    fn put_member(&mut self, member: Member) {
        let Some(mut calendar) = self.cache.get(&CALENDARS, member.calendar_id().as_str()) else {
            return;
        };
        let id = calendar.id().as_str().to_owned();
        calendar.replace_member(member);
        self.batch
            .put(&CALENDARS, &id, calendar)
            .remove(&BOOTSTRAPS, &id);
    }

    fn remove_member(&mut self, id: &MemberId) {
        for (key, mut calendar) in self.cache.entries(&CALENDARS) {
            if calendar.members().iter().any(|member| member.id() == id) {
                calendar.remove_member(id);
                self.batch
                    .put(&CALENDARS, &key, calendar)
                    .remove(&BOOTSTRAPS, &key);
            }
        }
    }

    fn put_event(&mut self, event: CalendarEvent) {
        let key = event_key(event.calendar_id(), event.id());
        self.event_ids(event.calendar_id())
            .insert(event.id().clone());
        self.batch.put(&EVENTS, &key, event);
    }

    fn remove_event(&mut self, calendar: &CalendarId, id: &EventId) {
        self.event_ids(calendar).remove(id);
        self.batch.remove(&EVENTS, &event_key(calendar, id));
    }

    fn put_alarm(&mut self, alarm: CalendarAlarm) {
        let id = alarm.id().as_str().to_owned();
        self.batch.put(&ALARMS, &id, alarm);
    }

    fn remove_alarm(&mut self, id: &AlarmId) {
        self.batch.remove(&ALARMS, id.as_str());
    }

    fn put_subscription(&mut self, subscription: Subscription) {
        let id = subscription.calendar_id().as_str().to_owned();
        self.batch.put(&SUBSCRIPTIONS, &id, subscription);
    }

    fn put_user_settings(&mut self, settings: UserSettings) {
        self.batch.put(&SETTINGS, SINGLETON_ID, settings);
    }

    fn reload_calendar(&mut self, calendar: &CalendarId) {
        self.batch.remove(&BOOTSTRAPS, calendar.as_str());
        self.remove_calendar_events(calendar);
    }

    fn reload(&mut self) {
        self.batch
            .clear(&CALENDARS)
            .clear(&BOOTSTRAPS)
            .clear(&EVENTS)
            .clear(&EVENT_IDS)
            .clear(&ALARMS)
            .clear(&SUBSCRIPTIONS)
            .clear(&SETTINGS);
        self.event_ids.clear();
        self.events_cleared = true;
    }
}

impl CacheWriter<'_> {
    fn event_ids(&mut self, calendar: &CalendarId) -> &mut BTreeSet<EventId> {
        let (cache, cleared) = (self.cache, self.events_cleared);
        self.event_ids
            .entry(calendar.clone())
            .or_insert_with(|| match cleared {
                true => BTreeSet::new(),
                false => cache.get(&EVENT_IDS, calendar.as_str()).unwrap_or_default(),
            })
    }

    fn remove_calendar_events(&mut self, calendar: &CalendarId) {
        for id in mem::take(self.event_ids(calendar)) {
            self.batch.remove(&EVENTS, &event_key(calendar, &id));
        }
    }

    fn commit(self) {
        let CacheWriter {
            mut batch,
            event_ids,
            ..
        } = self;
        for (calendar, ids) in event_ids {
            if ids.is_empty() {
                batch.remove(&EVENT_IDS, calendar.as_str());
            } else {
                batch.put(&EVENT_IDS, calendar.as_str(), ids);
            }
        }
        batch.commit();
    }
}
