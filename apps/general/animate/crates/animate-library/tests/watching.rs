use animate_library::{Fit, Locations, Mismatch, Shelf};

use crate::support::{MIKU, Opened, names, put, vmd};

#[test]
fn changes_on_disk_reach_catalog() {
    let root = tempfile::tempdir().expect("temporary folder");
    Locations::new(root.path())
        .ensure()
        .expect("library created");
    let opened = Opened::new(Locations::new(root.path()));
    opened.until(|catalog| catalog.shelf(Shelf::Cameras).entries().is_empty());

    put(root.path(), "Cameras/shot.vmd", &vmd(0, 0, 2, 30));
    opened.until(|catalog| {
        catalog
            .shelf(Shelf::Cameras)
            .entries()
            .first()
            .is_some_and(|entry| entry.fit() == Fit::Fits)
    });

    put(root.path(), "Cameras/shot.vmd", &vmd(2, 0, 0, 30));
    opened.until(|catalog| {
        catalog
            .shelf(Shelf::Cameras)
            .entries()
            .first()
            .is_some_and(|entry| entry.fit() == Fit::Mismatch(Mismatch::Belongs(Shelf::Motions)))
    });

    put(root.path(), "Models/New/Figure/figure.pmx", MIKU);
    opened.until(|catalog| names(catalog, Shelf::Models) == ["figure"]);

    std::fs::remove_file(root.path().join("Cameras/shot.vmd")).expect("file removed");
    std::fs::remove_dir_all(root.path().join("Models/New")).expect("folder removed");
    opened.until(|catalog| {
        catalog.shelf(Shelf::Cameras).entries().is_empty()
            && catalog.shelf(Shelf::Models).entries().is_empty()
    });
}
