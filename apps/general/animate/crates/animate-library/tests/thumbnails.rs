use std::time::{Duration, SystemTime};

use animate_library::{FigureKey, Locations, Shelf, Thumbnail, Thumbnails, Variant};

use crate::support::{AVATAR, DANCE, MIKU, Opened, put};

const PNG_MAGIC: &[u8] = b"\x89PNG";

#[test]
fn stored_thumbnail_stays_fresh_until_source_changes() {
    let root = tempfile::tempdir().expect("temporary folder");
    let cache = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Models/miku.pmx", MIKU);
    let thumbnails = Thumbnails::new(cache.path());
    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();
    let entry = &catalog.shelf(Shelf::Models).entries()[0];

    assert_eq!(
        thumbnails.thumbnail(entry, Variant::Still),
        Thumbnail::Render
    );
    let stored = thumbnails
        .store(
            entry,
            Variant::Still,
            2,
            1,
            &[255, 0, 0, 255, 0, 0, 255, 255],
        )
        .expect("thumbnail stored");
    assert!(
        std::fs::read(&stored)
            .expect("thumbnail written")
            .starts_with(PNG_MAGIC)
    );
    assert_eq!(
        thumbnails.thumbnail(entry, Variant::Still),
        Thumbnail::Ready(stored)
    );

    let source = root.path().join("Models/miku.pmx");
    let earlier = SystemTime::now() - Duration::from_secs(3600);
    rikka_fs::set_modified(&source, earlier).expect("source touched");
    let catalog = opened
        .until(|catalog| catalog.shelf(Shelf::Models).entries()[0].modified() != entry.modified());
    let changed = &catalog.shelf(Shelf::Models).entries()[0];
    assert_eq!(thumbnails.cached(changed, Variant::Still), None);
}

#[test]
fn performed_thumbnails_belong_to_one_figure() {
    let root = tempfile::tempdir().expect("temporary folder");
    let cache = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Motions/dance.vmd", DANCE);
    put(root.path(), "figures/miku.pmx", MIKU);
    put(root.path(), "figures/other.pmx", &MIKU[..MIKU.len() - 1]);
    let miku = FigureKey::from_file(&root.path().join("figures/miku.pmx")).expect("figure key");
    let other = FigureKey::from_file(&root.path().join("figures/other.pmx")).expect("figure key");
    let thumbnails = Thumbnails::new(cache.path());
    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();
    let dance = &catalog.shelf(Shelf::Motions).entries()[0];

    thumbnails
        .store(dance, Variant::Performed(miku), 1, 1, &[0, 0, 0, 255])
        .expect("thumbnail stored");

    assert!(thumbnails.cached(dance, Variant::Performed(miku)).is_some());
    assert_eq!(thumbnails.cached(dance, Variant::Performed(other)), None);
    assert_eq!(thumbnails.cached(dance, Variant::Still), None);
}

#[test]
fn sidecar_pictures_come_first() {
    let root = tempfile::tempdir().expect("temporary folder");
    let cache = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Models/Solo/solo.pmx", MIKU);
    put(root.path(), "Models/Solo/Preview.png", b"picture");
    put(root.path(), "Models/Pair/left.pmx", MIKU);
    put(root.path(), "Models/Pair/right.pmx", MIKU);
    put(root.path(), "Models/Pair/RIGHT.JPG", b"picture");
    put(root.path(), "Models/Pair/preview.png", b"picture");
    let thumbnails = Thumbnails::new(cache.path());
    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();
    let thumbnail = |name: &str| {
        let entry = catalog
            .shelf(Shelf::Models)
            .entries()
            .iter()
            .find(|entry| &**entry.name() == name)
            .expect("entry listed");
        thumbnails.thumbnail(entry, Variant::Still)
    };

    assert_eq!(
        thumbnail("solo"),
        Thumbnail::Ready(root.path().join("Models/Solo/Preview.png"))
    );
    assert_eq!(
        thumbnail("right"),
        Thumbnail::Ready(root.path().join("Models/Pair/RIGHT.JPG"))
    );
    assert_eq!(thumbnail("left"), Thumbnail::Render);
}

#[test]
fn embedded_vrm_thumbnail_is_extracted_once() {
    let root = tempfile::tempdir().expect("temporary folder");
    let cache = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Models/avatar.vrm", AVATAR);
    let thumbnails = Thumbnails::new(cache.path());
    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();
    let avatar = &catalog.shelf(Shelf::Models).entries()[0];
    assert!(avatar.details().has_embedded_picture());

    assert_eq!(
        thumbnails.thumbnail(avatar, Variant::Still),
        Thumbnail::Extract
    );
    let extracted = thumbnails.extract(avatar).expect("thumbnail extracted");
    assert!(
        std::fs::read(&extracted)
            .expect("thumbnail written")
            .starts_with(PNG_MAGIC)
    );
    assert_eq!(
        thumbnails.thumbnail(avatar, Variant::Still),
        Thumbnail::Ready(extracted)
    );
}
