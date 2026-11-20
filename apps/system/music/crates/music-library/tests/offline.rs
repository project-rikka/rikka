use music_library::{FolderKind, Songs};

use crate::support::{self, CD, Fixture};

#[test]
fn offline_root_keeps_tracks_unavailable() {
    let fixture = Fixture::new();
    let drive = fixture.dir.path().join("media").join("Drive");
    support::song(
        &drive.join("Band/Record/01.flac"),
        CD,
        &support::tagged("Far away", "Band", "Record"),
    );
    let library = fixture.open();
    library.add_folder(&drive).unwrap();
    support::scan(&library);
    let id = library.songs(Songs::default()).unwrap()[0].id;
    library.set_favourite(id, true).unwrap();

    let unplugged = fixture.dir.path().join("unplugged");
    std::fs::rename(&drive, &unplugged).unwrap();
    support::scan(&library);
    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].id, id);
    assert!(!songs[0].available);
    let folder = library
        .folders()
        .unwrap()
        .into_iter()
        .find(|folder| folder.kind == FolderKind::Added)
        .unwrap();
    assert!(!folder.online);
    let albums = library.albums(Default::default()).unwrap();
    assert!(!albums[0].available);

    std::fs::rename(&unplugged, &drive).unwrap();
    support::scan(&library);
    let songs = library.songs(Songs::default()).unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].id, id);
    assert!(songs[0].available);
    assert!(songs[0].listening.favourite);
}
