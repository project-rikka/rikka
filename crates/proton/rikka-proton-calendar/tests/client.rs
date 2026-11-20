use jiff::Timestamp;
use rikka_proton_cache::Cache;
use rikka_proton_calendar::{
    Attendee, CalendarBook, CalendarEvent, CalendarId, CalendarInvitation, DateValue, Error, Event,
    EventVerification, ModelEventId, Organizer, Permissions, attendee_token,
};
use rikka_proton_pgp::{LockedKey, Pgp, SignatureContext, SymmetricKey, VerificationContext};
use serde_json::{Value, json};

use crate::ical::NEW_YORK_VTIMEZONE;
use crate::support::{
    CALENDAR_PASSPHRASE, base64_bytes, calendar_fixture, decoded_query, signed_in,
};
use rikka_test_proton::{ADDRESS_ID, EMAIL, Record, Request, json_response, new_key, not_found};

const CALENDAR: &str = "/calendar/v2/calendar/bootstrap";

fn stored_from(payload: &Value, id: &str) -> Value {
    let authored = |cards: &Value| -> Value {
        Value::Array(
            cards
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|mut card| {
                    card["Author"] = json!(EMAIL);
                    card
                })
                .collect(),
        )
    };
    json!({
        "ID": id,
        "CalendarID": "calendar",
        "Author": EMAIL,
        "SharedKeyPacket": payload["SharedKeyPacket"],
        "CalendarKeyPacket": payload["CalendarKeyPacket"],
        "SharedEvents": authored(&payload["SharedEventContent"]),
        "CalendarEvents": authored(&payload["CalendarEventContent"]),
        "AttendeesEvents": authored(&payload["AttendeesEventContent"]),
        "AttendeesInfo": {"Attendees": [], "MoreAttendees": 0},
        "Notifications": payload["Notifications"],
    })
}

#[tokio::test]
async fn unlock_opens_rotated_keys() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let fixture = calendar_fixture(&pgp, &address_key);
    let (bootstrap, passphrases) = (fixture.bootstrap.clone(), fixture.passphrases.clone());
    let signed = signed_in(address_key, move |request: Request| match request.route() {
        ("GET", CALENDAR) => json_response(200, &bootstrap),
        ("GET", "/calendar/v1/calendar/passphrases") => json_response(200, &passphrases),
        _ => not_found(),
    })
    .await;

    let unlocked = signed
        .client
        .unlock(&CalendarId::from("calendar"))
        .await
        .unwrap();

    assert_eq!(unlocked.keys().len(), 2);
    assert_eq!(unlocked.member().id().as_str(), "member");
    assert_eq!(unlocked.address_id().as_str(), ADDRESS_ID);
    assert_eq!(
        unlocked.primary_key().unwrap().fingerprint(),
        fixture.key.fingerprint()
    );
    assert!(
        unlocked
            .keys()
            .find_private(&fixture.old_key.fingerprint())
            .is_some()
    );
    assert_eq!(unlocked.settings().default_event_duration(), 30);
}

#[tokio::test]
async fn rejects_calendar_without_member() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let fixture = calendar_fixture(&pgp, &address_key);
    let mut bootstrap = fixture.bootstrap;
    let passphrases = fixture.passphrases;
    bootstrap["Members"][0]["AddressID"] = json!("someone-else");
    let signed = signed_in(address_key, move |request: Request| match request.route() {
        ("GET", CALENDAR) => json_response(200, &bootstrap),
        ("GET", "/calendar/v1/calendar/passphrases") => json_response(200, &passphrases),
        _ => not_found(),
    })
    .await;

    assert!(matches!(
        signed.client.unlock(&CalendarId::from("calendar")).await,
        Err(Error::NotMember(_))
    ));
}

#[tokio::test]
async fn synced_events_round_trip() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let fixture = calendar_fixture(&pgp, &address_key);
    let (bootstrap, passphrases) = (fixture.bootstrap.clone(), fixture.passphrases.clone());
    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(address_key, move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", CALENDAR) => json_response(200, &bootstrap),
            ("GET", "/calendar/v1/calendar/passphrases") => json_response(200, &passphrases),
            ("GET", "/core/v4/addresses/canonical") => json_response(
                200,
                &json!({
                    "Code": 1001,
                    "Responses": [{"Email": "Guest@Example.com", "Response": {"Code": 1000, "CanonicalEmail": "guest@example.com"}}]
                }),
            ),
            ("PUT", "/calendar/v1/calendar/events/sync") => {
                let body = request.json();
                let count = body["Events"].as_array().map_or(0, Vec::len);
                let responses: Vec<Value> = (0..count)
                    .map(|index| json!({"Index": index, "Response": {"Code": 1000, "Event": {"ID": format!("event-{index}")}}}))
                    .collect();
                json_response(200, &json!({"Code": 1001, "Responses": responses}))
            }
            _ => not_found(),
        }
    })
    .await;
    let unlocked = signed
        .client
        .unlock(&CalendarId::from("calendar"))
        .await
        .unwrap();

    let start = DateValue::parse("20240610T090000", Some("Europe/Paris")).unwrap();
    let event = Event::new("sync@proton.me", start)
        .with_summary("Review")
        .with_organizer(Organizer::new(EMAIL))
        .with_attendee(Attendee::new("Guest@Example.com"));
    let outcomes = signed
        .client
        .sync(&unlocked)
        .create(event)
        .delete(&rikka_proton_calendar::EventId::from("old"))
        .execute()
        .await
        .unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|outcome| outcome.is_ok()));
    assert_eq!(
        outcomes[0].body().event().map(|event| event.id().as_str()),
        Some("event-0")
    );

    let body = requests_record
        .requests()
        .iter()
        .find(|request| request.path == "/calendar/v1/calendar/events/sync")
        .map(Request::json)
        .unwrap();
    assert_eq!(body["MemberID"], "member");
    let created = &body["Events"][0];
    assert_eq!(created["Overwrite"], 0);
    assert_eq!(created["Event"]["Permissions"], 1);
    assert_eq!(created["Event"]["IsOrganizer"], 1);
    assert_eq!(created["Event"]["Notifications"], Value::Null);
    assert_eq!(
        created["Event"]["Attendees"][0]["Token"],
        attendee_token("guest@example.com", "sync@proton.me")
    );
    assert_eq!(body["Events"][1], json!({"ID": "old", "DeletionReason": 0}));

    let record: CalendarEvent =
        serde_json::from_value(stored_from(&created["Event"], "event-0")).unwrap();
    let decrypted = signed.client.open(&unlocked, &record).await.unwrap();
    assert_eq!(decrypted.verification(), EventVerification::Successful);
    assert_eq!(decrypted.event().summary().as_deref(), Some("Review"));
    assert!(decrypted.uses_default_notifications());
    assert_eq!(decrypted.event().alarms().unwrap().len(), 1);
}

#[tokio::test]
async fn sharing_sends_passphrase_session_key() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let invitee = new_key(&pgp, "friend@proton.test");
    let fixture = calendar_fixture(&pgp, &address_key);
    let (bootstrap, passphrases) = (fixture.bootstrap.clone(), fixture.passphrases.clone());
    let armored = invitee.public_key().export_armored().unwrap();
    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(address_key.clone(), move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", CALENDAR) => json_response(200, &bootstrap),
            ("GET", "/calendar/v1/calendar/passphrases") => json_response(200, &passphrases),
            ("GET", "/core/v4/keys/all") => json_response(
                200,
                &json!({"Code": 1000, "Address": {"Keys": [{"Flags": 3, "PublicKey": armored}]}, "ProtonMX": true}),
            ),
            ("POST", "/calendar/v1/calendar/members") => json_response(200, &json!({"Code": 1000})),
            _ => not_found(),
        }
    })
    .await;
    let unlocked = signed
        .client
        .unlock(&CalendarId::from("calendar"))
        .await
        .unwrap();

    signed
        .client
        .share(&unlocked)
        .invite("friend@proton.test", Permissions::FULL_VIEW)
        .send()
        .await
        .unwrap();

    let body = requests_record
        .requests()
        .iter()
        .find(|request| request.method == "POST")
        .map(Request::json)
        .unwrap();
    assert_eq!(body["AddressID"], ADDRESS_ID);
    assert_eq!(body["Members"][0]["Email"], "friend@proton.test");
    assert_eq!(body["Members"][0]["Permissions"], 96);

    let packet = base64_bytes(body["Members"][0]["PassphraseKeyPacket"].as_str().unwrap());
    let session = pgp.decrypt().with_key(invitee).session_key(packet).unwrap();
    let member_passphrase = fixture.bootstrap["Passphrase"]["MemberPassphrases"][0]["Passphrase"]
        .as_str()
        .unwrap()
        .to_owned();
    let decrypted = pgp
        .decrypt()
        .with_session_key(session.clone())
        .message(&member_passphrase)
        .unwrap();
    assert_eq!(decrypted.data(), CALENDAR_PASSPHRASE.as_bytes());

    let status = pgp
        .verify()
        .with_key(address_key.public_key())
        .with_context(VerificationContext::new("calendar.sharing.invite").required(true))
        .detached(session.as_bytes(), body["Signature"].as_str().unwrap());
    assert!(status.is_valid(), "{status}");
}

#[tokio::test]
async fn accept_verifies_and_signs_passphrase() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let sender = new_key(&pgp, "sharer@proton.test");
    let session = rikka_proton_pgp::SessionKey::generate();
    let passphrase = pgp
        .encrypt()
        .to(address_key.public_key())
        .with_session_key(session.clone())
        .as_utf8()
        .armored_message(CALENDAR_PASSPHRASE)
        .unwrap();
    let signature = pgp
        .sign()
        .with_key(sender.clone())
        .with_context(SignatureContext::new("calendar.sharing.invite").critical(true))
        .armored_detached(session.as_bytes())
        .unwrap();
    let invitation: CalendarInvitation = serde_json::from_value(json!({
        "CalendarInvitationID": "invitation",
        "CalendarID": "shared",
        "Email": EMAIL,
        "Passphrase": passphrase,
        "PassphraseID": "pass",
        "Permissions": 112,
        "Status": 0,
        "Signature": signature
    }))
    .unwrap();

    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(address_key.clone(), move |request: Request| {
        log.push(&request);
        match request.route() {
            ("PUT", "/calendar/v1/shared/invitations/address-value/accept") => {
                json_response(200, &json!({"Code": 1000}))
            }
            _ => not_found(),
        }
    })
    .await;

    let stranger = new_key(&pgp, "stranger@proton.test");
    assert!(matches!(
        signed
            .client
            .accept_invitation(&invitation, &[stranger.public_key()])
            .await,
        Err(Error::PassphraseSignature)
    ));
    signed
        .client
        .accept_invitation(&invitation, &[sender.public_key()])
        .await
        .unwrap();

    let accepted = requests_record
        .requests()
        .iter()
        .find(|request| request.method == "PUT")
        .map(Request::json)
        .unwrap();
    let status = pgp
        .verify()
        .with_key(address_key.public_key())
        .as_utf8()
        .detached(CALENDAR_PASSPHRASE, accepted["Signature"].as_str().unwrap());
    assert!(status.is_valid(), "{status}");
}

#[tokio::test]
async fn new_calendar_key_locked_by_passphrase() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(address_key.clone(), move |request: Request| {
        log.push(&request);
        match request.route() {
            ("POST", "/calendar/v1") => json_response(
                200,
                &json!({"Code": 1000, "Calendar": {"ID": "fresh", "Type": 0, "Members": [{"ID": "m", "Name": "Work"}]}}),
            ),
            ("POST", "/calendar/v1/fresh/keys") => json_response(200, &json!({"Code": 1000})),
            _ => not_found(),
        }
    })
    .await;

    let calendar = signed
        .client
        .new_calendar("Work")
        .color("#123456")
        .create()
        .await
        .unwrap();
    assert_eq!(calendar.id().as_str(), "fresh");
    assert_eq!(calendar.name(), "Work");

    let request = requests_record.requests();
    let created = request
        .iter()
        .find(|request| request.path == "/calendar/v1")
        .map(Request::json)
        .unwrap();
    assert_eq!(created["Name"], "Work");
    assert_eq!(created["Color"], "#123456");
    assert_eq!(created["Display"], 1);
    assert_eq!(created["AddressID"], ADDRESS_ID);

    let setup = request
        .iter()
        .find(|request| request.path == "/calendar/v1/fresh/keys")
        .map(Request::json)
        .unwrap();
    let session = pgp
        .decrypt()
        .with_key(address_key.clone())
        .session_key(base64_bytes(
            setup["Passphrase"]["KeyPacket"].as_str().unwrap(),
        ))
        .unwrap();
    let decrypted = pgp
        .decrypt()
        .with_session_key(session)
        .as_utf8()
        .message(base64_bytes(
            setup["Passphrase"]["DataPacket"].as_str().unwrap(),
        ))
        .unwrap();
    let passphrase = decrypted.data().to_vec();
    assert_eq!(passphrase.len(), 44);
    let locked = LockedKey::import(setup["PrivateKey"].as_str().unwrap()).unwrap();
    assert!(locked.unlock(&passphrase).is_ok());
    let status = pgp
        .verify()
        .with_key(address_key.public_key())
        .as_utf8()
        .detached(&passphrase, setup["Signature"].as_str().unwrap());
    assert!(status.is_valid(), "{status}");
}

#[tokio::test]
async fn import_normalizes_zones_and_orders_edits() {
    let pgp = Pgp::new();
    let address_key = new_key(&pgp, EMAIL);
    let fixture = calendar_fixture(&pgp, &address_key);
    let (bootstrap, passphrases) = (fixture.bootstrap.clone(), fixture.passphrases.clone());
    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(address_key, move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", CALENDAR) => json_response(200, &bootstrap),
            ("GET", "/calendar/v1/calendar/passphrases") => json_response(200, &passphrases),
            ("PUT", "/calendar/v1/calendar/events/sync") => {
                let count = request.json()["Events"].as_array().map_or(0, Vec::len);
                let responses: Vec<Value> = (0..count)
                    .map(|index| match index {
                        2 => json!({"Index": index, "Response": {"Code": 2001, "Error": "refused"}}),
                        _ => json!({"Index": index, "Response": {"Code": 1000, "Event": {"ID": format!("e{index}")}}}),
                    })
                    .collect();
                json_response(200, &json!({"Code": 1001, "Responses": responses}))
            }
            _ => not_found(),
        }
    })
    .await;
    let unlocked = signed
        .client
        .unlock(&CalendarId::from("calendar"))
        .await
        .unwrap();

    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Other//EN\r\n{NEW_YORK_VTIMEZONE}\
BEGIN:VEVENT\r\nUID:override@x\r\nRECURRENCE-ID;TZID=Eastern Custom:20240108T090000\r\n\
DTSTART;TZID=Eastern Custom:20240108T100000\r\nSUMMARY:Moved\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:override@x\r\nDTSTART;TZID=Eastern Custom:20240101T090000\r\n\
RRULE:FREQ=WEEKLY;UNTIL=20240129T090000\r\nORGANIZER:mailto:a@b.c\r\nATTENDEE:mailto:d@e.f\r\nSUMMARY:Weekly\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART:20240301T120000\r\nSUMMARY:Floating\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n"
    );
    let report = signed
        .client
        .import(&unlocked, &ics, "Europe/Paris")
        .await
        .unwrap();
    assert_eq!(report.imported.len(), 2);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].reason, "refused");
    assert_eq!(report.failed[0].uid.as_deref(), Some("override@x"));

    let body = requests_record
        .requests()
        .iter()
        .find(|request| request.method == "PUT")
        .map(Request::json)
        .unwrap();
    assert_eq!(body["IsImport"], 1);
    let signed_parts: Vec<String> = body["Events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            assert_eq!(item["Overwrite"], 1);
            item["Event"]["SharedEventContent"][0]["Data"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert!(signed_parts[0].contains("DTSTART:20240101T140000Z"));
    assert!(signed_parts[0].contains("UNTIL=20240129T140000Z"));
    assert!(!signed_parts[0].contains("ORGANIZER"));
    assert!(signed_parts[1].contains("DTSTART;TZID=Europe/Paris:20240301T120000"));
    assert!(signed_parts[1].contains("@proton.me"));
    assert!(signed_parts[2].contains("RECURRENCE-ID:20240108T140000Z"));
}

#[tokio::test(flavor = "multi_thread")]
async fn events_update_calendar_cache() {
    let pgp = Pgp::new();
    let signed = signed_in(new_key(&pgp, EMAIL), |request: Request| match request.route() {
        ("GET", "/calendar/v1") => json_response(
            200,
            &json!({"Code": 1000, "Calendars": [{"ID": "calendar", "Type": 0, "Members": []}]}),
        ),
        ("GET", CALENDAR) => json_response(
            200,
            &json!({"Code": 1000, "Keys": [], "Members": [], "Passphrase": {"ID": "passphrase"}}),
        ),
        ("GET", "/calendar/v1/calendar/modelevents/m1") => json_response(
            200,
            &json!({
                "Code": 1000,
                "CalendarModelEventID": "m2",
                "CalendarEvents": [
                    {"ID": "e1", "Action": 1, "Event": {"ID": "e1", "CalendarID": "calendar", "UID": "u1"}}
                ]
            }),
        ),
        ("GET", "/calendar/v1/calendar/modelevents/m2") => json_response(
            200,
            &json!({"Code": 1000, "CalendarModelEventID": "m3", "Refresh": 1}),
        ),
        _ => not_found(),
    })
    .await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client = signed.client.with_cache(cache.clone());
    let calendar = CalendarId::from("calendar");
    assert!(client.cached_book().is_none());

    client.calendars().await.unwrap();
    client.bootstrap(&calendar).await.unwrap();
    let first = client
        .model_events(&calendar, &ModelEventId::from("m1"))
        .await
        .unwrap();
    client.apply_model_events(&calendar, &first);
    cache.flush().unwrap();

    assert_eq!(client.cached_events(&calendar).len(), 1);
    let book = client.cached_book().unwrap();
    assert!(book.calendar(&calendar).is_some());
    assert_eq!(book.events_of(&calendar).count(), 1);
    assert_eq!(
        client
            .cached_bootstrap(&calendar)
            .unwrap()
            .passphrase()
            .id()
            .as_str(),
        "passphrase"
    );

    let reload = client.model_events(&calendar, first.id()).await.unwrap();
    assert!(reload.needs_reload());
    client.apply_model_events(&calendar, &reload);
    assert!(client.cached_events(&calendar).is_empty());
    assert!(client.cached_bootstrap(&calendar).is_none());
}

#[tokio::test]
async fn model_events_update_calendar_book() {
    let pgp = Pgp::new();
    let signed = signed_in(new_key(&pgp, EMAIL), |request: Request| match request.route() {
        ("GET", "/calendar/v1/calendar/modelevents/latest") => {
            json_response(200, &json!({"Code": 1000, "CalendarModelEventID": "m1"}))
        }
        ("GET", "/calendar/v1/calendar/modelevents/m1") => json_response(
            200,
            &json!({
                "Code": 1000,
                "CalendarModelEventID": "m2",
                "CalendarEvents": [
                    {"ID": "e1", "Action": 1, "Event": {"ID": "e1", "CalendarID": "calendar", "UID": "u1"}},
                    {"ID": "e2", "Action": 1, "Event": {"ID": "e2", "CalendarID": "calendar", "UID": "u2"}}
                ],
                "CalendarAlarms": [{"ID": "a1", "Action": 1, "Alarm": {"ID": "a1", "Occurrence": 1000, "Trigger": "-PT15M", "EventID": "e1"}}]
            }),
        ),
        ("GET", "/calendar/v1/calendar/modelevents/m2") => json_response(
            200,
            &json!({"Code": 1000, "CalendarModelEventID": "m3", "CalendarEvents": [{"ID": "e2", "Action": 0}]}),
        ),
        _ => not_found(),
    })
    .await;

    let calendar = CalendarId::from("calendar");
    let latest: ModelEventId = signed.client.latest_model_event(&calendar).await.unwrap();
    let mut book = CalendarBook::new();
    let first = signed
        .client
        .model_events(&calendar, &latest)
        .await
        .unwrap();
    first.apply(&calendar, &mut book);
    assert_eq!(book.events_of(&calendar).count(), 2);
    let alarm = book.alarms().next().unwrap();
    assert_eq!(
        alarm.fires_at().unwrap(),
        jiff::Timestamp::from_second(100).unwrap()
    );

    signed
        .client
        .model_events(&calendar, first.id())
        .await
        .unwrap()
        .apply(&calendar, &mut book);
    let left: Vec<&str> = book.events_of(&calendar).map(|event| event.uid()).collect();
    assert_eq!(left, vec!["u1"]);
}

#[tokio::test]
async fn window_query_requests_each_type_once() {
    let pgp = Pgp::new();
    let requests_record = Record::default();
    let log = requests_record.clone();
    let signed = signed_in(new_key(&pgp, EMAIL), move |request: Request| {
        log.push(&request);
        match request.route() {
            ("GET", "/calendar/v1/calendar/events") => {
                let kind = decoded_query(&request).contains("Type=1");
                let events = if kind {
                    json!([{"ID": "e1"}, {"ID": "e2"}])
                } else {
                    json!([{"ID": "e1"}])
                };
                json_response(200, &json!({"Code": 1000, "Events": events}))
            }
            _ => not_found(),
        }
    })
    .await;

    let from: Timestamp = "2024-06-01T00:00:00Z".parse().unwrap();
    let to: Timestamp = "2024-07-01T00:00:00Z".parse().unwrap();
    let events = signed
        .client
        .events_between(&CalendarId::from("calendar"), from, to)
        .timezone("Europe/Paris")
        .list()
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

    let requests = requests_record.requests();
    let queries: Vec<String> = requests
        .iter()
        .filter(|request| request.path == "/calendar/v1/calendar/events")
        .map(decoded_query)
        .collect();
    assert_eq!(queries.len(), 4);
    for kind in 0..4 {
        assert!(
            queries
                .iter()
                .any(|query| query.contains(&format!("Type={kind}")))
        );
    }
    assert!(queries[0].contains(&format!("Start={}", from.as_second())));
    assert!(queries[0].contains("Timezone=Europe/Paris"));
}

#[tokio::test]
async fn only_proton_attendees_get_session_key() {
    let pgp = Pgp::new();
    let proton = new_key(&pgp, "friend@proton.test");
    let armored = proton.public_key().export_armored().unwrap();
    let signed = signed_in(new_key(&pgp, EMAIL), move |request: Request| match request.route() {
        ("GET", "/core/v4/keys/all") if decoded_query(&request).contains("friend@proton.test") => json_response(
            200,
            &json!({"Code": 1000, "Address": {"Keys": [{"Flags": 3, "Primary": 1, "PublicKey": armored}]}, "ProtonMX": true}),
        ),
        ("GET", "/core/v4/keys/all") => json_response(200, &json!({"Code": 1000, "Address": {"Keys": []}, "ProtonMX": false})),
        _ => not_found(),
    })
    .await;

    let event = Event::new(
        "party@proton.me",
        DateValue::utc("2024-06-01T10:00:00Z".parse().unwrap()),
    )
    .with_organizer(Organizer::new(EMAIL))
    .with_attendee(Attendee::new(EMAIL))
    .with_attendee(Attendee::new("friend@proton.test"))
    .with_attendee(Attendee::new("stranger@example.com"));
    let keys = signed.client.proton_attendee_keys(&event).await.unwrap();

    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].0, "friend@proton.test");
    assert_eq!(keys[0].1.fingerprint(), proton.fingerprint());

    assert!(matches!(
        signed.client.encryption_key("stranger@example.com").await,
        Err(Error::NoInviteeKey(_))
    ));
}
