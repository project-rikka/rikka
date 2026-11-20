use files_core::Location;
use files_places::{add, remove, reorder};

#[test]
fn pins_share_gtk_bookmarks_file() {
    let config = tempfile::tempdir().expect("create temporary directory");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", config.path()) };
    let file = config.path().join("gtk-3.0/bookmarks");
    std::fs::create_dir_all(file.parent().unwrap()).expect("create config directory");
    std::fs::write(&file, "file:///mnt/work Projects\n").expect("write bookmarks");
    let photos = Location::from_path("/home/eldo/写真");
    let work = Location::from_path("/mnt/work");

    add(&photos, None).expect("add bookmark");
    add(&photos, Some("Again")).expect("add bookmark");
    assert_eq!(
        std::fs::read_to_string(&file).expect("read bookmarks"),
        "file:///mnt/work Projects\nfile:///home/eldo/%E5%86%99%E7%9C%9F\n"
    );

    reorder(&photos, &work).expect("reorder bookmarks");
    assert_eq!(
        std::fs::read_to_string(&file).expect("read bookmarks"),
        "file:///home/eldo/%E5%86%99%E7%9C%9F\nfile:///mnt/work Projects\n"
    );

    remove(&photos).expect("remove bookmark");
    assert_eq!(
        std::fs::read_to_string(&file).expect("read bookmarks"),
        "file:///mnt/work Projects\n"
    );
}
