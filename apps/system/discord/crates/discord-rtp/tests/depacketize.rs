use discord_core::VideoCodec;
use discord_rtp::depacketize;

type Case = (VideoCodec, &'static [&'static [u8]], &'static [u8]);

#[test]
fn payloads_become_decoder_bitstreams() {
    let cases: [Case; 7] = [
        (
            VideoCodec::Vp8,
            &[
                &[0x90, 0x80, 0x81, 0x23, 0x10, 0xAA],
                &[0x80, 0x80, 0x81, 0x23, 0xBB],
                &[0x00, 0xCC],
            ],
            &[0x10, 0xAA, 0xBB, 0xCC],
        ),
        (
            VideoCodec::Vp9,
            &[&[0x88, 0x05, 0xAA, 0xBB], &[0x84, 0x05, 0xCC]],
            &[0xAA, 0xBB, 0xCC],
        ),
        (
            VideoCodec::Av1,
            &[&[0x18, 0x30, 0xAA, 0xBB, 0xCC]],
            &[0x32, 0x03, 0xAA, 0xBB, 0xCC],
        ),
        (
            VideoCodec::Av1,
            &[&[0x50, 0x30, 1, 2], &[0x90, 3, 4]],
            &[0x32, 0x04, 1, 2, 3, 4],
        ),
        (
            VideoCodec::Av1,
            &[&[0x20, 0x01, 0x10, 0x30, 0xEE]],
            &[0x12, 0x00, 0x32, 0x01, 0xEE],
        ),
        (
            VideoCodec::H265,
            &[
                &[0x02, 0x01, 0xAA],
                &[0x62, 0x01, 0x93, 0xB1],
                &[0x62, 0x01, 0x53, 0xB2],
            ],
            &[
                0, 0, 0, 1, 0x02, 0x01, 0xAA, 0, 0, 0, 1, 0x26, 0x01, 0xB1, 0xB2,
            ],
        ),
        (
            VideoCodec::H265,
            &[&[0x60, 0x01, 0, 3, 0x02, 0x01, 0xCC, 0, 3, 0x02, 0x01, 0xDD]],
            &[0, 0, 0, 1, 0x02, 0x01, 0xCC, 0, 0, 0, 1, 0x02, 0x01, 0xDD],
        ),
    ];
    for (codec, payloads, expected) in cases {
        assert_eq!(
            depacketize(codec, payloads).as_deref(),
            Some(expected),
            "{codec:?}"
        );
    }
}

#[test]
fn broken_fragments_do_not_depacketize() {
    let cases: [(VideoCodec, &[&[u8]]); 4] = [
        (VideoCodec::Av1, &[&[0x50, 0x30, 1, 2]]),
        (VideoCodec::Av1, &[&[0x90, 3, 4]]),
        (VideoCodec::H265, &[&[0x64, 0x01, 0xAA]]),
        (VideoCodec::Vp8, &[&[0x90, 0x80, 0x81]]),
    ];
    for (codec, payloads) in cases {
        assert_eq!(depacketize(codec, payloads), None, "{codec:?}");
    }
}
