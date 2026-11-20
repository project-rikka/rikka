use music_library::Songs;

use crate::support::{self, CD, Fixture};

#[test]
fn schema_change_keeps_listening_data() {
    let fixture = Fixture::new();
    support::song(
        &fixture.path("Kept/1.flac"),
        CD,
        &support::tagged("Favourite", "Artist", "Album"),
    );
    support::song(
        &fixture.path("Kept/2.flac"),
        CD,
        &support::tagged("Ordinary", "Artist", "Album"),
    );
    let (favourite, ordinary) = {
        let library = fixture.open();
        support::scan(&library);
        let songs = library.songs(Songs::default()).unwrap();
        library.set_favourite(songs[0].id, true).unwrap();
        library.played(songs[0].id).unwrap();
        (songs[0].id, songs[1].id)
    };

    let connection = rusqlite::Connection::open(fixture.database()).unwrap();
    connection
        .pragma_update(None, "user_version", 9999)
        .unwrap();
    connection
        .execute_batch("DROP TABLE tracks; CREATE TABLE tracks (stale INTEGER);")
        .unwrap();
    drop(connection);

    let library = fixture.open();
    support::scan(&library);
    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].id, favourite);
    assert_eq!(songs[1].id, ordinary);
    assert!(songs[0].listening.favourite);
    assert_eq!(songs[0].listening.plays, 1);
    assert!(!songs[1].listening.favourite);
}
