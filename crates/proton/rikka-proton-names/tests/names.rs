use rikka_proton_names::{NAME_LIMIT, numbered_local_name, numbered_name};

#[test]
fn copies_are_numbered_from_one_before_the_extension() {
    for (name, number, expected) in [
        ("holiday.txt", 1, "holiday (1).txt"),
        ("archive.tar.gz", 2, "archive.tar (2).gz"),
        ("notes", 1, "notes (1)"),
        (".bashrc", 1, ".bashrc (1)"),
        ("trailing.", 3, "trailing. (3)"),
    ] {
        assert_eq!(numbered_name(name, number), expected, "{name}");
    }
    let long = format!("{}.txt", "長".repeat(200));
    let copy = numbered_local_name(&long, 1);
    assert!(copy.len() <= NAME_LIMIT, "{}", copy.len());
    assert!(copy.ends_with("長 (1).txt"), "{copy}");
    assert!(numbered_name(&long, 1).len() > NAME_LIMIT);
}
