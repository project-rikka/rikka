use std::time::{Duration, Instant};

use crate::common::{PATIENCE, Server, Wayland};
use rikka_compositor_seat::SeatEvent;
use rikka_wayland_client::zwp_idle_inhibit_manager_v1;
use rikka_wayland_client::{ext_idle_notification_v1, ext_idle_notifier_v1};

const INHIBITED_TIMEOUT_MS: u32 = 20;

const INPUT_IDLE_TIMEOUT_MS: u32 = 60;

fn wait_for(client: &mut Wayland, count: usize) {
    let deadline = Instant::now() + PATIENCE;
    while client.received.idle.len() < count {
        assert!(
            client.wait_readable(deadline),
            "saw only {:?}",
            client.received.idle
        );
        client.roundtrip();
    }
}

#[test]
fn inhibitors_skip_input_idle_notifications() {
    let server = Server::start();
    let mut client = server.connect();
    let seat = client.seat(0);
    let target = client.mapped(10, 10);
    let inhibitors: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1 = client.global(1);
    let inhibitor = inhibitors.create_inhibitor(&target.surface);
    client.roundtrip();
    let id = target.id;
    let listed = server.call(move |state| {
        let surface = state.surface(id);
        let listed = state.seats.idle_inhibitors().collect::<Vec<_>>() == vec![surface];
        rikka_compositor_seat::set_idle_inhibited(state, true);
        listed && state.events.contains(&SeatEvent::IdleInhibitors)
    });
    assert!(listed);

    let notifier: ext_idle_notifier_v1::ExtIdleNotifierV1 = client.global(2);
    let requested = Instant::now();
    let _inhibited = notifier.get_idle_notification(INHIBITED_TIMEOUT_MS, &seat);
    let _input = notifier.get_input_idle_notification(INPUT_IDLE_TIMEOUT_MS, &seat);
    client.roundtrip();
    wait_for(&mut client, 1);
    assert!(requested.elapsed() >= Duration::from_millis(u64::from(INPUT_IDLE_TIMEOUT_MS)));
    assert_eq!(client.received.idle.len(), 1);

    inhibitor.destroy();
    client.roundtrip();
    server.call(|state| rikka_compositor_seat::set_idle_inhibited(state, false));
    wait_for(&mut client, 2);
    assert!(
        client
            .received
            .idle
            .iter()
            .all(|event| matches!(event, ext_idle_notification_v1::Event::Idled))
    );
}
