use std::sync::mpsc::{RecvTimeoutError, channel};

use rikka_nix::{Event, Interest, Monitor};
use rikka_test_bus::{PrivateBus, TIMEOUT};

#[test]
fn watch_ends_with_its_monitor_and_reports_a_lost_bus() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    bus.stand_in_for_system_bus();

    let (heard, events) = channel();
    let monitor = Monitor::watch(Interest::Progress, move |event| {
        heard.send(event).ok();
    })
    .expect("it watches");
    assert_eq!(monitor.running().expect("it asks the bus"), None);
    drop(monitor);
    assert_eq!(
        events.recv_timeout(TIMEOUT),
        Err(RecvTimeoutError::Disconnected)
    );

    let (heard, events) = channel();
    let _monitor = Monitor::watch(Interest::Progress, move |event| {
        heard.send(event).ok();
    })
    .expect("it watches");
    drop(bus);
    assert_eq!(events.recv_timeout(TIMEOUT), Ok(Event::Disconnected));
}
