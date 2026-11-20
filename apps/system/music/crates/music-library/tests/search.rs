use std::time::Duration;

use lofty::tag::ItemKey;
use music_core::Isrc;

use crate::support::{self, CD, Fixture, Sound};

#[test]
fn search_matches_substrings_ignoring_accents() {
    let fixture = Fixture::new();
    support::song(
        &fixture.path("J/1.flac"),
        CD,
        &[
            (ItemKey::TrackTitle, "初恋"),
            (ItemKey::TrackArtist, "宇多田ヒカル"),
            (ItemKey::AlbumTitle, "初恋"),
        ],
    );
    support::song(
        &fixture.path("B/1.flac"),
        CD,
        &[
            (ItemKey::TrackTitle, "Halo"),
            (ItemKey::TrackArtist, "Beyoncé"),
            (ItemKey::AlbumTitle, "I Am... Sasha Fierce"),
            (ItemKey::Composer, "Ryan Tedder"),
        ],
    );
    let library = fixture.open();
    support::scan(&library);

    let cases: [(&str, &[&str]); 7] = [
        ("宇多田", &["初恋"]),
        ("多田", &["初恋"]),
        ("ヒカル 初恋", &["初恋"]),
        ("beyonce", &["Halo"]),
        ("SASHA", &["Halo"]),
        ("tedder", &["Halo"]),
        ("nothing like this", &[]),
    ];
    for (query, expected) in cases {
        let results = library.search(query, 20).unwrap();
        let titles: Vec<&str> = results
            .tracks
            .iter()
            .map(|track| track.title.as_str())
            .collect();
        assert_eq!(titles, expected, "{query}");
    }
    let results = library.search("ｂｅｙｏｎｃｅ", 20).unwrap();
    assert_eq!(results.artists.len(), 1);
    assert_eq!(results.artists[0].name, "Beyoncé");
    assert_eq!(results.albums.len(), 1);
    let results = library.search("宇多", 20).unwrap();
    assert_eq!(results.artists[0].name, "宇多田ヒカル");
}

#[test]
fn lookup_matches_isrc_then_title_and_length() {
    let fixture = Fixture::new();
    support::song(
        &fixture.path("M/1.flac"),
        Sound {
            rate: 44_100,
            bits: 16,
            seconds: 200,
        },
        &[
            (ItemKey::TrackTitle, "Bohemian Rhapsody"),
            (ItemKey::TrackArtist, "Queen"),
            (ItemKey::AlbumTitle, "A Night at the Opera"),
            (ItemKey::Isrc, "GBUM71029604"),
        ],
    );
    let library = fixture.open();
    support::scan(&library);

    let isrc = Isrc::parse("GB-UM7-10-29604").unwrap();
    let by_isrc = library.by_isrc(&isrc).unwrap();
    assert_eq!(by_isrc.len(), 1);
    assert_eq!(by_isrc[0].isrc.as_ref(), Some(&isrc));

    let window = Duration::from_secs(3);
    let cases = [
        ("bohemian rhapsody", "QUEEN", 201, 1),
        ("Bohemian Rhapsody", "Queen", 210, 0),
        ("Bohemian Rhapsody", "Someone Else", 200, 0),
        ("Another One", "Queen", 200, 0),
    ];
    for (title, artist, seconds, expected) in cases {
        let results = library
            .matching(title, artist, Duration::from_secs(seconds), window)
            .unwrap();
        assert_eq!(results.len(), expected, "{title} / {artist} / {seconds}");
    }
    let path = &library.track(by_isrc[0].id).unwrap().unwrap().path;
    assert!(path.ends_with("M/1.flac"));
}
