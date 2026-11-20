use std::time::{Duration, Instant};

use rikka_rate_control::{Limits, LossBased};

const RTT: Duration = Duration::from_millis(50);

fn controller(start_bps: u64) -> LossBased {
    LossBased::new(Limits {
        lowest_bps: 300_000,
        start_bps,
        highest_bps: 10_000_000,
    })
}

#[test]
fn reported_loss_backs_off_once_per_pause() {
    let start = Instant::now();
    let mut loss = controller(8_000_000);
    let rates: Vec<u64> = (0..6)
        .map(|step| loss.report(0.2, start + Duration::from_millis(100) * step, RTT))
        .collect();
    assert_eq!(
        rates,
        vec![
            7_200_000, 7_200_000, 7_200_000, 7_200_000, 6_480_000, 6_480_000
        ]
    );
    assert_eq!(loss.loss(), 0.2);
}

#[test]
fn quiet_reports_grow_within_receiver_estimate() {
    let start = Instant::now();
    let mut loss = controller(2_000_000);
    loss.report(0.0, start, RTT);
    let grown = loss.report(0.01, start + Duration::from_secs(1), RTT);
    assert_eq!(grown, 2_160_000);
    loss.set_receiver_estimate(1_000_000);
    assert_eq!(loss.rate_bps(), 1_000_000);
    assert_eq!(loss.highest_bps(), 1_000_000);
    let capped = loss.report(0.0, start + Duration::from_secs(2), RTT);
    assert_eq!(capped, 1_000_000);
    loss.set_receiver_estimate(100_000);
    assert_eq!(loss.rate_bps(), 300_000);
}
