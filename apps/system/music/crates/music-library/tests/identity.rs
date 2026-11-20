use music_library::{Songs, TrackId};

use crate::support::{self, CD, Fixture};

fn only_song(library: &music_library::Library) -> music_library::Track {
    let mut songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1, "{songs:#?}");
    songs.remove(0)
}

fn listened(library: &music_library::Library, id: TrackId) {
    library.set_favourite(id, true).unwrap();
    library.played(id).unwrap();
    library.played(id).unwrap();
}

#[test]
fn move_keeps_id_and_listening() {
    let fixture = Fixture::new();
    let original = support::song(
        &fixture.path("Inbox/track.flac"),
        CD,
        &support::tagged("Wandering", "Artist", "Album"),
    );
    let library = fixture.open();
    support::scan(&library);
    let id = only_song(&library).id;
    listened(&library, id);

    let renamed = fixture.path("Artist/Album/01 Wandering.flac");
    std::fs::create_dir_all(renamed.parent().unwrap()).unwrap();
    std::fs::rename(&original, &renamed).unwrap();
    let summary = support::scan(&library);
    assert_eq!(summary.moved, 1, "{summary:?}");

    let song = only_song(&library);
    assert_eq!(song.id, id);
    assert_eq!(song.path, renamed);
    assert!(song.listening.favourite);
    assert_eq!(song.listening.plays, 2);
}

#[test]
fn copy_then_delete_keeps_id() {
    let fixture = Fixture::new();
    let original = support::song(
        &fixture.path("Old/song.flac"),
        CD,
        &support::tagged("Copied", "Artist", "Album"),
    );
    let library = fixture.open();
    support::scan(&library);
    let id = only_song(&library).id;
    listened(&library, id);

    let copy = fixture.path("New/song.flac");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(&original, &copy).unwrap();
    support::scan(&library);
    std::fs::remove_file(&original).unwrap();
    support::scan(&library);

    let song = only_song(&library);
    assert_eq!(song.id, id);
    assert_eq!(song.path, copy);
    assert!(song.listening.favourite);
    assert_eq!(song.listening.plays, 2);
    assert_eq!(library.favourites().unwrap().len(), 1);
}

#[test]
fn returning_file_keeps_id() {
    let fixture = Fixture::new();
    let path = support::song(
        &fixture.path("Back/song.flac"),
        CD,
        &support::tagged("Returning", "Artist", "Album"),
    );
    let library = fixture.open();
    support::scan(&library);
    let id = only_song(&library).id;
    library.set_favourite(id, true).unwrap();

    let aside = fixture.dir.path().join("aside.flac");
    std::fs::rename(&path, &aside).unwrap();
    support::scan(&library);
    assert!(library.songs(Songs::default()).unwrap().is_empty());
    assert!(library.favourites().unwrap().is_empty());

    std::fs::rename(&aside, &path).unwrap();
    support::scan(&library);
    let song = only_song(&library);
    assert_eq!(song.id, id);
    assert!(song.listening.favourite);
}

#[test]
fn unknown_track_listening_is_rejected() {
    let fixture = Fixture::new();
    let library = fixture.open();
    assert!(matches!(
        library.played(TrackId::new(4242)),
        Err(music_library::Error::UnknownTrack(_))
    ));
}
