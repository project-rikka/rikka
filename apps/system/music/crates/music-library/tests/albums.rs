use lofty::tag::ItemKey;
use music_core::Codec;
use music_library::{AlbumSort, Albums, Counts, Order};

use crate::support::{self, CD, Fixture, Sound};

#[test]
fn tracks_group_by_tags_and_folders() {
    let fixture = Fixture::new();
    for (file, artist) in [
        ("01.flac", "Alpha"),
        ("02.flac", "Beta"),
        ("03.flac", "Gamma"),
    ] {
        support::song(
            &fixture.path(&format!("Hits/{file}")),
            CD,
            &[
                (ItemKey::TrackTitle, file),
                (ItemKey::TrackArtist, artist),
                (ItemKey::AlbumTitle, "Now 99"),
                (ItemKey::FlagCompilation, "1"),
            ],
        );
    }
    for disc in ["1", "2"] {
        for track in ["1", "2"] {
            support::song(
                &fixture.path(&format!("Epic/CD{disc}/{track}.flac")),
                CD,
                &[
                    (ItemKey::TrackTitle, &format!("{disc}-{track}")),
                    (ItemKey::TrackArtist, "Band"),
                    (ItemKey::AlbumArtist, "Band"),
                    (ItemKey::AlbumTitle, "Epic"),
                    (ItemKey::DiscNumber, disc),
                    (ItemKey::TrackNumber, track),
                ],
            );
        }
    }
    for folder in ["Release A", "Release B"] {
        support::song(
            &fixture.path(&format!("{folder}/x.flac")),
            CD,
            &[
                (ItemKey::TrackTitle, folder),
                (ItemKey::TrackArtist, "Solo"),
                (ItemKey::AlbumTitle, "Split Up"),
                (
                    ItemKey::MusicBrainzReleaseId,
                    "5b11f4ce-a62d-471e-81fc-a69a8278c7da",
                ),
            ],
        );
    }
    let library = fixture.open();
    support::scan(&library);

    let albums = library
        .albums(Albums::new(AlbumSort::Title).order(Order::Ascending))
        .unwrap();
    let summary: Vec<(&str, &str, u32, u32, bool)> = albums
        .iter()
        .map(|album| {
            (
                album.title.as_str(),
                album.artist.as_str(),
                album.tracks,
                album.discs,
                album.compilation,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("Epic", "Band", 4, 2, false),
            ("Now 99", "Various Artists", 3, 1, true),
            ("Split Up", "Solo", 2, 1, false),
        ]
    );

    let epic = library.album(albums[0].id).unwrap().unwrap();
    let order: Vec<&str> = epic
        .tracks
        .iter()
        .map(|track| track.title.as_str())
        .collect();
    assert_eq!(order, ["1-1", "1-2", "2-1", "2-2"]);

    let artists = library.artists().unwrap();
    let names: Vec<&str> = artists.iter().map(|artist| artist.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Band", "Beta", "Gamma", "Solo"]);
    let alpha = library.artist(artists[0].id).unwrap().unwrap();
    assert!(alpha.albums.is_empty());
    assert_eq!(alpha.appearances.len(), 1);
    assert_eq!(alpha.appearances[0].title, "Now 99");
    let band = library.artist(artists[1].id).unwrap().unwrap();
    assert_eq!(band.albums.len(), 1);
    assert_eq!(
        library
            .albums(Albums::default().artist(artists[0].id))
            .unwrap()[0]
            .title,
        "Now 99"
    );

    let titles = |tracks: Vec<music_library::Track>| -> Vec<String> {
        tracks.into_iter().map(|track| track.title).collect()
    };
    assert_eq!(
        titles(library.artist_tracks(artists[1].id).unwrap()),
        ["1-1", "1-2", "2-1", "2-2"]
    );
    assert_eq!(
        titles(library.artist_tracks(artists[0].id).unwrap()),
        ["01.flac"]
    );
    assert_eq!(
        library
            .tracks_of_albums(&[albums[0].id, albums[2].id])
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        library.counts().unwrap(),
        Counts {
            songs: 9,
            albums: 3,
            artists: 5,
        }
    );
}

#[test]
fn albums_report_fidelity_and_genres() {
    let fixture = Fixture::new();
    let hi_res = Sound {
        rate: 96_000,
        bits: 24,
        seconds: 2,
    };
    for track in ["1", "2"] {
        support::song(
            &fixture.path(&format!("Hi/{track}.flac")),
            hi_res,
            &[
                (ItemKey::TrackTitle, track),
                (ItemKey::TrackArtist, "Audiophile"),
                (ItemKey::AlbumTitle, "Hi Res"),
                (ItemKey::Genre, "Jazz"),
                (ItemKey::RecordingDate, "2019-03-01"),
            ],
        );
    }
    support::song(
        &fixture.path("Cd/1.flac"),
        CD,
        &[
            (ItemKey::TrackTitle, "Plain"),
            (ItemKey::TrackArtist, "Listener"),
            (ItemKey::AlbumTitle, "Red Book"),
            (ItemKey::Genre, "Rock"),
            (ItemKey::RecordingDate, "1999"),
        ],
    );
    let library = fixture.open();
    support::scan(&library);

    let albums = library.albums(Albums::new(AlbumSort::Year)).unwrap();
    let titles: Vec<&str> = albums.iter().map(|album| album.title.as_str()).collect();
    assert_eq!(titles, ["Red Book", "Hi Res"]);
    let hi = &albums[1];
    assert_eq!(hi.year, Some(2019));
    assert_eq!(hi.fidelity.codec, Codec::Flac);
    assert!(hi.fidelity.is_high());
    assert_eq!(
        hi.fidelity.resolution,
        Some(music_core::Resolution {
            bits: 24,
            rate: 96_000
        })
    );
    assert!(!albums[0].fidelity.is_high());

    let genres = library.genres().unwrap();
    let names: Vec<(&str, u32)> = genres
        .iter()
        .map(|genre| (genre.name.as_str(), genre.tracks))
        .collect();
    assert_eq!(names, [("Jazz", 2), ("Rock", 1)]);
    let jazz = library
        .albums(Albums::default().genre(genres[0].id))
        .unwrap();
    assert_eq!(jazz.len(), 1);
    assert_eq!(jazz[0].title, "Hi Res");

    let newest = library.recently_added(1).unwrap();
    assert_eq!(newest.len(), 1);
}
