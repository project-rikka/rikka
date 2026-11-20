use jiff::tz::TimeZone;
use rikka_proton_calendar::{
    DateValue, Event, Frequency, Rule, RuleEnd, Series, WeekdayNum, Zones,
};

const ZONE: &str = "America/New_York";

fn expanded(start: &str, rule: &str, exdates: &[&str], limit: usize) -> Vec<String> {
    let start = DateValue::parse(start, Some(ZONE)).unwrap();
    let mut event = Event::new("rfc-example", start).with_rule(&rule.parse().unwrap());
    for exdate in exdates {
        event = event.with_exception(DateValue::parse(exdate, Some(ZONE)).unwrap());
    }
    Series::single(&event)
        .first(limit, &Zones::new(), &TimeZone::UTC)
        .unwrap()
        .into_iter()
        .map(|instance| instance.local_start.to_string())
        .collect()
}

fn at_nine(days: &[&str]) -> Vec<String> {
    days.iter().map(|day| format!("{day}T090000")).collect()
}

type Example = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Vec<String>,
);

#[test]
fn rfc_5545_recurrence_examples() {
    let cases: Vec<Example> = vec![
        (
            "daily for 10 occurrences",
            "19970902T090000",
            "FREQ=DAILY;COUNT=10",
            &[],
            at_nine(&[
                "19970902", "19970903", "19970904", "19970905", "19970906", "19970907", "19970908",
                "19970909", "19970910", "19970911",
            ]),
        ),
        (
            "every 10 days, 5 occurrences",
            "19970902T090000",
            "FREQ=DAILY;INTERVAL=10;COUNT=5",
            &[],
            at_nine(&["19970902", "19970912", "19970922", "19971002", "19971012"]),
        ),
        (
            "weekly for 10 occurrences",
            "19970902T090000",
            "FREQ=WEEKLY;COUNT=10",
            &[],
            at_nine(&[
                "19970902", "19970909", "19970916", "19970923", "19970930", "19971007", "19971014",
                "19971021", "19971028", "19971104",
            ]),
        ),
        (
            "weekly on Tuesday and Thursday for five weeks",
            "19970902T090000",
            "FREQ=WEEKLY;UNTIL=19971007T000000Z;WKST=SU;BYDAY=TU,TH",
            &[],
            at_nine(&[
                "19970902", "19970904", "19970909", "19970911", "19970916", "19970918", "19970923",
                "19970925", "19970930", "19971002",
            ]),
        ),
        (
            "every other week on Monday, Wednesday and Friday",
            "19970901T090000",
            "FREQ=WEEKLY;INTERVAL=2;UNTIL=19971224T000000Z;WKST=SU;BYDAY=MO,WE,FR",
            &[],
            at_nine(&[
                "19970901", "19970903", "19970905", "19970915", "19970917", "19970919", "19970929",
                "19971001", "19971003", "19971013", "19971015", "19971017", "19971027", "19971029",
                "19971031", "19971110", "19971112", "19971114", "19971124", "19971126", "19971128",
                "19971208", "19971210", "19971212", "19971222",
            ]),
        ),
        (
            "monthly on the first Friday for 10 occurrences",
            "19970905T090000",
            "FREQ=MONTHLY;COUNT=10;BYDAY=1FR",
            &[],
            at_nine(&[
                "19970905", "19971003", "19971107", "19971205", "19980102", "19980206", "19980306",
                "19980403", "19980501", "19980605",
            ]),
        ),
        (
            "monthly on the second-to-last Monday for 6 months",
            "19970922T090000",
            "FREQ=MONTHLY;COUNT=6;BYDAY=-2MO",
            &[],
            at_nine(&[
                "19970922", "19971020", "19971117", "19971222", "19980119", "19980216",
            ]),
        ),
        (
            "monthly on the third-to-the-last day",
            "19970928T090000",
            "FREQ=MONTHLY;BYMONTHDAY=-3",
            &[],
            at_nine(&[
                "19970928", "19971029", "19971128", "19971229", "19980129", "19980226",
            ]),
        ),
        (
            "monthly on the 2nd and 15th for 10 occurrences",
            "19970902T090000",
            "FREQ=MONTHLY;COUNT=10;BYMONTHDAY=2,15",
            &[],
            at_nine(&[
                "19970902", "19970915", "19971002", "19971015", "19971102", "19971115", "19971202",
                "19971215", "19980102", "19980115",
            ]),
        ),
        (
            "yearly in June and July for 10 occurrences",
            "19970610T090000",
            "FREQ=YEARLY;COUNT=10;BYMONTH=6,7",
            &[],
            at_nine(&[
                "19970610", "19970710", "19980610", "19980710", "19990610", "19990710", "20000610",
                "20000710", "20010610", "20010710",
            ]),
        ),
        (
            "every third year on the 1st, 100th and 200th day",
            "19970101T090000",
            "FREQ=YEARLY;INTERVAL=3;COUNT=10;BYYEARDAY=1,100,200",
            &[],
            at_nine(&[
                "19970101", "19970410", "19970719", "20000101", "20000409", "20000718", "20030101",
                "20030410", "20030719", "20060101",
            ]),
        ),
        (
            "every Thursday in March",
            "19970313T090000",
            "FREQ=YEARLY;BYMONTH=3;BYDAY=TH",
            &[],
            at_nine(&[
                "19970313", "19970320", "19970327", "19980305", "19980312", "19980319", "19980326",
                "19990304", "19990311", "19990318", "19990325",
            ]),
        ),
        (
            "Monday of week number 20",
            "19970512T090000",
            "FREQ=YEARLY;BYWEEKNO=20;BYDAY=MO",
            &[],
            at_nine(&["19970512", "19980511", "19990517"]),
        ),
        (
            "every Friday the 13th",
            "19970902T090000",
            "FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13",
            &["19970902T090000"],
            at_nine(&["19980213", "19980313", "19981113", "19990813", "20001013"]),
        ),
        (
            "the first Saturday that follows the first Sunday",
            "19970913T090000",
            "FREQ=MONTHLY;BYDAY=SA;BYMONTHDAY=7,8,9,10,11,12,13",
            &[],
            at_nine(&[
                "19970913", "19971011", "19971108", "19971213", "19980110", "19980207",
            ]),
        ),
        (
            "United States presidential election day",
            "19961105T090000",
            "FREQ=YEARLY;INTERVAL=4;BYMONTH=11;BYDAY=TU;BYMONTHDAY=2,3,4,5,6,7,8",
            &[],
            at_nine(&["19961105", "20001107", "20041102"]),
        ),
        (
            "the third Tuesday, Wednesday or Thursday for 3 months",
            "19970904T090000",
            "FREQ=MONTHLY;COUNT=3;BYDAY=TU,WE,TH;BYSETPOS=3",
            &[],
            at_nine(&["19970904", "19971007", "19971106"]),
        ),
        (
            "the second-to-last weekday of the month",
            "19970929T090000",
            "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-2",
            &[],
            at_nine(&[
                "19970929", "19971030", "19971127", "19971230", "19980129", "19980226", "19980330",
            ]),
        ),
        (
            "every 15 minutes for 6 occurrences",
            "19970902T090000",
            "FREQ=MINUTELY;INTERVAL=15;COUNT=6",
            &[],
            ["090000", "091500", "093000", "094500", "100000", "101500"]
                .iter()
                .map(|time| format!("19970902T{time}"))
                .collect(),
        ),
        (
            "every hour and a half for 4 occurrences",
            "19970902T090000",
            "FREQ=MINUTELY;INTERVAL=90;COUNT=4",
            &[],
            ["090000", "103000", "120000", "133000"]
                .iter()
                .map(|time| format!("19970902T{time}"))
                .collect(),
        ),
        (
            "weekly with WKST=MO",
            "19970805T090000",
            "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=MO",
            &[],
            at_nine(&["19970805", "19970810", "19970819", "19970824"]),
        ),
        (
            "weekly with WKST=SU",
            "19970805T090000",
            "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=SU",
            &[],
            at_nine(&["19970805", "19970817", "19970819", "19970831"]),
        ),
        (
            "invalid dates are skipped",
            "20070115T090000",
            "FREQ=MONTHLY;BYMONTHDAY=15,30;COUNT=5",
            &[],
            at_nine(&["20070115", "20070130", "20070215", "20070315", "20070330"]),
        ),
    ];

    for (name, start, rule, exdates, expected) in cases {
        let bounded = rule.contains("COUNT") || rule.contains("UNTIL");
        let limit = if bounded {
            expected.len() + 5
        } else {
            expected.len()
        };
        let found = expanded(start, rule, exdates, limit);
        assert_eq!(found, expected, "{name}");
    }
}

#[test]
fn every_20_minutes_matches_daily_rule() {
    let daily = expanded(
        "19970902T090000",
        "FREQ=DAILY;BYHOUR=9,10,11,12,13,14,15,16;BYMINUTE=0,20,40",
        &[],
        60,
    );
    let minutely = expanded(
        "19970902T090000",
        "FREQ=MINUTELY;INTERVAL=20;BYHOUR=9,10,11,12,13,14,15,16",
        &[],
        60,
    );
    assert_eq!(daily, minutely);
    assert_eq!(daily[23], "19970902T164000");
    assert_eq!(daily[24], "19970903T090000");
}

#[test]
fn daily_rule_follows_dst_offset() {
    let start = DateValue::parse("19971024T090000", Some(ZONE)).unwrap();
    let event = Event::new("dst", start).with_rule(&"FREQ=DAILY;COUNT=4".parse().unwrap());
    let instants: Vec<String> = Series::single(&event)
        .first(4, &Zones::new(), &TimeZone::UTC)
        .unwrap()
        .into_iter()
        .map(|instance| instance.start.to_string())
        .collect();
    assert_eq!(
        instants,
        vec![
            "1997-10-24T13:00:00Z",
            "1997-10-25T13:00:00Z",
            "1997-10-26T14:00:00Z",
            "1997-10-27T14:00:00Z",
        ]
    );
}

#[test]
fn impossible_rules_terminate() {
    let found = expanded(
        "20070101T090000",
        "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=30",
        &[],
        5,
    );
    assert_eq!(found, vec!["20070101T090000"]);
}

#[test]
fn rules_round_trip() {
    let cases = [
        "FREQ=DAILY;COUNT=10",
        "FREQ=WEEKLY;UNTIL=19971007T000000Z;BYDAY=TU,TH;WKST=SU",
        "FREQ=MONTHLY;INTERVAL=2;BYDAY=1SU,-1SU",
        "FREQ=YEARLY;BYMONTHDAY=-1,15;BYMONTH=1,2,3",
        "FREQ=YEARLY;BYDAY=MO;BYWEEKNO=20",
        "FREQ=HOURLY;BYSECOND=0,30;BYMINUTE=5;BYHOUR=9",
        "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-2",
        "FREQ=YEARLY;BYYEARDAY=1,100,-1;X-NAME=VALUE",
    ];
    for text in cases {
        let rule: Rule = text.parse().unwrap();
        assert_eq!(rule.to_string(), text);
    }

    let rule: Rule = "freq=monthly;byday=+2we;count=3".parse().unwrap();
    assert_eq!(rule.frequency, Frequency::Monthly);
    assert_eq!(rule.end, RuleEnd::Count(3));
    assert_eq!(
        rule.by_day,
        vec![WeekdayNum::nth(2, jiff::civil::Weekday::Wednesday)]
    );

    for invalid in [
        "COUNT=3",
        "FREQ=SOMETIMES",
        "FREQ=DAILY;COUNT=2;UNTIL=20200101T000000Z",
        "FREQ=DAILY;BYHOUR=24",
        "FREQ=MONTHLY;BYMONTHDAY=0",
        "FREQ=WEEKLY;BYDAY=XX",
        "FREQ=DAILY;INTERVAL=0",
    ] {
        assert!(invalid.parse::<Rule>().is_err(), "{invalid}");
    }
}

#[test]
fn distant_windows_skip_iteration() {
    let start = DateValue::parse("19700101T000000", Some(ZONE)).unwrap();
    let event =
        Event::new("minutely", start).with_rule(&"FREQ=MINUTELY;INTERVAL=7".parse().unwrap());
    let from: jiff::Timestamp = "2024-01-17T00:00:00Z".parse().unwrap();
    let to: jiff::Timestamp = "2024-01-18T00:00:00Z".parse().unwrap();

    let clock = std::time::Instant::now();
    let instances = Series::single(&event)
        .between(from, to, &Zones::new(), &TimeZone::UTC)
        .unwrap();
    assert!(
        clock.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        clock.elapsed()
    );

    assert!(
        (200..=210).contains(&instances.len()),
        "{}",
        instances.len()
    );
    let first = &instances[0];
    assert!(first.start >= from);
    let minutes = (first.start.as_second()
        - "1970-01-01T05:00:00Z"
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_second())
        / 60;
    assert_eq!(minutes % 7, 0);
    assert!(
        instances
            .windows(2)
            .all(|pair| pair[1].start.as_second() - pair[0].start.as_second() == 420)
    );
}

#[test]
fn seek_matches_iteration() {
    let cases = [
        ("19900102T093000", "FREQ=WEEKLY;INTERVAL=3;BYDAY=TU,FR"),
        (
            "19850131T080000",
            "FREQ=MONTHLY;BYMONTHDAY=-1,15;BYHOUR=8,20",
        ),
        ("19960229T120000", "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29"),
        ("19971225T000000", "FREQ=DAILY;INTERVAL=11"),
        ("20000105T060000", "FREQ=HOURLY;INTERVAL=13;BYDAY=MO,WE"),
        (
            "19900101T000000",
            "FREQ=YEARLY;BYWEEKNO=1,-1;BYDAY=MO;UNTIL=20400101T000000Z",
        ),
    ];
    let from: jiff::Timestamp = "2023-12-20T00:00:00Z".parse().unwrap();
    let to: jiff::Timestamp = "2024-03-05T00:00:00Z".parse().unwrap();

    for (start, rule) in cases {
        let start = DateValue::parse(start, Some(ZONE)).unwrap();
        let event = Event::new("seek", start).with_rule(&rule.parse().unwrap());
        let series = Series::single(&event);
        let windowed = series
            .between(from, to, &Zones::new(), &TimeZone::UTC)
            .unwrap();
        let walked: Vec<_> = series
            .first(40_000, &Zones::new(), &TimeZone::UTC)
            .unwrap()
            .into_iter()
            .filter(|instance| instance.overlaps(from, to))
            .collect();
        assert!(!walked.is_empty(), "{rule}");
        assert_eq!(windowed, walked, "{rule}");
    }
}

#[test]
fn vtimezones_resolve_distant_dates() {
    let calendar = rikka_proton_calendar::ICalendar::parse(&format!(
        "BEGIN:VCALENDAR\r\n{}END:VCALENDAR\r\n",
        crate::ical::NEW_YORK_VTIMEZONE
    ))
    .unwrap();
    let zones = calendar.zones().unwrap();
    let custom = zones.resolve("Eastern Custom").unwrap();
    let named = Zones::new().resolve(ZONE).unwrap();

    let clock = std::time::Instant::now();
    for year in [2030, 2100, 2400, 3000] {
        let summer = jiff::civil::date(year, 7, 4).at(12, 0, 0, 0);
        let winter = jiff::civil::date(year, 1, 4).at(12, 0, 0, 0);
        for local in [summer, winter] {
            assert_eq!(
                custom.to_timestamp(local).unwrap(),
                named.to_timestamp(local).unwrap(),
                "{local}"
            );
        }
    }
    assert!(
        clock.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        clock.elapsed()
    );
}
