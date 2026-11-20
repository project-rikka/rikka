use std::path::{Path, PathBuf};

use rikka_image::Image;
use rikka_types::Color;

fn wallpaper(dir: &Path, size: u32, fill: [u8; 4]) -> PathBuf {
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for _ in 0..size {
            let clear = y >= size * 3 / 4;
            pixels.extend_from_slice(&if clear { [0, 0, 0, 0] } else { fill });
        }
    }
    let path = dir.join("dusk.png");
    Image::from_rgba8(size, size, pixels)
        .expect("valid picture")
        .save(&path)
        .expect("write wallpaper");
    path
}

#[test]
fn seed_is_cached_until_file_changes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = wallpaper(dir.path(), 64, [0x21, 0x96, 0xf3, 0xff]);

    assert_eq!(
        rikka_theme_seed::seed(&path).expect("seeds"),
        Color::hex(0x2196f3),
        "a transparent band is not a colour of its own"
    );
    assert_eq!(
        rikka_theme_seed::seed(&path).expect("seeds again"),
        Color::hex(0x2196f3)
    );

    let repainted = wallpaper(dir.path(), 80, [0x4c, 0xaf, 0x50, 0xff]);
    assert_eq!(repainted, path);
    assert_eq!(
        rikka_theme_seed::seed(&path).expect("seeds repainted wallpaper"),
        Color::hex(0x4caf50)
    );
}

#[test]
fn rejects_missing_or_non_image_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let notes = dir.path().join("notes.txt");
    std::fs::write(&notes, b"not a wallpaper").expect("write succeeds");

    assert!(rikka_theme_seed::seed(dir.path().join("nothing.png")).is_err());
    assert!(rikka_theme_seed::seed(&notes).is_err());
}
