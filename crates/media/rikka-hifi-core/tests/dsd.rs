use rikka_hifi_core::{DSD_SILENCE, DopPacker, DsdPacker, DsdToPcm, SampleFormat};

#[test]
fn dop_frames_alternate_markers() {
    let dsd = [0x11, 0x21, 0x12, 0x22, 0x13, 0x23, 0x14, 0x24];
    let cases: [(SampleFormat, Vec<u8>); 3] = [
        (
            SampleFormat::S32,
            vec![
                0, 0x12, 0x11, 0x05, 0, 0x22, 0x21, 0x05, 0, 0x14, 0x13, 0xFA, 0, 0x24, 0x23, 0xFA,
            ],
        ),
        (
            SampleFormat::S24,
            vec![
                0x12, 0x11, 0x05, 0, 0x22, 0x21, 0x05, 0, 0x14, 0x13, 0xFA, 0, 0x24, 0x23, 0xFA, 0,
            ],
        ),
        (
            SampleFormat::S24Packed,
            vec![
                0x12, 0x11, 0x05, 0x22, 0x21, 0x05, 0x14, 0x13, 0xFA, 0x24, 0x23, 0xFA,
            ],
        ),
    ];
    for (format, expected) in cases {
        let mut packed = Vec::new();
        DopPacker::new(2, format).pack(&dsd, &mut packed);
        assert_eq!(packed, expected, "DoP in {format}");
    }
}

#[test]
fn dop_splits_input_and_pads_tail() {
    let mut whole = Vec::new();
    DopPacker::new(2, SampleFormat::S24Packed).pack(&[1, 2, 3, 4, 5, 6], &mut whole);

    let mut split = Vec::new();
    let mut packer = DopPacker::new(2, SampleFormat::S24Packed);
    packer.pack(&[1], &mut split);
    packer.pack(&[2, 3, 4, 5], &mut split);
    packer.pack(&[6], &mut split);
    packer.finish(&mut split);

    assert_eq!(&split[..6], &whole[..]);
    assert_eq!(&split[6..], &[DSD_SILENCE, 5, 0xFA, DSD_SILENCE, 6, 0xFA]);
}

#[test]
fn native_dsd_words_are_oldest_byte_first() {
    let dsd = [0xA0, 0xB0, 0xA1, 0xB1, 0xA2, 0xB2, 0xA3, 0xB3];
    let cases: [(SampleFormat, Vec<u8>); 4] = [
        (SampleFormat::Dsd8, dsd.to_vec()),
        (
            SampleFormat::Dsd16Be,
            vec![0xA0, 0xA1, 0xB0, 0xB1, 0xA2, 0xA3, 0xB2, 0xB3],
        ),
        (
            SampleFormat::Dsd32Be,
            vec![0xA0, 0xA1, 0xA2, 0xA3, 0xB0, 0xB1, 0xB2, 0xB3],
        ),
        (
            SampleFormat::Dsd32Le,
            vec![0xA3, 0xA2, 0xA1, 0xA0, 0xB3, 0xB2, 0xB1, 0xB0],
        ),
    ];
    for (format, expected) in cases {
        let mut packed = Vec::new();
        DsdPacker::new(2, format).pack(&dsd, &mut packed);
        assert_eq!(packed, expected, "{format}");
    }
}

#[test]
fn dsd_to_pcm_keeps_level() {
    let mut converter = DsdToPcm::new(2_822_400, 1);
    assert_eq!(converter.rate(), 88_200);

    let mut silence = Vec::new();
    converter.process(&vec![DSD_SILENCE; 8192], &mut silence);
    assert_eq!(silence.len(), 8192 / 4);
    assert!(silence.iter().all(|sample| sample.abs() < 1e-3));

    let mut high = Vec::new();
    converter.process(&vec![0xFF; 8192], &mut high);
    let settled = &high[high.len() / 2..];
    assert!(settled.iter().all(|sample| (sample - 1.0).abs() < 1e-3));
}
