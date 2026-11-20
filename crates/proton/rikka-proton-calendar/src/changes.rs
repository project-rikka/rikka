use jiff::Timestamp;
use std::collections::BTreeMap;

use rikka_proton_account::{EventId as CoreEventId, Refresh};
use rikka_proton_api::change;
use serde::{Deserialize, Serialize};

use crate::bootstrap::UserSettings;
use crate::calendar::{Calendar, Member};
use crate::calendar_event::CalendarEvent;
use crate::identifier::{AlarmId, CalendarId, EventId, MemberId, ModelEventId};
use crate::subscription::Subscription;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarAlarm {
    #[serde(rename = "ID", default)]
    id: AlarmId,
    #[serde(
        rename = "Occurrence",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    occurrence: Timestamp,
    #[serde(rename = "Trigger", default)]
    trigger: String,
    #[serde(rename = "Action", default)]
    action: i32,
    #[serde(rename = "EventID", default)]
    event_id: EventId,
    #[serde(rename = "MemberID", default)]
    member_id: MemberId,
}

impl CalendarAlarm {
    #[must_use]
    pub fn id(&self) -> &AlarmId {
        &self.id
    }

    #[must_use]
    pub fn occurrence(&self) -> Timestamp {
        self.occurrence
    }

    #[must_use]
    pub fn trigger(&self) -> &str {
        &self.trigger
    }

    #[must_use]
    pub fn action(&self) -> i32 {
        self.action
    }

    #[must_use]
    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    #[must_use]
    pub fn member_id(&self) -> &MemberId {
        &self.member_id
    }

    pub fn fires_at(&self) -> crate::error::Result<Timestamp> {
        let trigger: crate::duration::Duration = self.trigger.parse()?;
        Ok(self
            .occurrence
            .checked_add(jiff::SignedDuration::from_secs(trigger.total_seconds()))?)
    }
}

change!(CalendarChange, CalendarId, calendar, Calendar, "Calendar");
change!(MemberChange, MemberId, member, Member, "Member");
change!(EventChange, EventId, event, CalendarEvent, "Event");
change!(AlarmChange, AlarmId, alarm, CalendarAlarm, "Alarm");
change!(
    SubscriptionChange,
    CalendarId,
    subscription,
    Subscription,
    "CalendarSubscription"
);

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelEvents {
    #[serde(rename = "CalendarModelEventID", default)]
    id: ModelEventId,
    #[serde(rename = "Refresh", default)]
    refresh: Refresh,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
    #[serde(rename = "CalendarEvents", default)]
    events: Vec<EventChange>,
    #[serde(rename = "CalendarAlarms", default)]
    alarms: Vec<AlarmChange>,
    #[serde(rename = "CalendarSubscriptions", default)]
    subscriptions: Vec<SubscriptionChange>,
}

impl ModelEvents {
    #[must_use]
    pub fn id(&self) -> &ModelEventId {
        &self.id
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn needs_reload(&self) -> bool {
        !self.refresh.is_none()
    }

    #[must_use]
    pub fn events(&self) -> &[EventChange] {
        &self.events
    }

    #[must_use]
    pub fn alarms(&self) -> &[AlarmChange] {
        &self.alarms
    }

    #[must_use]
    pub fn subscriptions(&self) -> &[SubscriptionChange] {
        &self.subscriptions
    }

    pub fn apply(&self, calendar: &CalendarId, model: &mut impl CalendarModel) {
        if self.needs_reload() {
            model.reload_calendar(calendar);
            return;
        }
        for change in &self.events {
            match change.event() {
                Some(event) if !change.is_deleted() => model.put_event(event.clone()),
                _ => model.remove_event(calendar, change.id()),
            }
        }
        for change in &self.alarms {
            match change.alarm() {
                Some(alarm) if !change.is_deleted() => model.put_alarm(alarm.clone()),
                _ => model.remove_alarm(change.id()),
            }
        }
        for change in &self.subscriptions {
            if let Some(subscription) = change.subscription().filter(|_| !change.is_deleted()) {
                model.put_subscription(subscription.clone());
            }
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreEvents {
    #[serde(rename = "EventID", default)]
    id: CoreEventId,
    #[serde(rename = "Refresh", default)]
    refresh: Refresh,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
    #[serde(rename = "Calendars", default)]
    calendars: Vec<CalendarChange>,
    #[serde(rename = "CalendarMembers", default)]
    members: Vec<MemberChange>,
    #[serde(rename = "CalendarUserSettings", default)]
    user_settings: Option<UserSettings>,
}

impl CoreEvents {
    #[must_use]
    pub fn id(&self) -> &CoreEventId {
        &self.id
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.more
    }

    #[must_use]
    pub fn needs_reload(&self) -> bool {
        self.refresh.includes_everything()
    }

    #[must_use]
    pub fn calendars(&self) -> &[CalendarChange] {
        &self.calendars
    }

    #[must_use]
    pub fn members(&self) -> &[MemberChange] {
        &self.members
    }

    #[must_use]
    pub fn user_settings(&self) -> Option<&UserSettings> {
        self.user_settings.as_ref()
    }

    pub fn apply(&self, model: &mut impl CalendarModel) {
        if self.needs_reload() {
            model.reload();
            return;
        }
        for change in &self.calendars {
            match change.calendar() {
                Some(calendar) if !change.is_deleted() => model.put_calendar(calendar.clone()),
                _ => model.remove_calendar(change.id()),
            }
        }
        for change in &self.members {
            match change.member() {
                Some(member) if !change.is_deleted() => model.put_member(member.clone()),
                _ => model.remove_member(change.id()),
            }
        }
        if let Some(settings) = &self.user_settings {
            model.put_user_settings(settings.clone());
        }
    }
}

pub trait CalendarModel {
    fn put_calendar(&mut self, calendar: Calendar);
    fn remove_calendar(&mut self, id: &CalendarId);
    fn put_member(&mut self, member: Member);
    fn remove_member(&mut self, id: &MemberId);
    fn put_event(&mut self, event: CalendarEvent);
    fn remove_event(&mut self, calendar: &CalendarId, id: &EventId);
    fn put_alarm(&mut self, alarm: CalendarAlarm);
    fn remove_alarm(&mut self, id: &AlarmId);
    fn put_subscription(&mut self, subscription: Subscription);
    fn put_user_settings(&mut self, settings: UserSettings);
    fn reload_calendar(&mut self, calendar: &CalendarId);
    fn reload(&mut self);
}

#[derive(Debug, Clone, Default)]
pub struct CalendarBook {
    calendars: BTreeMap<CalendarId, Calendar>,
    events: BTreeMap<(CalendarId, EventId), CalendarEvent>,
    alarms: BTreeMap<AlarmId, CalendarAlarm>,
    subscriptions: BTreeMap<CalendarId, Subscription>,
    user_settings: Option<UserSettings>,
    stale: Vec<CalendarId>,
    stale_all: bool,
}

impl CalendarBook {
    #[must_use]
    pub fn new() -> CalendarBook {
        CalendarBook::default()
    }

    #[must_use]
    pub fn calendar(&self, id: &CalendarId) -> Option<&Calendar> {
        self.calendars.get(id)
    }

    pub fn calendars(&self) -> impl Iterator<Item = &Calendar> {
        self.calendars.values()
    }

    #[must_use]
    pub fn event(&self, calendar: &CalendarId, id: &EventId) -> Option<&CalendarEvent> {
        self.events.get(&(calendar.clone(), id.clone()))
    }

    pub fn events_of<'a>(
        &'a self,
        calendar: &'a CalendarId,
    ) -> impl Iterator<Item = &'a CalendarEvent> + 'a {
        self.events
            .iter()
            .filter(move |((event_calendar, _), _)| event_calendar == calendar)
            .map(|(_, event)| event)
    }

    pub fn alarms(&self) -> impl Iterator<Item = &CalendarAlarm> {
        self.alarms.values()
    }

    #[must_use]
    pub fn subscription(&self, calendar: &CalendarId) -> Option<&Subscription> {
        self.subscriptions.get(calendar)
    }

    #[must_use]
    pub fn user_settings(&self) -> Option<&UserSettings> {
        self.user_settings.as_ref()
    }

    #[must_use]
    pub fn stale_calendars(&self) -> &[CalendarId] {
        &self.stale
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.stale_all
    }

    pub fn insert_calendar(&mut self, calendar: Calendar) {
        self.put_calendar(calendar);
    }

    pub fn insert_event(&mut self, event: CalendarEvent) {
        self.put_event(event);
    }
}

impl CalendarModel for CalendarBook {
    fn put_calendar(&mut self, calendar: Calendar) {
        self.calendars.insert(calendar.id().clone(), calendar);
    }

    fn remove_calendar(&mut self, id: &CalendarId) {
        self.calendars.remove(id);
        self.events.retain(|(calendar, _), _| calendar != id);
        self.subscriptions.remove(id);
    }

    fn put_member(&mut self, member: Member) {
        let Some(calendar) = self.calendars.get_mut(member.calendar_id()) else {
            return;
        };
        calendar.replace_member(member);
    }

    fn remove_member(&mut self, id: &MemberId) {
        for calendar in self.calendars.values_mut() {
            calendar.remove_member(id);
        }
    }

    fn put_event(&mut self, event: CalendarEvent) {
        self.events
            .insert((event.calendar_id().clone(), event.id().clone()), event);
    }

    fn remove_event(&mut self, calendar: &CalendarId, id: &EventId) {
        self.events.remove(&(calendar.clone(), id.clone()));
    }

    fn put_alarm(&mut self, alarm: CalendarAlarm) {
        self.alarms.insert(alarm.id().clone(), alarm);
    }

    fn remove_alarm(&mut self, id: &AlarmId) {
        self.alarms.remove(id);
    }

    fn put_subscription(&mut self, subscription: Subscription) {
        self.subscriptions
            .insert(subscription.calendar_id().clone(), subscription);
    }

    fn put_user_settings(&mut self, settings: UserSettings) {
        self.user_settings = Some(settings);
    }

    fn reload_calendar(&mut self, calendar: &CalendarId) {
        self.events
            .retain(|(event_calendar, _), _| event_calendar != calendar);
        if !self.stale.contains(calendar) {
            self.stale.push(calendar.clone());
        }
    }

    fn reload(&mut self) {
        *self = CalendarBook {
            stale_all: true,
            ..CalendarBook::default()
        };
    }
}
