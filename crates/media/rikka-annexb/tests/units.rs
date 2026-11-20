use rikka_annexb::units;

#[test]
fn units_split_at_three_and_four_byte_start_codes() {
    let cases: [(&[u8], Vec<&[u8]>); 4] = [
        (
            &[0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3],
            vec![&[0x67, 1, 2], &[0x68, 3]],
        ),
        (&[0, 0, 1, 0x65, 0, 0, 0, 1, 0x41], vec![&[0x65], &[0x41]]),
        (&[0, 0, 1, 0, 0, 1, 0x09], vec![&[0x09]]),
        (&[7, 8, 9], vec![]),
    ];
    for (stream, expected) in cases {
        assert_eq!(units(stream), expected, "{stream:02x?}");
    }
}
