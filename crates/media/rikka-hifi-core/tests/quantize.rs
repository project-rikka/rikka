use rikka_hifi_core::{Quantizer, SampleFormat};

fn sixteen(value: i16) -> f32 {
    f32::from(value) / 32_768.0
}

fn twenty_four(value: i32) -> f32 {
    value as f32 / 8_388_608.0
}

#[test]
fn undithered_samples_are_bit_exact() {
    let cases: [(SampleFormat, f32, &[u8]); 9] = [
        (SampleFormat::S16, sixteen(-32_768), &[0x00, 0x80]),
        (
            SampleFormat::S16,
            sixteen(12_345),
            &12_345_i16.to_le_bytes(),
        ),
        (SampleFormat::S24Packed, sixteen(1), &[0x00, 0x01, 0x00]),
        (
            SampleFormat::S24Packed,
            twenty_four(-1),
            &[0xFF, 0xFF, 0xFF],
        ),
        (
            SampleFormat::S24,
            twenty_four(0x12_3456),
            &[0x56, 0x34, 0x12, 0x00],
        ),
        (
            SampleFormat::S24,
            twenty_four(-8_388_608),
            &[0x00, 0x00, 0x80, 0xFF],
        ),
        (SampleFormat::S32, sixteen(-1), &[0x00, 0x00, 0xFF, 0xFF]),
        (
            SampleFormat::S32,
            twenty_four(0x7F_FFFF),
            &[0x00, 0xFF, 0xFF, 0x7F],
        ),
        (SampleFormat::F32, 0.25, &0.25_f32.to_le_bytes()),
    ];
    for (format, sample, expected) in cases {
        let mut out = Vec::new();
        Quantizer::new(format).write(&[sample], &mut out);
        assert_eq!(out, expected, "{format} of {sample}");
    }
}

#[test]
fn sixteen_bit_survives_thirty_two_bit_container() {
    let samples: Vec<f32> = (i16::MIN..=i16::MAX).map(sixteen).collect();
    let mut out = Vec::new();
    Quantizer::new(SampleFormat::S32).write(&samples, &mut out);
    for (value, bytes) in (i16::MIN..=i16::MAX).zip(out.as_chunks::<4>().0) {
        assert_eq!(i32::from_le_bytes(*bytes), i32::from(value) << 16);
    }
}

#[test]
fn full_scale_clips_instead_of_wrapping() {
    let mut out = Vec::new();
    Quantizer::new(SampleFormat::S16).write(&[1.5, -1.5], &mut out);
    assert_eq!(out, [0xFF, 0x7F, 0x00, 0x80]);
}

#[test]
fn dither_moves_at_most_one_step() {
    let samples: Vec<f32> = (0..4096)
        .map(|at| sixteen((at % 200) as i16 - 100) + 0.3 / 32_768.0)
        .collect();
    let mut out = Vec::new();
    let mut quantizer = Quantizer::new(SampleFormat::S16).dithered(true);
    quantizer.write(&samples, &mut out);
    let mut moved = 0;
    for (sample, bytes) in samples.iter().zip(out.as_chunks::<2>().0) {
        let exact = f64::from(*sample) * 32_768.0;
        let quantized = f64::from(i16::from_le_bytes(*bytes));
        assert!(
            (quantized - exact).abs() <= 1.31,
            "{quantized} strayed from {exact}"
        );
        moved += usize::from(quantized != exact.round());
    }
    assert!(moved > 0, "dither never changed a sample");
}

#[test]
fn significant_bits_fit_container() {
    let mut out = Vec::new();
    Quantizer::new(SampleFormat::S32)
        .significant_bits(24)
        .write(&[1.0 / 2_147_483_648.0 * 300.0], &mut out);
    assert_eq!(i32::from_le_bytes([out[0], out[1], out[2], out[3]]), 256);
}
