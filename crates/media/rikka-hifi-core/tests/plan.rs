use rikka_hifi_core::{Capabilities, Carriage, Plan, SampleFormat, SourceFormat};

fn usb_dac() -> Capabilities {
    Capabilities {
        formats: vec![
            SampleFormat::S16,
            SampleFormat::S24Packed,
            SampleFormat::S32,
            SampleFormat::Dsd32Be,
        ],
        rates: vec![
            44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000,
        ],
        channels: vec![2],
    }
}

fn plain_card() -> Capabilities {
    Capabilities {
        formats: vec![SampleFormat::S16, SampleFormat::S24Packed],
        rates: vec![48_000, 96_000],
        channels: vec![2, 6],
    }
}

fn plan(format: SampleFormat, rate: u32, channels: u16, carriage: Carriage) -> Plan {
    Plan {
        rate,
        channels,
        format,
        carriage,
    }
}

#[test]
fn exclusive_plan_follows_source() {
    let cases = [
        (
            SourceFormat::pcm(44_100, 2).bits(16),
            usb_dac(),
            plan(SampleFormat::S32, 44_100, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::pcm(96_000, 2).bits(24),
            usb_dac(),
            plan(SampleFormat::S32, 96_000, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::pcm(44_100, 2).bits(16),
            plain_card(),
            plan(SampleFormat::S24Packed, 48_000, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::pcm(24_000, 2),
            plain_card(),
            plan(SampleFormat::S24Packed, 48_000, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::pcm(192_000, 2).bits(24),
            plain_card(),
            plan(SampleFormat::S24Packed, 96_000, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::pcm(48_000, 1),
            plain_card(),
            plan(SampleFormat::S24Packed, 48_000, 2, Carriage::Pcm),
        ),
        (
            SourceFormat::dsd(2_822_400, 2),
            usb_dac(),
            plan(SampleFormat::Dsd32Be, 88_200, 2, Carriage::NativeDsd),
        ),
        (
            SourceFormat::dsd(5_644_800, 2),
            Capabilities {
                formats: vec![SampleFormat::S24, SampleFormat::S32],
                ..usb_dac()
            },
            plan(SampleFormat::S32, 352_800, 2, Carriage::Dop),
        ),
        (
            SourceFormat::dsd(2_822_400, 2),
            plain_card(),
            plan(SampleFormat::S24Packed, 96_000, 2, Carriage::DsdToPcm),
        ),
    ];
    for (source, device, expected) in cases {
        assert_eq!(
            Plan::exclusive(&source, &device),
            Some(expected),
            "{source:?}"
        );
    }
}

#[test]
fn device_without_pcm_formats_is_rejected() {
    let device = Capabilities {
        formats: vec![SampleFormat::Dsd8],
        ..usb_dac()
    };
    assert_eq!(
        Plan::exclusive(&SourceFormat::pcm(44_100, 2), &device),
        None
    );
}

#[test]
fn shared_plan_uses_source_rate_f32() {
    assert_eq!(
        Plan::shared(&SourceFormat::pcm(88_200, 2).bits(24)),
        plan(SampleFormat::F32, 88_200, 2, Carriage::Pcm)
    );
    assert_eq!(
        Plan::shared(&SourceFormat::dsd(11_289_600, 2)),
        plan(SampleFormat::F32, 352_800, 2, Carriage::DsdToPcm)
    );
}

#[test]
fn dsd_support_lists_rates() {
    let support = usb_dac().dsd();
    assert_eq!(support.native, [2_822_400, 5_644_800, 11_289_600]);
    assert_eq!(support.dop, [2_822_400, 5_644_800]);
}
