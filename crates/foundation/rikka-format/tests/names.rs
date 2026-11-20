use std::cmp::Ordering;

use rikka_format::{natural_cmp, safe_file_name, truncate, unique_name};

const CORPUS: &[&str] = &[
    "",
    "0",
    "00",
    "1",
    "01",
    "007",
    "07",
    "7",
    "10",
    "a",
    "A",
    "a1",
    "a01",
    "a1b",
    "1a",
    "01a",
    "a-b",
    "a.b",
    "file1.txt",
    "file2.txt",
    "file10.txt",
    "File10.txt",
    "s1e2",
    "s01e2",
    "s1e10",
    "s01e10",
    "第２話",
    "第１０話",
    "ページ2",
    "ページ10",
    "Ä",
    "ä",
    "a99999999999999999999999999999999999999999999999999",
    "a999999999999999999999999999999999999999999999999999",
    "a00000000000000000000000000000000000000000000000001",
];

fn sorted<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    names
}

#[test]
fn natural_cmp_orders_numbers() {
    assert_eq!(
        sorted(["file10.txt", "file2.txt", "file1.txt"]),
        ["file1.txt", "file2.txt", "file10.txt"]
    );
    assert_eq!(
        sorted(["banana", "Apple", "apple", "Banana"]),
        ["Apple", "apple", "Banana", "banana"]
    );
    assert_eq!(sorted(["007", "7", "07"]), ["7", "07", "007"]);
    assert_eq!(
        sorted(["s1e2", "s01e10", "s1e10", "s01e2"]),
        ["s1e2", "s01e2", "s1e10", "s01e10"]
    );
    assert_eq!(
        natural_cmp(
            "a99999999999999999999999999999999",
            "a99999999999999999999999999999999999"
        ),
        Ordering::Less
    );
    assert_eq!(natural_cmp("第１０話", "第２話"), Ordering::Less);
}

#[test]
fn natural_cmp_is_total_order() {
    for one in CORPUS {
        for other in CORPUS {
            let order = natural_cmp(one, other);
            assert_eq!(
                order == Ordering::Equal,
                one == other,
                "{one:?} against {other:?}"
            );
            assert_eq!(
                order,
                natural_cmp(other, one).reverse(),
                "{one:?} against {other:?}"
            );
            if order != Ordering::Less {
                continue;
            }
            for third in CORPUS {
                if natural_cmp(other, third) == Ordering::Less {
                    assert_eq!(
                        natural_cmp(one, third),
                        Ordering::Less,
                        "{one:?} < {other:?} < {third:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn unique_name_numbers_before_extension() {
    let cases: [(&str, &[&str], &str); 6] = [
        ("写真.png", &[], "写真.png"),
        ("a.txt", &["a.txt"], "a (2).txt"),
        ("rikka.tar.gz", &["rikka.tar.gz"], "rikka (2).tar.gz"),
        (".gitignore", &[".gitignore"], ".gitignore (2)"),
        ("Makefile", &["Makefile"], "Makefile (2)"),
        ("a", &["a", "a (2)", "a (3)", "a (4)"], "a (5)"),
    ];
    for (name, taken, expected) in cases {
        assert_eq!(
            unique_name(name, |candidate| taken.contains(&candidate)),
            expected,
            "{name} beside {taken:?}"
        );
    }
}

#[test]
fn suggested_file_names_cannot_leave_their_folder() {
    let cases = [
        ("report.pdf", Some("report.pdf")),
        ("../../etc/passwd", Some("passwd")),
        ("C:\\Users\\a\\evil.exe", Some("evil.exe")),
        (" a\u{0}b\n.txt ", Some("ab.txt")),
        (".bashrc", Some(".bashrc")),
        ("", None),
        (".", None),
        ("..", None),
        ("dir/", None),
        ("a/..", None),
    ];
    for (suggested, expected) in cases {
        assert_eq!(
            safe_file_name(suggested).as_deref(),
            expected,
            "{suggested:?}"
        );
    }
}

#[test]
fn truncate_counts_chars_with_ellipsis() {
    assert_eq!(truncate("short", 8), "short");
    assert_eq!(truncate("abcdefgh", 4), "abcd…");
    assert_eq!(truncate("ありがとうございます", 3), "ありが…");
}
