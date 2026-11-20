use std::time::Duration;

use music_core::{AlbumRef, Artwork, Id, Kind, Mark, Queue, Repeat, Service, Track};

fn track(name: &str) -> Track {
    Track {
        service: Service {
            slug: "queued",
            name: "Queued",
            mark: Mark::Symbol('\u{e030}'),
            kind: Kind::Streaming,
        },
        id: Id::new(name),
        name: name.to_owned(),
        artists: Vec::new(),
        album: AlbumRef::default(),
        artwork: Artwork::default(),
        duration: Duration::from_secs(60),
        explicit: false,
        playable: true,
        number: 1,
        fidelity: None,
        isrc: None,
    }
}

fn names<'a>(tracks: impl Iterator<Item = &'a Track>) -> Vec<&'a str> {
    tracks.map(|track| track.name.as_str()).collect()
}

fn started(from: usize) -> Queue {
    let mut queue = Queue::new(7);
    queue.start(vec![track("a"), track("b"), track("c")], from);
    queue
}

#[test]
fn queue_stops_at_end() {
    let mut queue = started(1);

    assert_eq!(queue.current().unwrap().name, "b");
    assert_eq!(queue.advance(true).unwrap().name, "c");
    assert!(queue.advance(true).is_none());
}

#[test]
fn repeat_all_wraps_repeat_one_stays() {
    let mut queue = started(2);
    queue.set_repeat(Repeat::All);
    assert_eq!(queue.advance(true).unwrap().name, "a");

    queue.set_repeat(Repeat::One);
    assert_eq!(queue.advance(true).unwrap().name, "a");
    assert_eq!(queue.advance(false).unwrap().name, "b");
}

#[test]
fn shuffle_keeps_current_and_each_track_once() {
    let mut queue = started(1);
    queue.set_shuffle(true);

    assert_eq!(queue.current().unwrap().name, "b");
    let mut rest = names(queue.upcoming());
    rest.sort_unstable();
    assert_eq!(rest, vec!["a", "c"]);

    queue.set_shuffle(false);
    assert_eq!(queue.current().unwrap().name, "b");
    assert_eq!(names(queue.upcoming()), vec!["c"]);
}

#[test]
fn play_next_follows_current_when_shuffled() {
    let mut queue = started(0);
    queue.play_next(track("n"));
    assert_eq!(names(queue.upcoming()), vec!["n", "b", "c"]);

    queue.set_shuffle(true);
    queue.play_next(track("m"));
    assert_eq!(queue.upcoming().next().unwrap().name, "m");

    queue.set_shuffle(false);
    assert_eq!(names(queue.upcoming()), vec!["m", "n", "b", "c"]);
}

#[test]
fn removed_track_stays_gone_after_unshuffle() {
    let mut queue = started(0);
    queue.set_shuffle(true);
    let removed = queue.upcoming().next().unwrap().name.clone();

    assert_eq!(queue.remove_upcoming(0).unwrap().name, removed);
    assert!(queue.remove_upcoming(5).is_none());

    queue.set_shuffle(false);
    assert_eq!(queue.current().unwrap().name, "a");
    assert_eq!(queue.len(), 2);
    assert!(!names(queue.upcoming()).contains(&removed.as_str()));
}

#[test]
fn following_matches_advance() {
    for (repeat, shuffle, from) in [
        (Repeat::Off, false, 0),
        (Repeat::Off, false, 2),
        (Repeat::All, false, 2),
        (Repeat::One, false, 1),
        (Repeat::Off, true, 0),
        (Repeat::All, true, 2),
    ] {
        let mut queue = started(from);
        queue.set_shuffle(shuffle);
        queue.set_repeat(repeat);
        let following = queue.following().map(|track| track.name.clone());
        let advanced = queue.advance(true).map(|track| track.name.clone());
        assert_eq!(
            following, advanced,
            "{repeat:?}, shuffled {shuffle}, from {from}"
        );
    }
}
