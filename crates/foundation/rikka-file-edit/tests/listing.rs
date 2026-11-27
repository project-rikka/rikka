use rikka_file_edit::{Edit, Listing};

use crate::support::Project;

const SOURCE: &str = "one\ntwo\nthree\n";

#[test]
fn numbered_listing_anchors_edits() {
    let project = Project::open();
    project.write_file("src/a.rs", SOURCE);
    let two = project.at("src/a.rs", 2);

    assert!(
        project.listing("src/a.rs").starts_with("     1#"),
        "{}",
        project.listing("src/a.rs")
    );
    project
        .edit(
            "src/a.rs",
            vec![Edit::Replace {
                first: two,
                last: two,
                text: "TWO".to_owned(),
            }],
        )
        .expect("a listed anchor applies");
    assert_eq!(project.read_file("src/a.rs"), "one\nTWO\nthree\n");
}

#[test]
fn search_output_is_anchored_only_where_it_matches() {
    let project = Project::open();
    project.write_file("src/a.rs", SOURCE);
    project.write_file("src/b.rs", "one\r\ntwo\r\n");
    project.write_file(".rikka/tmp/run.log", SOURCE);
    let elsewhere = tempfile::tempdir().expect("somewhere else");
    std::fs::write(elsewhere.path().join("a.rs"), SOURCE).expect("it is written");
    let outside = elsewhere.path().join("a.rs").display().to_string();

    let a = |line: usize| project.at("src/a.rs", line).to_string();
    let b1 = project.at("src/b.rs", 1).to_string();
    let named = Listing::new().file("src/a.rs");
    let cases = [
        (
            Listing::new(),
            "src/a.rs:2:two\nsrc/a.rs-3-three\n--\n".to_owned(),
            format!("src/a.rs:{}:two\nsrc/a.rs-{}-three\n--\n", a(2), a(3)),
        ),
        (
            named.clone(),
            "2:two\n3-three".to_owned(),
            format!("{}:two\n{}-three", a(2), a(3)),
        ),
        (
            Listing::new().folder("src"),
            "a.rs:1:one\n".to_owned(),
            format!("a.rs:{}:one\n", a(1)),
        ),
        (
            Listing::new().file("src/b.rs"),
            "     1\tone\r\n".to_owned(),
            format!("     {b1}\tone\r\n"),
        ),
        (
            named.clone(),
            "     2\tTWO\n     9\tnine\n".to_owned(),
            "     2\tTWO\n     9\tnine\n".to_owned(),
        ),
        (
            Listing::new(),
            "src/a.rs:2:three\n".to_owned(),
            "src/a.rs:2:three\n".to_owned(),
        ),
        (
            Listing::new(),
            format!("{outside}:1:one\n"),
            format!("{outside}:1:one\n"),
        ),
        (
            Listing::new()
                .file(".rikka/tmp/run.log")
                .except(".rikka/tmp"),
            "     1\tone\n".to_owned(),
            "     1\tone\n".to_owned(),
        ),
        (
            named,
            "2026-11-27 12:30:45 two\n".to_owned(),
            "2026-11-27 12:30:45 two\n".to_owned(),
        ),
    ];
    for (listing, output, anchored) in cases {
        assert_eq!(
            project.files.anchor(&listing, &output),
            anchored,
            "{output}"
        );
    }
}
