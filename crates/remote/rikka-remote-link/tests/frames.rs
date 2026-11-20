use std::time::{Duration, Instant};

use rikka_rate_control::{History, Sent};
use rikka_remote_link::{
    Assembler, Chain, FeedbackBuilder, FrameReport, Packetizer, REORDER_GRACE, SHARD_SILENCE,
    VideoFrame, acknowledgement,
};
use rikka_remote_wire::{FrameFlags, ServerDatagram};

const DATAGRAM: usize = 1200;

fn frame(number: u32, reference: u32, bytes: usize) -> VideoFrame {
    VideoFrame {
        stream: 1,
        frame: number,
        reference,
        flags: FrameFlags {
            intra: number == reference,
            refinement: false,
        },
        capture_us: number * 16_667,
        data: (0..bytes)
            .map(|index| (index * 7 + number as usize) as u8)
            .collect(),
    }
}

fn shards(frame: &VideoFrame, protection: f64) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    Packetizer::new()
        .packetize(frame, DATAGRAM, protection, &mut out)
        .unwrap();
    out
}

fn push(assembler: &mut Assembler, datagram: &[u8], now: Instant) -> Option<VideoFrame> {
    let ServerDatagram::Video(shard) = ServerDatagram::decode(datagram).unwrap() else {
        panic!("the packetizer made something other than a video shard");
    };
    assembler.push(&shard, now)
}

#[test]
fn frames_survive_losses_within_their_protection() {
    let now = Instant::now();
    for bytes in [1, 900, 1162, 50_000, 400_000] {
        let original = frame(0, 0, bytes);
        let datagrams = shards(&original, 0.25);
        assert!(datagrams.iter().all(|datagram| datagram.len() <= DATAGRAM));
        let mut assembler = Assembler::new();
        let mut restored = None;
        for (index, datagram) in datagrams.iter().enumerate() {
            if index % 5 == 1 {
                continue;
            }
            if let Some(done) = push(&mut assembler, datagram, now) {
                restored = Some(done);
            }
        }
        assert_eq!(restored.as_ref(), Some(&original), "{bytes} bytes");
    }
}

#[test]
fn unrecoverable_frames_are_reported_lost() {
    let start = Instant::now();
    let mut assembler = Assembler::new();
    let first = shards(&frame(0, 0, 20_000), 0.1);
    for datagram in first.iter().skip(5) {
        assert_eq!(push(&mut assembler, datagram, start), None);
    }
    for datagram in &shards(&frame(2, 1, 3000), 0.1) {
        push(&mut assembler, datagram, start);
    }
    assert_eq!(assembler.expire(start), Vec::<u32>::new());
    assert_eq!(assembler.expire(start + REORDER_GRACE), vec![0, 1]);
    assert_eq!(assembler.next_deadline(), None);

    let mut silent = Assembler::new();
    for datagram in shards(&frame(5, 5, 20_000), 0.0).iter().skip(1) {
        push(&mut silent, datagram, start);
    }
    assert_eq!(silent.expire(start + SHARD_SILENCE), vec![5]);
}

#[test]
fn only_frames_with_a_decoded_reference_are_decodable() {
    let mut chain = Chain::new();
    let intra = frame(0, 0, 10);
    assert!(chain.decodable(&intra));
    chain.decoded(&intra);
    assert!(chain.decodable(&frame(1, 0, 10)));
    assert!(!chain.decodable(&frame(3, 2, 10)));
    chain.decoded(&frame(1, 0, 10));
    assert_eq!(chain.newest_good(), Some(1));
    let newer_stream = VideoFrame {
        stream: 2,
        ..frame(4, 1, 10)
    };
    assert!(!chain.decodable(&newer_stream));
    assert_eq!(chain.newest_good(), None);
}

#[test]
fn a_failed_frame_and_everything_after_it_stop_counting_as_decoded() {
    let mut chain = Chain::new();
    for (number, reference) in [(0, 0), (1, 0), (2, 1), (3, 2)] {
        let decoded = frame(number, reference, 10);
        assert!(chain.decodable(&decoded));
        chain.decoded(&decoded);
    }
    chain.retract(1, 2);
    assert_eq!(chain.newest_good(), Some(1));
    assert!(!chain.decodable(&frame(4, 3, 10)));
    assert!(chain.decodable(&frame(4, 1, 10)));
    chain.retract(9, 0);
    assert_eq!(chain.newest_good(), Some(1));
}

#[test]
fn feedback_reports_every_sent_datagram() {
    let start = Instant::now();
    let mut history = History::new();
    let mut builder = FeedbackBuilder::new(start);
    for seq in 0..2000_u32 {
        let at = start + Duration::from_micros(u64::from(seq) * 100);
        history.sent(Sent {
            seq,
            at,
            bytes: 1200,
            probe: None,
        });
        if seq % 10 != 3 {
            builder.received(seq, at + Duration::from_millis(20));
        }
    }
    builder.lost_frame(2, 7);
    builder.lost_frame(1, 6);
    let feedbacks = builder.build(
        FrameReport {
            stream: 2,
            good: Some(9),
        },
        1100,
    );
    assert!(feedbacks.len() > 1);
    let mut results = Vec::new();
    for feedback in &feedbacks {
        assert_eq!((feedback.stream, feedback.good), (2, Some(9)));
        assert_eq!(feedback.lost, vec![7]);
        results.extend(history.acknowledge(acknowledgement(feedback)));
    }
    assert_eq!(results.len(), 2000);
    for (seq, result) in results.iter().enumerate() {
        assert_eq!(result.sent.seq, seq as u32);
        assert_eq!(result.arrival_us.is_none(), seq % 10 == 3, "seq {seq}");
    }
    let delays: Vec<i64> = results
        .iter()
        .filter_map(|result| {
            let sent = result.sent.at.duration_since(start).as_micros() as i64;
            result.arrival_us.map(|arrival| arrival - sent)
        })
        .collect();
    assert!(delays.iter().all(|delay| (delay - 20_000).abs() <= 25));
}
