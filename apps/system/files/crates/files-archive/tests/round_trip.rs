use std::path::{Path, PathBuf};
use std::sync::Arc;

use files_archive::{ArchiveBackend, Format, Level, Packing};
use files_core::Location;
use files_vfs::{Backend, ListOptions, Operation};
use rikka_file_operations::Store;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("files-archive-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create directory");
        Self(path)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent directory"))
            .expect("create parent directories");
        std::fs::write(path, contents).expect("write file");
    }

    fn at(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn finished(job: &Operation) {
    let outcome = job.wait();
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    assert!(!outcome.cancelled);
}

fn backend() -> ArchiveBackend {
    ArchiveBackend::new(Arc::new(rikka_mime::Database::load()))
}

fn names(entries: &[files_core::Entry]) -> Vec<&str> {
    let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    names.sort_unstable();
    names
}

fn read(backend: &ArchiveBackend, at: &Location) -> String {
    let mut reader = backend.read(at).expect("open entry");
    let mut text = String::new();
    std::io::Read::read_to_string(&mut reader, &mut text).expect("read entry");
    text
}

#[test]
fn packed_folder_round_trips() {
    let sandbox = Sandbox::new("round-trip");
    sandbox.write("photos/a.txt", "hello");
    sandbox.write("photos/2026/b.txt", "world");
    let archive = sandbox.at("out.zip");

    let packing = files_archive::compress(Packing {
        sources: vec![sandbox.at("photos")],
        into: archive.clone(),
        format: Format::Zip,
        level: Level::Balanced,
        password: None,
    });
    finished(&packing);
    assert!(archive.is_file());

    let backend = backend();
    let root = backend
        .browse(&Location::from_path(&archive), None)
        .expect("open written archive");
    assert_eq!(root.name(), "out.zip");

    let top = backend
        .list(&root, ListOptions::default())
        .expect("list directory");
    assert_eq!(names(&top), ["photos"]);
    assert!(top[0].is_directory());

    let inside = root.child("photos");
    let listing = backend
        .list(&inside, ListOptions::default())
        .expect("list directory");
    assert_eq!(names(&listing), ["2026", "a.txt"]);
    assert_eq!(read(&backend, &inside.child("a.txt")), "hello");
    assert_eq!(
        read(&backend, &inside.child("2026").child("b.txt")),
        "world"
    );

    assert_eq!(
        backend.parent_of(&root),
        Some(Location::from_path(&sandbox.0))
    );

    let into = sandbox.at("restored");
    std::fs::create_dir_all(&into).expect("create target directory");
    let unpacking = files_archive::extract(archive, into.clone(), None);
    finished(&unpacking);

    assert_eq!(
        std::fs::read_to_string(into.join("photos/a.txt")).expect("read extracted file"),
        "hello"
    );
    assert_eq!(
        std::fs::read_to_string(into.join("photos/2026/b.txt")).expect("read extracted file"),
        "world"
    );
}

#[test]
fn packing_keeps_executable_bit() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("executable");
    sandbox.write("bin/run.sh", "#!/bin/sh\n");
    std::fs::set_permissions(
        sandbox.at("bin/run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .expect("write executable");
    let archive = sandbox.at("bin.zip");

    finished(&files_archive::compress(Packing {
        sources: vec![sandbox.at("bin")],
        into: archive.clone(),
        format: Format::Zip,
        level: Level::Balanced,
        password: None,
    }));

    let backend = backend();
    let root = backend
        .browse(&Location::from_path(&archive), None)
        .expect("create archive");
    let listing = backend
        .list(&root.child("bin"), ListOptions::default())
        .expect("list directory");
    assert_eq!(listing[0].mode & 0o111, 0o111, "{:o}", listing[0].mode);

    let into = sandbox.at("out");
    std::fs::create_dir_all(&into).expect("create target directory");
    finished(&files_archive::extract(archive, into.clone(), None));

    let unpacked = std::fs::metadata(into.join("bin/run.sh")).expect("read extracted file");
    assert_eq!(unpacked.permissions().mode() & 0o111, 0o111);
}

#[test]
fn loose_files_extract_into_named_folder() {
    let sandbox = Sandbox::new("loose");
    sandbox.write("a.txt", "one");
    sandbox.write("b.txt", "two");
    let archive = sandbox.at("loose.tar.gz");

    let packing = files_archive::compress(Packing {
        sources: vec![sandbox.at("a.txt"), sandbox.at("b.txt")],
        into: archive.clone(),
        format: Format::TarGzip,
        level: Level::Fastest,
        password: None,
    });
    finished(&packing);

    let into = sandbox.at("out");
    std::fs::create_dir_all(&into).expect("create target directory");
    let unpacking = files_archive::extract(archive, into.clone(), None);
    finished(&unpacking);

    assert_eq!(
        std::fs::read_to_string(into.join("loose/a.txt")).expect("read extracted file"),
        "one"
    );
    assert!(!Path::new(&into.join("a.txt")).exists());
}

#[test]
fn changed_archive_is_reindexed() {
    let sandbox = Sandbox::new("stale");
    sandbox.write("first.txt", "one");
    let archive = sandbox.at("changing.zip");

    let pack = |name: &str| {
        finished(&files_archive::compress(Packing {
            sources: vec![sandbox.at(name)],
            into: archive.clone(),
            format: Format::Zip,
            level: Level::Balanced,
            password: None,
        }));
    };
    pack("first.txt");

    let backend = backend();
    let root = backend
        .browse(&Location::from_path(&archive), None)
        .expect("create archive");
    assert_eq!(
        names(
            &backend
                .list(&root, ListOptions::default())
                .expect("list directory")
        ),
        ["first.txt"]
    );

    std::fs::remove_file(&archive).expect("remove archive");
    sandbox.write("second.txt", "two");
    pack("second.txt");

    assert_eq!(
        names(
            &backend
                .list(&root, ListOptions::default())
                .expect("list directory")
        ),
        ["second.txt"]
    );
}

#[test]
fn compressed_folder_copy_reads_once() {
    let sandbox = Sandbox::new("copy-out");
    for index in 0..8 {
        sandbox.write(&format!("many/file{index}.txt"), &"x".repeat(4096));
    }
    let archive = sandbox.at("many.tar.gz");
    finished(&files_archive::compress(Packing {
        sources: vec![sandbox.at("many")],
        into: archive.clone(),
        format: Format::TarGzip,
        level: Level::Fastest,
        password: None,
    }));

    let backend = Arc::new(backend());
    let root = backend
        .browse(&Location::from_path(&archive), None)
        .expect("create archive");
    let inside = root.child("many");
    let entries: Vec<Location> = backend
        .list(&inside, ListOptions::default())
        .expect("list directory")
        .into_iter()
        .map(|entry| entry.location)
        .collect();
    assert_eq!(entries.len(), 8);

    let mut order = Vec::new();
    let mut read = Vec::new();
    backend
        .read_each(&entries, &mut |position, source| {
            let mut text = String::new();
            let reader = source.expect("open entry");
            std::io::Read::read_to_string(reader, &mut text).expect("read entry");
            order.push(position);
            read.push(text.len());
            Ok(())
        })
        .expect("read requested files");

    assert_eq!(read, vec![4096; 8], "every file arrives whole");
    let mut seen = order.clone();
    seen.sort_unstable();
    assert_eq!(seen, (0..8).collect::<Vec<usize>>(), "and each one once");

    let mut vfs = files_vfs::Vfs::new(Arc::new(rikka_mime::Database::load()));
    vfs.register(backend);
    let into = sandbox.at("out");
    std::fs::create_dir_all(&into).expect("create target directory");
    let copying = rikka_file_operations::run(
        Arc::new(vfs),
        files_vfs::Task::Copy {
            sources: vec![inside],
            into: Location::from_path(&into),
        },
    );
    finished(&copying);

    for index in 0..8 {
        assert_eq!(
            std::fs::read_to_string(into.join(format!("many/file{index}.txt")))
                .expect("read copied file")
                .len(),
            4096
        );
    }
}
