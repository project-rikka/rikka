use std::time::Duration;

use lofty::tag::ItemKey;
use music_library::{Songs, Summary};

use crate::support::{self, CD, Fixture};

#[test]
fn rescan_is_idempotent() {
    let fixture = Fixture::new();
    support::song(
        &fixture.path("Artist/Album/01.flac"),
        CD,
        &support::tagged("One", "Artist", "Album"),
    );
    support::song(
        &fixture.path("Artist/Album/02.flac"),
        CD,
        &support::tagged("Two", "Artist", "Album"),
    );
    support::song(
        &fixture.path("Artist/.hidden/03.flac"),
        CD,
        &support::tagged("Hidden", "Artist", "Album"),
    );
    support::song(
        &fixture.path("Playlists/04.flac"),
        CD,
        &support::tagged("Excluded", "Artist", "Album"),
    );
    std::fs::write(fixture.path("Artist/Album/notes.txt"), b"not music").unwrap();
    let library = fixture.open();
    support::scan(&library);

    let songs = library.songs(Songs::default()).unwrap();
    let titles: Vec<&str> = songs.iter().map(|song| song.title.as_str()).collect();
    assert_eq!(titles, ["One", "Two"]);
    assert!(songs.iter().all(|song| song.available));
    assert_eq!(songs[0].duration, Duration::from_secs(1));

    let again = support::scan(&library);
    assert_eq!(
        again,
        Summary {
            unchanged: 2,
            ..Summary::default()
        }
    );
}

#[test]
fn changed_and_deleted_files_detected() {
    let fixture = Fixture::new();
    let first = support::song(
        &fixture.path("A/01.flac"),
        CD,
        &support::tagged("Before", "Artist", "Album"),
    );
    let second = support::song(
        &fixture.path("A/02.flac"),
        CD,
        &support::tagged("Doomed", "Artist", "Album"),
    );
    let library = fixture.open();
    support::scan(&library);
    let id = library.songs(Songs::default()).unwrap()[0].id;

    support::song(&first, CD, &support::tagged("After", "Artist", "Album"));
    std::fs::File::options()
        .write(true)
        .open(&first)
        .unwrap()
        .set_modified(std::time::SystemTime::now() + Duration::from_secs(2))
        .unwrap();
    std::fs::remove_file(&second).unwrap();
    support::scan(&library);

    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "After");
    assert_eq!(songs[0].id, id);
}

#[test]
fn symlinks_neither_loop_nor_duplicate() {
    let fixture = Fixture::new();
    support::song(
        &fixture.path("Real/01.flac"),
        CD,
        &[(ItemKey::TrackTitle, "Only once")],
    );
    std::os::unix::fs::symlink(fixture.path("Real"), fixture.path("Alias")).unwrap();
    std::os::unix::fs::symlink(&fixture.music, fixture.path("Real/Loop")).unwrap();
    let library = fixture.open();
    support::scan(&library);

    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Only once");
}

#[test]
fn untagged_files_use_file_name() {
    let fixture = Fixture::new();
    support::song(&fixture.path("Loose/Untitled Demo.flac"), CD, &[]);
    let library = fixture.open();
    support::scan(&library);

    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Untitled Demo");
    assert_eq!(songs[0].album.title, "Loose");
}

#[test]
fn folders_added_indexed_removed_forgotten() {
    let fixture = Fixture::new();
    let external = fixture.dir.path().join("Drive");
    support::song(
        &external.join("x.flac"),
        CD,
        &support::tagged("External", "Someone", "Elsewhere"),
    );
    let library = fixture.open();
    library.add_folder(&external).unwrap();
    support::scan(&library);
    assert_eq!(library.songs(Songs::default()).unwrap().len(), 1);
    assert_eq!(library.folders().unwrap().len(), 2);

    assert!(
        library
            .remove_folder(&external.canonicalize().unwrap())
            .unwrap()
    );
    support::scan(&library);
    assert!(library.songs(Songs::default()).unwrap().is_empty());
}
