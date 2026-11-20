use std::time::{Duration, Instant};

use rikka_compositor_restarts::{Restart, Restarts};

fn after(millis: u64, crashes: usize) -> Restart {
    Restart::After {
        delay: Duration::from_millis(millis),
        crashes,
    }
}

#[test]
fn crashes_back_off_then_give_up_unless_guarding() {
    let start = Instant::now();
    let mut restarts = Restarts::default();
    let delays: Vec<Restart> = (0..4)
        .map(|crash| restarts.crashed(start + Duration::from_secs(crash), false))
        .collect();
    assert_eq!(
        delays,
        [
            after(500, 1),
            after(1000, 2),
            after(2000, 3),
            after(4000, 4)
        ]
    );
    assert_eq!(
        restarts.crashed(start + Duration::from_secs(4), false),
        Restart::GiveUp
    );
    assert_eq!(
        restarts.crashed(start + Duration::from_secs(5), true),
        after(8000, 6)
    );
}

#[test]
fn old_crashes_are_forgotten() {
    let start = Instant::now();
    let mut restarts = Restarts::default();
    for crash in 0..4 {
        restarts.crashed(start + Duration::from_secs(crash), false);
    }
    assert_eq!(
        restarts.crashed(start + Duration::from_secs(70), false),
        after(500, 1)
    );
    restarts.forget();
    assert_eq!(
        restarts.crashed(start + Duration::from_secs(71), false),
        after(500, 1)
    );
}
