use rikka_nix_log::{Count, Decoder, Realisation, Tally};

const BUILD: &str = include_str!("fixtures/build.jsonl");

const DERIVATION: &str = "/nix/store/jyd2wmqr563640s1kgffsn4vl2znmcpa-rikka-probe-fixture-7181.drv";

fn decoded(decoder: &mut Decoder, lines: &[&str]) -> Vec<String> {
    lines.iter().flat_map(|line| decoder.decode(line)).collect()
}

#[test]
fn recorded_build_reads_like_nix() {
    let mut decoder = Decoder::new();
    let lines: Vec<&str> = BUILD.lines().collect();
    let building = lines
        .iter()
        .position(|line| line.contains(r#""fields":[0,1,1,0]"#))
        .expect("the fixture builds");

    let shown = decoded(&mut decoder, &lines[..=building]);
    assert_eq!(decoder.realisation(), Realisation::Running);
    assert_eq!(
        decoder.tally().builds,
        Count {
            done: 0,
            expected: 1,
            running: 1,
            failed: 0,
        }
    );

    let rest = decoded(&mut decoder, &lines[building + 1..]);
    assert_eq!(
        [shown, rest].concat(),
        [
            "this derivation will be built:".to_owned(),
            format!("  {DERIVATION}"),
            format!("building '{DERIVATION}'"),
            "rikka-probe-fixture-7181> hello".to_owned(),
            "rikka-probe-fixture-7181> world".to_owned(),
        ]
    );
    assert_eq!(decoder.realisation(), Realisation::Finished);
    assert_eq!(decoder.tally().builds.done, 1);
    assert_eq!(decoder.error(), None);
}

#[test]
fn plain_lines_lose_terminal_controls() {
    let cases = [
        ("\u{1b}[31;1merror:\u{1b}[0m bad", "error: bad"),
        ("50%\r100%", "100%"),
        ("line\r", "line"),
        ("tab\there", "tab\there"),
        (
            "\u{1b}]8;;https://nixos.org\u{7}link\u{1b}]8;;\u{1b}\\",
            "link",
        ),
        ("@nix {broken", "@nix {broken"),
        ("", ""),
    ];
    for (line, shown) in cases {
        assert_eq!(Decoder::new().decode(line), [shown], "{line:?}");
    }
}

#[test]
fn messages_show_up_to_info_level() {
    let mut decoder = Decoder::new();
    let cases = [
        (
            r#"@nix {"action":"msg","level":1,"msg":"warning: dirty tree"}"#,
            vec!["warning: dirty tree"],
        ),
        (
            r#"@nix {"action":"msg","level":4,"msg":"evaluating file"}"#,
            vec![],
        ),
        (
            r#"@nix {"action":"start","id":1,"level":4,"parent":0,"text":"downloading 'x'","type":101}"#,
            vec![],
        ),
        (
            r#"@nix {"action":"start","id":2,"level":3,"parent":0,"text":"","type":104}"#,
            vec![],
        ),
        (r#"@nix {"action":"setPhase","id":3}"#, vec![]),
    ];
    for (line, shown) in cases {
        assert_eq!(decoder.decode(line), shown, "{line}");
    }
}

#[test]
fn error_message_is_kept_for_the_reason() {
    let mut decoder = Decoder::new();
    let shown = decoder.decode(
        r#"@nix {"action":"msg","level":0,"msg":"\u001b[31;1merror:\u001b[0m\n       attribute 'nope' missing\n"}"#,
    );
    assert_eq!(shown, ["error:", "       attribute 'nope' missing"]);
    assert_eq!(
        decoder.error(),
        Some("error:\n       attribute 'nope' missing")
    );

    let shown = decoder.decode(
        r#"@nix {"action":"msg","column":null,"file":null,"level":0,"line":null,"msg":"\u001b[31;1merror:\u001b[0m\n       … while calling the 'throw' builtin\n\n       \u001b[31;1merror:\u001b[0m boom here","raw_msg":"boom here"}"#,
    );
    assert_eq!(
        shown,
        [
            "error:",
            "       … while calling the 'throw' builtin",
            "",
            "       error: boom here",
        ]
    );
    assert_eq!(decoder.error(), Some("boom here"));
}

#[test]
fn expected_work_follows_nix_progress_rules() {
    let mut decoder = Decoder::new();
    decoded(
        &mut decoder,
        &[
            r#"@nix {"action":"start","id":1,"level":0,"parent":0,"text":"","type":102}"#,
            r#"@nix {"action":"start","id":2,"level":0,"parent":1,"text":"","type":104}"#,
            r#"@nix {"action":"start","id":3,"level":0,"parent":1,"text":"","type":103}"#,
            r#"@nix {"action":"result","fields":[101,1000],"id":1,"type":106}"#,
            r#"@nix {"action":"result","fields":[100,5000],"id":1,"type":106}"#,
            r#"@nix {"action":"result","fields":[0,2,1,0],"id":2,"type":105}"#,
            r#"@nix {"action":"result","fields":[1,3,1,0],"id":3,"type":105}"#,
            r#"@nix {"action":"start","id":4,"level":3,"parent":3,"text":"","type":100}"#,
            r#"@nix {"action":"result","fields":[2500,4000,0,0],"id":4,"type":105}"#,
        ],
    );
    assert_eq!(
        decoder.tally(),
        Tally {
            builds: Count {
                done: 0,
                expected: 2,
                running: 1,
                failed: 0,
            },
            paths: Count {
                done: 1,
                expected: 3,
                running: 1,
                failed: 0,
            },
            copied: Count {
                done: 2500,
                expected: 5000,
                running: 0,
                failed: 0,
            },
            downloaded: Count {
                done: 0,
                expected: 1000,
                running: 0,
                failed: 0,
            },
        }
    );
    assert_eq!(decoder.tally().fraction(), Some(0.2));

    decoded(
        &mut decoder,
        &[
            r#"@nix {"action":"stop","id":4}"#,
            r#"@nix {"action":"result","fields":[2,2,0,0],"id":2,"type":105}"#,
            r#"@nix {"action":"result","fields":[3,3,0,0],"id":3,"type":105}"#,
            r#"@nix {"action":"stop","id":2}"#,
            r#"@nix {"action":"stop","id":3}"#,
            r#"@nix {"action":"stop","id":1}"#,
        ],
    );
    let tally = decoder.tally();
    assert_eq!((tally.builds.done, tally.builds.expected), (2, 2));
    assert_eq!((tally.paths.done, tally.paths.expected), (3, 3));
    assert_eq!((tally.copied.done, tally.copied.expected), (2500, 2500));
    assert_eq!(tally.downloaded, Count::default());
    assert_eq!(tally.fraction(), Some(1.0));
    assert_eq!(decoder.realisation(), Realisation::Finished);
}

#[test]
fn output_tail_keeps_only_command_output() {
    let mut decoder = Decoder::new();
    for line in [
        "building the system configuration...",
        r#"@nix {"action":"msg","level":1,"msg":"evaluation warning: portals"}"#,
        "one",
        "two",
        "three",
        "Failed to start transient service unit: Access denied",
    ] {
        decoder.decode(line);
    }
    assert_eq!(
        decoder.output_tail().collect::<Vec<_>>(),
        [
            "one",
            "two",
            "three",
            "Failed to start transient service unit: Access denied"
        ]
    );
}
