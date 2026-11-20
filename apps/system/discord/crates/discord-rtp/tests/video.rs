use std::time::{Duration, Instant};

use discord_core::VideoCodec;
use discord_rtp::{
    Arrived, Assembled, Header, Payload, Plain, VideoReceiver, VideoSender, nack, packetize_h264,
    picture_loss,
};

const SOURCE: u32 = 0x5000;
const REPAIR: u32 = 0x5001;
const ME: u32 = 7;
const ROUND_TRIP: Duration = Duration::from_millis(50);

fn wire(plain: &Plain) -> (Header, Vec<u8>) {
    let packet = plain.packet(0);
    let header = Header::parse(&packet).unwrap();
    (header, packet[header.clear + header.extension..].to_vec())
}

fn vp8(sequence: u16, marker: bool, payload: &[u8]) -> (Header, Vec<u8>) {
    let header = Header {
        padded: false,
        marker,
        payload_type: Payload::Video(VideoCodec::Vp8).payload_type(),
        sequence,
        timestamp: 3000,
        ssrc: SOURCE,
        clear: 12,
        extension: 0,
    };
    (header, payload.to_vec())
}

#[test]
fn sent_h264_frames_reassemble() {
    let start = Instant::now();
    let mut sender = VideoSender::new(SOURCE, REPAIR);
    let mut receiver = VideoReceiver::default();
    receiver.announce(SOURCE, Some(REPAIR));
    let mut frame = vec![0, 0, 0, 1, 0x65];
    frame.extend((0..2600).map(|index| (index % 200) as u8 + 1));
    let mut arrived = Vec::new();
    for (index, captured) in [Duration::ZERO, Duration::from_millis(33)]
        .into_iter()
        .enumerate()
    {
        let plains = sender.frame(packetize_h264(&frame), captured);
        assert_eq!(plains.len(), 3);
        for plain in &plains {
            let (header, payload) = wire(plain);
            assert_eq!(header.payload_type, 101);
            let kind = Payload::from_type(header.payload_type).unwrap();
            arrived.extend(receiver.packet(header, kind, payload, start));
        }
        assert_eq!(arrived.len(), index, "frame {index}");
    }
    assert_eq!(arrived[0].frame.data, frame);
    assert!(arrived[0].frame.keyframe);
    assert_eq!(arrived[0].codec, VideoCodec::H264);
}

#[test]
fn repairs_carry_original_sequence() {
    let mut sender = VideoSender::new(SOURCE, REPAIR);
    let plains = sender.frame(vec![vec![0x41, 1, 2], vec![0x41, 3]], Duration::ZERO);
    let (first, _) = wire(&plains[0]);
    let (last, _) = wire(&plains[1]);
    assert_eq!(last.sequence, first.sequence.wrapping_add(1));
    assert!(!first.marker && last.marker);
    let repaired = sender.repair(first.sequence).unwrap();
    let (header, payload) = wire(&repaired);
    assert_eq!(header.ssrc, REPAIR);
    assert_eq!(
        Payload::from_type(header.payload_type),
        Some(Payload::Repair(VideoCodec::H264))
    );
    assert_eq!(header.timestamp, first.timestamp);
    let mut original = first.sequence.to_be_bytes().to_vec();
    original.extend_from_slice(&[0x41, 1, 2]);
    assert_eq!(payload, original);
    assert!(sender.repair(first.sequence.wrapping_sub(1)).is_none());
}

#[test]
fn receiver_asks_for_gaps_and_takes_repairs() {
    let now = Instant::now();
    let mut receiver = VideoReceiver::default();
    receiver.announce(SOURCE, Some(REPAIR));
    let kind = Payload::Video(VideoCodec::Vp8);
    for (sequence, marker, payload) in [
        (100, false, &[0x90, 0x80, 0x81, 0x23, 0x10, 0xAA][..]),
        (102, true, &[0x80, 0x80, 0x81, 0x23, 0xCC][..]),
    ] {
        let (header, payload) = vp8(sequence, marker, payload);
        assert!(receiver.packet(header, kind, payload, now).is_empty());
    }
    assert_eq!(
        receiver.feedback(ME, now, ROUND_TRIP),
        vec![nack(ME, SOURCE, &[101]).unwrap(), picture_loss(ME, SOURCE)]
    );
    assert!(receiver.feedback(ME, now, ROUND_TRIP).is_empty());
    let (mut header, _) = vp8(7, false, &[]);
    header.ssrc = REPAIR;
    header.payload_type = Payload::Repair(VideoCodec::Vp8).payload_type();
    let repair = [0, 101, 0x80, 0x80, 0x81, 0x23, 0xBB];
    let arrived = receiver.packet(
        header,
        Payload::Repair(VideoCodec::Vp8),
        repair.to_vec(),
        now,
    );
    assert_eq!(
        arrived,
        vec![Arrived {
            source: SOURCE,
            codec: VideoCodec::Vp8,
            frame: Assembled {
                timestamp: 3000,
                keyframe: true,
                data: vec![0x10, 0xAA, 0xBB, 0xCC],
            },
        }]
    );
}
