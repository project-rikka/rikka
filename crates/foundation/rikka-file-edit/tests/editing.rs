use rikka_file_edit::{Edit, Line};

use crate::support::{Project, anchor};

#[test]
fn edit_writes_file_and_reports_anchored_excerpt() {
    let project = Project::open();
    project.write_file("a.txt", "one\ntwo\nthree\nfour\nfive\n");

    let outcome = project
        .apply(
            "a.txt",
            vec![project.lines("a.txt", 2, 3, "TWO\nTHREE\nEXTRA")],
        )
        .expect("it is applied");

    assert_eq!(
        project.read_file("a.txt"),
        "one\nTWO\nTHREE\nEXTRA\nfour\nfive\n"
    );
    assert_eq!(outcome.summary, "ok a.txt (6 lines, +1)");
    assert_eq!(
        outcome.excerpt.to_string(),
        project.listing("a.txt").trim_end()
    );
}

#[test]
fn edits_anchor_in_file_before_call() {
    let five = "1\n2\n3\n4\n5\n";
    type Edits = fn(&Project) -> Vec<Edit>;
    let cases: [(&str, Edits, &str); 9] = [
        (
            "one line",
            |p| vec![p.lines("a", 2, 2, "two")],
            "1\ntwo\n3\n4\n5\n",
        ),
        ("deletion", |p| vec![p.lines("a", 2, 4, "")], "1\n5\n"),
        (
            "after",
            |p| vec![p.after("a", 2, "x\ny\n")],
            "1\n2\nx\ny\n3\n4\n5\n",
        ),
        (
            "before",
            |p| vec![p.before("a", 1, "0")],
            "0\n1\n2\n3\n4\n5\n",
        ),
        ("end", |p| vec![p.after("a", 5, "6")], "1\n2\n3\n4\n5\n6\n"),
        (
            "blank line",
            |p| vec![p.after("a", 1, "\n")],
            "1\n\n2\n3\n4\n5\n",
        ),
        (
            "original numbering",
            |p| {
                vec![
                    p.lines("a", 5, 5, "five"),
                    p.after("a", 1, "a\nb\nc"),
                    p.lines("a", 3, 4, ""),
                ]
            },
            "1\na\nb\nc\n2\nfive\n",
        ),
        (
            "around one range",
            |p| {
                vec![
                    p.after("a", 3, "after"),
                    p.lines("a", 2, 3, "two\nthree"),
                    p.before("a", 2, "before"),
                ]
            },
            "1\nbefore\ntwo\nthree\nafter\n4\n5\n",
        ),
        (
            "same gap in call order",
            |p| vec![p.after("a", 1, "x"), p.before("a", 2, "y")],
            "1\nx\ny\n2\n3\n4\n5\n",
        ),
    ];
    for (case, edits, became) in cases {
        let project = Project::open();
        project.write_file("a", five);

        project.apply("a", edits(&project)).expect(case);

        assert_eq!(project.read_file("a"), became, "{case}");
    }
}

#[test]
fn rejected_edit_writes_nothing() {
    type Edits = fn(&Project) -> Vec<Edit>;
    let cases: [(Edits, &str); 7] = [
        (|_| vec![lines("2", "x")], "`2` has no tag."),
        (
            |_| vec![lines("9#ab", "x")],
            "`9#ab` is past the end; the file has 3 lines.",
        ),
        (
            |p| {
                vec![Edit::Replace {
                    first: p.at("a.txt", 3),
                    last: p.at("a.txt", 1),
                    text: "x".to_owned(),
                }]
            },
            "error: reversed_range",
        ),
        (
            |p| vec![p.lines("a.txt", 1, 2, "x"), p.lines("a.txt", 2, 3, "y")],
            "Edits 1 and 2 both reach line 2.",
        ),
        (
            |p| vec![p.lines("a.txt", 1, 3, "x"), p.after("a.txt", 2, "y")],
            "Edits 1 and 2 both reach line 2.",
        ),
        (
            |p| vec![p.lines("a.txt", 2, 2, "     2#ab\ttwo")],
            "error: listing_prefix",
        ),
        (|p| vec![p.lines("a.txt", 2, 2, "two")], "error: unchanged"),
    ];
    for (edits, reason) in cases {
        let project = Project::open();
        project.write_file("a.txt", "one\ntwo\none\n");

        let refused = project.edit("a.txt", edits(&project)).expect_err(reason);

        assert!(refused.contains(reason), "{reason} in {refused}");
        assert!(refused.contains("nothing was written"), "{refused}");
        assert_eq!(project.read_file("a.txt"), "one\ntwo\none\n");
    }

    let project = Project::open();
    project.write_file("a.txt", "one\n");
    let empty = project.edit("a.txt", Vec::new()).expect_err("no edits");
    assert!(empty.contains("error: empty"), "{empty}");
    let missing = project
        .edit("nowhere.rs", vec![lines("1#ab", "x")])
        .expect_err("it is not there");
    assert!(missing.contains("not_found"), "{missing}");
}

#[test]
fn stale_anchor_shows_current_anchors() {
    let project = Project::open();
    let numbers: String = (1..=20).map(|n| format!("{n}\n")).collect();
    project.write_file("a.txt", &numbers);
    let ten = project.at("a.txt", 10);

    project
        .edit("a.txt", vec![project.after("a.txt", 3, "x\ny")])
        .expect("it shifts the lines below");
    let refused = project
        .edit(
            "a.txt",
            vec![Edit::Replace {
                first: ten,
                last: ten,
                text: "TEN".to_owned(),
            }],
        )
        .expect_err("line 10 moved");

    assert!(
        refused.starts_with("error: stale_anchor a.txt: nothing was written."),
        "{refused}"
    );
    assert!(
        refused.contains(&format!("`{ten}` is not line 10 as it reads now.")),
        "{refused}"
    );
    for line in 9..=12 {
        let current = project.at("a.txt", line).to_string();
        assert!(
            refused.contains(&format!("{line:>6}#{}\t{}", tag(&current), line - 2)),
            "line {line} in {refused}"
        );
    }
    let moved = project.at("a.txt", 12);
    assert!(
        refused.contains("Take anchors from these lines"),
        "{refused}"
    );

    project
        .edit(
            "a.txt",
            vec![Edit::Replace {
                first: moved,
                last: moved,
                text: "TEN".to_owned(),
            }],
        )
        .expect("the shown anchor applies");
    assert_eq!(project.read_file("a.txt").lines().nth(11), Some("TEN"));
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
                project.lines("a.txt", 12, 12, "c\nd\ne\nf\ng\nh\ni"),
                project.lines("a.txt", 26, 26, ""),
            ],
        )
        .expect("it is applied");

    let shown: Vec<String> = outcome
        .excerpt
        .to_string()
        .lines()
        .map(|row| match row.split_once('\t') {
            Some((anchor, text)) => format!(
                "{}\t{text}",
                anchor.trim_start().split('#').next().unwrap_or_default()
            ),
            None => row.trim().to_owned(),
        })
        .collect();
    assert_eq!(
        shown,
        [
            "10\t10", "11\t11", "12\tc", "13\td", "…", "17\th", "18\ti", "19\t13", "20\t14", "…",
            "30\t24", "31\t25", "32\t27", "33\t28",
        ]
    );
}

#[test]
fn edit_keeps_line_endings() {
    let project = Project::open();
    project.write_file("windows.txt", "one\r\ntwo\r\n");
    project.write_file("unix.txt", "one\ntwo\n");
    project.write_file("mixed.txt", "one\r\ntwo\nthree\r\nfour\n");
    project.write_file("open.txt", "one\ntwo");
    project.write_file("inserted.txt", "one\r\ntwo\nthree\r\n");

    project
        .edit(
            "windows.txt",
            vec![
                project.lines("windows.txt", 2, 2, "TWO\r\nAND\r\n"),
                project.lines("windows.txt", 1, 1, "ONE"),
            ],
        )
        .expect("it is applied");
    project
        .edit(
            "unix.txt",
            vec![project.lines("unix.txt", 2, 2, "TWO\r\nAND\n")],
        )
        .expect("it is applied");
    project
        .edit(
            "mixed.txt",
            vec![project.lines("mixed.txt", 1, 3, "ONE\nTWO\nTHREE")],
        )
        .expect("it is applied");
    project
        .edit("open.txt", vec![project.after("open.txt", 2, "three")])
        .expect("it is applied");
    project
        .edit(
            "inserted.txt",
            vec![
                project.after("inserted.txt", 1, "after"),
                project.before("inserted.txt", 2, "before"),
            ],
        )
        .expect("it is applied");

    assert_eq!(project.read_file("windows.txt"), "ONE\r\nTWO\r\nAND\r\n");
    assert_eq!(project.read_file("unix.txt"), "one\nTWO\nAND\n");
    assert_eq!(
        project.read_file("mixed.txt"),
        "ONE\r\nTWO\nTHREE\r\nfour\n"
    );
    assert_eq!(project.read_file("open.txt"), "one\ntwo\nthree");
    assert_eq!(
        project.read_file("inserted.txt"),
        "one\r\nafter\r\nbefore\ntwo\nthree\r\n"
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
                project.lines("a.txt", 2, 2, "TWO"),
                project.lines("a.txt", 16, 16, "SIXTEEN"),
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

fn lines(at: &str, text: &str) -> Edit {
    Edit::Replace {
        first: anchor(at),
        last: anchor(at),
        text: text.to_owned(),
    }
}

fn tag(anchor: &str) -> &str {
    anchor.split_once('#').map_or("", |(_, tag)| tag)
}
