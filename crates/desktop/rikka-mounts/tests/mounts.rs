use std::path::Path;

#[test]
fn root_is_mounted() {
    let mounts = rikka_mounts::mounts();
    assert!(
        mounts.iter().any(|mount| mount.at == Path::new("/")),
        "{mounts:?}"
    );
    let root = rikka_mounts::containing(Path::new("/")).expect("a root mount");
    assert_eq!(root.at, Path::new("/"));
    assert!(rikka_mounts::top_directories().contains(&root.at));
}
