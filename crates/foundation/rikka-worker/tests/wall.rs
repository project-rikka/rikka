use std::os::fd::AsFd as _;
use std::time::{Duration, SystemTime};

use rikka_worker::{Expiry, WallAlarm, event_fd};

const PATIENCE: Duration = Duration::from_secs(5);

#[test]
fn wall_alarm_fires_after_deadline() {
    let now = SystemTime::now();
    for due in [
        now + Duration::from_millis(30),
        now - Duration::from_secs(60),
    ] {
        let alarm = WallAlarm::new().expect("a wall alarm");
        let (_notifier, events) = event_fd().expect("an eventfd");
        alarm.arm(due).expect("an armed alarm");

        let [fired] = events
            .poll_readable([alarm.as_fd()], PATIENCE)
            .expect("a wait");
        assert!(fired, "{due:?}");
        assert_eq!(alarm.read().expect("a read"), Some(Expiry::Elapsed));
        assert_eq!(alarm.read().expect("a second read"), None);
    }
}

#[test]
fn disarmed_wall_alarm_never_fires() {
    let alarm = WallAlarm::new().expect("a wall alarm");
    let (_notifier, events) = event_fd().expect("an eventfd");
    alarm
        .arm(SystemTime::now() + Duration::from_millis(10))
        .expect("an armed alarm");
    alarm.disarm().expect("a disarmed alarm");

    let [fired] = events
        .poll_readable([alarm.as_fd()], Duration::from_millis(60))
        .expect("a wait");
    assert!(!fired);
    assert_eq!(alarm.read().expect("an empty read"), None);
}
