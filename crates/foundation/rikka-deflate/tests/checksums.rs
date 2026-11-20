use rikka_deflate::{Crc32, adler32, crc32};

use crate::support::noise;

#[test]
fn checksums_match_their_reference_values() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(adler32(b""), 1);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);

    let data = noise(100_003, 6);
    let mut reference = flate2::Crc::new();
    reference.update(&data);
    assert_eq!(crc32(&data), reference.sum());
}

#[test]
fn crc_of_pieces_equals_crc_of_the_whole() {
    let data = noise(4_099, 7);

    for split in [0, 1, 7, 8, 9, 2_048, 4_098, 4_099] {
        let mut crc = Crc32::new();
        crc.update(&data[..split]);
        crc.update(&data[split..]);
        assert_eq!(crc.finish(), crc32(&data), "split at {split}");
    }
}
