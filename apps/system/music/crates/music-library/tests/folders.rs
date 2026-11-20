use lofty::tag::ItemKey;

use crate::support::{self, CD, Fixture};

#[test]
fn folder_plays_files_then_subfolders() {
    let fixture = Fixture::new();
    for (path, title, number) in [
        ("Mix/top.flac", "top", "1"),
        ("Mix/beta/2.flac", "beta two", "2"),
        ("Mix/beta/1.flac", "beta one", "1"),
        ("Mix/Alpha/x.flac", "alpha", "1"),
        ("Mix/Alpha/deep/y.flac", "alpha deep", "1"),
        ("Other/z.flac", "other", "1"),
    ] {
        support::song(
            &fixture.path(path),
            CD,
            &[
                (ItemKey::TrackTitle, title),
                (ItemKey::TrackArtist, "Band"),
                (ItemKey::TrackNumber, number),
            ],
        );
    }
    let library = fixture.open();
    support::scan(&library);

    let mix = fixture.path("Mix");
    let played: Vec<String> = library
        .tracks_under(&mix)
        .unwrap()
        .into_iter()
        .map(|track| track.title)
        .collect();
    assert_eq!(
        played,
        ["top", "alpha", "alpha deep", "beta one", "beta two"]
    );
    assert_eq!(library.count_under(&mix).unwrap(), 5);
    assert_eq!(library.count_under(&fixture.path("Mix/beta")).unwrap(), 2);
    assert_eq!(library.count_under(&fixture.music).unwrap(), 6);
}
