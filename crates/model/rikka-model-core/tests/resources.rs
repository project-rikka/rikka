use std::fs;

use rikka_model_core::Error;

use rikka_model_core::path::relative_parts;
use rikka_model_core::{
    DirectoryResolver, DirectorySink, MemoryResolver, MemorySink, NoResolver, NoSink, Resolver,
    Sink,
};

fn folder() -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    fs::create_dir(folder.path().join("Textures")).expect("a subfolder");
    fs::write(folder.path().join("Textures/Skin.png"), b"skin").expect("a texture");
    folder
}

#[test]
fn resolver_finds_windows_and_miscased_paths() {
    let folder = folder();
    let directory = DirectoryResolver::new(folder.path());
    let memory = MemoryResolver::new().with_file("Textures/Skin.png", b"skin".to_vec());

    for name in [
        "Textures\\Skin.png",
        "textures/skin.PNG",
        "Textures/../Textures/Skin.png",
    ] {
        assert_eq!(
            directory
                .read(name, u64::MAX)
                .expect("the texture")
                .as_ref(),
            b"skin",
            "{name}"
        );
        assert_eq!(
            memory.read(name, u64::MAX).expect("the texture").as_ref(),
            b"skin",
            "{name}"
        );
    }
    assert!(memory.read("other.png", u64::MAX).is_err());
    assert!(NoResolver.read("skin.png", u64::MAX).is_err());
}

#[test]
fn resolver_rejects_oversized_and_non_files() {
    let folder = folder();
    let directory = DirectoryResolver::new(folder.path());
    let memory = MemoryResolver::new().with_file("Textures/Skin.png", b"skin".to_vec());

    for resolver in [&directory as &dyn Resolver, &memory] {
        let refused = resolver.read("Textures/Skin.png", 3);
        assert!(
            matches!(refused, Err(Error::LimitsExceeded { .. })),
            "{refused:?}"
        );
        assert!(resolver.read("Textures/Skin.png", 4).is_ok());
    }
    assert!(matches!(
        directory.read("Textures", u64::MAX),
        Err(Error::Read { .. })
    ));
}

#[test]
fn resolver_rejects_escaping_paths_and_symlinks() {
    let folder = folder();
    std::os::unix::fs::symlink("/etc", folder.path().join("outside")).expect("a link");
    let resolver = DirectoryResolver::new(folder.path());

    for name in [
        "../../etc/passwd",
        "/etc/passwd",
        "C:\\Windows\\win.ini",
        "outside/passwd",
    ] {
        assert!(resolver.read(name, u64::MAX).is_err(), "{name}");
    }
    assert_eq!(relative_parts("a/../../secret"), None);
}

#[test]
fn directory_sink_sanitizes_names() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let sink = DirectorySink::new(folder.path());

    sink.write("Textures\\skin.png", b"pixels")
        .expect("a write");
    sink.write("/etc/passwd", b"pixels").expect("a write");

    assert_eq!(
        fs::read(folder.path().join("Textures/skin.png")).expect("the file"),
        b"pixels"
    );
    assert_eq!(
        fs::read(folder.path().join("etc/passwd")).expect("the file"),
        b"pixels"
    );

    let memory = MemorySink::new();
    memory
        .write("Textures/Skin.png", b"pixels")
        .expect("a write");
    assert_eq!(memory.get("textures\\skin.png"), Some(b"pixels".to_vec()));
    assert_eq!(memory.names(), vec!["textures/skin.png"]);
}

#[test]
fn directory_sink_rejects_escapes() {
    let outside = tempfile::tempdir().expect("a folder outside");
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::os::unix::fs::symlink(outside.path(), folder.path().join("outside")).expect("a link");
    let stolen = outside.path().join("stolen.png");
    std::os::unix::fs::symlink(&stolen, folder.path().join("skin.png")).expect("a link");
    let sink = DirectorySink::new(folder.path());

    assert!(sink.write("../escaped.png", b"pixels").is_err());
    assert!(sink.write("outside/deeper/escaped.png", b"pixels").is_err());
    assert!(sink.write("skin.png", b"pixels").is_err());
    assert!(NoSink.write("skin.png", b"pixels").is_err());

    assert!(
        !folder
            .path()
            .parent()
            .expect("a parent")
            .join("escaped.png")
            .exists()
    );
    assert!(
        !outside.path().join("deeper").exists(),
        "a refused write made a folder outside the root"
    );
    assert!(!stolen.exists(), "a refused write followed a link out");
}
