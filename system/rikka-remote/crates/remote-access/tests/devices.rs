use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use remote_access::{Devices, Throttle};

const DAY: u64 = 24 * 60 * 60;

#[test]
fn throttle_backs_off_after_free_attempts() {
    let mut throttle = Throttle::default();
    let now = Instant::now();
    for _ in 0..2 {
        throttle.failed(now);
        assert_eq!(throttle.wait(now), None);
    }
    let expected = [1, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300];
    for seconds in expected {
        throttle.failed(now);
        assert_eq!(throttle.wait(now), Some(Duration::from_secs(seconds)));
    }
    assert_eq!(throttle.wait(now + Duration::from_secs(300)), None);
    throttle.succeeded();
    throttle.failed(now);
    assert_eq!(throttle.wait(now), None);
}

#[test]
fn remembered_devices_survive_a_restart_and_can_be_forgotten() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote/devices");
    let mut devices = Devices::default();
    let laptop = devices.remember("laptop", 1000).unwrap();
    let phone = devices.remember("phone\nforged 0 0 entry", 1000).unwrap();
    assert_ne!(laptop, phone);
    devices.save(&path).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let stored = std::fs::read_to_string(&path).unwrap();
    assert!(!stored.contains(&format!("{:02x}", laptop.as_bytes()[0]).repeat(32)));
    assert_eq!(stored.lines().count(), 2);

    let mut loaded = Devices::load(&path).unwrap();
    assert_eq!(
        loaded.recall(laptop.as_bytes(), 2000).as_deref(),
        Some("laptop")
    );
    assert_eq!(loaded.recall(&[0; 32], 2000), None);
    assert_eq!(loaded.recall(&laptop.as_bytes()[..31], 2000), None);
    assert!(loaded.forget(phone.as_bytes()));
    assert_eq!(loaded.recall(phone.as_bytes(), 2000), None);
    assert!(
        Devices::load(&folder.path().join("missing"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn devices_expire_unused_and_evict_the_least_recent() {
    let mut devices = Devices::default();
    let idle = devices.remember("idle", 0).unwrap();
    let busy = devices.remember("busy", 0).unwrap();
    assert!(devices.recall(busy.as_bytes(), 80 * DAY).is_some());
    assert_eq!(devices.recall(idle.as_bytes(), 91 * DAY), None);
    assert!(devices.recall(busy.as_bytes(), 91 * DAY).is_some());

    let mut crowded = Devices::default();
    let oldest = crowded.remember("first", 1).unwrap();
    let rest: Vec<_> = (0..32)
        .map(|index| crowded.remember("next", 2 + index).unwrap())
        .collect();
    assert_eq!(crowded.len(), 32);
    assert_eq!(crowded.recall(oldest.as_bytes(), 100), None);
    assert!(
        rest.iter()
            .all(|token| crowded.recall(token.as_bytes(), 100).is_some())
    );
}
