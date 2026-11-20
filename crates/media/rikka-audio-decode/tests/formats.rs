use rikka_audio_decode::Codec;

use crate::tones::{self, FRAMES, RATE};

enum Length {
    Exact { reported: bool },
    Within(u64),
}

struct Case {
    name: &'static str,
    bytes: &'static [u8],
    codec: Codec,
    lossless: bool,
    length: Length,
    aligned: bool,
}

const fn case(
    name: &'static str,
    bytes: &'static [u8],
    codec: Codec,
    lossless: bool,
    length: Length,
    aligned: bool,
) -> Case {
    Case {
        name,
        bytes,
        codec,
        lossless,
        length,
        aligned,
    }
}

const EXACT: Length = Length::Exact { reported: true };
const UNREPORTED: Length = Length::Exact { reported: false };

const CASES: &[Case] = &[
    case("flac", tones::FLAC, Codec::Flac, true, EXACT, true),
    case("alac", tones::ALAC, Codec::Alac, true, EXACT, true),
    case("wavpack", tones::WAVPACK, Codec::WavPack, true, EXACT, true),
    case("ape", tones::APE, Codec::Ape, true, EXACT, true),
    case("mp3", tones::MP3, Codec::Mp3, false, EXACT, true),
    case("vorbis", tones::VORBIS, Codec::Vorbis, false, EXACT, true),
    case("opus", tones::OPUS, Codec::Opus, false, EXACT, true),
    case("aac edit list", tones::AAC, Codec::Aac, false, EXACT, true),
    case(
        "aac itunsmpb",
        tones::AAC_ITUNES,
        Codec::Aac,
        false,
        EXACT,
        true,
    ),
    case(
        "he-aac",
        tones::HE_AAC,
        Codec::HeAac,
        false,
        UNREPORTED,
        false,
    ),
    case(
        "he-aac v2",
        tones::HE_AAC_V2,
        Codec::HeAacV2,
        false,
        UNREPORTED,
        false,
    ),
    case(
        "mp2",
        tones::MP2,
        Codec::Mp2,
        false,
        Length::Within(1152),
        false,
    ),
    case(
        "wma",
        tones::WMA,
        Codec::Wma,
        false,
        Length::Within(2048),
        false,
    ),
    case(
        "ac-3",
        tones::AC3,
        Codec::Ac3,
        false,
        Length::Within(1536),
        false,
    ),
    case(
        "e-ac-3",
        tones::EAC3,
        Codec::Eac3,
        false,
        Length::Within(1536),
        false,
    ),
];

#[test]
fn every_format_family_round_trips() {
    let reference = tones::reference();
    for case in CASES {
        let name = case.name;
        let info = rikka_audio_decode::read(case.bytes)
            .probe()
            .unwrap_or_else(|error| {
                panic!("{name}: {error}");
            });
        assert_eq!((info.sample_rate, info.channels), (RATE, 2), "{name}");
        assert_eq!(info.encoding.codec, case.codec, "{name}");
        assert_eq!(info.encoding.lossless, case.lossless, "{name}");
        assert!(info.encoding.bitrate.is_some_and(|rate| rate > 0), "{name}");

        let sound = rikka_audio_decode::read(case.bytes).decode().unwrap();
        let frames = sound.frames();
        match case.length {
            Length::Exact { reported } => {
                assert_eq!(frames, FRAMES, "{name} decodes its true length");
                if reported {
                    assert_eq!(info.frames, Some(FRAMES), "{name} reports its true length");
                }
            }
            Length::Within(slack) => {
                assert!(frames.abs_diff(FRAMES) <= slack, "{name}: {frames} frames");
            }
        }
        if case.lossless {
            assert_eq!(sound.samples(), reference.as_slice(), "{name} is lossless");
        }
        if case.aligned {
            assert_eq!(
                tones::lag(&reference, sound.samples()),
                0,
                "{name} starts where the source starts"
            );
        }
    }
}
