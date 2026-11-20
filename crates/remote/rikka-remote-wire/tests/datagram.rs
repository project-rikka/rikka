use rikka_remote_wire::{
    Arrival, AudioPacket, ClientDatagram, DELTA_UNIT_US, Feedback, FrameFlags, Motion, Position,
    ServerDatagram, VIDEO_HEADER_BYTES, VideoShard, encode_padding, encode_pointer,
};

#[test]
fn video_shards_follow_the_documented_layout() {
    let shard = VideoShard {
        seq: 0x0102_0304,
        stream: 0x0506,
        frame: 0x0708_090A,
        reference: 0x0B0C_0D0E,
        flags: FrameFlags {
            intra: true,
            refinement: false,
        },
        capture_us: 0x1112_1314,
        frame_bytes: 0x2122_2324,
        block_offset: 0x3132_3334,
        block: 0x4142,
        blocks: 0x5152,
        data_shards: 0x61,
        parity_shards: 0x62,
        shard: 0x63,
        payload: &[0xAA, 0xBB],
    };
    let mut bytes = Vec::new();
    shard.encode(&mut bytes);
    let expected: Vec<u8> = [
        &[0x01][..],
        &[0x04, 0x03, 0x02, 0x01],
        &[0x06, 0x05],
        &[0x0A, 0x09, 0x08, 0x07],
        &[0x0E, 0x0D, 0x0C, 0x0B],
        &[0x01],
        &[0x14, 0x13, 0x12, 0x11],
        &[0x24, 0x23, 0x22, 0x21],
        &[0x34, 0x33, 0x32, 0x31],
        &[0x42, 0x41],
        &[0x52, 0x51],
        &[0x61, 0x62, 0x63, 0x00],
        &[0x02, 0x00],
        &[0xAA, 0xBB],
    ]
    .concat();
    assert_eq!(bytes, expected);
    assert_eq!(bytes.len(), VIDEO_HEADER_BYTES + 2);
}

#[test]
fn server_datagrams_round_trip() {
    let video = VideoShard {
        seq: 4,
        stream: 1,
        frame: 10,
        reference: 9,
        flags: FrameFlags {
            intra: false,
            refinement: true,
        },
        capture_us: 99,
        frame_bytes: 3000,
        block_offset: 0,
        block: 0,
        blocks: 1,
        data_shards: 3,
        parity_shards: 1,
        shard: 3,
        payload: &[1; 1000],
    };
    let mut bytes = Vec::new();
    video.encode(&mut bytes);
    assert_eq!(
        ServerDatagram::decode(&bytes),
        Ok(ServerDatagram::Video(video))
    );

    for previous in [None, Some(&[4_u8, 5][..])] {
        let audio = AudioPacket {
            seq: 5,
            timestamp: 4800,
            primary: &[1, 2, 3],
            previous,
        };
        bytes.clear();
        audio.encode(&mut bytes);
        assert_eq!(
            ServerDatagram::decode(&bytes),
            Ok(ServerDatagram::Audio(audio))
        );
    }

    bytes.clear();
    encode_padding(6, 1200, &mut bytes);
    assert_eq!(bytes.len(), 1200);
    assert_eq!(
        ServerDatagram::decode(&bytes),
        Ok(ServerDatagram::Padding { seq: 6 })
    );

    bytes.clear();
    encode_pointer(7, -3, 1080, &mut bytes);
    assert_eq!(
        ServerDatagram::decode(&bytes),
        Ok(ServerDatagram::Pointer {
            seq: 7,
            x: -3,
            y: 1080
        })
    );
}

#[test]
fn shards_outside_their_block_are_rejected() {
    let mut bytes = Vec::new();
    VideoShard {
        seq: 0,
        stream: 0,
        frame: 0,
        reference: 0,
        flags: FrameFlags::default(),
        capture_us: 0,
        frame_bytes: 10,
        block_offset: 0,
        block: 0,
        blocks: 1,
        data_shards: 2,
        parity_shards: 1,
        shard: 3,
        payload: &[0; 5],
    }
    .encode(&mut bytes);
    assert!(ServerDatagram::decode(&bytes).is_err());
}

#[test]
fn feedback_keeps_arrival_times_within_one_unit() {
    let arrivals = [
        (100, 4_294_966_000_u32),
        (101, 4_294_966_013),
        (103, 4_294_967_280),
        (104, 4_294_967_200),
        (107, 300),
    ];
    let feedback = Feedback {
        base_seq: 100,
        count: 9,
        arrivals: arrivals
            .iter()
            .map(|&(seq, at_us)| Arrival { seq, at_us })
            .collect(),
        stream: 3,
        good: Some(41),
        lost: vec![39, 40],
    };
    let mut bytes = Vec::new();
    ClientDatagram::Feedback(feedback.clone()).encode(&mut bytes);
    assert_eq!(bytes.len(), Feedback::bytes_for(9, 5, 2));
    let ClientDatagram::Feedback(decoded) = ClientDatagram::decode(&bytes).unwrap() else {
        panic!("decoded something other than feedback");
    };
    assert_eq!(decoded.base_seq, 100);
    assert_eq!(decoded.count, 9);
    assert_eq!(decoded.stream, 3);
    assert_eq!(decoded.good, Some(41));
    assert_eq!(decoded.lost, vec![39, 40]);
    let seqs: Vec<u32> = decoded.arrivals.iter().map(|arrival| arrival.seq).collect();
    assert_eq!(seqs, vec![100, 101, 103, 104, 107]);
    for (sent, got) in feedback.arrivals.iter().zip(&decoded.arrivals) {
        let error = got.at_us.wrapping_sub(sent.at_us) as i32;
        assert!(
            error.unsigned_abs() <= DELTA_UNIT_US / 2,
            "seq {} drifted by {error} us",
            sent.seq
        );
    }
}

#[test]
fn motion_round_trips() {
    let motion = ClientDatagram::Motion(Motion {
        seq: 65535,
        at: Position { x: 1, y: 65535 },
    });
    let mut bytes = Vec::new();
    motion.encode(&mut bytes);
    assert_eq!(ClientDatagram::decode(&bytes), Ok(motion));
}
