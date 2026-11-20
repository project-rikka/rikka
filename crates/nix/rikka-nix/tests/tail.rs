use rikka_nix::{Gap, Tail};

fn lines(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

#[test]
fn tail_keeps_each_line_once() {
    struct Case {
        name: &'static str,
        known: &'static [&'static str],
        from: u64,
        arriving: &'static [&'static str],
        fresh: &'static [&'static str],
    }
    let cases = [
        Case {
            name: "first page",
            known: &[],
            from: 0,
            arriving: &["one", "two"],
            fresh: &["one", "two"],
        },
        Case {
            name: "next batch",
            known: &["one"],
            from: 4,
            arriving: &["two", "three"],
            fresh: &["two", "three"],
        },
        Case {
            name: "overlapping batch",
            known: &["one", "two"],
            from: 0,
            arriving: &["one", "two", "three"],
            fresh: &["three"],
        },
        Case {
            name: "repeated batch",
            known: &["one", "two"],
            from: 4,
            arriving: &["two"],
            fresh: &[],
        },
        Case {
            name: "empty line",
            known: &["", "x"],
            from: 1,
            arriving: &["x", ""],
            fresh: &[""],
        },
    ];
    for case in cases {
        let mut tail = Tail::new();
        let known = lines(case.known);
        assert_eq!(
            tail.accept(0, &known),
            Ok(known.as_slice()),
            "{}",
            case.name
        );
        let arriving = lines(case.arriving);
        assert_eq!(
            tail.accept(case.from, &arriving),
            Ok(lines(case.fresh).as_slice()),
            "{}",
            case.name
        );
        let everything: u64 = [case.known, case.fresh]
            .concat()
            .iter()
            .map(|line| line.len() as u64 + 1)
            .sum();
        assert_eq!(tail.next(), everything, "{}", case.name);
    }
}

#[test]
fn tail_refuses_a_gap() {
    let mut tail = Tail::new();
    tail.accept(0, &lines(&["one"])).expect("it starts at zero");
    assert_eq!(
        tail.accept(9, &lines(&["three"])),
        Err(Gap {
            expected: 4,
            found: 9,
        })
    );
    assert_eq!(tail.next(), 4);
}
