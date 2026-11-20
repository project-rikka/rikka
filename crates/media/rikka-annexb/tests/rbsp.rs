use rikka_annexb::{BitReader, BitWriter, insert_emulation_prevention, strip_emulation_prevention};

#[test]
fn emulation_prevention_round_trips() {
    let cases: [(&[u8], &[u8]); 3] = [
        (&[0, 0, 1, 7], &[0, 0, 3, 1, 7]),
        (&[0, 0, 0, 0, 2], &[0, 0, 3, 0, 0, 3, 2]),
        (&[0, 4, 0, 0, 5], &[0, 4, 0, 0, 5]),
    ];
    for (raw, escaped) in cases {
        assert_eq!(insert_emulation_prevention(raw), escaped, "{raw:02x?}");
        assert_eq!(strip_emulation_prevention(escaped), raw, "{escaped:02x?}");
    }
}

#[test]
fn exp_golomb_codes_read_back() {
    let mut writer = BitWriter::default();
    for value in [0, 1, 2, 7, 255] {
        writer.golomb(value);
    }
    for value in [0, -1, 1, -40] {
        writer.signed_golomb(value);
    }
    let bytes = writer.finish();
    assert_eq!(bytes[0] >> 5, 0b101, "0 is 1, 1 is 010");
    let mut reader = BitReader::new(&bytes);
    for value in [0, 1, 2, 7, 255] {
        assert_eq!(reader.golomb(), Some(value));
    }
    for value in [0, -1, 1, -40] {
        assert_eq!(reader.signed_golomb(), Some(value));
    }
    assert_eq!(reader.bit(), Some(1), "trailing stop bit");
}
