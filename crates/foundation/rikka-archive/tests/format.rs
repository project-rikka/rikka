use rikka_archive::Format;

#[test]
fn splits_name_into_stem_and_format() {
    let cases = [
        ("photos.zip", Some(("photos", Format::Zip))),
        ("Backup.TAR.GZ", Some(("Backup", Format::TarGzip))),
        ("source.tgz", Some(("source", Format::TarGzip))),
        ("logs.tbz2", Some(("logs", Format::TarBzip2))),
        ("notes.txt.zst", Some(("notes.txt", Format::Zstd))),
        ("写真.7z", Some(("写真", Format::SevenZ))),
        (".zip", None),
        ("readme.md", None),
    ];
    for (name, expected) in cases {
        assert_eq!(Format::split(name), expected, "{name}");
    }
}
