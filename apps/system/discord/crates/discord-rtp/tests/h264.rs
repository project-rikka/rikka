use discord_core::VideoCodec;
use discord_rtp::{MAX_PAYLOAD, depacketize, packetize_h264, prepare_h264};

const START: [u8; 4] = [0, 0, 0, 1];
const SPS: [u8; 9] = [0x67, 0x42, 0xC0, 0x1F, 0xDA, 0x01, 0x40, 0x16, 0xE4];
const PPS: [u8; 4] = [0x68, 0xCE, 0x3C, 0x80];

fn annex_b(units: &[&[u8]]) -> Vec<u8> {
    units
        .iter()
        .flat_map(|unit| START.iter().chain(unit.iter()))
        .copied()
        .collect()
}

fn slice(kind: u8, len: usize) -> Vec<u8> {
    std::iter::once(kind)
        .chain((1..len).map(|index| (index % 251) as u8 + 1))
        .collect()
}

fn depacketized(payloads: &[Vec<u8>]) -> Option<Vec<u8>> {
    let borrowed: Vec<&[u8]> = payloads.iter().map(Vec::as_slice).collect();
    depacketize(VideoCodec::H264, &borrowed)
}

#[test]
fn small_units_share_one_stap_a() {
    let idr = slice(0x65, 30);
    let frame = annex_b(&[&SPS, &PPS, &idr]);
    let payloads = packetize_h264(&frame);
    let mut expected = vec![0x78, 0, 9];
    expected.extend_from_slice(&SPS);
    expected.extend_from_slice(&[0, 4]);
    expected.extend_from_slice(&PPS);
    expected.extend_from_slice(&[0, 30]);
    expected.extend_from_slice(&idr);
    assert_eq!(payloads, vec![expected]);
    assert_eq!(depacketized(&payloads), Some(frame));
}

#[test]
fn large_unit_splits_into_fu_a() {
    let idr = slice(0x65, 3000);
    let frame = annex_b(&[&idr]);
    let payloads = packetize_h264(&frame);
    assert!(payloads.iter().all(|payload| payload.len() <= MAX_PAYLOAD));
    let headers: Vec<[u8; 2]> = payloads
        .iter()
        .map(|payload| [payload[0], payload[1]])
        .collect();
    assert_eq!(headers, vec![[0x7C, 0x85], [0x7C, 0x05], [0x7C, 0x45]]);
    assert_eq!(depacketized(&payloads), Some(frame));
}

#[test]
fn mixed_frame_round_trips() {
    let big = slice(0x41, 2500);
    let small = slice(0x41, 40);
    let frame = annex_b(&[&small, &big, &small, &small]);
    let payloads = packetize_h264(&frame);
    assert_eq!(payloads.len(), 5);
    assert_eq!(depacketized(&payloads), Some(frame));
}

#[test]
fn prepared_sps_is_stable() {
    let idr = slice(0x65, 20);
    let frame = annex_b(&[&SPS, &PPS, &idr]);
    let prepared = prepare_h264(&frame);
    assert_ne!(prepared, frame);
    assert!(prepared.ends_with(&annex_b(&[&PPS, &idr])));
    assert_eq!(prepare_h264(&prepared), prepared);
}

#[test]
fn malformed_payloads_do_not_depacketize() {
    for payload in [vec![], vec![0x7C], vec![0x78, 0, 9, 0x67], vec![0x1E, 1]] {
        assert_eq!(
            depacketized(std::slice::from_ref(&payload)),
            None,
            "{payload:?}"
        );
    }
}
