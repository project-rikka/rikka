use rikka_hifi_core::{
    Carriage, Codec, Pipeline, Plan, Processor, SampleFormat, SourceFormat, Stage, VolumeControl,
};

fn flac(rate: u32, bits: u32) -> SourceFormat {
    SourceFormat::pcm(rate, 2).bits(bits).codec(Codec::Flac)
}

fn exclusive(rate: u32, format: SampleFormat) -> Plan {
    Plan {
        rate,
        channels: 2,
        format,
        carriage: Carriage::Pcm,
    }
}

fn sixteen_bit_ramp() -> (Vec<i16>, Vec<f32>) {
    let values: Vec<i16> = (0..2048)
        .map(|at| (at * 37 % 65_536 - 32_768) as i16)
        .collect();
    let samples = values
        .iter()
        .map(|value| f32::from(*value) / 32_768.0)
        .collect();
    (values, samples)
}

#[test]
fn unity_gain_is_bit_exact() {
    let (values, samples) = sixteen_bit_ramp();
    let mut pipeline = Pipeline::new(&flac(44_100, 16), exclusive(44_100, SampleFormat::S16));
    pipeline.set_replay_gain(0.0);
    pipeline.set_volume(Some(1.0));
    let mut out = Vec::new();
    pipeline.pcm(&samples, &mut out);
    pipeline.finish(&mut out);

    let samples: Vec<i16> = out
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| i16::from_le_bytes(*bytes))
        .collect();
    assert_eq!(samples, values);
    assert_eq!(
        pipeline.stages(),
        [Stage::Volume {
            level: 1.0,
            by: VolumeControl::Software
        }]
    );
}

#[test]
fn dither_only_altered_integer_output() {
    let cases = [
        (exclusive(44_100, SampleFormat::S32), -3.0, None, true),
        (exclusive(44_100, SampleFormat::S32), 0.0, Some(0.5), true),
        (exclusive(96_000, SampleFormat::S32), 0.0, None, true),
        (exclusive(44_100, SampleFormat::S32), 0.0, None, false),
        (exclusive(44_100, SampleFormat::F32), -3.0, None, false),
    ];
    for (plan, replay_gain, volume, dithered) in cases {
        let mut pipeline = Pipeline::new(&flac(44_100, 16), plan);
        pipeline.set_replay_gain(replay_gain);
        pipeline.set_volume(volume);
        let dither = pipeline
            .stages()
            .iter()
            .any(|stage| matches!(stage, Stage::Dither { bits: 32 }));
        assert_eq!(
            dither, dithered,
            "{plan:?} at {replay_gain} dB, volume {volume:?}"
        );
    }
}

#[test]
fn stages_follow_processing_order() {
    let source = SourceFormat::pcm(44_100, 1).bits(24);
    let mut pipeline = Pipeline::new(&source, exclusive(96_000, SampleFormat::S24Packed));
    pipeline.set_replay_gain(-2.5);
    pipeline.set_volume(Some(0.8));
    assert_eq!(
        pipeline.stages(),
        [
            Stage::Resample {
                from: 44_100,
                to: 96_000,
                by: Processor::Rikka
            },
            Stage::Remix {
                from: 1,
                to: 2,
                by: Processor::Rikka
            },
            Stage::ReplayGain { db: -2.5 },
            Stage::Volume {
                level: 0.8,
                by: VolumeControl::Software
            },
            Stage::Dither { bits: 24 },
        ]
    );
}

#[test]
fn gain_scales_samples() {
    let mut pipeline = Pipeline::new(&flac(48_000, 24), exclusive(48_000, SampleFormat::F32));
    pipeline.set_volume(Some(0.5));
    let mut out = Vec::new();
    pipeline.pcm(&[0.5, -0.25], &mut out);
    assert_eq!(
        out,
        [0.25_f32.to_le_bytes(), (-0.125_f32).to_le_bytes()].concat()
    );
}

#[test]
fn resampling_produces_target_rate() {
    let mut pipeline = Pipeline::new(&flac(44_100, 16), exclusive(88_200, SampleFormat::S32));
    let mut out = Vec::new();
    pipeline.pcm(&vec![0.0; 44_100 * 2], &mut out);
    pipeline.finish(&mut out);
    let frames = out.len() / 8;
    assert!((88_100..88_200 + 1024).contains(&frames), "{frames} frames");
}

#[test]
fn dsd_ignores_gain_and_reports_dop() {
    let source = SourceFormat::dsd(2_822_400, 2);
    let mut dop = Pipeline::new(
        &source,
        Plan {
            rate: 176_400,
            channels: 2,
            format: SampleFormat::S24Packed,
            carriage: Carriage::Dop,
        },
    );
    dop.set_replay_gain(-6.0);
    dop.set_volume(Some(0.5));
    assert_eq!(dop.stages(), [Stage::Dop]);

    let mut out = Vec::new();
    dop.dsd(&[0x69; 8], &mut out);
    assert_eq!(out.len(), 2 * 2 * 3);

    let converted = Pipeline::new(&source, Plan::shared(&source));
    assert_eq!(
        converted.stages(),
        [Stage::DsdToPcm {
            from: 2_822_400,
            to: 88_200
        }]
    );

    let decoded = SourceFormat::pcm(352_800, 2).encoding(source.encoding);
    let pipeline = Pipeline::new(&decoded, Plan::shared(&decoded));
    assert_eq!(
        pipeline.stages(),
        [Stage::DsdToPcm {
            from: 2_822_400,
            to: 352_800
        }]
    );
}
