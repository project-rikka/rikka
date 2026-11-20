use lofty::tag::ItemKey;
use music_core::{Isrc, Track};

use crate::support::{CD, Fixture, block, streaming, tagged};

fn coded(mut track: Track, isrc: &str) -> Track {
    track.isrc = Isrc::parse(isrc);
    track
}

#[test]
fn streaming_song_finds_local_copy() {
    let fixture = Fixture::new();
    let mut coded_items = tagged("Tracks of Code", "Coder", "Codes");
    coded_items.push((ItemKey::Isrc, "USRC17607839"));
    fixture.song("C/1.flac", CD, &coded_items);
    fixture.song("Q/1.flac", CD, &tagged("Yesterday", "The Beatles", "Help!"));
    let fixture_library = fixture.open();
    let catalog = fixture_library.local.catalog.as_ref();

    let cases: [(&str, Track, Option<&str>); 6] = [
        (
            "the same code under another title",
            coded(streaming("Renamed", "Somebody", 300), "USRC17607839"),
            Some("Tracks of Code"),
        ),
        (
            "the same title, artist and length",
            streaming("Yesterday", "The Beatles", 2),
            Some("Yesterday"),
        ),
        (
            "a remastered title",
            streaming("Yesterday - Remastered 2009", "The Beatles", 2),
            Some("Yesterday"),
        ),
        (
            "a much longer song",
            streaming("Yesterday", "The Beatles", 120),
            None,
        ),
        (
            "another artist",
            streaming("Yesterday", "Boyz II Men", 2),
            None,
        ),
        (
            "a song not in the library",
            streaming("Help", "Nobody", 2),
            None,
        ),
    ];
    for (case, target, expected) in cases {
        let matched = block(music_blend::find(catalog, &target)).unwrap();
        assert_eq!(
            matched.as_ref().map(|track| track.name.as_str()),
            expected,
            "{case}"
        );
        if let Some(matched) = matched {
            assert!(matched.service.is_local(), "{case}");
        }
    }
}
