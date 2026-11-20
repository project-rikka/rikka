use std::path::{Path, PathBuf};
use std::time::Duration;

use rikka_xcursor::{Cursor, Error, Theme};

struct Image {
    nominal: u32,
    side: u32,
    hotspot: (u32, u32),
    delay: u32,
    fill: u8,
}

fn still(nominal: u32, fill: u8) -> Image {
    Image {
        nominal,
        side: nominal,
        hotspot: (nominal / 4, nominal / 8),
        delay: 0,
        fill,
    }
}

fn xcursor(images: &[Image]) -> Vec<u8> {
    let word = u32::to_le_bytes;
    let mut file = Vec::new();
    file.extend(b"Xcur");
    file.extend(word(16));
    file.extend(word(0x1_0000));
    file.extend(word(images.len() as u32 + 1));
    let comment = 16 + (images.len() + 1) * 12;
    file.extend(word(0xfffe_0001));
    file.extend(word(1));
    file.extend(word(comment as u32));
    let mut position = comment + 20 + 4;
    for image in images {
        file.extend(word(0xfffd_0002));
        file.extend(word(image.nominal));
        file.extend(word(position as u32));
        position += 36 + (image.side * image.side * 4) as usize;
    }
    file.extend(word(20));
    file.extend(word(0xfffe_0001));
    file.extend(word(1));
    file.extend(word(1));
    file.extend(word(4));
    file.extend(b"test");
    for image in images {
        for field in [
            36,
            0xfffd_0002,
            image.nominal,
            1,
            image.side,
            image.side,
            image.hotspot.0,
            image.hotspot.1,
            image.delay,
        ] {
            file.extend(word(field));
        }
        file.extend(std::iter::repeat_n(
            image.fill,
            (image.side * image.side * 4) as usize,
        ));
    }
    file
}

fn install(root: &Path, theme: &str, inherits: Option<&str>, cursors: &[(&str, u8)]) {
    let folder = root.join(theme);
    std::fs::create_dir_all(folder.join("cursors")).unwrap();
    if let Some(inherits) = inherits {
        let index = format!("[Icon Theme]\nName={theme}\nInherits = {inherits}\n");
        std::fs::write(folder.join("index.theme"), index).unwrap();
    }
    for (name, fill) in cursors {
        std::fs::write(
            folder.join("cursors").join(name),
            xcursor(&[still(24, *fill)]),
        )
        .unwrap();
    }
}

fn fill_of(theme: &Theme, cursor: &str) -> Option<u8> {
    let path = theme.find(cursor)?;
    Some(Cursor::read(&path, 24).unwrap().frames[0].bgra[0])
}

#[test]
fn nearest_nominal_size_is_read_with_its_frames() {
    let spinning = |nominal: u32, delay: u32, fill: u8| Image {
        delay,
        ..still(nominal, fill)
    };
    let file = xcursor(&[
        still(24, 1),
        spinning(32, 40, 2),
        spinning(32, 60, 3),
        still(48, 4),
    ]);

    let cases = [
        (1, 24, 1),
        (27, 24, 1),
        (30, 32, 2),
        (41, 48, 1),
        (400, 48, 1),
    ];
    for (asked, nominal, frames) in cases {
        let cursor = Cursor::parse(&file, asked).unwrap();
        assert_eq!(
            (cursor.size, cursor.frames.len()),
            (nominal, frames),
            "{asked}"
        );
    }

    let cursor = Cursor::parse(&file, 32).unwrap();
    let (first, second) = (&cursor.frames[0], &cursor.frames[1]);
    assert_eq!((first.width, first.height, first.hotspot), (32, 32, (8, 4)));
    assert_eq!(first.delay, Duration::from_millis(40));
    assert_eq!(second.delay, Duration::from_millis(60));
    assert_eq!(first.bgra, vec![2; 32 * 32 * 4]);
    assert_eq!(second.bgra, vec![3; 32 * 32 * 4]);
}

#[test]
fn damaged_files_are_refused() {
    let whole = xcursor(&[still(24, 9)]);
    assert!(matches!(
        Cursor::parse(b"PNG\x89....", 24),
        Err(Error::NotXcursor)
    ));
    assert!(matches!(
        Cursor::parse(&whole[..10], 24),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(
        Cursor::parse(&whole[..whole.len() - 7], 24),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(
        Cursor::parse(&xcursor(&[]), 24),
        Err(Error::NoImage)
    ));

    let mut huge = whole.clone();
    let header = huge.len() - 24 * 24 * 4 - 36;
    huge[header + 16..header + 20].copy_from_slice(&0x1_0000u32.to_le_bytes());
    assert!(matches!(
        Cursor::parse(&huge, 24),
        Err(Error::Oversized {
            width: 0x1_0000,
            ..
        })
    ));
}

#[test]
fn themes_search_folders_in_order_then_what_they_inherit() {
    let home = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    let search: Vec<PathBuf> = vec![home.path().into(), system.path().into()];

    install(home.path(), "Mine", None, &[("pointer", 1)]);
    install(
        system.path(),
        "Mine",
        Some("Base, Loop"),
        &[("pointer", 2), ("text", 3)],
    );
    install(
        system.path(),
        "Base",
        Some("Mine"),
        &[("text", 4), ("wait", 5)],
    );
    install(
        system.path(),
        "Loop",
        Some("Loop"),
        &[("wait", 6), ("grab", 7)],
    );
    install(system.path(), "default", None, &[("grab", 8), ("help", 9)]);
    install(system.path(), "Other", None, &[("crosshair", 10)]);

    let theme = Theme::load_from("Mine", &search);
    let found: Vec<Option<u8>> = ["pointer", "text", "wait", "grab", "help", "crosshair"]
        .into_iter()
        .map(|cursor| fill_of(&theme, cursor))
        .collect();
    assert_eq!(
        found,
        vec![Some(1), Some(3), Some(5), Some(7), Some(9), None]
    );

    let missing = Theme::load_from("Nowhere", &search);
    assert_eq!(fill_of(&missing, "help"), Some(9));
    assert_eq!(fill_of(&missing, "pointer"), None);
    assert_eq!(Theme::load_from("../Mine", &search), missing);
}
