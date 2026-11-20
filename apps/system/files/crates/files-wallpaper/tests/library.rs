use std::fs;
use std::path::{Path, PathBuf};

use files_wallpaper::{Library, is_wallpaper};
use rikka_dirs::UserDirs;

struct Places {
    outside: PathBuf,
    library: Library,
}

fn places() -> Places {
    let library = Library::session().expect("wallpaper folder");
    let outside = UserDirs::load()
        .wallpapers()
        .expect("wallpaper folder")
        .with_file_name("Downloads");
    fs::create_dir_all(&outside).expect("create outside folder");
    Places { outside, library }
}

fn write(folder: &Path, name: &str, contents: &[u8]) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, contents).expect("write file");
    path
}

#[test]
fn wallpaper_kinds_are_pictures_and_movies() {
    let cases = [
        ("photo.jpg", true),
        ("screen.PNG", true),
        ("loop.webm", true),
        ("clip.mp4", true),
        ("song.flac", false),
        ("notes.txt", false),
        ("archive.zip", false),
        ("noextension", false),
    ];
    for (name, expected) in cases {
        assert_eq!(is_wallpaper(name), expected, "{name}");
    }
}

#[test]
fn file_inside_library_is_held_by_its_name() {
    let places = places();
    let inside = write(places.library.folder(), "sunset.jpg", b"sunset");
    assert_eq!(
        places.library.holding(&inside).expect("look up"),
        Some("sunset.jpg".to_owned())
    );
}

#[test]
fn identical_copy_is_held_under_its_library_name() {
    let places = places();
    write(places.library.folder(), "beach (2).png", b"same pixels");
    write(places.library.folder(), "other.png", b"different!!");
    let source = write(&places.outside, "beach.png", b"same pixels");
    assert_eq!(
        places.library.holding(&source).expect("look up"),
        Some("beach (2).png".to_owned())
    );
}

#[test]
fn same_name_with_other_contents_is_not_held() {
    let places = places();
    write(places.library.folder(), "beach.png", b"old pixels");
    let source = write(&places.outside, "beach.png", b"new pixels");
    assert_eq!(places.library.holding(&source).expect("look up"), None);
}

#[test]
fn identical_file_of_other_kind_is_not_held() {
    let places = places();
    write(places.library.folder(), "beach.txt", b"same pixels");
    let source = write(&places.outside, "beach.png", b"same pixels");
    assert_eq!(places.library.holding(&source).expect("look up"), None);
}

#[test]
fn missing_source_fails() {
    let places = places();
    assert!(
        places
            .library
            .holding(&places.outside.join("gone.png"))
            .is_err()
    );
}

#[test]
fn placed_file_gets_id_only_inside_library() {
    let places = places();
    let folder = places.library.folder().to_path_buf();
    assert_eq!(
        places.library.id_of(&folder.join("beach (2).png")),
        Some("beach (2).png".to_owned())
    );
    assert_eq!(
        places.library.id_of(&places.outside.join("beach.png")),
        None
    );
}
