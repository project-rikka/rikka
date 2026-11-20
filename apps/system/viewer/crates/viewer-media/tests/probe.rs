use std::fs::File;
use std::io::Write;

use viewer_media::{Extent, Kind, probe};

fn written(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rikka-viewer-probe-{name}"));
    let mut file = File::create(&path).expect("a file to probe");
    file.write_all(bytes).expect("writing the file");
    path
}

#[test]
fn probes_picture_without_decoding() {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&7_u32.to_be_bytes());
    png.extend_from_slice(&5_u32.to_be_bytes());
    let path = written("picture.png", &png);

    let Ok(Some(media)) = probe(&path) else {
        panic!("a PNG is media");
    };
    assert_eq!(media.kind(), Kind::Picture);
    assert_eq!(media.format.name(), "PNG");
    assert_eq!(media.extent, Extent::new(7, 5));
    assert_eq!(media.bytes, png.len() as u64);
}

#[test]
fn unknown_file_reading_as_text_is_text() {
    let path = written("notes", b"nothing here but prose\n");
    let Ok(Some(media)) = probe(&path) else {
        panic!("prose is media");
    };
    assert_eq!(media.kind(), Kind::Text);

    let path = written("program", b"\x7FELF\x02\x01\x01\0\0\0\0\0\0\0\0\0");
    assert!(matches!(probe(&path), Ok(None)));
}

#[test]
fn missing_file_is_reported() {
    let missing = std::env::temp_dir().join("rikka-viewer-probe-no-such-file");
    std::fs::remove_file(&missing).ok();
    assert!(probe(&missing).is_err());

    assert!(probe(&std::env::temp_dir()).is_err(), "nor is a directory");
}
