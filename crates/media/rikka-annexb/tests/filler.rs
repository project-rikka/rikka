use rikka_annexb::{NalSyntax, append_without_filler};

#[test]
fn filler_units_are_dropped_and_others_kept() {
    let h264_slice = [0, 0, 0, 1, 0x65, 0xAA, 0xBB];
    let h264_filler = [0, 0, 1, 0x0C, 0xFF, 0xFF];
    let h265_slice = [0, 0, 0, 1, 0x26, 0x01, 0xCC];
    let h265_filler = [0, 0, 1, 0x4C, 0x01, 0xFF];
    let cases: [(NalSyntax, Vec<u8>, Vec<u8>); 4] = [
        (
            NalSyntax::H264,
            [h264_slice.as_slice(), &h264_filler].concat(),
            h264_slice.to_vec(),
        ),
        (
            NalSyntax::H264,
            [h264_filler.as_slice(), &h264_slice].concat(),
            h264_slice.to_vec(),
        ),
        (
            NalSyntax::H265,
            [h265_slice.as_slice(), &h265_filler, &h265_slice].concat(),
            [h265_slice.as_slice(), &h265_slice].concat(),
        ),
        (NalSyntax::H265, vec![1, 2, 3], vec![1, 2, 3]),
    ];
    for (syntax, stream, expected) in cases {
        let mut out = Vec::new();
        append_without_filler(&stream, syntax, &mut out);
        assert_eq!(out, expected, "{syntax:?} {stream:02x?}");
    }
}
