use std::time::{Duration, Instant};

use rikka_rate_control::{Pacer, ProbeCluster};

#[test]
fn video_leaves_at_the_pacing_rate() {
    let start = Instant::now();
    let mut pacer = Pacer::new(8_000_000);
    for _ in 0..100 {
        pacer.push_video(0, vec![0; 1000]);
    }
    assert!(pacer.queued_video() >= Duration::from_millis(99));
    let mut now = start;
    let mut sent = 0;
    while sent < 100 {
        while pacer.pop(now).is_some() {
            sent += 1;
        }
        now = pacer
            .wake_at(now)
            .unwrap_or(now)
            .max(now + Duration::from_micros(100));
    }
    let took = now.duration_since(start);
    assert!(
        (Duration::from_millis(90)..Duration::from_millis(110)).contains(&took),
        "100 kB at 8 Mbit/s took {took:?}"
    );
}

#[test]
fn urgent_datagrams_jump_the_queue() {
    let now = Instant::now();
    let mut pacer = Pacer::new(1_000_000);
    for _ in 0..10 {
        pacer.push_video(1, vec![1; 1200]);
    }
    while pacer.pop(now).is_some() {}
    pacer.push_urgent(vec![2; 100]);
    assert_eq!(pacer.wake_at(now), Some(now));
    assert_eq!(pacer.pop(now).map(|out| out.packet[0]), Some(2));
}

#[test]
fn withdrawn_frames_leave_the_queue() {
    let now = Instant::now();
    let mut pacer = Pacer::new(1_000_000);
    for frame in [4, 4, 5, 4, 6] {
        pacer.push_video(frame, vec![frame as u8; 1000]);
    }
    assert_eq!(pacer.withdraw(4), 3);
    assert!(!pacer.holds_frame(4));
    assert_eq!(pacer.queued_video(), Duration::from_millis(16));
    let mut sent = Vec::new();
    let mut at = now;
    while sent.len() < 2 {
        if let Some(out) = pacer.pop(at) {
            sent.push(out.packet[0]);
        }
        at += Duration::from_millis(1);
    }
    assert_eq!(sent, vec![5, 6]);
}

#[test]
fn probes_go_out_as_padding_at_their_own_rate() {
    let start = Instant::now();
    let mut pacer = Pacer::new(1_000_000);
    pacer.probe(
        ProbeCluster {
            id: 4,
            rate_bps: 9_600_000,
            packets: 5,
        },
        vec![3; 1200],
        start,
    );
    let mut now = start;
    let mut times = Vec::new();
    while let Some(at) = pacer.wake_at(now) {
        now = at;
        let out = pacer.pop(now).unwrap();
        assert_eq!(out.probe, Some(4));
        assert_eq!(out.packet, vec![3; 1200]);
        times.push(now);
    }
    assert_eq!(times.len(), 5);
    assert_eq!(times[4].duration_since(times[0]), Duration::from_millis(4));
}
