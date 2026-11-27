use animate_library::{Details, Fit, Location, Locations, Mismatch, Shelf};

use crate::support::{BARE, CLIP_ONLY, DANCE, MIKU, Opened, glb, names, put, stl, vmd, wav};

#[test]
fn library_reads_shelves_through_aliases() {
    let root = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Models/Miku/miku.pmx", MIKU);
    put(root.path(), "Models/Miku/tex/skin.png", b"png");
    put(root.path(), "motion/Dances/dance.vmd", DANCE);
    put(root.path(), "BackGround/hall.stl", &stl(2));
    put(root.path(), "Wave/song.wav", &wav(800));
    put(root.path(), "Camera/.hidden.vmd", &vmd(0, 0, 3, 90));
    put(root.path(), "loose.pmx", MIKU);
    put(root.path(), "Other/misc.pmx", MIKU);

    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();

    assert_eq!(names(&catalog, Shelf::Models), ["miku"]);
    assert_eq!(names(&catalog, Shelf::Motions), ["dance"]);
    assert_eq!(names(&catalog, Shelf::Stages), ["hall"]);
    assert_eq!(names(&catalog, Shelf::Music), ["song"]);
    assert!(catalog.shelf(Shelf::Cameras).entries().is_empty());
    assert_eq!(&*catalog.shelf(Shelf::Models).folders()[0], "Miku");
    assert_eq!(catalog.shelf(Shelf::Models).folders().len(), 1);
    assert_eq!(
        &*catalog.shelf(Shelf::Motions).entries()[0].folder().clone(),
        "Dances"
    );
    let song = &catalog.shelf(Shelf::Music).entries()[0];
    assert!(matches!(song.details(), Details::Sound(sound) if (sound.seconds - 0.1).abs() < 0.01));
    for shelf in Shelf::ALL {
        assert!(
            catalog
                .shelf(shelf)
                .entries()
                .iter()
                .all(|entry| entry.fit() == Fit::Fits)
        );
    }
}

#[test]
fn folder_filter_includes_nested_folders() {
    let root = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Motions/top.vmd", DANCE);
    put(root.path(), "Motions/Idol/one.vmd", DANCE);
    put(root.path(), "Motions/Idol/Encore/two.vmd", DANCE);
    put(root.path(), "Motions/Idolized/three.vmd", DANCE);

    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.until(|catalog| catalog.shelf(Shelf::Motions).entries().len() == 4);
    let motions = catalog.shelf(Shelf::Motions);

    let folders: Vec<&str> = motions.folders().iter().map(|folder| &**folder).collect();
    assert_eq!(folders, ["Idol", "Idol/Encore", "Idolized"]);
    let idol: Vec<&str> = motions
        .within("Idol")
        .map(|entry| &**entry.name())
        .collect();
    assert_eq!(idol, ["one", "two"]);
    assert_eq!(motions.within("").count(), 4);
}

#[test]
fn content_that_misfits_its_shelf_is_marked() {
    let root = tempfile::tempdir().expect("temporary folder");
    put(root.path(), "Cameras/body.vmd", &vmd(4, 0, 0, 60));
    put(root.path(), "Cameras/camera.vmd", &vmd(0, 0, 4, 60));
    put(root.path(), "Motions/camera.vmd", &vmd(0, 0, 4, 60));
    put(root.path(), "Motions/nothing.vmd", &vmd(0, 0, 0, 0));
    put(root.path(), "Motions/miku.pmx", MIKU);
    put(root.path(), "Models/clip.glb", &glb(CLIP_ONLY));
    put(root.path(), "Models/bare.gltf", BARE.as_bytes());
    put(root.path(), "Accessories/song.wav", &wav(80));

    let opened = Opened::new(Locations::new(root.path()));
    let catalog = opened.probed();
    let fit = |shelf: Shelf, name: &str| {
        catalog
            .shelf(shelf)
            .entries()
            .iter()
            .find(|entry| &**entry.name() == name)
            .map(|entry| entry.fit())
            .expect("entry listed")
    };

    assert_eq!(
        fit(Shelf::Cameras, "body"),
        Fit::Mismatch(Mismatch::Belongs(Shelf::Motions))
    );
    assert_eq!(fit(Shelf::Cameras, "camera"), Fit::Fits);
    assert_eq!(
        fit(Shelf::Motions, "camera"),
        Fit::Mismatch(Mismatch::Belongs(Shelf::Cameras))
    );
    assert_eq!(
        fit(Shelf::Motions, "nothing"),
        Fit::Mismatch(Mismatch::Empty)
    );
    assert_eq!(
        fit(Shelf::Motions, "miku"),
        Fit::Mismatch(Mismatch::Belongs(Shelf::Models))
    );
    assert_eq!(
        fit(Shelf::Models, "clip"),
        Fit::Mismatch(Mismatch::Belongs(Shelf::Motions))
    );
    assert_eq!(fit(Shelf::Models, "bare"), Fit::Mismatch(Mismatch::Empty));
    assert_eq!(
        fit(Shelf::Accessories, "song"),
        Fit::Mismatch(Mismatch::Belongs(Shelf::Music))
    );
}

#[test]
fn extra_folders_join_their_shelf_at_runtime() {
    let home = tempfile::tempdir().expect("temporary folder");
    let extra = tempfile::tempdir().expect("temporary folder");
    put(home.path(), "Motions/home.vmd", DANCE);
    put(extra.path(), "Pack/added.vmd", DANCE);

    let opened = Opened::new(Locations::new(home.path()));
    opened.until(|catalog| names(catalog, Shelf::Motions) == ["home"]);

    let extended =
        Locations::new(home.path()).with_extra([Location::shelf(extra.path(), Shelf::Motions)]);
    opened.library.set_locations(extended);
    let catalog = opened.until(|catalog| catalog.shelf(Shelf::Motions).entries().len() == 2);
    assert_eq!(names(&catalog, Shelf::Motions), ["home", "added"]);

    opened.library.set_locations(Locations::new(home.path()));
    opened.until(|catalog| names(catalog, Shelf::Motions) == ["home"]);
}

#[test]
fn missing_location_is_reported() {
    let home = tempfile::tempdir().expect("temporary folder");
    let gone = home.path().join("gone");
    let locations = Locations::new(home.path()).with_extra([Location::shelf(&gone, Shelf::Music)]);

    let opened = Opened::new(locations);
    let catalog = opened.until(|catalog| !catalog.unavailable().is_empty());
    assert_eq!(catalog.unavailable()[0].location.root, gone);
}

#[test]
fn ensure_creates_only_missing_shelves() {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("Animate");
    std::fs::create_dir_all(home.join("Model")).expect("folder created");

    Locations::new(&home).ensure().expect("library created");

    let mut folders: Vec<String> = std::fs::read_dir(&home)
        .expect("library listed")
        .map(|child| {
            child
                .expect("child")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    folders.sort();
    assert_eq!(
        folders,
        [
            "Accessories",
            "Cameras",
            "Model",
            "Motions",
            "Music",
            "Stages"
        ]
    );
}
