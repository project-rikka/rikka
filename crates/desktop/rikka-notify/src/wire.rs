use std::collections::HashMap;

use zbus::message::Message;
use zvariant::{Structure, StructureBuilder, Value};

use crate::event::{CloseReason, Event, Id};
use crate::notification::Notification;
use crate::picture::{Icon, Picture};

pub(crate) const NAME: &str = "org.freedesktop.Notifications";
pub(crate) const PATH: &str = "/org/freedesktop/Notifications";

const BITS_PER_SAMPLE: i32 = 8;

const CHANNELS: i32 = 4;

pub(crate) type Notify<'a> = (
    &'a str,
    u32,
    &'a str,
    &'a str,
    &'a str,
    Vec<&'a str>,
    HashMap<&'static str, Value<'a>>,
    i32,
);

pub(crate) fn notify(notification: &Notification) -> Notify<'_> {
    let app_icon = match &notification.icon {
        Some(Icon::Named(name)) => name.as_str(),
        Some(Icon::Picture(_)) | None => "",
    };
    let actions = notification
        .actions
        .iter()
        .flat_map(|action| [action.key.as_str(), action.label.as_str()])
        .collect();
    (
        &notification.app_name,
        notification.replaces.map_or(0, Id::get),
        app_icon,
        &notification.summary,
        &notification.body,
        actions,
        hints(notification),
        notification.timeout.milliseconds(),
    )
}

fn hints(notification: &Notification) -> HashMap<&'static str, Value<'_>> {
    let mut hints = HashMap::new();
    hints.insert("urgency", Value::U8(notification.urgency.level()));
    if let Some(entry) = &notification.desktop_entry {
        hints.insert("desktop-entry", Value::from(entry.as_str()));
    }
    if let Some(category) = &notification.category {
        hints.insert("category", Value::from(category.as_str()));
    }
    if notification.transient {
        hints.insert("transient", Value::Bool(true));
    }
    if notification.resident {
        hints.insert("resident", Value::Bool(true));
    }
    if let Some(Icon::Picture(picture)) = &notification.icon {
        hints.insert("image-data", Value::Structure(image_data(picture)));
    }
    hints
}

fn image_data(picture: &Picture) -> Structure<'_> {
    let side = |length: u32| i32::try_from(length).unwrap_or(i32::MAX);
    StructureBuilder::new()
        .add_field(side(picture.width()))
        .add_field(side(picture.height()))
        .add_field(side(picture.stride()))
        .add_field(true)
        .add_field(BITS_PER_SAMPLE)
        .add_field(CHANNELS)
        .add_field(picture.pixels())
        .build()
        .expect("seven fields always make a structure")
}

pub(crate) fn event_of(message: &Message) -> Option<Event> {
    let header = message.header();
    let member = header.member()?;
    let body = message.body();
    match member.as_str() {
        "ActionInvoked" => {
            let (id, key): (u32, String) = body.deserialize().ok()?;
            Some(Event::ActionInvoked {
                id: Id::new(id)?,
                key,
            })
        }
        "ActivationToken" => {
            let (id, token): (u32, String) = body.deserialize().ok()?;
            Some(Event::ActivationToken {
                id: Id::new(id)?,
                token,
            })
        }
        "NotificationClosed" => {
            let (id, reason): (u32, u32) = body.deserialize().ok()?;
            Some(Event::Closed {
                id: Id::new(id)?,
                reason: CloseReason::from_code(reason),
            })
        }
        _ => None,
    }
}
