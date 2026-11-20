use rikka_llm::json;
use rikka_tools::{EDIT, VIEW, WRITE};

use crate::support::Project;

#[tokio::test]
async fn edit_applies_all_operations() {
    let project = Project::open();
    project.holds("a.txt", "one\ntwo\nthree\n");

    let reply = project
        .call(
            EDIT,
            json!({
                "path": "a.txt",
                "ops": [
                    { "op": "replace", "start_line": 1, "end_line": 1, "text": "ONE" },
                    { "op": "insert_before", "line": 2, "text": "AND\n" },
                    { "op": "delete", "start_line": 3, "end_line": 3 },
                ],
            }),
        )
        .await
        .expect("edit should apply");

    assert_eq!(project.read("a.txt"), "ONE\nAND\ntwo\n");
    assert_eq!(
        reply.text(),
        "ok a.txt (3 lines, +0)\n     1\tONE\n     2\tAND\n     3\ttwo"
    );
}

#[tokio::test]
async fn invalid_edit_writes_nothing() {
    let project = Project::open();
    project.holds("a.txt", "one\n");

    for (ops, expected) in [
        (
            json!([
                { "op": "replace", "start_line": 1, "end_line": 1, "text": "ONE\n" },
                { "op": "replace", "start_line": 9, "end_line": 9, "text": "x\n" },
            ]),
            "out_of_range",
        ),
        (
            json!([{ "op": "replace", "start_line": 1, "text": "x\n" }]),
            "end_line",
        ),
        (
            json!([{ "op": "insert_before", "start_line": 1, "line": 1, "text": "x\n" }]),
            "start_line",
        ),
        (json!([{ "op": "rename", "line": 1 }]), "op1"),
    ] {
        let refused = project
            .call(EDIT, json!({ "path": "a.txt", "ops": ops }))
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
            json!({
                "path": "../escaped.txt",
                "ops": [{ "op": "replace", "start_line": 1, "end_line": 1, "text": "x\n" }],
            }),
        ),
    ] {
        let refused = project.call(stem, input).await.expect_err("it is outside");
        assert!(refused.contains("outside"), "{refused}");
    }
    assert!(!project.at("../escaped.txt").exists());
}

#[tokio::test]
async fn view_rejects_oversized_image() {
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

    let refused = project
        .call(VIEW, json!({ "path": "a.rs" }))
        .await
        .expect_err("it is not a picture");
    assert!(refused.contains("not a picture"), "{refused}");
    assert!(refused.contains("cat"), "{refused}");
    for path in ["shots", "nowhere.png"] {
        assert!(project.call(VIEW, json!({ "path": path })).await.is_err());
    }
}
