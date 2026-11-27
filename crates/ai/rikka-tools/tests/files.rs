use rikka_llm::json;
use rikka_tools::{EDIT, VIEW, WRITE};

use crate::support::Project;

#[tokio::test]
async fn edit_applies_one_replacement_or_several() {
    let project = Project::open();
    project.holds("a.rs", "fn a() {\n    one();\n}\n");

    let reply = project
        .call(
            EDIT,
            json!({ "path": "a.rs", "old_string": "one()", "new_string": "two()" }),
        )
        .await
        .expect("edit should apply");
    assert_eq!(project.read("a.rs"), "fn a() {\n    two();\n}\n");
    assert_eq!(
        reply.text(),
        "ok a.rs (3 lines, +0)\n     1\tfn a() {\n     2\t    two();\n     3\t}"
    );

    project
        .call(
            EDIT,
            json!({
                "path": "a.rs",
                "edits": [
                    { "old_string": "fn a()", "new_string": "fn b()" },
                    { "old_string": "two", "new_string": "three", "replace_all": true },
                    { "old_string": "}\n", "new_string": "}\n\nfn c() {}\n", "replace_all": null },
                ],
            }),
        )
        .await
        .expect("edits should apply");
    assert_eq!(
        project.read("a.rs"),
        "fn b() {\n    three();\n}\n\nfn c() {}\n"
    );
}

#[tokio::test]
async fn invalid_edit_writes_nothing() {
    let project = Project::open();
    project.holds("a.txt", "one\n");

    for (input, expected) in [
        (
            json!({ "path": "a.txt", "old_string": "one", "new_string": "x", "edits": [] }),
            "not both",
        ),
        (
            json!({ "path": "a.txt", "old_string": "one" }),
            "new_string is missing",
        ),
        (
            json!({ "path": "a.txt" }),
            "set old_string and new_string, or edits",
        ),
        (
            json!({
                "path": "a.txt",
                "replace_all": true,
                "edits": [{ "old_string": "one", "new_string": "x" }],
            }),
            "inside each",
        ),
        (
            json!({
                "path": "a.txt",
                "edits": [{ "old_string": "one", "new_string": "x", "line": 1 }],
            }),
            "unknown field",
        ),
        (
            json!({ "path": "a.txt", "old_string": "two", "new_string": "x" }),
            "no_match",
        ),
    ] {
        let refused = project
            .call(EDIT, input)
            .await
            .expect_err("the edit is refused");
        assert!(refused.contains(expected), "{refused}");
        assert_eq!(project.read("a.txt"), "one\n");
    }
}

#[tokio::test]
async fn write_creates_parents_and_reports_overwrite() {
    let project = Project::open();

    let reply = project
        .call(
            WRITE,
            json!({ "path": "src/deep/new.rs", "content": "a\nb\n" }),
        )
        .await
        .expect("file should be written");
    assert_eq!(project.read("src/deep/new.rs"), "a\nb\n");
    assert!(reply.text().contains("2 lines"), "{}", reply.text());

    let reply = project
        .call(
            WRITE,
            json!({ "path": "src/deep/new.rs", "content": "c\n" }),
        )
        .await
        .expect("file should be written");
    assert!(
        reply.text().starts_with("Rewrote src/deep/new.rs"),
        "{}",
        reply.text()
    );
    assert_eq!(project.read("src/deep/new.rs"), "c\n");
}

#[tokio::test]
async fn writes_outside_project_are_rejected() {
    let project = Project::open();

    for (stem, input) in [
        (WRITE, json!({ "path": "../escaped.txt", "content": "x" })),
        (WRITE, json!({ "path": "/tmp/escaped.txt", "content": "x" })),
        (
            EDIT,
            json!({ "path": "../escaped.txt", "old_string": "a", "new_string": "b" }),
        ),
    ] {
        let refused = project.call(stem, input).await.expect_err("it is outside");
        assert!(refused.contains("outside"), "{refused}");
    }
    assert!(!project.at("../escaped.txt").exists());
}

#[tokio::test]
async fn view_shows_pictures_and_refuses_the_rest() {
    let project = Project::open();
    std::fs::write(project.at("shot.png"), [0x89, b'P', b'N', b'G'])
        .expect("file should be written");
    project.holds("a.rs", "fn main() {}");
    std::fs::create_dir(project.at("shots")).expect("directory should be created");

    let reply = project
        .call(VIEW, json!({ "path": "shot.png" }))
        .await
        .expect("view should return the picture");
    assert_eq!(reply.text(), "shot.png, 4 bytes");
    assert_eq!(reply.displayed_images().len(), 1);

    for (path, expected) in [
        ("a.rs", "not a picture"),
        ("shots", "is a folder"),
        ("nowhere.png", "there is no file"),
    ] {
        let refused = project
            .call(VIEW, json!({ "path": path }))
            .await
            .expect_err("it is refused");
        assert!(refused.contains(expected), "{path}: {refused}");
    }
}
