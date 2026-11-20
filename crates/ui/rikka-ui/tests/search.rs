use rikka_ui::search::{Hits, Options, Query, Tally};

fn spans(pattern: &str, options: Options, text: &str) -> Vec<(usize, usize)> {
    Query::new(pattern, options)
        .find_all(text)
        .into_iter()
        .map(|found| (found.start, found.end))
        .collect()
}

#[test]
fn query_options_control_matching() {
    let plain = Options::default();
    let regex = Options {
        regex: true,
        ..plain
    };
    let case = Options {
        case_sensitive: true,
        ..plain
    };
    let word = Options {
        whole_word: true,
        ..plain
    };
    let word_regex = Options {
        whole_word: true,
        ..regex
    };

    for (pattern, options, text, found) in [
        ("a.c", plain, "a.c abc", &[(0, 3)][..]),
        ("a.c", regex, "a.c abc", &[(0, 3), (4, 7)]),
        ("rikka", plain, "Rikka rikka", &[(0, 5), (6, 11)]),
        ("rikka", case, "Rikka rikka", &[(6, 11)]),
        ("cat", word, "cat cats", &[(0, 3)]),
        ("cat|dog", word_regex, "dogs dog", &[(5, 8)]),
        ("x*", regex, "abc", &[]),
    ] {
        assert_eq!(
            spans(pattern, options, text),
            found,
            "{pattern:?} {options:?}"
        );
    }
}

#[test]
fn invalid_pattern_reports_error() {
    let regex = Options {
        regex: true,
        ..Options::default()
    };
    let query = Query::new("(unclosed", regex);

    assert!(query.is_broken());
    assert!(!query.is_readable());
    assert!(query.find_all("unclosed").is_empty());
}

#[test]
fn match_navigation_wraps_from_viewport() {
    let mut found = Hits::default();
    found.refresh(&Query::new("one", Options::default()), "one one one");

    assert_eq!(found.step(0..0, true), Some(0..3));
    assert_eq!(found.step(0..3, true), Some(4..7));
    assert_eq!(found.tally(), Tally::new(3, Some(1)));
    assert_eq!(found.step(8..11, true), Some(0..3));
    assert_eq!(found.step(0..0, false), Some(8..11));
    assert_eq!(found.step(4..7, false), Some(0..3));
    assert_eq!(found.aim(4), Some(4..7), "current match not selected");

    found.refresh(&Query::new("one", Options::default()), "nothing here");
    assert_eq!(found.step(0..0, true), None);
    assert_eq!(found.tally(), Tally::new(0, None));
}
