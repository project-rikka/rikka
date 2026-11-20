use rikka_file_edit::Line;

use crate::support::{Project, delete_op, insert_op, replace_op};

#[test]
fn edit_writes_file_and_reports_summary() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\nthree\nfour\nfive\n");

    let summary = project
        .edit("a.txt", vec![replace_op(2, 3, "TWO\nTHREE\nEXTRA")])
        .expect("it is applied");

    assert_eq!(
        project.read_file("a.txt"),
        "one\nTWO\nTHREE\nEXTRA\nfour\nfive\n"
    );
    assert_eq!(summary, "ok a.txt (6 lines, +1)");
}

#[test]
fn edit_combines_insert_delete_replace() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\nthree\nfour\nfive\n");

    let summary = project
        .edit(
            "a.txt",
            vec![
                insert_op(1, "zero\n"),
                delete_op(3, 3),
                replace_op(5, 5, "FIVE"),
            ],
        )
        .expect("it is applied");

    assert_eq!(project.read_file("a.txt"), "zero\none\ntwo\nfour\nFIVE\n");
    assert_eq!(summary, "ok a.txt (5 lines, +0)");
}

#[test]
fn rejected_edit_writes_nothing() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\nthree\n");

    let cases = [
        (
            vec![replace_op(1, 2, "x\n"), replace_op(2, 3, "y\n")],
            "overlap",
        ),
        (
            vec![replace_op(1, 1, "x\n"), replace_op(40, 41, "y\n")],
            "out_of_range",
        ),
        (vec![replace_op(3, 2, "x\n")], "backwards_range"),
    ];
    for (ops, reason) in cases {
        let refused = project.edit("a.txt", ops).expect_err(reason);
        assert!(refused.contains(reason), "{refused}");
        assert_eq!(project.read_file("a.txt"), "one\ntwo\nthree\n");
    }

    let missing = project
        .edit("nowhere.rs", vec![replace_op(1, 1, "x\n")])
        .expect_err("it is not there");
    assert!(missing.contains("not_found"), "{missing}");
}

#[test]
fn replace_and_insert_split_lines() {
    let three = "one\ntwo\nthree\n";
    let cases = [
        (three, replace_op(2, 2, "TWO"), "one\nTWO\nthree\n"),
        (three, replace_op(2, 2, "TWO\n"), "one\nTWO\n\nthree\n"),
        (three, replace_op(2, 2, "\nTWO"), "one\n\nTWO\nthree\n"),
        (three, replace_op(2, 2, ""), "one\nthree\n"),
        (three, insert_op(2, "new"), "one\nnew\ntwo\nthree\n"),
        (three, insert_op(2, "new\n"), "one\nnew\ntwo\nthree\n"),
        (three, insert_op(2, "new\n\n"), "one\nnew\n\ntwo\nthree\n"),
        (three, insert_op(4, "end"), "one\ntwo\nthree\nend\n"),
        ("one\ntwo", replace_op(2, 2, "TWO\n"), "one\nTWO\n"),
    ];
    for (write_file, op, became) in cases {
        let project = Project::open();
        project.write_file("a.txt", write_file);

        let apply = project
            .apply("a.txt", vec![op.clone()])
            .expect("it is applied");

        let read_file = project.read_file("a.txt");
        assert_eq!(read_file, became, "{op:?}");
        let lines: Vec<&str> = read_file.lines().collect();
        for row in apply.excerpt.to_string().lines() {
            let (number, text) = row.trim_start().split_once('\t').expect("a numbered line");
            let number: usize = number.parse().expect("a line number");
            assert_eq!(lines.get(number - 1), Some(&text), "{op:?} shows {row}");
        }
    }
}

#[test]
fn edit_excerpt_shows_context_lines() {
    let project = Project::open();
    let numbers: String = (1..=30).map(|n| format!("{n}\n")).collect();
    project.write_file("a.txt", &numbers);

    let apply = project
        .apply(
            "a.txt",
            vec![
                insert_op(4, "a\nb\n"),
                replace_op(12, 12, "c\nd\ne\nf\ng\nh\ni"),
                delete_op(26, 26),
            ],
        )
        .expect("it is applied");

    assert_eq!(
        apply.excerpt.to_string(),
        [
            "     2\t2",
            "     3\t3",
            "     4\ta",
            "     5\tb",
            "     6\t4",
            "     7\t5",
            "     …",
            "    12\t10",
            "    13\t11",
            "    14\tc",
            "    15\td",
            "     …",
            "    19\th",
            "    20\ti",
            "    21\t13",
            "    22\t14",
            "     …",
            "    32\t24",
            "    33\t25",
            "    34\t27",
            "    35\t28",
        ]
        .join("\n")
    );
}

#[test]
fn edit_keeps_line_endings() {
    let project = Project::open();
    project.write_file("windows.txt", "one\r\ntwo\r\n");
    project.write_file("unix.txt", "one\ntwo\n");

    project
        .edit("windows.txt", vec![replace_op(2, 2, "TWO\r\nAND")])
        .expect("it is applied");
    project
        .edit("unix.txt", vec![replace_op(2, 2, "TWO\r\nAND")])
        .expect("it is applied");

    assert_eq!(project.read_file("windows.txt"), "one\r\nTWO\r\nAND\r\n");
    assert_eq!(project.read_file("unix.txt"), "one\nTWO\nAND\n");
}

#[test]
fn refilled_file_keeps_trailing_newline() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\n");

    project
        .edit("a.txt", vec![delete_op(1, 2)])
        .expect("it is applied");
    assert_eq!(project.read_file("a.txt"), "");

    project
        .edit("a.txt", vec![insert_op(1, "new\n")])
        .expect("it is applied");
    assert_eq!(project.read_file("a.txt"), "new\n");
}

#[test]
fn change_reports_hunks() {
    let project = Project::open();
    let numbers: String = (1..=16).map(|n| format!("{n}\n")).collect();
    project.write_file("a.txt", &numbers);

    let apply = project
        .apply(
            "a.txt",
            vec![replace_op(2, 2, "TWO"), replace_op(16, 16, "SIXTEEN")],
        )
        .expect("it is applied");

    assert_eq!(apply.change.path, "a.txt");
    assert_eq!((apply.change.added, apply.change.removed), (2, 2));
    assert_eq!(
        apply.change.hunks.len(),
        2,
        "changes far apart are two hunks"
    );
    assert!(
        apply.change.hunks[0]
            .lines
            .contains(&Line::Removed("2".to_owned()))
    );
    assert!(
        apply.change.hunks[0]
            .lines
            .contains(&Line::Added("TWO".to_owned()))
    );
}
