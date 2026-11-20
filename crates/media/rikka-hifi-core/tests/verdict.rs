use rikka_hifi_core::{
    Codec, DeviceId, ExclusivePath, OutputPath, Processor, SampleFormat, SharedPath, SignalPath,
    SinkPath, SourceFormat, Stage, Verdict, VolumeControl,
};

fn flac(rate: u32, bits: u32) -> SourceFormat {
    SourceFormat::pcm(rate, 2).bits(bits).codec(Codec::Flac)
}

fn exclusive(format: SampleFormat) -> OutputPath {
    OutputPath::Exclusive(ExclusivePath {
        device: DeviceId::new("K11", 0),
        name: "FiiO K11".into(),
        rate: 44_100,
        channels: 2,
        format,
        dop: false,
    })
}

fn sink(rate: u32, format: SampleFormat, bluetooth: Option<&str>) -> SinkPath {
    SinkPath {
        name: "alsa_output.usb".into(),
        description: "USB DAC".into(),
        rate: Some(rate),
        format: Some(format),
        channels: Some(2),
        bluetooth: bluetooth.map(str::to_owned),
        card: None,
        hardware_rates: Vec::new(),
    }
}

fn shared(stream_rate: u32, sink: SinkPath) -> SharedPath {
    SharedPath {
        stream_rate,
        stream_channels: 2,
        stream_volume: 1.0,
        sink: Some(sink),
        sink_volume: None,
        clock_rate: Some(48_000),
        forced_rate: None,
        allowed_rates: vec![44_100, 48_000],
    }
}

fn path(source: SourceFormat, stages: Vec<Stage>, output: OutputPath) -> SignalPath {
    SignalPath {
        source,
        stages,
        output,
    }
}

fn hardware(level: f32) -> Stage {
    Stage::Volume {
        level,
        by: VolumeControl::Hardware,
    }
}

#[test]
fn verdict_reflects_output() {
    let shared_hifi = shared(96_000, sink(96_000, SampleFormat::S32, None));
    let resampled = shared(44_100, sink(48_000, SampleFormat::S32, None));
    let cases = [
        (
            path(
                flac(44_100, 16),
                vec![hardware(0.3)],
                exclusive(SampleFormat::S32),
            ),
            Verdict::BitPerfect,
        ),
        (
            path(
                SourceFormat::dsd(2_822_400, 2),
                vec![Stage::Dop],
                exclusive(SampleFormat::S32),
            ),
            Verdict::BitPerfect,
        ),
        (
            path(
                flac(44_100, 16),
                vec![Stage::Remix {
                    from: 1,
                    to: 2,
                    by: Processor::Rikka,
                }],
                exclusive(SampleFormat::S32),
            ),
            Verdict::Lossless,
        ),
        (
            path(
                SourceFormat::pcm(44_100, 2),
                vec![],
                exclusive(SampleFormat::S32),
            ),
            Verdict::Lossless,
        ),
        (
            path(
                flac(96_000, 24),
                shared_hifi.stages(),
                OutputPath::Shared(shared_hifi.clone()),
            ),
            Verdict::Lossless,
        ),
        (
            path(
                flac(44_100, 16),
                vec![
                    Stage::Volume {
                        level: 0.5,
                        by: VolumeControl::Software,
                    },
                    Stage::Dither { bits: 32 },
                ],
                exclusive(SampleFormat::S32),
            ),
            Verdict::HighQuality,
        ),
        (
            path(
                flac(44_100, 16),
                resampled.stages(),
                OutputPath::Shared(resampled.clone()),
            ),
            Verdict::HighQuality,
        ),
        (
            path(flac(96_000, 24), vec![], exclusive(SampleFormat::S16)),
            Verdict::Lossy,
        ),
        (
            path(
                flac(96_000, 24),
                vec![Stage::Resample {
                    from: 96_000,
                    to: 48_000,
                    by: Processor::Rikka,
                }],
                exclusive(SampleFormat::S32),
            ),
            Verdict::Lossy,
        ),
        (
            path(
                SourceFormat::pcm(44_100, 2)
                    .codec(Codec::Vorbis)
                    .bitrate(320_000),
                vec![],
                exclusive(SampleFormat::S32),
            ),
            Verdict::Lossy,
        ),
        (
            path(
                flac(48_000, 16),
                vec![],
                OutputPath::Shared(shared(
                    48_000,
                    sink(48_000, SampleFormat::S16, Some("ldac")),
                )),
            ),
            Verdict::Lossy,
        ),
    ];
    for (signal, expected) in cases {
        assert_eq!(signal.verdict(), expected, "{signal:#?}");
    }
}

#[test]
fn shared_path_reports_pipewire_graph() {
    let mut graph = shared(44_100, sink(192_000, SampleFormat::S32, None));
    graph.stream_volume = 0.7;
    graph.sink_volume = Some(0.4);
    assert_eq!(
        graph.stages(),
        [
            Stage::Volume {
                level: 0.7,
                by: VolumeControl::PipeWire
            },
            Stage::Resample {
                from: 44_100,
                to: 192_000,
                by: Processor::PipeWire
            },
            Stage::Volume {
                level: 0.4,
                by: VolumeControl::Sink
            },
        ]
    );

    graph.sink = None;
    graph.forced_rate = Some(44_100);
    assert_eq!(graph.graph_rate(), Some(44_100));
}
