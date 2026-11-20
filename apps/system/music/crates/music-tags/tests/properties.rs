use std::path::PathBuf;

use music_core::Codec;
use music_tags::Error;
use rikka_audio_decode::WavDepth;

use crate::support::{self, Sample};

struct Case {
    name: &'static str,
    make: fn(&std::path::Path) -> PathBuf,
    codec: Codec,
    lossless: bool,
    rate: Option<u32>,
    bits: Option<u8>,
    seconds: f64,
}

#[test]
fn containers_report_codec_and_resolution() {
    let cases = [
        Case {
            name: "wav pcm 16",
            make: |dir| support::wav(dir, "a.wav", 44_100, WavDepth::Pcm16, 1.0),
            codec: Codec::Pcm,
            lossless: true,
            rate: Some(44_100),
            bits: Some(16),
            seconds: 1.0,
        },
        Case {
            name: "wav pcm 24 at 96 kHz",
            make: |dir| support::wav(dir, "b.wav", 96_000, WavDepth::Pcm24, 0.5),
            codec: Codec::Pcm,
            lossless: true,
            rate: Some(96_000),
            bits: Some(24),
            seconds: 0.5,
        },
        Case {
            name: "flac",
            make: |dir| support::flac(dir, "c.flac", 96_000, 24, 192_000),
            codec: Codec::Flac,
            lossless: true,
            rate: Some(96_000),
            bits: Some(24),
            seconds: 2.0,
        },
        Case {
            name: "mp3",
            make: |dir| support::mp3(dir, "d.mp3", 106),
            codec: Codec::Mp3,
            lossless: false,
            rate: Some(44_100),
            bits: None,
            seconds: 2.77,
        },
        Case {
            name: "opus",
            make: |dir| support::opus(dir, "e.opus", 3),
            codec: Codec::Opus,
            lossless: false,
            rate: Some(44_100),
            bits: None,
            seconds: 3.0,
        },
        Case {
            name: "aac in mp4",
            make: |dir| support::mp4(dir, "f.m4a", Sample::Aac),
            codec: Codec::Aac,
            lossless: false,
            rate: Some(44_100),
            bits: None,
            seconds: 2.0,
        },
        Case {
            name: "alac in mp4",
            make: |dir| {
                support::mp4(
                    dir,
                    "g.m4a",
                    Sample::Alac {
                        bits: 24,
                        rate: 96_000,
                    },
                )
            },
            codec: Codec::Alac,
            lossless: true,
            rate: Some(96_000),
            bits: Some(24),
            seconds: 2.0,
        },
    ];
    let dir = support::folder();
    for case in cases {
        let read = music_tags::read((case.make)(dir.path()))
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let audio = read.audio;
        assert_eq!(audio.codec, case.codec, "{}", case.name);
        assert_eq!(audio.lossless, case.lossless, "{}", case.name);
        assert_eq!(audio.sample_rate, case.rate, "{}", case.name);
        assert_eq!(audio.bit_depth, case.bits, "{}", case.name);
        assert_eq!(audio.channels, Some(2), "{}", case.name);
        let off = (audio.duration.as_secs_f64() - case.seconds).abs();
        assert!(off < 0.05, "{}: {:?}", case.name, audio.duration);
    }
}

#[test]
fn untagged_formats_are_named() {
    let dir = support::folder();
    let cases: [(&str, &[u8], Error); 4] = [
        (
            "a.tak",
            b"tBaK\0\0\0\0\0\0\0\0",
            Error::Unsupported(Codec::Tak),
        ),
        ("b.tta", b"TTA1\x01\0\x02\0", Error::Unsupported(Codec::Tta)),
        (
            "c.wma",
            &[
                0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62,
                0xCE, 0x6C, 0, 0,
            ],
            Error::Unsupported(Codec::Wma),
        ),
        ("d.bin", b"nothing musical here", Error::Unknown),
    ];
    for (name, bytes, expected) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let error = music_tags::read(&path).expect_err(name);
        assert_eq!(error, expected, "{name}");
        assert!(error.is_unsupported(), "{name}");
    }
}

#[test]
fn damaged_file_returns_error() {
    let dir = support::folder();
    let path = dir.path().join("broken.flac");
    std::fs::write(&path, b"fLaC\x80\0\0\x22tiny").unwrap();
    assert!(music_tags::read(&path).is_err());
}
