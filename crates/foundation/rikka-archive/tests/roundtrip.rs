use std::fs;

use rikka_archive::{Archive, ArchiveWriter, CreateOptions, Format};

fn containers() -> impl Iterator<Item = Format> {
    Format::ALL
        .into_iter()
        .filter(|format| format.is_container())
}

fn single_file_formats() -> impl Iterator<Item = Format> {
    Format::ALL
        .into_iter()
        .filter(|format| !format.is_container())
}

fn payload() -> Vec<u8> {
    (0..400_000u32).map(|index| (index % 251) as u8).collect()
}

#[test]
fn containers_round_trip() {
    for format in containers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("test.{}", format.extension()));
        let data = payload();

        let mut writer = ArchiveWriter::create_as(&path, format, CreateOptions::new()).unwrap();
        writer.add_directory("docs").unwrap();
        writer.add_bytes("docs/readme.txt", b"hello").unwrap();
        writer.add_bytes("data.bin", &data).unwrap();
        writer.finish().unwrap();

        assert_eq!(
            rikka_archive::detect(&path).unwrap(),
            format,
            "{format} was not detected from its contents"
        );

        let mut archive = Archive::open(&path).unwrap();
        assert_eq!(archive.format(), format);
        assert_eq!(archive.len(), 3, "{format} entry count");
        assert_eq!(
            archive.read_entry_by_name("docs/readme.txt").unwrap(),
            b"hello",
            "{format} preview"
        );
        assert_eq!(archive.read_entry_by_name("data.bin").unwrap(), data);

        let out = directory.path().join("out");
        let summary = archive.extract(&out).unwrap();
        assert_eq!(summary.files, 2, "{format} extracted files");
        assert_eq!(summary.directories, 1, "{format} extracted directories");
        assert_eq!(
            fs::read(out.join("docs").join("readme.txt")).unwrap(),
            b"hello"
        );
        assert_eq!(fs::read(out.join("data.bin")).unwrap(), data);
    }
}

#[test]
fn single_file_formats_round_trip() {
    for format in single_file_formats() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("notes.txt.{}", format.extension()));
        let data = payload();

        let mut writer = ArchiveWriter::create_as(&path, format, CreateOptions::new()).unwrap();
        writer.add_bytes("notes.txt", &data).unwrap();
        writer.finish().unwrap();

        assert_eq!(rikka_archive::detect(&path).unwrap(), format);

        let mut archive = Archive::open(&path).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(
            archive.entries()[0].name(),
            "notes.txt",
            "{format} entry name"
        );
        assert_eq!(archive.read_entry(0).unwrap(), data);

        let out = directory.path().join("out");
        archive.extract(&out).unwrap();
        assert_eq!(fs::read(out.join("notes.txt")).unwrap(), data);
    }
}

#[test]
fn detects_compressed_tar_vs_single_file() {
    let directory = tempfile::tempdir().unwrap();

    let bare = directory.path().join("lone.gz");
    let mut writer = ArchiveWriter::create_as(&bare, Format::Gzip, CreateOptions::new()).unwrap();
    writer.add_bytes("lone.txt", b"just a file").unwrap();
    writer.finish().unwrap();

    let wrapped = directory.path().join("bundle.gz");
    let mut writer =
        ArchiveWriter::create_as(&wrapped, Format::TarGzip, CreateOptions::new()).unwrap();
    writer.add_bytes("a.txt", b"first").unwrap();
    writer.add_bytes("b.txt", b"second").unwrap();
    writer.finish().unwrap();

    assert_eq!(rikka_archive::detect(&bare).unwrap(), Format::Gzip);
    assert_eq!(rikka_archive::detect(&wrapped).unwrap(), Format::TarGzip);
    assert_eq!(Archive::open(&wrapped).unwrap().len(), 2);
}

#[test]
fn directory_trees_round_trip() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("project");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::write(source.join("src").join("main.rs"), "fn main() {}").unwrap();
    fs::write(source.join("README.md"), "# project").unwrap();

    let path = directory.path().join("project.7z");
    rikka_archive::compress(&path, [&source]).unwrap();

    let entries = rikka_archive::list(&path).unwrap();
    let names: Vec<&str> = entries.iter().map(rikka_archive::Entry::name).collect();
    assert!(names.contains(&"project/README.md"), "{names:?}");
    assert!(names.contains(&"project/src/main.rs"), "{names:?}");

    let out = directory.path().join("out");
    rikka_archive::extract(&path, &out).unwrap();
    assert_eq!(
        fs::read_to_string(out.join("project").join("src").join("main.rs")).unwrap(),
        "fn main() {}"
    );
}

#[test]
fn symlinks_round_trip() {
    for format in [Format::Zip, Format::SevenZ, Format::Tar, Format::TarGzip] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(format!("links.{}", format.extension()));

        let mut writer = ArchiveWriter::create_as(&path, format, CreateOptions::new()).unwrap();
        writer.add_bytes("dir/target.txt", b"contents").unwrap();
        writer.add_symlink("dir/link.txt", "target.txt").unwrap();
        writer.finish().unwrap();

        let mut archive = Archive::open(&path).unwrap();
        let index = archive.find("dir/link.txt").unwrap();
        assert!(
            archive.entries()[index].is_symlink(),
            "{format} lost the link marker"
        );
        assert_eq!(archive.read_link(index).unwrap(), "target.txt", "{format}");
    }
}
