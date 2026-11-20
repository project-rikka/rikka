use rikka_text_font::{Error, FontFile};

#[test]
fn face_index_out_of_range_is_error() {
    let data = rikka_test_fonts::font_bytes("Inter.ttc");
    let file = FontFile::new(&data).unwrap();
    let count = file.face_count();
    assert_eq!(
        file.face(count).unwrap_err(),
        Error::FaceIndex {
            index: count,
            count
        }
    );
}

#[test]
fn rejects_non_font_data() {
    assert_eq!(FontFile::new(b"GIF89a").unwrap_err(), Error::UnknownFormat);
    assert_eq!(FontFile::new(&[0, 1]).unwrap_err(), Error::UnknownFormat);
    assert_eq!(
        FontFile::new(b"ttcf\0\x01\0\0\xff\xff\xff\xff").unwrap_err(),
        Error::Malformed
    );
}

#[test]
fn every_installed_font_parses_without_panicking() {
    let mut faces = 0;
    for path in crate::support::all_font_files() {
        let data = std::fs::read(path).unwrap();
        let file = FontFile::new(&data).unwrap();
        for face in file.faces() {
            let font = face.unwrap();
            assert!(font.glyph_count() > 0, "{}", path.display());
            faces += 1;
        }
    }
    assert!(faces > 200);
}
