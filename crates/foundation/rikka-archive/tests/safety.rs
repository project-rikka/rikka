use std::fs::File;
use std::path::Path;

use rikka_archive::{Archive, ArchiveWriter, CreateOptions, Error, Format};

fn hostile_zip(path: &Path, name: &str) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    writer
        .start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut writer, b"pwned").unwrap();
    writer.finish().unwrap();
}

#[test]
fn rejects_traversing_entry_names() {
    for name in ["../escaped.txt", "a/../../escaped.txt", "..\\escaped.txt"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("evil.zip");
        hostile_zip(&path, name);

        let out = directory.path().join("out");
        let result = Archive::open(&path).unwrap().extract(&out);

        assert!(
            matches!(result, Err(Error::UnsafeEntryPath(_))),
            "entry {name:?} was accepted: {result:?}"
        );
        assert!(
            !directory.path().join("escaped.txt").exists(),
            "entry {name:?} escaped the destination"
        );
    }
}

#[test]
fn rooted_names_extract_inside_destination() {
    for (name, expected) in [
        ("/etc/rikka-test", "etc/rikka-test"),
        ("C:\\Windows\\rikka-test", "Windows/rikka-test"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rooted.zip");
        hostile_zip(&path, name);

        let mut archive = Archive::open(&path).unwrap();
        assert_eq!(archive.entries()[0].name(), expected);

        let out = directory.path().join("out");
        archive.extract(&out).unwrap();
        assert!(out.join(expected).is_file());
    }
}

#[test]
fn rejects_writing_hostile_name() {
    let directory = tempfile::tempdir().unwrap();
    let mut writer = ArchiveWriter::create(directory.path().join("out.zip")).unwrap();

    assert!(matches!(
        writer.add_bytes("../escape.txt", b"nope"),
        Err(Error::UnsafeEntryPath(_))
    ));
    assert!(matches!(
        writer.add_directory("../escape"),
        Err(Error::UnsafeEntryPath(_))
    ));
}

#[test]
fn rejects_symlink_outside_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evil.tar");

    let mut writer = ArchiveWriter::create_as(&path, Format::Tar, CreateOptions::new()).unwrap();
    writer.add_symlink("escape", "../../elsewhere").unwrap();
    writer.finish().unwrap();

    let out = directory.path().join("out");
    let result = Archive::open(&path).unwrap().extract(&out);
    assert!(
        matches!(result, Err(Error::UnsafeEntryPath(_))),
        "an escaping symlink was accepted: {result:?}"
    );
}

#[test]
fn entry_cannot_replace_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evil.zip");
    hostile_zip(&path, "./");

    let out = directory.path().join("out");
    let result = Archive::open(&path).unwrap().extract(&out);
    assert!(
        matches!(result, Err(Error::UnsafeEntryPath(_))),
        "an empty entry name was accepted: {result:?}"
    );
}

#[test]
fn rejects_symlink_chains_leaving_destination() {
    let chains: [&[(&str, &str)]; 2] = [
        &[("a/b", ".."), ("a/b/evil", "../../elsewhere")],
        &[("up/deep/top", "../.."), ("evil", "up/deep/top/../..")],
    ];
    for chain in chains {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("evil.tar");

        let mut writer =
            ArchiveWriter::create_as(&path, Format::Tar, CreateOptions::new()).unwrap();
        for (name, target) in chain {
            writer.add_symlink(name, target).unwrap();
        }
        writer.finish().unwrap();

        let out = directory.path().join("nested").join("out");
        let result = Archive::open(&path).unwrap().extract(&out);
        assert!(
            matches!(result, Err(Error::UnsafeEntryPath(_))),
            "chain {chain:?} was accepted: {result:?}"
        );
        assert!(
            out.join("evil").symlink_metadata().is_err(),
            "chain {chain:?} left a link leaving the destination"
        );
    }
}

#[test]
fn rejects_entry_below_linked_folder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.zip");
    hostile_zip(&path, "linked/file.txt");

    let outside = directory.path().join("outside");
    let out = directory.path().join("out");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(&outside, out.join("linked")).unwrap();

    let result = Archive::open(&path).unwrap().extract(&out);
    assert!(
        matches!(result, Err(Error::UnsafeEntryPath(_))),
        "an entry was written through a link: {result:?}"
    );
    assert!(!outside.join("file.txt").exists());
}
