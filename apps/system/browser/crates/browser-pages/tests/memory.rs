use std::time::Duration;

use browser_pages::{TabActivity, discardable_tabs, is_exempt, is_pressed};
use browser_store::{Store, TabId, Timestamp};

const MINUTE: Duration = Duration::from_secs(60);

fn idle(tab: TabId, now: Timestamp, minutes: u32) -> TabActivity {
    TabActivity {
        tab,
        live: true,
        seen: false,
        last_active: now.saturating_sub(MINUTE * minutes),
        playing: false,
        asking: false,
        exempt: false,
    }
}

#[test]
fn only_idle_hidden_quiet_tabs_are_discarded_oldest_first() {
    let store = Store::in_memory().expect("an in-memory store opens");
    let tabs: Vec<TabId> = (0..8).map(|_| store.new_tab_id()).collect();
    let now = Timestamp::now();
    let activities = [
        idle(tabs[0], now, 40),
        idle(tabs[1], now, 90),
        idle(tabs[2], now, 5),
        TabActivity {
            live: false,
            ..idle(tabs[3], now, 90)
        },
        TabActivity {
            seen: true,
            ..idle(tabs[4], now, 90)
        },
        TabActivity {
            playing: true,
            ..idle(tabs[5], now, 90)
        },
        TabActivity {
            asking: true,
            ..idle(tabs[6], now, 90)
        },
        TabActivity {
            exempt: true,
            ..idle(tabs[7], now, 90)
        },
    ];
    assert_eq!(
        discardable_tabs(&activities, now, MINUTE * 30),
        [tabs[1], tabs[0]]
    );
    assert_eq!(
        discardable_tabs(&activities, now, MINUTE * 2),
        [tabs[1], tabs[0], tabs[2]]
    );
}

#[test]
fn exemptions_cover_listed_sites_and_their_subdomains() {
    let never = ["Docs.Example.com.".to_owned(), "mail.test".to_owned()];
    let cases = [
        (Some("docs.example.com"), true),
        (Some("a.docs.example.com"), true),
        (Some("example.com"), false),
        (Some("notmail.test"), false),
        (None, false),
    ];
    for (host, expected) in cases {
        assert_eq!(is_exempt(host, &never), expected, "{host:?}");
    }
}

#[test]
fn memory_pressure_follows_the_ten_second_averages() {
    let cases = [
        (
            "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
            false,
        ),
        (
            "some avg10=41.50 avg60=1.00 avg300=0.10 total=9\nfull avg10=2.00 avg60=0.00 avg300=0.00 total=1\n",
            true,
        ),
        (
            "some avg10=12.00 avg60=80.00 avg300=90.00 total=9\nfull avg10=10.00 avg60=0.00 avg300=0.00 total=1\n",
            true,
        ),
        (
            "some avg10=39.99 avg60=99.00 avg300=99.00 total=9\nfull avg10=9.99 avg60=99.00 avg300=99.00 total=1\n",
            false,
        ),
        ("", false),
        ("some avg10=many\n", false),
    ];
    for (pressure, expected) in cases {
        assert_eq!(is_pressed(pressure), expected, "{pressure}");
    }
}
