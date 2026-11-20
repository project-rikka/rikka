use jiff::Timestamp;
use rikka_proton_calendar::{
    Alarm, AlarmAction, Attendee, CalendarEvent, CalendarSettings, DateValue, Duration, Event,
    EventStatus, EventVerification, Notification, NotificationKind, OpeningKeys, Organizer,
    ParticipationStatus, SealOptions, SealingKeys, SessionKeys, Trigger, attendee_token,
    decrypt_event, seal_event,
};
use rikka_proton_contentline::ContentLine;
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, PublicKey};
use serde_json::{Value, json};

use rikka_test_proton::new_key;

const AUTHOR: &str = "author@proton.test";

fn event() -> Event {
    let start = DateValue::parse("20240610T090000", Some("Europe/Paris")).unwrap();
    let mut event = Event::new("crypto@proton.me", start)
        .with_end(DateValue::parse("20240610T100000", Some("Europe/Paris")).unwrap())
        .with_summary("Secret plans")
        .with_description("Bring the documents")
        .with_location("Room 4")
        .with_status(EventStatus::Confirmed)
        .with_stamp("2024-06-01T00:00:00Z".parse::<Timestamp>().unwrap())
        .with_organizer(Organizer::new(AUTHOR))
        .with_attendee(
            Attendee::new("guest@example.com")
                .with_token("guest-token")
                .with_status(ParticipationStatus::Accepted),
        )
        .with_alarm(Alarm::new(
            AlarmAction::Display,
            Trigger::before_start(Duration::minutes(15)),
        ));
    event
        .component_mut()
        .push(ContentLine::new("COMMENT", "calendar only"));
    event
        .component_mut()
        .push(ContentLine::new("X-CUSTOM", "kept"));
    event
}

struct Keys {
    pgp: Pgp,
    address: PrivateKey,
    calendar: PrivateKey,
}

fn keys() -> Keys {
    let pgp = Pgp::new();
    Keys {
        address: new_key(&pgp, AUTHOR),
        calendar: new_key(&pgp, "calendar@proton.test"),
        pgp,
    }
}

fn sealed(keys: &Keys) -> Value {
    let options = SealOptions {
        creating: true,
        notifications: Some(vec![Notification::new(
            NotificationKind::Device,
            Duration::minutes(15),
        )]),
        color: Some("#FF0000".to_owned()),
        prod_id: "-//Proton AG//test 1.0//EN".to_owned(),
        ..SealOptions::default()
    };
    let (payload, _) = seal_event(
        &keys.pgp,
        &event(),
        &SealingKeys {
            address_key: keys.address.clone(),
            calendar_key: keys.calendar.public_key(),
        },
        &SessionKeys::default(),
        &options,
    )
    .unwrap();
    serde_json::to_value(&payload).unwrap()
}

fn stored(payload: &Value) -> CalendarEvent {
    let with_author = |cards: &Value| -> Value {
        Value::Array(
            cards
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|mut card| {
                    card["Author"] = json!(AUTHOR);
                    card
                })
                .collect(),
        )
    };
    let attendees: Vec<Value> = payload["Attendees"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, attendee)| json!({"ID": format!("attendee-{index}"), "Token": attendee["Token"], "Status": attendee["Status"]}))
        .collect();
    serde_json::from_value(json!({
        "ID": "event",
        "CalendarID": "calendar",
        "UID": "crypto@proton.me",
        "Author": AUTHOR,
        "SharedKeyPacket": payload["SharedKeyPacket"],
        "CalendarKeyPacket": payload["CalendarKeyPacket"],
        "SharedEvents": with_author(&payload["SharedEventContent"]),
        "CalendarEvents": with_author(&payload["CalendarEventContent"]),
        "AttendeesEvents": with_author(&payload["AttendeesEventContent"]),
        "AttendeesInfo": {"Attendees": attendees, "MoreAttendees": 0},
        "Notifications": payload["Notifications"],
        "Color": payload["Color"],
    }))
    .unwrap()
}

fn opened_with(
    keys: &Keys,
    record: &CalendarEvent,
    author: &[PublicKey],
) -> rikka_proton_calendar::DecryptedEvent {
    let ring = KeyRing::new().with_private(keys.calendar.clone());
    decrypt_event(
        &keys.pgp,
        record,
        OpeningKeys {
            calendar: &ring,
            address: None,
            author,
        },
        Some(&CalendarSettings::default()),
    )
    .unwrap()
}

#[test]
fn event_parts_match_clients() {
    let keys = keys();
    let payload = sealed(&keys);

    let shared = payload["SharedEventContent"].as_array().unwrap();
    assert_eq!(shared[0]["Type"], 2);
    let signed = shared[0]["Data"].as_str().unwrap();
    assert!(signed.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:"));
    for present in [
        "UID:",
        "DTSTART;TZID=Europe/Paris",
        "DTEND",
        "ORGANIZER",
        "DTSTAMP",
    ] {
        assert!(signed.contains(present), "{present}");
    }
    for absent in [
        "SUMMARY",
        "DESCRIPTION",
        "LOCATION",
        "ATTENDEE",
        "STATUS",
        "COMMENT",
        "VALARM",
    ] {
        assert!(!signed.contains(absent), "{absent}");
    }
    assert_eq!(shared[1]["Type"], 3);
    assert!(payload["SharedKeyPacket"].is_string());

    let calendar = payload["CalendarEventContent"].as_array().unwrap();
    assert_eq!(calendar.len(), 2);
    assert!(
        calendar[0]["Data"]
            .as_str()
            .is_some_and(|data| data.contains("STATUS:CONFIRMED"))
    );
    assert_eq!(calendar[1]["Type"], 3);
    assert!(payload["CalendarKeyPacket"].is_string());

    assert_eq!(payload["AttendeesEventContent"][0]["Type"], 3);
    assert_eq!(
        payload["Attendees"],
        json!([{"Token": "guest-token", "Status": 3, "Comment": null}])
    );
    assert_eq!(
        payload["Notifications"],
        json!([{"Type": 1, "Trigger": "-PT15M"}])
    );
    assert_eq!(payload["Color"], "#FF0000");
}

#[test]
fn sealed_events_open_verified() {
    let keys = keys();
    let record = stored(&sealed(&keys));
    let decrypted = opened_with(&keys, &record, &[keys.address.public_key()]);

    assert_eq!(decrypted.verification(), EventVerification::Successful);
    assert!(!decrypted.uses_default_notifications());
    let event = decrypted.event();
    assert_eq!(event.summary().as_deref(), Some("Secret plans"));
    assert_eq!(event.description().as_deref(), Some("Bring the documents"));
    assert_eq!(event.location().as_deref(), Some("Room 4"));
    assert_eq!(event.status(), Some(EventStatus::Confirmed));
    assert_eq!(
        event.component().text("COMMENT").as_deref(),
        Some("calendar only")
    );
    assert_eq!(event.component().text("X-CUSTOM").as_deref(), Some("kept"));
    assert_eq!(event.color().as_deref(), Some("#FF0000"));
    let guest = event.attendee("guest@example.com").unwrap();
    assert_eq!(guest.status(), ParticipationStatus::Accepted);
    assert_eq!(guest.token(), Some("guest-token"));
    assert_eq!(
        event.alarms().unwrap()[0].trigger().unwrap(),
        Trigger::before_start(Duration::minutes(15))
    );
    assert!(decrypted.session_keys().shared.is_some());
    assert!(decrypted.session_keys().calendar.is_some());
}

#[test]
fn author_keys_affect_verification() {
    let keys = keys();
    let record = stored(&sealed(&keys));
    let stranger = new_key(&keys.pgp, "stranger@proton.test").public_key();

    assert_eq!(
        opened_with(&keys, &record, &[stranger]).verification(),
        EventVerification::Failed
    );
    assert_eq!(
        opened_with(&keys, &record, &[]).verification(),
        EventVerification::NotVerified
    );
}

#[test]
fn updates_reuse_session_keys() {
    let keys = keys();
    let record = stored(&sealed(&keys));
    let decrypted = opened_with(&keys, &record, &[keys.address.public_key()]);

    let changed = decrypted.event().clone().with_summary("Changed plans");
    let (update, _) = seal_event(
        &keys.pgp,
        &changed,
        &SealingKeys {
            address_key: keys.address.clone(),
            calendar_key: keys.calendar.public_key(),
        },
        decrypted.session_keys(),
        &SealOptions::default(),
    )
    .unwrap();
    let mut update = serde_json::to_value(&update).unwrap();
    assert!(update.get("SharedKeyPacket").is_none());
    assert!(update.get("CalendarKeyPacket").is_none());
    update["SharedKeyPacket"] = record_packet(&record, true);
    update["CalendarKeyPacket"] = record_packet(&record, false);

    let reopened = opened_with(&keys, &stored(&update), &[keys.address.public_key()]);
    assert_eq!(reopened.event().summary().as_deref(), Some("Changed plans"));
    assert_eq!(reopened.verification(), EventVerification::Successful);
    assert!(reopened.uses_default_notifications());
}

fn record_packet(record: &CalendarEvent, shared: bool) -> Value {
    let packet = match shared {
        true => record.shared_key_packet(),
        false => record.calendar_key_packet(),
    };
    json!(packet)
}

#[test]
fn legacy_text_signatures_verify() {
    let keys = keys();
    let data = "BEGIN:VCALENDAR\nVERSION:2.0 \nBEGIN:VEVENT\nUID:legacy@proton.me\nDTSTART:20240101T000000Z\nEND:VEVENT\nEND:VCALENDAR";
    let stripped: String = data
        .split('\n')
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    let signature = keys
        .pgp
        .sign()
        .with_key(keys.address.clone())
        .as_utf8()
        .armored_detached(stripped)
        .unwrap();
    let record: CalendarEvent = serde_json::from_value(json!({
        "ID": "legacy",
        "SharedEvents": [{"Type": 2, "Data": data, "Signature": signature, "Author": AUTHOR}]
    }))
    .unwrap();

    let decrypted = opened_with(&keys, &record, &[keys.address.public_key()]);
    assert_eq!(decrypted.verification(), EventVerification::Successful);
    assert_eq!(decrypted.event().uid().as_deref(), Some("legacy@proton.me"));
}

#[test]
fn attendee_tokens_hash_uid_and_email() {
    assert_eq!(
        attendee_token("c", "ab"),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
    assert_eq!(
        attendee_token("", ""),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
}
