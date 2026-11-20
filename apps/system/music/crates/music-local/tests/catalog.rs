use lofty::tag::ItemKey;
use music_core::{Codec, Error, Fidelity, Isrc, Resolution, Shelf};

use crate::support::{CD, Fixture, HI_RES, block, png, tagged};

#[test]
fn songs_map_to_tracks_with_fidelity_and_artwork() {
    let fixture = Fixture::new();
    let mut items = tagged("Brave Shine", "Aimer", "Daydream");
    items.extend([(ItemKey::TrackNumber, "3"), (ItemKey::Isrc, "JPU901500571")]);
    fixture.song_with("Aimer/Daydream/03.flac", HI_RES, &items, Some(&png(64)));
    fixture.song(
        "Aimer/Daydream/04.flac",
        CD,
        &tagged("Insane Dream", "Aimer", "Daydream"),
    );
    let fixture_library = fixture.open();
    let catalog = &fixture_library.local.catalog;

    let results = block(catalog.search("brave")).unwrap();
    assert_eq!(results.tracks.len(), 1);
    let track = &results.tracks[0];
    assert!(track.service.is_local());
    assert!(track.playable);
    assert_eq!(track.name, "Brave Shine");
    assert_eq!(track.number, 3);
    assert_eq!(track.isrc, Isrc::parse("JPU901500571"));
    let fidelity = track.fidelity.unwrap();
    assert_eq!(
        fidelity,
        Fidelity::lossless(
            Codec::Flac,
            Resolution {
                bits: 24,
                rate: 96_000
            }
        )
    );
    assert!(fidelity.is_high());
    let cover = track.artwork.fitting(64).expect("the cover");
    let file = cover.strip_prefix("file://").expect("a file URI");
    assert!(std::path::Path::new(file).is_file());
    assert!(
        track
            .artwork
            .pictures()
            .iter()
            .any(|picture| picture.width == 64)
    );

    let album = block(catalog.album(track.album.id.as_ref().unwrap())).unwrap();
    assert_eq!(album.album.name, "Daydream");
    let names: Vec<&str> = album
        .tracks
        .iter()
        .map(|track| track.name.as_str())
        .collect();
    assert_eq!(names, ["Brave Shine", "Insane Dream"]);
    assert!(!album.tracks[1].fidelity.unwrap().is_high());

    let artist = block(catalog.artist(track.artists[0].id.as_ref().unwrap())).unwrap();
    assert_eq!(artist.artist.name, "Aimer");
    assert_eq!(artist.popular.len(), 2);
    assert_eq!(artist.albums.len() + artist.singles.len(), 1);
}

#[test]
fn favourites_are_saved_songs_and_shelf() {
    let fixture = Fixture::new();
    fixture.song("A/1.flac", CD, &tagged("One", "Band", "First"));
    fixture.song("A/2.flac", CD, &tagged("Two", "Band", "First"));
    let fixture_library = fixture.open();
    let catalog = &fixture_library.local.catalog;
    let one = block(catalog.search("one")).unwrap().tracks[0].clone();

    block(catalog.set_saved(&one.id, true)).unwrap();
    let saved = block(catalog.saved_tracks()).unwrap();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].is(&one));
    let home = block(catalog.home()).unwrap();
    assert!(
        home.shelves
            .iter()
            .any(|shelf| matches!(shelf, Shelf::RecentlyAdded(albums) if albums.len() == 1))
    );
    assert!(
        home.shelves
            .iter()
            .any(|shelf| matches!(shelf, Shelf::Favourites(tracks) if tracks.len() == 1))
    );

    block(catalog.set_saved(&one.id, false)).unwrap();
    assert!(block(catalog.saved_tracks()).unwrap().is_empty());
    assert!(fixture_library.library.favourites().unwrap().is_empty());
}

#[test]
fn service_playlists_are_unsupported() {
    let fixture = Fixture::new();
    fixture.song("A/1.flac", CD, &tagged("One", "Band", "First"));
    let fixture_library = fixture.open();
    let catalog = &fixture_library.local.catalog;

    assert!(block(catalog.playlists()).unwrap().is_empty());
    assert!(matches!(
        block(catalog.create_playlist("rikka: Night", "rikka:x")),
        Err(Error::Unsupported(_))
    ));
}
