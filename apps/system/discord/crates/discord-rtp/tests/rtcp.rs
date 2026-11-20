use discord_rtp::{
    Feedback, ReportBlock, SenderInfo, nack, parse_rtcp, picture_loss, receiver_report,
    sender_report,
};

const BLOCK: ReportBlock = ReportBlock {
    source: 0x0A0B_0C0D,
    fraction_lost: 64,
    lost: 300,
    highest: 70_000,
    jitter: 12,
    last_report: 0x1234_5678,
    since_report: 655,
};

#[test]
fn compound_feedback_parses() {
    let mut compound = vec![0x81, 200, 0, 12];
    compound.extend_from_slice(&[0, 0, 0, 1]);
    compound.extend_from_slice(&[0xE1, 0, 0, 0, 0x80, 0, 0, 0]);
    compound.extend_from_slice(&[0, 0, 0, 9, 0, 0, 0, 10, 0, 0, 0, 11]);
    compound.extend_from_slice(&[0x0A, 0x0B, 0x0C, 0x0D, 64, 0, 1, 44]);
    compound.extend_from_slice(&[0, 1, 0x11, 0x70, 0, 0, 0, 12]);
    compound.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0, 0, 2, 143]);
    compound.extend_from_slice(&[0x81, 205, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2, 0, 100, 0x80, 0x01]);
    compound.extend_from_slice(&[0x81, 206, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2]);
    compound.extend_from_slice(&[0x8F, 206, 0, 5, 0, 0, 0, 1, 0, 0, 0, 0]);
    compound.extend_from_slice(b"REMB");
    compound.extend_from_slice(&[1, 0x0D, 0xE8, 0x48, 0, 0, 0, 2]);
    compound.extend_from_slice(&[
        0x84, 206, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 3, 1, 0, 0, 0,
    ]);
    assert_eq!(
        parse_rtcp(&compound),
        vec![
            Feedback::Sender {
                source: 1,
                ntp: 0xE100_0000_8000_0000,
            },
            Feedback::Report(vec![BLOCK]),
            Feedback::Nack {
                media: 2,
                lost: vec![100, 101, 116],
            },
            Feedback::PictureLoss { media: 2 },
            Feedback::Remb {
                bitrate: 1_000_000,
                media: vec![2],
            },
            Feedback::PictureLoss { media: 3 },
        ]
    );
}

#[test]
fn truncated_compound_keeps_whole_packets() {
    let mut compound = picture_loss(1, 2);
    compound.extend_from_slice(&[0x81, 201, 0, 7, 0, 0]);
    assert_eq!(
        parse_rtcp(&compound),
        vec![Feedback::PictureLoss { media: 2 }]
    );
}

#[test]
fn built_packets_follow_rfc_layout() {
    assert_eq!(
        nack(1, 2, &[100, 101, 116, 117, 200]),
        Some(vec![
            0x81, 205, 0, 5, 0, 0, 0, 1, 0, 0, 0, 2, 0, 100, 0x80, 0x01, 0, 117, 0, 0, 0, 200, 0,
            0,
        ])
    );
    assert_eq!(nack(1, 2, &[]), None);
    assert_eq!(
        picture_loss(1, 2),
        [0x81, 206, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2]
    );
    let sent = SenderInfo {
        source: 5,
        ntp: 0x0102_0304_0506_0708,
        timestamp: 90_000,
        packets: 10,
        octets: 12_000,
    };
    let report = sender_report(sent);
    assert_eq!(report.len(), 28);
    assert_eq!(report[..4], [0x80, 200, 0, 6]);
    assert_eq!(
        parse_rtcp(&report),
        vec![
            Feedback::Sender {
                source: 5,
                ntp: 0x0102_0304_0506_0708,
            },
            Feedback::Report(Vec::new()),
        ]
    );
    let received = receiver_report(7, &[BLOCK]);
    assert_eq!(received[..8], [0x81, 201, 0, 7, 0, 0, 0, 7]);
    assert_eq!(parse_rtcp(&received), vec![Feedback::Report(vec![BLOCK])]);
}
