use std::path::Path;

use rikka_svg::Svg;

const TILE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
    <rect width="10" height="10" fill="#00ff00"/>
</svg>"##;

fn linking(href: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
    <image href="{href}" width="10" height="10"/>
</svg>"#
    )
}

fn paints(document: &Svg) -> bool {
    let raster = document.render(10, 10).expect("raster rendered");
    raster.pixels().as_chunks::<4>().0[55][3] != 0
}

fn opened(folder: &Path, href: &str) -> Svg {
    let path = folder.join("document.svg");
    std::fs::write(&path, linking(href)).expect("document written");
    Svg::open(&path).expect("valid document")
}

#[test]
fn links_reach_only_files_beside_the_document() {
    let root = tempfile::tempdir().expect("scratch folder");
    let folder = root.path().join("icons");
    std::fs::create_dir(&folder).expect("folder created");
    let outside = root.path().join("outside.svg");
    std::fs::write(folder.join("tile.svg"), TILE).expect("tile written");
    std::fs::write(&outside, TILE).expect("outside written");
    std::os::unix::fs::symlink(&outside, folder.join("escape.svg")).expect("link created");
    let absolute = outside.to_str().expect("utf-8 path");

    assert!(paints(&opened(&folder, "tile.svg")), "sibling");
    for refused in ["../outside.svg", "escape.svg", absolute, "/dev/zero"] {
        assert!(!paints(&opened(&folder, refused)), "{refused}");
    }
}

#[test]
fn documents_from_bytes_open_no_files() {
    let root = tempfile::tempdir().expect("scratch folder");
    let tile = root.path().join("tile.svg");
    std::fs::write(&tile, TILE).expect("tile written");

    for href in [tile.to_str().expect("utf-8 path"), "/dev/zero"] {
        let document = Svg::from_bytes(linking(href).as_bytes()).expect("valid document");
        assert!(!paints(&document), "{href}");
    }
}
