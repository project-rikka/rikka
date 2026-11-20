use std::time::Duration;

use music_blend::find;
use music_core::{Credit, Isrc, Track};

use crate::support::{Shelf, settled, track};

fn song(service: &str, id: &str, name: &str, artist: &str, seconds: u64) -> Track {
    let mut made = track(service, id);
    made.name = name.into();
    made.artists = vec![Credit {
        id: None,
        name: artist.into(),
    }];
    made.duration = Duration::from_secs(seconds);
    made
}

fn best_match(target: &Track, candidates: Vec<Track>) -> Option<String> {
    let shelf = Shelf::new("qobuz");
    for candidate in candidates {
        shelf.stock(candidate);
    }
    settled(find(shelf.as_ref(), target))
        .unwrap()
        .map(|track| track.id.to_string())
}

#[test]
fn matching_isrc_settles_match() {
    let mut target = song("spotify", "a", "One", "Band", 200);
    let mut other = song("qobuz", "1", "Completely Different", "Other", 10);
    target.isrc = Isrc::parse("USUM71703861");
    other.isrc = target.isrc.clone();

    assert_eq!(best_match(&target, vec![other]).as_deref(), Some("1"));
}

#[test]
fn title_decorations_match_but_versions_differ() {
    let yesterday = song(
        "spotify",
        "a",
        "Yesterday - Remastered 2009",
        "The Beatles",
        125,
    );
    let cases = [
        (&yesterday, ("Yesterday", "The Beatles", 126), true),
        (
            &yesterday,
            ("Yesterday (Official Audio)", "The Beatles - Topic", 0),
            true,
        ),
        (
            &song("spotify", "b", "ＹＯＡＳＯＢＩ", "夜に駆ける", 261),
            ("yoasobi", "夜に駆ける", 260),
            true,
        ),
        (
            &song("spotify", "c", "Déjà Vu", "Beyoncé", 240),
            ("Deja Vu", "Beyonce", 241),
            true,
        ),
        (&yesterday, ("Yesterday (Live)", "The Beatles", 125), false),
        (&yesterday, ("Tomorrow", "The Beatles", 125), false),
        (&yesterday, ("Yesterday", "Boyz II Men", 125), false),
        (&yesterday, ("Yesterday", "The Beatles", 190), false),
    ];

    for (target, (name, artist, seconds), same) in cases {
        let candidate = song("qobuz", "1", name, artist, seconds);
        assert_eq!(
            best_match(target, vec![candidate]).is_some(),
            same,
            "{} against {name} by {artist}",
            target.name
        );
    }
}

#[test]
fn closest_playable_candidate_wins() {
    let target = song("spotify", "a", "Song", "Singer", 180);
    let mut greyed = song("qobuz", "1", "Song", "Singer", 180);
    greyed.playable = false;

    let chosen = best_match(
        &target,
        vec![
            greyed,
            song("qobuz", "2", "Song", "Singer", 187),
            song("qobuz", "3", "Song", "Singer", 181),
        ],
    );

    assert_eq!(chosen.as_deref(), Some("3"));
}
