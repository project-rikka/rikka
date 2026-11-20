use rikka_dirs::UserDirs;

#[test]
fn file_of_names_only_plain_files_in_the_library() {
    let folder = UserDirs::load().wallpapers().expect("wallpaper folder");
    std::fs::create_dir_all(folder.join("nested")).expect("library folder");
    std::fs::write(folder.join("sunset.png"), b"png").expect("wallpaper");
    std::fs::write(folder.join("nested/dusk.png"), b"png").expect("nested wallpaper");
    let outside = folder
        .parent()
        .expect("pictures folder")
        .join("outside.png");
    std::fs::write(&outside, b"png").expect("outside picture");

    let cases = [
        ("sunset.png", Some(folder.join("sunset.png"))),
        ("missing.png", None),
        ("nested", None),
        ("nested/dusk.png", None),
        ("../outside.png", None),
        (outside.to_str().expect("utf-8 path"), None),
        ("", None),
        (".", None),
    ];
    for (id, expected) in cases {
        assert_eq!(shell_wallpaper::file_of(id), expected, "{id:?}");
    }
}
