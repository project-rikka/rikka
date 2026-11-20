use std::path::{Path, PathBuf};

use ade_tree::{Entry, Tree, rebased};
use ade_watch::Watch;

fn project() -> Tree {
    let mut tree = Tree::new("/project", Watch::disabled());
    tree.opened(
        PathBuf::from("/project"),
        vec![
            Entry::new("src", true),
            Entry::new("Cargo.toml", false),
            Entry::new("README.md", false),
        ],
    );
    tree
}

#[test]
fn expanding_folder_reads_it_once() {
    let mut tree = project();
    let src = PathBuf::from("/project/src");
    assert_eq!(tree.toggle(&src), Some(src.clone()));
    assert!(tree.rows().iter().all(|row| row.depth() == 0));

    tree.opened(src.clone(), vec![Entry::new("main.rs", false)]);
    let rows = tree.rows();
    assert_eq!(rows[1].name(), "main.rs");
    assert_eq!(rows[1].depth(), 1);
    assert!(rows[0].is_open());

    assert_eq!(tree.toggle(&src), None, "what is read is not read again");
    assert_eq!(tree.rows().len(), 3);
    assert_eq!(tree.toggle(&src), None);
    assert_eq!(tree.rows().len(), 4);
}

#[test]
fn rebase_moves_descendants() {
    let (from, to) = (Path::new("/p/src"), Path::new("/p/lib"));

    assert_eq!(rebased(Path::new("/p/src"), from, to), Some(to.to_owned()));
    assert_eq!(
        rebased(Path::new("/p/src/a/b.rs"), from, to),
        Some(PathBuf::from("/p/lib/a/b.rs"))
    );
    assert_eq!(rebased(Path::new("/p/srcs/x"), from, to), None);
}
