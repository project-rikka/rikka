use rikka_file_edit::{Mark, Marks};

type Case = (&'static str, &'static str, &'static [(usize, Mark)]);

#[test]
fn marks_each_changed_run() {
    let cases: &[Case] = &[
        (
            "one\ntwo\nthree\n",
            "one\nTWO\nhalf\nthree\n",
            &[(2, Mark::Changed), (3, Mark::Changed)],
        ),
        (
            "one\ntwo\n",
            "one\nnew\nnewer\ntwo\n",
            &[(2, Mark::Added), (3, Mark::Added)],
        ),
        (
            "one\ntwo\nthree\nfour\n",
            "one\nfour\n",
            &[(2, Mark::Removed)],
        ),
        ("one\ntwo\nthree\n", "two\nthree\n", &[(1, Mark::Removed)]),
        ("one\ntwo\nthree\n", "one\n", &[(2, Mark::Removed)]),
        ("", "one\ntwo\n", &[(1, Mark::Added), (2, Mark::Added)]),
        ("one\ntwo\n", "one\ntwo\n", &[]),
        ("one\r\ntwo\r\n", "one\ntwo\n", &[]),
        ("one\r\ntwo\r\n", "one\nTWO\n", &[(2, Mark::Changed)]),
        (
            "a\nb\nc\nd\ne\nf\ng\nh\n",
            "a\nB\nc\nd\ne\nf\ng\nh\nnew\n",
            &[(2, Mark::Changed), (9, Mark::Added)],
        ),
    ];
    for &(before, after, marks) in cases {
        assert_eq!(
            Marks::diff(before, after).lines(),
            marks,
            "{before:?} → {after:?}"
        );
    }
}
