use std::path::Path;

use rikka_mime::{Database, MimeType};

const PNG_HEAD: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

fn shared_mime_info(root: &Path) -> Database {
    let mime = root.join("mime");
    std::fs::create_dir_all(&mime).expect("the mime folder");
    std::fs::write(
        mime.join("globs2"),
        "# generated\n\
         50:image/png:*.png\n\
         50:video/mp2t:*.ts\n\
         50:text/vnd.trolltech.linguist:*.ts\n\
         50:text/x-rust:*.rs\n",
    )
    .expect("globs2");
    let mut magic = b"MIME-Magic\0\n[50:image/png]\n>0=\x00\x08".to_vec();
    magic.extend_from_slice(b"\x89PNG\r\n\x1a\n\n[50:video/mp2t]\n>0=\x00\x01G\n");
    std::fs::write(mime.join("magic"), magic).expect("magic");
    std::fs::write(mime.join("aliases"), "image/x-png image/png\n").expect("aliases");
    std::fs::write(mime.join("subclasses"), "text/x-rust text/plain\n").expect("subclasses");
    Database::from_dirs(vec![root.join("missing"), root.to_path_buf()])
}

#[test]
fn files_are_typed_by_name_then_content() {
    let root = tempfile::tempdir().expect("a folder");
    let database = shared_mime_info(root.path());
    let cases: [(&str, &str, &[u8], &str); 5] = [
        (
            "an unambiguous glob decides",
            "plain.png",
            b"not really a png at all",
            "image/png",
        ),
        (
            "magic types an unnamed picture",
            "real.bin",
            PNG_HEAD,
            "image/png",
        ),
        (
            "magic picks among tied globs",
            "stream.ts",
            b"G\x40\x11\x10",
            "video/mp2t",
        ),
        ("an empty file", "empty", b"", MimeType::EMPTY),
        (
            "bytes without a zero are text",
            "nameless",
            b"just words\n",
            MimeType::TEXT,
        ),
    ];
    for (case, name, contents, expected) in cases {
        let path = root.path().join(name);
        std::fs::write(&path, contents).expect("the file");
        assert_eq!(database.for_file(&path).as_str(), expected, "{case}");
    }
    let binary = root.path().join("binary");
    std::fs::write(&binary, b"\x00\x01\x02garbage").expect("the file");
    assert_eq!(database.for_file(&binary).as_str(), MimeType::UNKNOWN);
}

#[test]
fn ancestry_follows_subclasses_and_aliases() {
    let root = tempfile::tempdir().expect("a folder");
    let database = shared_mime_info(root.path());
    let ancestry = |essence: &str| -> Vec<String> {
        database
            .ancestry(&MimeType::parse(essence).expect("a type"))
            .iter()
            .map(ToString::to_string)
            .collect()
    };

    assert_eq!(
        ancestry("text/x-rust"),
        ["text/x-rust", "text/plain", MimeType::UNKNOWN]
    );
    assert_eq!(ancestry("image/x-png"), ["image/png", MimeType::UNKNOWN]);
    assert_eq!(ancestry(MimeType::DIRECTORY), [MimeType::DIRECTORY]);
    assert_eq!(
        ancestry("x-scheme-handler/https"),
        ["x-scheme-handler/https"]
    );
}

#[test]
fn globs_for_follows_kinds_aliases_and_subclasses() {
    let root = tempfile::tempdir().expect("a folder");
    let database = shared_mime_info(root.path());
    assert_eq!(database.globs_for("image/x-png"), ["*.png"]);
    assert_eq!(database.globs_for("video/*"), ["*.ts"]);
    assert_eq!(database.globs_for("text/plain"), ["*.rs", "*.ts"]);
    assert!(database.globs_for("not a type").is_empty());
}

#[test]
fn builtin_types_files_without_shared_mime_info() {
    let database = Database::builtin();
    let root = tempfile::tempdir().expect("a folder");
    let cases: [(&str, &[u8], &str); 6] = [
        ("figure.pmx", b"anything", "model/x.pmx"),
        ("avatar.vrm", b"glTF\x02\0\0\0", "model/vrm"),
        (
            "take.rmotion",
            b"{\"format\":\"rikka-motion\"}",
            "model/x.rikka-motion",
        ),
        ("unnamed", PNG_HEAD, "image/png"),
        ("clip", b"\x1a\x45\xdf\xa3\x01", "video/x-matroska"),
        ("sound", b"RIFF\x24\0\0\0WAVEfmt ", "audio/x-wav"),
    ];
    for (name, contents, expected) in cases {
        let path = root.path().join(name);
        std::fs::write(&path, contents).expect("the file");
        assert_eq!(database.for_file(&path).as_str(), expected, "{name}");
    }

    let sounds = database.globs_for("audio/vnd.wave");
    assert_eq!(sounds, ["*.wav"]);
    let figures = database.globs_for("model/gltf-binary");
    for glob in ["*.glb", "*.vrm", "*.vrma"] {
        assert!(figures.contains(&glob.to_owned()), "{figures:?}");
    }
    let videos = database.globs_for("video/*");
    for glob in ["*.mp4", "*.mkv", "*.mov", "*.webm"] {
        assert!(videos.contains(&glob.to_owned()), "{videos:?}");
    }
}
