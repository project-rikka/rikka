use std::fs;
use std::path::PathBuf;

use viewer_text::TextFile;

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rikka-viewer-text-{name}"));
    let _ = fs::remove_file(&path);
    path
}

#[test]
fn save_keeps_source_encoding() {
    let path = scratch("shift-jis");
    let prose = "吾輩は猫である。名前はまだ無い。\n\
                 どこで生れたかとんと見当がつかぬ。\n";
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(prose);
    fs::write(&path, &bytes).expect("a scratch file");

    let read = TextFile::read(&path).expect("a readable file");
    assert_eq!(read.text, prose);
    assert_eq!(read.encoding, "Shift_JIS");
    assert!(!read.truncated);

    let edited = format!("{prose}名前はもうある。\n");
    let written = TextFile::write(&path, &edited, read.encoding, read.bom).expect("a write");
    assert_eq!(written.encoding, "Shift_JIS");
    assert!(!written.lossy);
    assert_eq!(fs::read(&path).expect("the file"), {
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(&edited);
        bytes.into_owned()
    });
    let _ = fs::remove_file(&path);
}

#[test]
fn unencodable_source_saves_as_utf8() {
    let path = scratch("utf-16");
    fs::write(&path, b"\xFF\xFEa\0b\0").expect("a scratch file");

    let read = TextFile::read(&path).expect("a readable file");
    assert_eq!(read.encoding, "UTF-16LE");

    let written = TextFile::write(&path, &read.text, read.encoding, read.bom).expect("a write");
    assert_eq!(written.encoding, "UTF-8");
    assert_eq!(fs::read(&path).expect("the file"), b"ab");
    let _ = fs::remove_file(&path);
}
#[test]
fn save_keeps_byte_order_mark() {
    let path = scratch("bom");
    fs::write(&path, b"\xEF\xBB\xBFline\n").expect("a scratch file");

    let read = TextFile::read(&path).expect("a readable file");
    assert_eq!(read.text, "line\n");
    assert!(read.bom);

    TextFile::write(&path, "edited\n", read.encoding, read.bom).expect("a write");
    assert_eq!(fs::read(&path).expect("the file"), b"\xEF\xBB\xBFedited\n");
    let _ = fs::remove_file(&path);
}

#[test]
fn reports_unencodable_character() {
    let path = scratch("lossy");
    let written = TextFile::write(&path, "日本語", "ISO-8859-1", false).expect("a write");
    assert!(written.lossy);
    let _ = fs::remove_file(&path);
}
