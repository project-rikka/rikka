use jiff::Timestamp;
use jiff::civil::date;
use rikka_proton_calendar::{
    AlarmAction, DateValue, Duration, Error, EventStatus, ICalendar, Method, ParticipationStatus,
    Role, Trigger, Zones, prepare_import, windows_zone,
};

pub(crate) const NEW_YORK_VTIMEZONE: &str = "BEGIN:VTIMEZONE\r
TZID:Eastern Custom\r
BEGIN:STANDARD\r
DTSTART:19671029T020000\r
RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU;UNTIL=20061029T060000Z\r
TZOFFSETFROM:-0400\r
TZOFFSETTO:-0500\r
TZNAME:EST\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:19870405T020000\r
RRULE:FREQ=YEARLY;BYMONTH=4;BYDAY=1SU;UNTIL=20060402T070000Z\r
TZOFFSETFROM:-0500\r
TZOFFSETTO:-0400\r
TZNAME:EDT\r
END:DAYLIGHT\r
BEGIN:DAYLIGHT\r
DTSTART:20070311T020000\r
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r
TZOFFSETFROM:-0500\r
TZOFFSETTO:-0400\r
TZNAME:EDT\r
END:DAYLIGHT\r
BEGIN:STANDARD\r
DTSTART:20071104T020000\r
RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\r
TZOFFSETFROM:-0400\r
TZOFFSETTO:-0500\r
TZNAME:EST\r
END:STANDARD\r
END:VTIMEZONE\r
";

fn calendar_text() -> String {
    format!(
        "BEGIN:VCALENDAR\r
VERSION:2.0\r
PRODID:-//Example//Test//EN\r
METHOD:REQUEST\r
{NEW_YORK_VTIMEZONE}BEGIN:VEVENT\r
UID:19970901T130000Z-123401@example.com\r
DTSTAMP:19970901T130000Z\r
DTSTART;TZID=Eastern Custom:19970903T163000\r
DTEND;TZID=Eastern Custom:19970903T190000\r
SUMMARY:Annual Employee Review\\, with\\; escapes\r
DESCRIPTION:Line one\\nLine two\r
STATUS:CONFIRMED\r
SEQUENCE:2\r
TRANSP:TRANSPARENT\r
ORGANIZER;CN=\"Boss, Big\":mailto:boss@example.com\r
ATTENDEE;CN=Employee;ROLE=OPT-PARTICIPANT;PARTSTAT=TENTATIVE;RSVP=TRUE:mailto:\r
 employee@example.com\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
TRIGGER;RELATED=END:-PT15M\r
DESCRIPTION:Reminder\r
REPEAT:2\r
DURATION:PT5M\r
END:VALARM\r
END:VEVENT\r
END:VCALENDAR\r
"
    )
}

#[test]
fn calendars_parse_into_events() {
    let calendar = ICalendar::parse(&calendar_text()).unwrap();
    assert_eq!(calendar.method(), Some(Method::Request));
    assert_eq!(calendar.timezones().count(), 1);

    let events = calendar.events().unwrap();
    let event = &events[0];
    assert_eq!(
        event.uid().as_deref(),
        Some("19970901T130000Z-123401@example.com")
    );
    assert_eq!(
        event.summary().as_deref(),
        Some("Annual Employee Review, with; escapes")
    );
    assert_eq!(event.description().as_deref(), Some("Line one\nLine two"));
    assert_eq!(event.status(), Some(EventStatus::Confirmed));
    assert_eq!(event.sequence(), 2);
    assert!(event.is_transparent());
    assert_eq!(
        event.start().unwrap(),
        DateValue::zoned(date(1997, 9, 3).at(16, 30, 0, 0), "Eastern Custom")
    );

    let organizer = event.organizer().unwrap();
    assert_eq!(organizer.email(), "boss@example.com");
    assert_eq!(organizer.name(), Some("Boss, Big"));
    let attendee = event.attendee("EMPLOYEE@example.com").unwrap();
    assert_eq!(attendee.role(), Role::Optional);
    assert_eq!(attendee.status(), ParticipationStatus::Tentative);
    assert!(attendee.rsvp());

    let alarms = event.alarms().unwrap();
    assert_eq!(alarms[0].action(), AlarmAction::Display);
    assert_eq!(
        alarms[0].trigger().unwrap(),
        Trigger::Relative {
            duration: Duration {
                negative: true,
                minutes: 15,
                ..Duration::default()
            },
            from_end: true,
        }
    );
    assert_eq!(alarms[0].repeat(), Some((2, Duration::minutes(5))));
}

#[test]
fn calendars_round_trip() {
    let calendar = ICalendar::parse(&calendar_text()).unwrap();
    let serialized = calendar.to_string();
    for physical in serialized.split("\r\n") {
        assert!(physical.len() <= 75);
    }
    assert_eq!(ICalendar::parse(&serialized).unwrap(), calendar);
}

#[test]
fn vtimezones_match_tzdb() {
    let calendar = ICalendar::parse(&calendar_text()).unwrap();
    let defined = calendar.zones().unwrap();
    let custom = defined.resolve("Eastern Custom").unwrap();
    let named = Zones::new().resolve("America/New_York").unwrap();

    let samples = [
        date(1997, 9, 3).at(16, 30, 0, 0),
        date(1998, 1, 15).at(9, 0, 0, 0),
        date(2006, 4, 2).at(3, 30, 0, 0),
        date(2007, 3, 11).at(2, 30, 0, 0),
        date(2007, 11, 4).at(1, 30, 0, 0),
        date(2024, 7, 1).at(12, 0, 0, 0),
        date(2024, 12, 24).at(18, 0, 0, 0),
    ];
    for local in samples {
        assert_eq!(
            custom.to_timestamp(local).unwrap(),
            named.to_timestamp(local).unwrap(),
            "{local}"
        );
    }
    let at: Timestamp = "2024-07-01T16:00:00Z".parse().unwrap();
    assert_eq!(custom.to_civil(at), date(2024, 7, 1).at(12, 0, 0, 0));
}

#[test]
fn rejects_unknown_zones_and_values() {
    let zones = Zones::new();
    assert!(matches!(
        zones.resolve("Mars/Olympus"),
        Err(Error::UnknownTimeZone(_))
    ));
    assert!(
        zones
            .resolve("/mozilla.org/20050126_1/Europe/Berlin")
            .is_ok()
    );
    assert!(zones.resolve("UTC").is_ok());

    assert!(matches!(
        ICalendar::parse("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nEND:VCALENDAR\r\n"),
        Err(Error::Unbalanced(_))
    ));
    assert!(matches!(
        ICalendar::parse("BEGIN:VCARD\r\nEND:VCARD\r\n"),
        Err(Error::NoCalendar)
    ));
}

#[test]
fn durations_follow_rfc_5545() {
    let cases = [
        ("P15DT5H0M20S", 15 * 86_400 + 5 * 3600 + 20, "P15DT5H20S"),
        ("P7W", 7 * 7 * 86_400, "P7W"),
        ("-PT15M", -900, "-PT15M"),
        ("+PT1H", 3600, "PT1H"),
        ("PT0S", 0, "PT0S"),
        ("P1D", 86_400, "P1D"),
    ];
    for (text, seconds, serialized) in cases {
        let duration: Duration = text.parse().unwrap();
        assert_eq!(duration.total_seconds(), seconds, "{text}");
        assert_eq!(duration.to_string(), serialized, "{text}");
    }
    for invalid in ["15M", "PT", "P1H", "PT1D", "P1W2X", ""] {
        assert!(invalid.parse::<Duration>().is_err(), "{invalid}");
    }
}

#[test]
fn date_values_parse_every_form() {
    let cases = [
        ("19970714", None, DateValue::Date(date(1997, 7, 14))),
        (
            "19980118T230000",
            None,
            DateValue::Floating(date(1998, 1, 18).at(23, 0, 0, 0)),
        ),
        (
            "19980119T070000Z",
            None,
            DateValue::Utc(date(1998, 1, 19).at(7, 0, 0, 0)),
        ),
        (
            "19980119T020000",
            Some("America/New_York"),
            DateValue::zoned(date(1998, 1, 19).at(2, 0, 0, 0), "America/New_York"),
        ),
    ];
    for (text, tzid, expected) in cases {
        let parsed = DateValue::parse(text, tzid).unwrap();
        assert_eq!(parsed, expected, "{text}");
        assert_eq!(parsed.to_string(), text);
    }
    assert!(DateValue::parse_date("19971332").is_none());
    assert!(DateValue::parse("19970714T25", None).is_none());
}

#[test]
fn windows_zone_names_map_to_iana() {
    let cases = [
        ("W. Europe Standard Time", Some("Europe/Berlin")),
        ("Eastern Standard Time", Some("America/New_York")),
        ("Tokyo Standard Time", Some("Asia/Tokyo")),
        (
            "(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna",
            Some("Europe/Amsterdam"),
        ),
        (
            "(UTC-05:00) Eastern Time (US & Canada)",
            Some("America/New_York"),
        ),
        (
            "Pacific Time (US & Canada) (UTC-08:00)",
            Some("America/Los_Angeles"),
        ),
        ("/mozilla.org/20050126_1/Europe/Paris", Some("Europe/Paris")),
        (
            "tzone://Microsoft/Custom/Asia/Kolkata",
            Some("Asia/Kolkata"),
        ),
        ("Europe/Zurich", Some("Europe/Zurich")),
        ("Customized Time Zone", None),
    ];
    for (tzid, expected) in cases {
        assert_eq!(Zones::supported_tzid(tzid).as_deref(), expected, "{tzid}");
    }
    assert_eq!(
        windows_zone("w europe standard time"),
        Some("Europe/Berlin")
    );

    let summer = date(2024, 7, 1).at(12, 0, 0, 0);
    let windows = Zones::new().resolve("W. Europe Standard Time").unwrap();
    assert_eq!(
        windows.to_timestamp(summer).unwrap(),
        "2024-07-01T10:00:00Z".parse::<Timestamp>().unwrap()
    );
}

#[test]
fn import_keeps_windows_zone_local_times() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:outlook@x\r\n\
DTSTART;TZID=W. Europe Standard Time:20240610T090000\r\n\
DTEND;TZID=\"(UTC+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna\":20240610T100000\r\n\
SUMMARY:Outlook\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let imported_at: Timestamp = "2024-06-01T08:00:00Z".parse().unwrap();
    let prepared = prepare_import(ics, "UTC", imported_at).unwrap();
    let event = &prepared.events[0];

    assert_eq!(
        event.start().unwrap(),
        DateValue::zoned(date(2024, 6, 10).at(9, 0, 0, 0), "Europe/Berlin")
    );
    assert_eq!(
        event.end().unwrap(),
        Some(DateValue::zoned(
            date(2024, 6, 10).at(10, 0, 0, 0),
            "Europe/Amsterdam"
        ))
    );
    assert_eq!(event.stamp().unwrap(), Some(DateValue::utc(imported_at)));
}
