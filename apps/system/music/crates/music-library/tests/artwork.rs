use music_library::{AlbumSort, Albums, Artwork};

use crate::support::{self, CD, Fixture};

fn widths(artwork: &Artwork) -> Vec<u32> {
    artwork.files.iter().map(|file| file.width).collect()
}

#[test]
fn artwork_prefers_embedded_then_named_then_lone() {
    let fixture = Fixture::new();
    let embedded = support::png(800, 800, 10);
    support::song_with(
        &fixture.path("Embedded/1.flac"),
        CD,
        &support::tagged("E", "A", "Embedded"),
        Some(&embedded),
    );
    std::fs::write(fixture.path("Embedded/cover.jpg"), support::png(64, 64, 20)).unwrap();

    support::song(
        &fixture.path("Named/1.flac"),
        CD,
        &support::tagged("N", "B", "Named"),
    );
    std::fs::write(fixture.path("Named/Folder.PNG"), support::png(400, 400, 30)).unwrap();
    std::fs::write(fixture.path("Named/back.png"), support::png(50, 50, 40)).unwrap();

    support::song(
        &fixture.path("Lone/CD1/1.flac"),
        CD,
        &support::tagged("L", "C", "Lone"),
    );
    std::fs::write(
        fixture.path("Lone/scan-0001.webp"),
        support::png(100, 100, 50),
    )
    .unwrap();

    support::song(
        &fixture.path("Several/1.flac"),
        CD,
        &support::tagged("S", "D", "Several"),
    );
    std::fs::write(fixture.path("Several/a.png"), support::png(10, 10, 60)).unwrap();
    std::fs::write(fixture.path("Several/b.png"), support::png(10, 10, 70)).unwrap();

    let library = fixture.open();
    support::scan(&library);
    let albums = library.albums(Albums::new(AlbumSort::Title)).unwrap();
    let art: Vec<(&str, Option<Vec<u32>>)> = albums
        .iter()
        .map(|album| (album.title.as_str(), album.artwork.as_ref().map(widths)))
        .collect();
    assert_eq!(
        art,
        [
            ("Embedded", Some(vec![300, 600, 800])),
            ("Lone", Some(vec![100])),
            ("Named", Some(vec![300, 400])),
            ("Several", None),
        ]
    );
    let embedded_art = albums[0].artwork.as_ref().unwrap();
    for file in &embedded_art.files {
        assert!(file.path.is_file(), "{}", file.path.display());
        assert!(file.path.starts_with(fixture.artwork()));
    }
    assert_eq!(
        std::fs::read(embedded_art.fitting(1000).unwrap()).unwrap(),
        embedded
    );
    let small = rikka_image::read(embedded_art.fitting(200).unwrap())
        .probe()
        .unwrap();
    assert_eq!((small.width, small.height), (300, 300));

    let song = &library.album(albums[0].id).unwrap().unwrap().tracks[0];
    assert_eq!(song.artwork.as_ref(), Some(embedded_art));
}
