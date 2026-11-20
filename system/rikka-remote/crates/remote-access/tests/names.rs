use remote_access::clean_file_name;

#[test]
fn file_names_stay_inside_folder() {
    let long = "あ".repeat(100);
    let cases = [
        ("photo.png", "photo.png".to_owned()),
        ("../../etc/passwd", "passwd".to_owned()),
        ("..\\..\\boot.ini", "boot.ini".to_owned()),
        ("/absolute/path/report.pdf", "report.pdf".to_owned()),
        (".bashrc", "bashrc".to_owned()),
        ("...", "file".to_owned()),
        ("..", "file".to_owned()),
        ("", "file".to_owned()),
        ("dir/", "file".to_owned()),
        ("  spaced name.txt  ", "spaced name.txt".to_owned()),
        ("line\nbreak\u{7}.txt", "linebreak.txt".to_owned()),
        (long.as_str(), "あ".repeat(66)),
    ];
    for (requested, expected) in cases {
        assert_eq!(clean_file_name(requested), expected, "{requested:?}");
    }
}
