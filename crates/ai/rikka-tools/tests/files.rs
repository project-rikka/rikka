use rikka_llm::json;
use rikka_tools::{BASH, EDIT, VIEW, WRITE};

use crate::support::Project;

#[tokio::test]
async fn numbered_reads_anchor_edits() {
    let project = Project::open();
    project.holds("src/a.rs", "fn a() {\n    one();\n}\n");

    let listed = project
        .call(
            BASH,
            json!({ "command": "cat -n src/a.rs | sed -n '1,3p'" }),
        )
        .await
        .expect("the file is listed");
    let reply = project
        .call(
            EDIT,
            json!({
                "path": "src/a.rs",
                "edits": [
                    { "lines": [anchor(listed.text(), 2)], "text": "    two();" },
                    { "after": anchor(listed.text(), 3), "text": "\nfn b() {}" },
                ],
            }),
        )
        .await
        .expect("the listed anchors apply");
    assert_eq!(
        project.read("src/a.rs"),
        "fn a() {\n    two();\n}\n\nfn b() {}\n"
    );
    assert!(
        reply.text().starts_with("ok src/a.rs (5 lines, +2)\n"),
        "{}",
        reply.text()
    );

    let excerpt = reply
        .text()
        .split_once('\n')
        .map_or("", |(_, excerpt)| excerpt);
    let renamed = project
        .call(
            EDIT,
            json!({
                "path": "src/a.rs",
                "edits": [{ "lines": [anchor(excerpt, 5)], "text": "fn c() {}" }],
            }),
        )
        .await
        .expect("the result's anchors apply");
    assert_eq!(
        project.read("src/a.rs"),
        "fn a() {\n    two();\n}\n\nfn c() {}\n"
    );

    let found = project
        .call(
            BASH,
            json!({ "command": "cd src && grep -n -E 'two|c\\(' a.rs" }),
        )
        .await
        .expect("the file is searched");
    assert_eq!(
        found.text(),
        format!(
            "{}:    two();\n{}:fn c() {{}}",
            anchor(excerpt, 2),
            anchor(renamed.text(), 5)
        )
    );
    let plain = project
        .call(BASH, json!({ "command": "cat src/a.rs" }))
        .await
        .expect("the file is read");
    assert!(!plain.text().contains('#'), "{}", plain.text());
}

#[tokio::test]
async fn invalid_edit_writes_nothing() {
    let project = Project::open();
    project.holds("a.txt", "one\n");

    for (input, expected) in [
        (json!({ "path": "a.txt" }), "missing field `edits`"),
        (
            json!({ "path": "a.txt", "old_string": "one", "new_string": "x" }),
            "unknown field `old_string`",
        ),
        (
            json!({ "path": "a.txt", "edits": [{ "lines": ["1"] }] }),
            "missing field `text`",
        ),
        (
            json!({ "path": "a.txt", "edits": [{ "lines": ["1"], "after": "1", "text": "x" }] }),
            "invalid edit 1: set exactly one of lines, after or before",
        ),
        (
            json!({ "path": "a.txt", "edits": [{ "lines": ["1", "1", "1"], "text": "x" }] }),
            "lines takes one anchor, or the first and last of a range",
        ),
        (
            json!({ "path": "a.txt", "edits": [{ "before": "one", "text": "x" }] }),
            "invalid anchor `one`",
        ),
        (
            json!({ "path": "a.txt", "edits": [{ "lines": ["1"], "text": "x" }] }),
            "`1` has no tag.",
        ),
    ] {
        let refused = project
            .call(EDIT, input)
            .await
            .expect_err("the edit is refused");
        assert!(refused.contains(expected), "{expected} in {refused}");
        assert_eq!(project.read("a.txt"), "one\n");
    }
}

fn anchor(listing: &str, line: usize) -> String {
    listing
        .lines()
        .find_map(|row| {
            let (anchor, _) = row.trim_start().split_once(['\t', ':'])?;
            anchor
                .starts_with(&format!("{line}#"))
                .then(|| anchor.to_owned())
        })
        .unwrap_or_else(|| panic!("line {line} is anchored in {listing}"))
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
            json!({ "path": "../escaped.txt", "edits": [{ "lines": ["1#ab"], "text": "b" }] }),
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
