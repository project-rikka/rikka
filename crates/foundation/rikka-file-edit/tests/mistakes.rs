use crate::support::{Project, delete_op, insert_op, replace_op};

const USES: &str = "use a::One;\nuse b::Two;\nuse c::Three;\n";

#[test]
fn rejects_echoed_neighbour_line() {
    let cases = [
        (
            replace_op(2, 2, "use a::One;\nuse d::Four;"),
            "The text for line 2 begins with line 1, which stays above it:",
            "start the range at 1",
        ),
        (
            replace_op(1, 1, "use z::Zero;\nuse b::Two;"),
            "The text for line 1 ends with line 2, which stays below it:",
            "end the range at 2",
        ),
        (
            replace_op(1, 2, "use z::Zero;\nuse y::Nine;\nuse c::Three;"),
            "The text for lines 1-2 ends with line 3, which stays below it:",
            "end the range at 3",
        ),
        (
            insert_op(3, "use b::Two;\nuse d::Four;\n"),
            "The text put before line 3 begins with line 2, which stays above it:",
            "replace line 2 with the text if it should appear twice",
        ),
        (
            insert_op(2, "use d::Four;\nuse b::Two;\n"),
            "The text put before line 2 ends with line 2, which stays below it:",
            "replace line 2 with the text if it should appear twice",
        ),
    ];
    for (op, expected, cure) in cases {
        let project = Project::open();
        project.write_file("uses.txt", USES);

        let refused = project.edit("uses.txt", vec![op]).expect_err(expected);

        assert!(
            refused.starts_with("error: echoed_line uses.txt: nothing was written."),
            "{refused}"
        );
        assert!(refused.contains(expected), "{refused}");
        assert!(refused.contains(cure), "{refused}");
        assert!(refused.contains("     2\tuse b::Two;"), "{refused}");
        assert_eq!(project.read_file("uses.txt"), USES);
    }
}

#[test]
fn allows_intended_neighbour_repeats() {
    let cases = [
        (
            "x();\nx();\ny();\n",
            vec![replace_op(2, 3, "x();\nz();")],
            "x();\nx();\nz();\n",
        ),
        (
            USES,
            vec![replace_op(2, 2, "use b::Two;\nuse b::Two;")],
            "use a::One;\nuse b::Two;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            USES,
            vec![
                replace_op(1, 1, "use z::Zero;"),
                replace_op(2, 2, "use a::One;\nuse b::Two;"),
            ],
            "use z::Zero;\nuse a::One;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            "a {\n}\n}\n",
            vec![replace_op(2, 2, "}\n}")],
            "a {\n}\n}\n}\n",
        ),
        (
            "one\n\ntwo\n",
            vec![insert_op(3, "\nthree\n")],
            "one\n\n\nthree\ntwo\n",
        ),
    ];
    for (write_file, ops, became) in cases {
        let project = Project::open();
        project.write_file("a.txt", write_file);

        project.edit("a.txt", ops).expect(became);

        assert_eq!(project.read_file("a.txt"), became);
    }
}

const RUST: &str = "fn one() {\n    let a = 1;\n}\n\nfn two() {\n    let b = 2;\n}\n";

#[test]
fn rejects_edit_that_breaks_syntax() {
    let cases = [
        (
            "a.rs",
            RUST,
            replace_op(2, 2, "    let a = 10;\n}"),
            "rust that no longer parses",
        ),
        (
            "a.rs",
            RUST,
            replace_op(2, 3, "    let a = 10;"),
            "rust that no longer parses",
        ),
        ("a.rs", RUST, delete_op(3, 3), "- 1 `{` more than `}`\n"),
        (
            "a.json",
            "{\n  \"a\": 1,\n  \"b\": 2\n}\n",
            replace_op(3, 3, "  \"b\": 2,"),
            "json that no longer parses",
        ),
        (
            "a.comp",
            "void main() {\n    x = f(1);\n}\n",
            replace_op(2, 2, "    x = f(1;"),
            "1 `(` more than `)`",
        ),
        (
            "a.comp",
            "void main() {\n    x = 1;\n}\n",
            delete_op(3, 3),
            "1 `{` more than `}`",
        ),
    ];
    for (path, write_file, op, expected) in cases {
        let project = Project::open();
        project.write_file(path, write_file);

        let refused = project.edit(path, vec![op]).expect_err(expected);

        assert!(
            refused.starts_with(&format!(
                "error: broken_syntax {path}: nothing was written."
            )),
            "{refused}"
        );
        assert!(refused.contains(expected), "{refused}");
        assert!(
            refused.contains("The changed places would read:\n"),
            "{refused}"
        );
        assert!(refused.contains("They read now:\n"), "{refused}");
        assert_eq!(project.read_file(path), write_file);
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
                replace_op(2, 2, "    let a = 10;"),
                insert_op(8, "\nfn three() {}\n"),
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
            vec![insert_op(2, "    if (c == '(') { g(\"{\"); }\n")],
        )
        .expect("brackets inside literals are not counted");

    let broken = "fn one() {\n    let a = ;\n}\n\nfn two() {\n    let b = 2;\n}\n";
    project.write_file("b.rs", broken);
    project
        .edit("b.rs", vec![replace_op(6, 6, "    let b = 20;")])
        .expect("it breaks nothing more");
    let refused = project
        .edit("b.rs", vec![replace_op(6, 6, "    let b = ;")])
        .expect_err("it breaks a second place");
    assert!(refused.contains("broken_syntax"), "{refused}");
    project
        .edit("b.rs", vec![replace_op(2, 2, "    let a = 1;")])
        .expect("it mends the first place");
    assert_eq!(project.read_file("b.rs"), RUST.replace("b = 2", "b = 20"));
}
