use std::time::Duration;

use lofty::tag::ItemKey;
use rikka_audio_decode::Codec;

use crate::support;

#[test]
fn dsd_headers_give_format_and_tags() {
    let dir = support::folder();
    let id3 = support::id3v2(&[
        (ItemKey::TrackTitle, "Kind of Blue"),
        (ItemKey::TrackArtist, "Miles Davis"),
        (ItemKey::TrackNumber, "2"),
    ]);
    let cases = [
        (
            "dsf dsd64",
            support::dsf(dir.path(), "a.dsf", 2_822_400, 2_822_400 * 3, &id3),
            2_822_400,
            Duration::from_secs(3),
            true,
        ),
        (
            "dsf dsd256 untagged",
            support::dsf(dir.path(), "b.dsf", 11_289_600, 11_289_600, &[]),
            11_289_600,
            Duration::from_secs(1),
            false,
        ),
        (
            "dff dsd128",
            support::dff(dir.path(), "c.dff", 5_644_800, 5_644_800 / 8, &id3),
            5_644_800,
            Duration::from_secs_f64(0.5),
            true,
        ),
        (
            "dff dsd512 untagged",
            support::dff(dir.path(), "d.dff", 22_579_200, 22_579_200 / 8, &[]),
            22_579_200,
            Duration::from_secs_f64(0.5),
            false,
        ),
    ];
    for (name, path, rate, duration, tagged) in cases {
        let read = rikka_audio_tags::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(read.audio.codec, Codec::Dsd, "{name}");
        assert!(read.audio.lossless, "{name}");
        assert_eq!(read.audio.sample_rate, Some(rate), "{name}");
        assert_eq!(read.audio.channels, Some(2), "{name}");
        assert_eq!(read.audio.bit_depth, Some(1), "{name}");
        let off = (read.audio.duration.as_secs_f64() - duration.as_secs_f64()).abs();
        assert!(off < 0.001, "{name}: {:?}", read.audio.duration);
        if tagged {
            assert_eq!(read.tags.title.as_deref(), Some("Kind of Blue"), "{name}");
            assert_eq!(read.tags.artists, ["Miles Davis"], "{name}");
            assert_eq!(read.tags.track.number, Some(2), "{name}");
        } else {
            assert_eq!(read.tags, rikka_audio_tags::Tags::default(), "{name}");
        }
    }
}

#[test]
fn truncated_dsd_header_is_unreadable() {
    let dir = support::folder();
    let path = dir.path().join("cut.dsf");
    std::fs::write(&path, b"DSD \x1c\0\0\0\0\0\0\0").unwrap();
    assert!(matches!(
        rikka_audio_tags::read(&path),
        Err(rikka_audio_tags::Error::Unreadable(_))
    ));
}
