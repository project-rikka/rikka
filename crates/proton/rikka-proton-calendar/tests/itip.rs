use jiff::Timestamp;
use rikka_proton_calendar::{
    Alarm, AlarmAction, Attendee, DateValue, Duration, Error, Event, EventStatus, Freshness, Itip,
    Method, Organizer, ParticipationStatus, Trigger, cancel_calendar, reply_calendar,
    request_calendar,
};

const PROD_ID: &str = "-//Proton AG//test 1.0//EN";

fn timestamp(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn organized() -> Event {
    let start = DateValue::parse("20240610T090000", Some("Europe/Paris")).unwrap();
    Event::new("meeting@proton.me", start.clone())
        .with_end(DateValue::parse("20240610T100000", Some("Europe/Paris")).unwrap())
        .with_rule(&"FREQ=WEEKLY;COUNT=3".parse().unwrap())
        .with_summary("Planning")
        .with_description("Private notes")
        .with_sequence(1)
        .with_stamp(timestamp("2024-06-01T08:00:00Z"))
        .with_organizer(Organizer::new("boss@proton.me").with_name("Boss"))
        .with_attendee(Attendee::new("ann@example.com").with_token("token-ann"))
        .with_attendee(Attendee::new("bob@example.com"))
        .with_alarm(Alarm::new(
            AlarmAction::Display,
            Trigger::before_start(Duration::minutes(10)),
        ))
}

#[test]
fn requests_strip_personal_data() {
    let mut event = organized();
    event.set_color(Some("#ABCDEF"));
    let sent = timestamp("2024-06-02T12:00:00Z");
    let calendar = request_calendar(&event, PROD_ID, Vec::new(), sent);

    assert_eq!(calendar.method(), Some(Method::Request));
    assert_eq!(calendar.prod_id().as_deref(), Some(PROD_ID));
    let invite = &calendar.events().unwrap()[0];
    assert!(invite.alarms().unwrap().is_empty());
    assert!(invite.color().is_none());
    assert_eq!(invite.stamp().unwrap(), Some(DateValue::utc(sent)));
    assert_eq!(invite.attendees().len(), 2);
    assert_eq!(invite.description().as_deref(), Some("Private notes"));

    let untitled = Event::new("x", DateValue::utc(sent)).with_end(DateValue::utc(sent));
    let request = request_calendar(&untitled, PROD_ID, Vec::new(), sent);
    let untitled = &request.events().unwrap()[0];
    assert_eq!(untitled.summary().as_deref(), Some(""));
    assert!(untitled.end().unwrap().is_none());
}

#[test]
fn replies_carry_only_replying_attendee() {
    let sent = timestamp("2024-06-03T07:00:00Z");
    let calendar = reply_calendar(
        &organized(),
        "ANN@example.com",
        ParticipationStatus::Accepted,
        PROD_ID,
        Vec::new(),
        sent,
    )
    .unwrap();

    assert_eq!(calendar.method(), Some(Method::Reply));
    let reply = &calendar.events().unwrap()[0];
    let attendees = reply.attendees();
    assert_eq!(attendees.len(), 1);
    assert_eq!(attendees[0].email(), "ann@example.com");
    assert_eq!(attendees[0].status(), ParticipationStatus::Accepted);
    assert!(attendees[0].name().is_none());
    assert!(attendees[0].token().is_none());
    assert_eq!(reply.sequence(), 1);
    assert_eq!(reply.uid().as_deref(), Some("meeting@proton.me"));
    assert_eq!(
        reply
            .organizer()
            .map(|organizer| organizer.email())
            .as_deref(),
        Some("boss@proton.me")
    );
    assert!(reply.description().is_none());
    assert!(reply.rule().unwrap().is_some());

    let single = organized()
        .with_recurrence_id(DateValue::parse("20240617T090000", Some("Europe/Paris")).unwrap());
    let single_reply = reply_calendar(
        &single,
        "ann@example.com",
        ParticipationStatus::Declined,
        PROD_ID,
        Vec::new(),
        sent,
    )
    .unwrap();
    assert!(single_reply.events().unwrap()[0].rule().unwrap().is_none());

    assert!(matches!(
        reply_calendar(
            &organized(),
            "eve@example.com",
            ParticipationStatus::Accepted,
            PROD_ID,
            Vec::new(),
            sent
        ),
        Err(Error::NotInvited(_))
    ));
}

#[test]
fn cancel_marks_event_cancelled() {
    let event = organized();
    let calendar = cancel_calendar(
        &event,
        &event.attendees(),
        PROD_ID,
        Vec::new(),
        timestamp("2024-06-04T00:00:00Z"),
    )
    .unwrap();

    assert_eq!(calendar.method(), Some(Method::Cancel));
    let cancelled = &calendar.events().unwrap()[0];
    assert_eq!(cancelled.status(), Some(EventStatus::Cancelled));
    assert_eq!(cancelled.attendees().len(), 2);
    assert!(
        cancelled
            .attendees()
            .iter()
            .all(|attendee| attendee.name().is_none())
    );
}

#[test]
fn replies_update_organizer_copy() {
    let reply_text = reply_calendar(
        &organized(),
        "bob@example.com",
        ParticipationStatus::Tentative,
        PROD_ID,
        Vec::new(),
        timestamp("2024-06-03T07:00:00Z"),
    )
    .unwrap()
    .to_string();

    let itip = Itip::parse(&reply_text).unwrap();
    assert_eq!(*itip.method(), Method::Reply);
    assert!(!itip.is_from_organizer());
    assert_eq!(itip.uid(), "meeting@proton.me");

    let mut copy = organized();
    assert!(itip.apply_reply(&mut copy).unwrap());
    assert_eq!(
        copy.attendee("bob@example.com")
            .map(|attendee| attendee.status()),
        Some(ParticipationStatus::Tentative)
    );
    assert!(!itip.apply_reply(&mut copy).unwrap());

    let mut newer = organized().with_sequence(4);
    assert!(!itip.apply_reply(&mut newer).unwrap());
    assert_eq!(itip.freshness(&newer).unwrap(), Freshness::Outdated);
    assert_eq!(
        itip.freshness(&organized().with_sequence(0)).unwrap(),
        Freshness::Newer
    );
}

#[test]
fn invitations_require_uid() {
    let empty = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nEND:VCALENDAR\r\n";
    assert!(matches!(Itip::parse(empty), Err(Error::NoEvents)));
    let anonymous = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nDTSTART:20240101T000000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    assert!(matches!(Itip::parse(anonymous), Err(Error::MissingUid)));

    let request = request_calendar(
        &organized(),
        PROD_ID,
        Vec::new(),
        timestamp("2024-06-02T12:00:00Z"),
    )
    .to_string();
    let itip = Itip::parse(&request).unwrap();
    assert!(itip.is_from_organizer());
    let invited = itip
        .invited_attendee(&["other@proton.me".to_owned(), "Ann@Example.com".to_owned()])
        .unwrap();
    assert_eq!(invited.token(), Some("token-ann"));
    assert!(itip.cancels(&organized()).is_ok_and(|cancels| !cancels));
}
