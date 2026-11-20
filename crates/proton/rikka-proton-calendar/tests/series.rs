use jiff::Timestamp;
use jiff::civil::date;
use jiff::tz::TimeZone;
use rikka_proton_calendar::{
    Alarm, AlarmAction, DateValue, Duration, Event, EventStatus, InstanceSource, Notification,
    NotificationKind, Series, Trigger, Zones, alarm_times,
};

const ZONE: &str = "Europe/Zurich";

fn timestamp(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn zoned(text: &str) -> DateValue {
    DateValue::parse(text, Some(ZONE)).unwrap()
}

fn weekly() -> Event {
    Event::new("weekly", zoned("20240101T100000"))
        .with_end(zoned("20240101T110000"))
        .with_rule(&"FREQ=WEEKLY;COUNT=6".parse().unwrap())
        .with_exception(zoned("20240115T100000"))
        .with_summary("Standup")
}

#[test]
fn series_merge_rules_and_overrides() {
    let mut main = weekly();
    main.component_mut()
        .push(zoned("20240103T150000").to_line("RDATE"));
    let moved = Event::new("weekly", zoned("20240108T140000"))
        .with_end(zoned("20240108T143000"))
        .with_recurrence_id(zoned("20240108T100000"))
        .with_summary("Moved standup");
    let cancelled = Event::new("weekly", zoned("20240122T100000"))
        .with_recurrence_id(zoned("20240122T100000"))
        .with_status(EventStatus::Cancelled);
    let overrides = [moved, cancelled];

    let instances = Series::new(&main, &overrides)
        .between(
            timestamp("2024-01-01T00:00:00Z"),
            timestamp("2024-03-01T00:00:00Z"),
            &Zones::new(),
            &TimeZone::UTC,
        )
        .unwrap();
    let found: Vec<(String, InstanceSource)> = instances
        .iter()
        .map(|instance| (instance.local_start.to_string(), instance.source))
        .collect();

    assert_eq!(
        found,
        vec![
            ("20240101T100000".to_owned(), InstanceSource::Series),
            ("20240103T150000".to_owned(), InstanceSource::Series),
            ("20240108T140000".to_owned(), InstanceSource::Override(0)),
            ("20240129T100000".to_owned(), InstanceSource::Series),
            ("20240205T100000".to_owned(), InstanceSource::Series),
        ]
    );
    assert_eq!(instances[0].start, timestamp("2024-01-01T09:00:00Z"));
    assert_eq!(instances[0].end, timestamp("2024-01-01T10:00:00Z"));
    assert_eq!(instances[2].end, timestamp("2024-01-08T13:30:00Z"));
}

#[test]
fn window_keeps_overlapping_instances() {
    let event = weekly();
    let series = Series::single(&event);
    let zones = Zones::new();

    let inside = series
        .between(
            timestamp("2024-01-08T09:30:00Z"),
            timestamp("2024-01-23T00:00:00Z"),
            &zones,
            &TimeZone::UTC,
        )
        .unwrap();
    assert_eq!(
        inside
            .iter()
            .map(|instance| instance.local_start.to_string())
            .collect::<Vec<_>>(),
        vec!["20240108T100000", "20240122T100000"]
    );

    let next = series
        .next_after(timestamp("2024-01-09T00:00:00Z"), &zones, &TimeZone::UTC)
        .unwrap()
        .unwrap();
    assert_eq!(next.local_start.to_string(), "20240122T100000");
    assert!(
        series
            .next_after(timestamp("2024-03-01T00:00:00Z"), &zones, &TimeZone::UTC)
            .unwrap()
            .is_none()
    );
}

#[test]
fn all_day_events_are_floating() {
    let event = Event::new("holiday", DateValue::Date(date(2024, 12, 24)))
        .with_end(DateValue::Date(date(2024, 12, 26)))
        .with_rule(&"FREQ=YEARLY;COUNT=2".parse().unwrap());
    let tokyo = Zones::lookup("Asia/Tokyo").unwrap();
    let instances = Series::single(&event)
        .first(5, &Zones::new(), &tokyo)
        .unwrap();

    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|instance| instance.all_day));
    assert_eq!(instances[0].start, timestamp("2024-12-23T15:00:00Z"));
    assert_eq!(instances[0].end, timestamp("2024-12-25T15:00:00Z"));
    assert_eq!(instances[1].local_start.to_string(), "20251224");
}

#[test]
fn alarms_trigger_relative_to_instance() {
    let event = weekly();
    let instances = Series::single(&event)
        .first(2, &Zones::new(), &TimeZone::UTC)
        .unwrap();

    let before = Alarm::new(
        AlarmAction::Display,
        Trigger::before_start(Duration::minutes(15)),
    );
    assert_eq!(
        alarm_times(&before, &instances[1], &Zones::new(), &TimeZone::UTC).unwrap(),
        vec![timestamp("2024-01-08T08:45:00Z")]
    );

    let day_before: Duration = "-P1D".parse().unwrap();
    let nominal = Alarm::new(AlarmAction::Email, Trigger::before_start(day_before));
    let dst = Event::new("dst", zoned("20240331T100000"));
    let dst_instance = Series::single(&dst)
        .first(1, &Zones::new(), &TimeZone::UTC)
        .unwrap();
    assert_eq!(
        alarm_times(&nominal, &dst_instance[0], &Zones::new(), &TimeZone::UTC).unwrap(),
        vec![timestamp("2024-03-30T09:00:00Z")]
    );

    let ending = Alarm::new(
        AlarmAction::Display,
        Trigger::Relative {
            duration: Duration::minutes(-5),
            from_end: true,
        },
    );
    assert_eq!(
        alarm_times(&ending, &instances[0], &Zones::new(), &TimeZone::UTC).unwrap(),
        vec![timestamp("2024-01-01T09:55:00Z")]
    );
}

#[test]
fn notifications_convert_to_alarms() {
    let start = zoned("20240101T100000");
    let absolute = Alarm::new(
        AlarmAction::Email,
        Trigger::Absolute(DateValue::utc(timestamp("2024-01-01T07:00:00Z"))),
    );
    let notification =
        Notification::from_alarm(&absolute, &start, &Zones::new(), &TimeZone::UTC).unwrap();
    assert_eq!(notification.kind(), NotificationKind::Email);
    assert_eq!(notification.trigger_text(), "-PT2H");

    let mixed = Alarm::new(
        AlarmAction::Display,
        Trigger::before_start("P1DT2H".parse().unwrap()),
    );
    let normalized =
        Notification::from_alarm(&mixed, &start, &Zones::new(), &TimeZone::UTC).unwrap();
    assert_eq!(normalized.trigger_text(), "-PT26H");

    let alarm = Notification::new(NotificationKind::Device, Duration::minutes(30))
        .to_alarm()
        .unwrap();
    assert_eq!(alarm.action(), AlarmAction::Display);
    assert_eq!(
        alarm.trigger().unwrap(),
        Trigger::before_start(Duration::minutes(30))
    );
}
