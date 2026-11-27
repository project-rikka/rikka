use rikka_file_edit::Replacement;

use crate::support::Project;

const USES: &str = "use a::One;\nuse b::Two;\nuse c::Three;\n";

#[test]
fn rejects_echoed_neighbour_line() {
    let cases = [
        (
            Replacement::new("use b::Two;\n", "use a::One;\nuse d::Four;\n"),
            "new_string begins with line 1, which stays above the replaced text:",
        ),
        (
            Replacement::new("use b::Two;", "use a::One;\nuse d::Four;"),
            "new_string begins with line 1, which stays above the replaced text:",
        ),
        (
            Replacement::new("use a::One;\n", "use z::Zero;\nuse b::Two;\n"),
            "new_string ends with line 2, which stays below the replaced text:",
        ),
        (
            Replacement::new(
                "use a::One;\nuse b::Two;\n",
                "use z::Zero;\nuse y::Nine;\nuse c::Three;\n",
            ),
            "new_string ends with line 3, which stays below the replaced text:",
        ),
    ];
    for (replacement, expected) in cases {
        let project = Project::open();
        project.write_file("uses.txt", USES);

        let refused = project
            .edit("uses.txt", vec![replacement])
            .expect_err(expected);

        assert!(
            refused.starts_with("error: echoed_line uses.txt: nothing was written."),
            "{refused}"
        );
        assert!(refused.contains(expected), "{refused}");
        assert!(
            refused.contains("include it in old_string to rewrite it too"),
            "{refused}"
        );
        assert!(refused.contains("     2\tuse b::Two;"), "{refused}");
        assert_eq!(project.read_file("uses.txt"), USES);
    }
}

#[test]
fn allows_intended_neighbour_repeats() {
    let cases = [
        (
            "x();\nx();\ny();\n",
            vec![Replacement::new("x();\ny();", "x();\nz();")],
            "x();\nx();\nz();\n",
        ),
        (
            USES,
            vec![Replacement::new(
                "use b::Two;\n",
                "use b::Two;\nuse b::Two;\n",
            )],
            "use a::One;\nuse b::Two;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            USES,
            vec![
                Replacement::new("use a::One;", "use z::Zero;"),
                Replacement::new("use b::Two;\n", "use a::One;\nuse b::Two;\n"),
            ],
            "use z::Zero;\nuse a::One;\nuse b::Two;\nuse c::Three;\n",
        ),
        (
            "a {\n}\n}\n",
            vec![Replacement::new("}\n}\n", "}\n}\n}\n")],
            "a {\n}\n}\n}\n",
        ),
        (
            "a();\nb();\nc();\n",
            vec![
                Replacement::new("c();", "b();"),
                Replacement::new("a();\nb();", "a();\nx();"),
            ],
            "a();\nx();\nb();\n",
        ),
        (
            "let x = a;\nlet y = a;\n",
            vec![Replacement::new("x = a", "x = y")],
            "let x = y;\nlet y = a;\n",
        ),
    ];
    for (written, replacements, became) in cases {
        let project = Project::open();
        project.write_file("a.txt", written);

        project.edit("a.txt", replacements).expect(became);

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
            Replacement::new("    let a = 1;\n", "    let a = 10;\n}\n"),
            "rust that no longer parses",
        ),
        (
            "a.rs",
            RUST,
            Replacement::new("    let a = 1;\n}\n", "    let a = 10;\n"),
            "rust that no longer parses",
        ),
        (
            "a.rs",
            RUST,
            Replacement::new("}\n\nfn two", "\nfn two"),
            "- 1 `{` more than `}`\n",
        ),
        (
            "a.json",
            "{\n  \"a\": 1,\n  \"b\": 2\n}\n",
            Replacement::new("\"b\": 2", "\"b\": 2,"),
            "json that no longer parses",
        ),
        (
            "a.comp",
            "void main() {\n    x = f(1);\n}\n",
            Replacement::new("f(1)", "f(1"),
            "1 `(` more than `)`",
        ),
        (
            "a.comp",
            "void main() {\n    x = 1;\n}\n",
            Replacement::new("    x = 1;\n}", "    x = 1;"),
            "1 `{` more than `}`",
        ),
    ];
    for (path, written, replacement, expected) in cases {
        let project = Project::open();
        project.write_file(path, written);

        let refused = project.edit(path, vec![replacement]).expect_err(expected);

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
                Replacement::new("let a = 1;", "let a = 10;"),
                Replacement::new(
                    "fn two() {\n    let b = 2;\n}\n",
                    "fn two() {\n    let b = 2;\n}\n\nfn three() {}\n",
                ),
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
            vec![Replacement::new(
                "void f(char c) {\n",
                "void f(char c) {\n    if (c == '(') { g(\"{\"); }\n",
            )],
        )
        .expect("brackets inside literals are not counted");

    let broken = "fn one() {\n    let a = ;\n}\n\nfn two() {\n    let b = 2;\n}\n";
    project.write_file("b.rs", broken);
    project
        .edit("b.rs", vec![Replacement::new("let b = 2;", "let b = 20;")])
        .expect("it breaks nothing more");
    let refused = project
        .edit("b.rs", vec![Replacement::new("let b = 20;", "let b = ;")])
        .expect_err("it breaks a second place");
    assert!(refused.contains("broken_syntax"), "{refused}");
    project
        .edit("b.rs", vec![Replacement::new("let a = ;", "let a = 1;")])
        .expect("it mends the first place");
    assert_eq!(project.read_file("b.rs"), RUST.replace("b = 2", "b = 20"));
}
