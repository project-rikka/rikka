use discord_rtp::{Cipher, Fields, HEADER_LEN, Header, KEY_LEN, Mode};

const MODES: [Mode; 2] = [Mode::Aes256Gcm, Mode::XChaCha20Poly1305];

fn key() -> [u8; KEY_LEN] {
    std::array::from_fn(|index| index as u8)
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn opus(payload: &[u8]) -> Vec<u8> {
    let mut packet = Header::opus(0x0102, 0x0304_0506, 0x0708_090A, false).to_vec();
    packet.extend_from_slice(payload);
    packet
}

#[test]
fn aes_rtpsize_matches_known_answer() {
    let cipher = Cipher::new(Mode::Aes256Gcm, &key());
    let sealed = cipher.seal_rtp(&opus(b"discord"), 7).unwrap();
    assert_eq!(
        sealed,
        hex(concat!(
            "80780102030405060708090a",
            "3b8ea7e2155a47",
            "e73a907203d474f218de9bdd227f3f13",
            "00000007",
        ))
    );
}

#[test]
fn sealed_rtp_opens_to_payload() {
    for mode in MODES {
        let cipher = Cipher::new(mode, &key());
        let packet = opus(b"opus frame");
        let sealed = cipher.seal_rtp(&packet, 0x0102_0304).unwrap();
        assert_eq!(sealed[..HEADER_LEN], packet[..HEADER_LEN], "{mode:?}");
        assert_eq!(sealed[sealed.len() - 4..], [1, 2, 3, 4], "{mode:?}");
        assert_eq!(sealed.len(), packet.len() + 16 + 4, "{mode:?}");
        let opened = cipher.open_rtp(&sealed).unwrap();
        assert_eq!(opened.payload, b"opus frame", "{mode:?}");
        assert_eq!(opened.header.sequence, 0x0102, "{mode:?}");
    }
}

#[test]
fn opened_rtp_drops_extension_and_padding() {
    for mode in MODES {
        let cipher = Cipher::new(mode, &key());
        let mut packet = Fields {
            marker: true,
            payload_type: 101,
            sequence: 9,
            timestamp: 90_000,
            ssrc: 42,
        }
        .header(true)
        .to_vec();
        packet[0] |= 0x20;
        packet.extend_from_slice(&[0xBE, 0xDE, 0, 1]);
        packet.extend_from_slice(&[0x50, 0x00, 0x07, 0x00]);
        packet.extend_from_slice(&[1, 2, 3, 0, 0, 3]);
        let sealed = cipher.seal_rtp(&packet, 5).unwrap();
        assert_eq!(
            sealed[..HEADER_LEN + 4],
            packet[..HEADER_LEN + 4],
            "{mode:?}"
        );
        let opened = cipher.open_rtp(&sealed).unwrap();
        assert_eq!(opened.payload, [1, 2, 3], "{mode:?}");
        assert!(opened.header.marker && opened.header.padded, "{mode:?}");
    }
}

#[test]
fn altered_packets_do_not_open() {
    for mode in MODES {
        let cipher = Cipher::new(mode, &key());
        let sealed = cipher.seal_rtp(&opus(b"frame"), 1).unwrap();
        let mut header = sealed.clone();
        header[3] ^= 1;
        let mut body = sealed.clone();
        body[HEADER_LEN] ^= 1;
        let other = Cipher::new(mode, &[9; KEY_LEN]);
        assert!(cipher.open_rtp(&header).is_none(), "{mode:?}");
        assert!(cipher.open_rtp(&body).is_none(), "{mode:?}");
        assert!(other.open_rtp(&sealed).is_none(), "{mode:?}");
    }
}

#[test]
fn sealed_rtcp_keeps_eight_bytes_clear() {
    for mode in MODES {
        let cipher = Cipher::new(mode, &key());
        let report = [0x81, 201, 0, 7, 0, 0, 0, 42, 1, 2, 3, 4];
        let sealed = cipher.seal_rtcp(&report, 3).unwrap();
        assert_eq!(sealed[..8], report[..8], "{mode:?}");
        assert_eq!(cipher.open_rtcp(&sealed).unwrap(), report, "{mode:?}");
    }
}

#[test]
fn modes_follow_discord_names() {
    let offered = [
        "aead_xchacha20_poly1305_rtpsize".to_owned(),
        "aead_aes256_gcm_rtpsize".to_owned(),
    ];
    assert_eq!(Mode::choose(&offered), Some(Mode::Aes256Gcm));
    assert_eq!(Mode::choose(&offered[..1]), Some(Mode::XChaCha20Poly1305));
    assert_eq!(Mode::choose(&["xsalsa20_poly1305".to_owned()]), None);
}
