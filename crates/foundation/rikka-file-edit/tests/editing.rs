use rikka_file_edit::{Line, Replacement};

use crate::support::Project;

#[test]
fn edit_writes_file_and_reports_summary() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\nthree\nfour\nfive\n");

    let outcome = project
        .apply(
            "a.txt",
            vec![Replacement::new("two\nthree", "TWO\nTHREE\nEXTRA")],
        )
        .expect("it is applied");

    assert_eq!(
        project.read_file("a.txt"),
        "one\nTWO\nTHREE\nEXTRA\nfour\nfive\n"
    );
    assert_eq!(outcome.summary, "ok a.txt (6 lines, +1)");
    assert_eq!(
        outcome.excerpt.to_string(),
        "     1\tone\n     2\tTWO\n     3\tTHREE\n     4\tEXTRA\n     5\tfour\n     6\tfive"
    );
}

#[test]
fn replacements_apply_in_order() {
    let three = "one\ntwo\nthree\n";
    let cases = [
        (
            three,
            vec![Replacement::new("two", "TWO")],
            "one\nTWO\nthree\n",
        ),
        (three, vec![Replacement::new("two\n", "")], "one\nthree\n"),
        (
            three,
            vec![Replacement::new("three\n", "three\nfour\n")],
            "one\ntwo\nthree\nfour\n",
        ),
        (
            "fn a() {}\nfn b() {}\n",
            vec![
                Replacement::new("fn a", "fn first"),
                Replacement::new("fn first() {}", "fn first() { b(); }"),
            ],
            "fn first() { b(); }\nfn b() {}\n",
        ),
        (
            "x = 1;\ny = x;\n",
            vec![Replacement::all("x", "w")],
            "w = 1;\ny = w;\n",
        ),
        (
            "one\ntwo",
            vec![Replacement::new("two", "TWO\n")],
            "one\nTWO\n",
        ),
    ];
    for (written, replacements, became) in cases {
        let project = Project::open();
        project.write_file("a.txt", written);

        let outcome = project
            .apply("a.txt", replacements.clone())
            .expect("it is applied");

        let read = project.read_file("a.txt");
        assert_eq!(read, became, "{replacements:?}");
        let lines: Vec<&str> = read.lines().collect();
        for row in outcome.excerpt.to_string().lines() {
            let (number, text) = row.trim_start().split_once('\t').expect("a numbered line");
            let number: usize = number.parse().expect("a line number");
            assert_eq!(
                lines.get(number - 1),
                Some(&text),
                "{replacements:?} shows {row}"
            );
        }
    }
}

#[test]
fn rejected_edit_writes_nothing() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\none\n");

    let cases = [
        (vec![Replacement::new("three", "x")], "no_match"),
        (vec![Replacement::new("one", "x")], "ambiguous"),
        (vec![Replacement::new("", "x")], "empty_old_string"),
        (vec![Replacement::new("two", "two")], "unchanged"),
        (
            vec![
                Replacement::new("two", "TWO"),
                Replacement::new("three", "x"),
            ],
            "no_match",
        ),
        (Vec::new(), "error: empty"),
    ];
    for (replacements, reason) in cases {
        let refused = project.edit("a.txt", replacements).expect_err(reason);
        assert!(refused.contains(reason), "{refused}");
        assert_eq!(project.read_file("a.txt"), "one\ntwo\none\n");
    }

    let missing = project
        .edit("nowhere.rs", vec![Replacement::new("a", "b")])
        .expect_err("it is not there");
    assert!(missing.contains("not_found"), "{missing}");
}

#[test]
fn missing_text_points_at_closest_lines() {
    let source = "fn main() {\n    let total = add(1, 2);\n    println!(\"{total}\");\n}\n";
    let cases = [
        (
            "let total =  add(1, 2);",
            [
                "The closest text, at line 2, differs only in whitespace:",
                "     2\t    let total = add(1, 2);",
            ],
        ),
        (
            "let totl = add(1, 2);",
            [
                "The closest text is at line 2:",
                "     2\t    let total = add(1, 2);",
            ],
        ),
        (
            "fn main() {\n  let total = add(1, 2);\n  println!(\"{total}\");\n}",
            [
                "The closest text, at lines 1-4, differs only in whitespace:",
                "     3\t    println!(\"{total}\");",
            ],
        ),
        (
            "     2\t    let total = add(1, 2);",
            ["line numbers shown before each line", "leave them out"],
        ),
        (
            "2:    let total = add(1, 2);\n3-    println!(\"{total}\");",
            ["line numbers shown before each line", "leave them out"],
        ),
        (
            "struct Unrelated;",
            ["Nothing in the file resembles it.", "does not appear"],
        ),
    ];
    for (old, expected) in cases {
        let project = Project::open();
        project.write_file("main.rs", source);

        let refused = project
            .edit("main.rs", vec![Replacement::new(old, "x")])
            .expect_err(old);

        assert!(
            refused.starts_with("error: no_match main.rs: nothing was written."),
            "{refused}"
        );
        for part in expected {
            assert!(refused.contains(part), "{part} in {refused}");
        }
    }
}

#[test]
fn repeated_text_lists_its_lines() {
    let project = Project::open();
    project.write_file("a.rs", "a();\nb();\na();\nc();\na();\n");

    let alone = project
        .edit("a.rs", vec![Replacement::new("a();", "x")])
        .expect_err("it is ambiguous");
    assert!(
        alone.contains("old_string appears 3 times, at lines 1, 3 and 5."),
        "{alone}"
    );

    let second = project
        .edit(
            "a.rs",
            vec![
                Replacement::new("b();", "B();"),
                Replacement::new("a();", "x"),
            ],
        )
        .expect_err("it is ambiguous");
    assert!(
        second.contains("Edit 2's old_string appears 3 times after edit 1, at lines 1, 3 and 5."),
        "{second}"
    );

    project.write_file("braces.rs", "}\n}\n}\n");
    let overlapping = project
        .edit("braces.rs", vec![Replacement::new("}\n}\n", "}\n")])
        .expect_err("overlapping occurrences are ambiguous");
    assert!(
        overlapping.contains("old_string appears 2 times, at lines 1 and 2."),
        "{overlapping}"
    );
    assert_eq!(project.read_file("braces.rs"), "}\n}\n}\n");
}

#[test]
fn edit_excerpt_shows_context_lines() {
    let project = Project::open();
    let numbers: String = (1..=30).map(|n| format!("{n}\n")).collect();
    project.write_file("a.txt", &numbers);

    let outcome = project
        .apply(
            "a.txt",
            vec![
                Replacement::new("\n12\n", "\nc\nd\ne\nf\ng\nh\ni\n"),
                Replacement::new("\n26\n", "\n"),
            ],
        )
        .expect("it is applied");

    assert_eq!(
        outcome.excerpt.to_string(),
        [
            "    10\t10",
            "    11\t11",
            "    12\tc",
            "    13\td",
            "     …",
            "    17\th",
            "    18\ti",
            "    19\t13",
            "    20\t14",
            "     …",
            "    30\t24",
            "    31\t25",
            "    32\t27",
            "    33\t28",
        ]
        .join("\n")
    );
}

#[test]
fn edit_keeps_line_endings() {
    let project = Project::open();
    project.write_file("windows.txt", "one\r\ntwo\r\n");
    project.write_file("unix.txt", "one\ntwo\n");
    project.write_file("mixed.txt", "one\r\ntwo\nthree\r\nfour\n");

    project
        .edit(
            "windows.txt",
            vec![
                Replacement::new("two\r\n", "TWO\r\nAND\r\n"),
                Replacement::new("one\n", "ONE\n"),
            ],
        )
        .expect("it is applied");
    project
        .edit("unix.txt", vec![Replacement::new("two\n", "TWO\r\nAND\n")])
        .expect("it is applied");
    project
        .edit(
            "mixed.txt",
            vec![Replacement::new("one\ntwo\nthree", "ONE\nTWO\nTHREE")],
        )
        .expect("it is applied");

    assert_eq!(project.read_file("windows.txt"), "ONE\r\nTWO\r\nAND\r\n");
    assert_eq!(project.read_file("unix.txt"), "one\nTWO\nAND\n");
    assert_eq!(
        project.read_file("mixed.txt"),
        "ONE\r\nTWO\nTHREE\r\nfour\n"
    );
}

#[test]
fn change_reports_hunks() {
    let project = Project::open();
    let numbers: String = (1..=16).map(|n| format!("{n}\n")).collect();
    project.write_file("a.txt", &numbers);

    let outcome = project
        .apply(
            "a.txt",
            vec![
                Replacement::new("\n2\n", "\nTWO\n"),
                Replacement::new("\n16\n", "\nSIXTEEN\n"),
            ],
        )
        .expect("it is applied");

    assert_eq!(outcome.change.path, "a.txt");
    assert_eq!((outcome.change.added, outcome.change.removed), (2, 2));
    assert_eq!(
        outcome.change.hunks.len(),
        2,
        "changes far apart are two hunks"
    );
    assert!(
        outcome.change.hunks[0]
            .lines
            .contains(&Line::Removed("2".to_owned()))
    );
    assert!(
        outcome.change.hunks[0]
            .lines
            .contains(&Line::Added("TWO".to_owned()))
    );
}
