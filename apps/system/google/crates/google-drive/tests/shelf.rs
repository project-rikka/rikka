use google_drive::{File, Scope, Shelf};

use crate::support::file;

fn parsed(id: &str, name: &str) -> File {
    serde_json::from_value(file(id, name, "text/plain")).expect("a file")
}

#[test]
fn listings_are_recalled_per_scope() {
    let folder = tempfile::tempdir().unwrap();
    let shelf = Shelf::new(folder.path());
    let files = vec![parsed("a", "a.txt"), parsed("b", "b.txt")];

    shelf
        .remember(&Scope::my_drive(), &files)
        .expect("a kept listing");
    shelf
        .remember(&Scope::Search("a".to_owned()), &files)
        .expect("searches are not kept");
    shelf
        .remember(&Scope::Folder("../escape".to_owned()), &files)
        .expect("odd ids are not kept");

    let recall = |scope: Scope| shelf.recall(&scope).expect("a readable listing");
    assert_eq!(recall(Scope::my_drive()), Some(files));
    assert_eq!(recall(Scope::Starred), None);
    assert_eq!(recall(Scope::Search("a".to_owned())), None);
    assert_eq!(recall(Scope::Folder("../escape".to_owned())), None);
    assert!(!folder.path().parent().unwrap().join("escape.json").exists());
}

#[test]
fn opened_copies_follow_the_file_version() {
    let folder = tempfile::tempdir().unwrap();
    let shelf = Shelf::new(folder.path());
    let mut file = parsed("a", "../../notes.txt");

    let first = shelf.opened(&file, None);
    file.version = Some(8);
    let second = shelf.opened(&file, Some("pdf"));

    assert!(first.starts_with(folder.path().join("opened").join("a-7")));
    assert!(second.starts_with(folder.path().join("opened").join("a-8")));
    assert_eq!(
        second.extension().and_then(|extension| extension.to_str()),
        Some("pdf")
    );
    assert!(first.parent().unwrap().starts_with(folder.path()));
}

#[test]
fn thumbnails_are_kept_with_their_measured_size() {
    let folder = tempfile::tempdir().unwrap();
    let shelf = Shelf::new(folder.path());
    let file = parsed("a", "a.png");
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\xdc\0\0\0\x7b\x08\x06\0\0\0";
    let jpeg = b"\xff\xd8\xff\xe0\0\x04JF\xff\xc2\0\x11\x08\0\x50\0\xa0\x03";

    assert_eq!(shelf.thumbnail(&file), None);
    let kept = shelf
        .keep_thumbnail(&file, png)
        .expect("a kept thumbnail")
        .expect("a picture");
    let wide = shelf
        .keep_thumbnail(&parsed("b", "b.jpg"), jpeg)
        .expect("a kept thumbnail")
        .expect("a picture");
    let refused = shelf
        .keep_thumbnail(&parsed("c", "c.txt"), b"<html>")
        .expect("no failure");

    assert_eq!((kept.width, kept.height), (220, 123));
    assert_eq!(
        kept.path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png")
    );
    assert_eq!((wide.width, wide.height), (160, 80));
    assert_eq!(refused, None);
    assert_eq!(shelf.thumbnail(&file), Some(kept));
}

#[test]
fn pruning_drops_what_exceeds_the_budget() {
    let folder = tempfile::tempdir().unwrap();
    let shelf = Shelf::new(folder.path());
    for id in ["a", "b", "c"] {
        let path = shelf.opened(&parsed(id, "data.bin"), None);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![0_u8; 1000]).unwrap();
    }

    shelf.prune_opened(2500).expect("a pruned shelf");

    let left = std::fs::read_dir(folder.path().join("opened"))
        .unwrap()
        .count();
    assert_eq!(left, 2);
}
