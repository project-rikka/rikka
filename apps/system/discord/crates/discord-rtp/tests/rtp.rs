use std::time::{Duration, Instant};

use discord_rtp::{DISCOVERY_LEN, Discovered, Fields, Header, VoiceStream, is_rtcp};

#[test]
fn header_reads_csrcs_extension_and_flags() {
    let packet = [
        0xB1, 0xE5, 0x12, 0x34, 0x0A, 0x0B, 0x0C, 0x0D, 0x11, 0x22, 0x33, 0x44, 0xCA, 0xFE, 0xBA,
        0xBE, 0xBE, 0xDE, 0x00, 0x02, 1, 2, 3, 4, 5, 6, 7, 8,
    ];
    assert_eq!(
        Header::parse(&packet),
        Some(Header {
            padded: true,
            marker: true,
            payload_type: 101,
            sequence: 0x1234,
            timestamp: 0x0A0B_0C0D,
            ssrc: 0x1122_3344,
            clear: 20,
            extension: 8,
        })
    );
    assert_eq!(Header::parse(&packet[..18]), None);
    assert_eq!(Header::parse(&[0x40; 12]), None);
}

#[test]
fn fields_write_rtp_header() {
    let fields = Fields {
        marker: true,
        payload_type: 101,
        sequence: 0xFFFF,
        timestamp: 7,
        ssrc: 9,
    };
    assert_eq!(
        fields.header(true),
        [0x90, 0xE5, 0xFF, 0xFF, 0, 0, 0, 7, 0, 0, 0, 9]
    );
    let parsed = Header::parse(&fields.header(false)).unwrap();
    assert_eq!(
        (
            parsed.marker,
            parsed.sequence,
            parsed.timestamp,
            parsed.ssrc
        ),
        (true, 0xFFFF, 7, 9)
    );
}

#[test]
fn rtcp_is_told_apart_from_rtp() {
    let cases: [(&[u8], bool); 4] = [
        (&[0x80, 200, 0, 6, 0, 0, 0, 1], true),
        (&[0x81, 206, 0, 2, 0, 0, 0, 1], true),
        (&[0x80, 0xE5, 0, 1, 0, 0, 0, 1], false),
        (&[0x80, 200, 0, 6], false),
    ];
    for (packet, rtcp) in cases {
        assert_eq!(is_rtcp(packet), rtcp, "{packet:?}");
    }
}

#[test]
fn voice_stream_numbers_opus_frames() {
    let origin = Instant::now();
    let mut stream = VoiceStream::new(77, origin);
    let first = Header::parse(&stream.next(origin)).unwrap();
    let second = Header::parse(&stream.next(origin)).unwrap();
    assert_eq!((first.payload_type, first.ssrc), (120, 77));
    assert!(first.marker && !second.marker);
    assert_eq!(second.sequence, first.sequence.wrapping_add(1));
    assert_eq!(second.timestamp, first.timestamp.wrapping_add(960));
    stream.talkspurt();
    let resumed = Header::parse(&stream.next(origin + Duration::from_secs(1))).unwrap();
    assert!(resumed.marker);
    assert_eq!(resumed.timestamp, first.timestamp.wrapping_add(48_000));
}

#[test]
fn discovery_reads_external_address() {
    let request = Discovered::request(0x0102_0304);
    assert_eq!(request[..8], [0, 1, 0, 70, 1, 2, 3, 4]);
    assert!(request[8..].iter().all(|&byte| byte == 0));
    let mut response = [0; DISCOVERY_LEN];
    response[..8].copy_from_slice(&[0, 2, 0, 70, 1, 2, 3, 4]);
    response[8..19].copy_from_slice(b"203.0.113.7");
    response[72..].copy_from_slice(&50_000_u16.to_be_bytes());
    assert_eq!(
        Discovered::from_response(&response, 0x0102_0304),
        Some(Discovered {
            address: "203.0.113.7".to_owned(),
            port: 50_000,
        })
    );
    assert_eq!(Discovered::from_response(&response, 5), None);
    assert_eq!(
        Discovered::from_response(&response[..73], 0x0102_0304),
        None
    );
}
