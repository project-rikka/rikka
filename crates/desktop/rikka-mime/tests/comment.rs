use rikka_mime::{Database, MimeType};

const PNG: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<mime-type xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\" type=\"image/png\">\n\
  <!--Created automatically by update-mime-database. DO NOT EDIT!-->\n\
  <comment>PNG image</comment>\n\
  <comment xml:lang=\"ja\">PNG 画像</comment>\n\
  <comment xml:lang=\"pt_BR\">Imagem PNG</comment>\n\
</mime-type>\n";

#[test]
fn comment_follows_language() {
    let root = tempfile::tempdir().expect("a folder");
    let file = root.path().join("mime/image/png.xml");
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
    std::fs::write(&file, PNG).expect("the file");
    let database = Database::from_dirs(vec![root.path().join("missing"), root.path().to_owned()]);
    let cases = [
        (
            "a regional tag falls back to its language",
            "image/png",
            "ja-JP",
            Some("PNG 画像"),
        ),
        (
            "hyphenated tags match",
            "image/png",
            "pt-BR",
            Some("Imagem PNG"),
        ),
        (
            "an unknown language falls back to English",
            "image/png",
            "de",
            Some("PNG image"),
        ),
        (
            "no language reads the plain comment",
            "image/png",
            "",
            Some("PNG image"),
        ),
        ("an unknown type has no comment", "image/webp", "ja", None),
    ];
    for (case, essence, language, expected) in cases {
        database.set_language(language);
        let mime = MimeType::parse(essence).expect("a type");
        assert_eq!(database.describe(&mime).as_deref(), expected, "{case}");
    }
}
