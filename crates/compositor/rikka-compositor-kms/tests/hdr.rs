use rikka_colorimetry::Primaries;
use rikka_compositor_kms::{HDR_OUTPUT_METADATA_SIZE, HdrMetadata};

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_ne_bytes([bytes[at], bytes[at + 1]])
}

#[test]
fn hdr_output_metadata_follows_the_uapi_layout() {
    let blob = HdrMetadata {
        primaries: Primaries::BT2020,
        max_luminance: 1000.0,
        min_luminance: 0.05,
        max_cll: 800.0,
        max_fall: 400.0,
    }
    .blob();
    assert_eq!(blob.len(), HDR_OUTPUT_METADATA_SIZE);
    assert_eq!(u32::from_ne_bytes([blob[0], blob[1], blob[2], blob[3]]), 0);
    assert_eq!(blob[4], 2, "SMPTE ST 2084 EOTF");
    assert_eq!(blob[5], 0, "static metadata type 1");
    let expected = [
        (6, 35_400),
        (8, 14_600),
        (10, 8_500),
        (12, 39_850),
        (14, 6_550),
        (16, 2_300),
        (18, 15_635),
        (20, 16_450),
        (22, 1000),
        (24, 500),
        (26, 800),
        (28, 400),
    ];
    for (at, value) in expected {
        assert_eq!(u16_at(&blob, at), value, "field at byte {at}");
    }
}
