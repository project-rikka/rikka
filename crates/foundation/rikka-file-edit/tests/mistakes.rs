use rikka_file_edit::Edit;

use crate::support::Project;

const USES: &str = "use a::One;\nuse b::Two;\nuse c::Three;\n";

type Edits = fn(&Project) -> Vec<Edit>;

#[test]
fn rejects_echoed_neighbour_line() {
    let cases: [(Edits, &str, &str); 5] = [
        (
            |p| vec![p.lines("uses.txt", 2, 2, "use a::One;\nuse d::Four;")],
            "The edit's text for line 2 begins with line 1, which stays above it:",
            "or start the range at `1#",
        ),
        (
            |p| vec![p.lines("uses.txt", 1, 1, "use z::Zero;\nuse b::Two;")],
            "The edit's text for line 1 ends with line 2, which stays below it:",
            "or end the range at `2#",
        ),
        (
            |p| {
                vec![p.lines(
                    "uses.txt",
                    1,
                    2,
                    "use z::Zero;\nuse y::Nine;\nuse c::Three;",
                )]
            },
            "The edit's text for lines 1-2 ends with line 3, which stays below it:",
            "or end the range at `3#",
        ),
        (
            |p| vec![p.after("uses.txt", 1, "use a::One;\nuse d::Four;")],
            "The edit's text put after line 1 begins with line 1, which stays above it:",
            "an insertion keeps it",
        ),
        (
            |p| vec![p.before("uses.txt", 3, "use d::Four;\nuse c::Three;")],
            "The edit's text put before line 3 ends with line 3, which stays below it:",
            "an insertion keeps it",
        ),
    ];
    for (edits, expected, advice) in cases {
        let project = Project::open();
        project.write_file("uses.txt", USES);

        let refused = project
            .edit("uses.txt", edits(&project))
            .expect_err(expected);

        assert!(
            refused.starts_with("error: echoed_line uses.txt: nothing was written."),
            "{refused}"
        );
        assert!(refused.contains(expected), "{refused}");
        assert!(refused.contains(advice), "{refused}");
        assert!(refused.contains("     2#"), "{refused}");
        assert_eq!(project.read_file("uses.txt"), USES);
    }
}

#[test]
fn allows_intended_neighbour_repeats() {
    let cases: [(&str, Edits, &str); 4] = [
        (
            "x();\nx();\ny();\n",
            |p| vec![p.lines("a.txt", 2, 3, "x();\nz();")],
            "x();\nx();\nz();\n",
        ),
        (
            USES,
            |p| vec![p.lines("a.txt", 2, 2, "use b::Two;\nuse b::Two;")],
            "use a::One;\nuse b::Two;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            USES,
            |p| {
                vec![
                    p.lines("a.txt", 1, 1, "use z::Zero;"),
                    p.lines("a.txt", 2, 2, "use a::One;\nuse b::Two;"),
                ]
            },
            "use z::Zero;\nuse a::One;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            "a {\n}\n}\n",
            |p| vec![p.after("a.txt", 2, "}")],
            "a {\n}\n}\n}\n",
        ),
    ];
    for (written, edits, became) in cases {
        let project = Project::open();
        project.write_file("a.txt", written);

        project.edit("a.txt", edits(&project)).expect(became);

        assert_eq!(project.read_file("a.txt"), became);
    }
}

const RUST: &str = "fn one() {\n    let a = 1;\n}\n\nfn two() {\n    let b = 2;\n}\n";

#[test]
fn rejects_edit_that_breaks_syntax() {
    type Case = (&'static str, &'static str, Edits, &'static str);
    let cases: [Case; 6] = [
        (
            "a.rs",
            RUST,
            |p| vec![p.lines("a.rs", 2, 2, "    let a = 10;\n}")],
            "rust that no longer parses",
        ),
        (
            "a.rs",
            RUST,
            |p| vec![p.lines("a.rs", 2, 3, "    let a = 10;")],
            "rust that no longer parses",
        ),
        (
            "a.rs",
            RUST,
            |p| vec![p.lines("a.rs", 3, 4, "")],
            "- 1 `{` more than `}`\n",
        ),
        (
            "a.json",
            "{\n  \"a\": 1,\n  \"b\": 2\n}\n",
            |p| vec![p.lines("a.json", 3, 3, "  \"b\": 2,")],
            "json that no longer parses",
        ),
        (
            "a.comp",
            "void main() {\n    x = f(1);\n}\n",
            |p| vec![p.lines("a.comp", 2, 2, "    x = f(1;")],
            "1 `(` more than `)`",
        ),
        (
            "a.comp",
            "void main() {\n    x = 1;\n}\n",
            |p| vec![p.lines("a.comp", 3, 3, "")],
            "1 `{` more than `}`",
        ),
    ];
    for (path, written, edits, expected) in cases {
        let project = Project::open();
        project.write_file(path, written);

        let refused = project.edit(path, edits(&project)).expect_err(expected);

        assert!(
            refused.starts_with(&format!(
                "error: broken_syntax {path}: nothing was written."
            )),
            "{refused}"
        );
        assert!(refused.contains(expected), "{refused}");
        let (proposed, current) = refused
            .split_once("They read now:\n")
            .expect("the current lines are shown");
        assert!(
            proposed.contains("The changed places would read:\n"),
            "{refused}"
        );
        assert!(
            !proposed
                .lines()
                .any(|row| row.contains('#') && row.contains('\t')),
            "{refused}"
        );
        assert!(current.contains("#"), "{refused}");
        assert_eq!(project.read_file(path), written);
    }
}

#[test]
fn broken_file_rejects_only_new_errors() {
    let project = Project::open();
    project.write_file("a.rs", RUST);
    project
        .edit(
            "a.rs",
            vec![
                project.lines("a.rs", 2, 2, "    let a = 10;"),
                project.after("a.rs", 7, "\nfn three() {}"),
            ],
        )
        .expect("it still parses");
    assert_eq!(
        project.read_file("a.rs"),
        "fn one() {\n    let a = 10;\n}\n\nfn two() {\n    let b = 2;\n}\n\nfn three() {}\n"
    );

    project.write_file("a.cu", "void f(char c) {\n}\n");
    project
        .edit(
            "a.cu",
            vec![project.after("a.cu", 1, "    if (c == '(') { g(\"{\"); }")],
        )
        .expect("brackets inside literals are not counted");

    let broken = "fn one() {\n    let a = ;\n}\n\nfn two() {\n    let b = 2;\n}\n";
    project.write_file("b.rs", broken);
    project
        .edit("b.rs", vec![project.lines("b.rs", 6, 6, "    let b = 20;")])
        .expect("it breaks nothing more");
    let refused = project
        .edit("b.rs", vec![project.lines("b.rs", 6, 6, "    let b = ;")])
        .expect_err("it breaks a second place");
    assert!(refused.contains("broken_syntax"), "{refused}");
    project
        .edit("b.rs", vec![project.lines("b.rs", 2, 2, "    let a = 1;")])
        .expect("it mends the first place");
    assert_eq!(project.read_file("b.rs"), RUST.replace("b = 2", "b = 20"));
}
