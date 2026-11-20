use std::time::Instant;

use rikka_terminal::{Options, Report, Session};
use rikka_types::{Key, Modifiers, Point, Size};

use crate::support::{Running, TIMEOUT, alone};

const SIZES: &str = r#"n=0; i=0; trap 'n=$((n+1))' WINCH; printf 'ready\n'; while :; do read line && i=$((i+1)) && printf 'reply %s winch %s size %s\n' "$i" "$n" "$(stty size)"; done"#;

const PAINTED: &str = r#"printf '\033[?25l'; i=0; while [ $i -lt 80 ]; do printf '\033[4%dm %02d \033[0m \033[1mbold\033[0m line %d\n' $((i % 7 + 1)) $i $i; i=$((i+1)); done; stty -echo; while read line; do printf '%s one\n\033[42m%s two\033[0m\n' "$line" "$line"; done"#;

struct Reply {
    winches: u32,
    size: String,
}

fn reply(running: &mut Running, count: u32) -> Reply {
    running.chord(Key::Enter, Modifiers::NONE);
    let expected = format!("reply {count} winch ");
    let output = running.wait_for(&expected, Modifiers::NONE);
    let line = output
        .lines()
        .find_map(|line| line.trim_end().strip_prefix(&expected))
        .expect("reply line");
    let (winches, size) = line.split_once(" size ").expect("reply holds a size");
    Reply {
        winches: winches.parse().expect("winch count"),
        size: size.to_owned(),
    }
}

#[test]
fn input_reaches_pty_and_output_renders() {
    let _alone = alone();
    let mut running = Running::start("cat");
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);

    for key in [Key::Char('h'), Key::Char('i'), Key::Enter] {
        running.chord(key, Modifiers::NONE);
    }

    running.wait_for("hi\nhi", Modifiers::NONE);
}

#[test]
fn drop_terminates_child() {
    let options = Options::new().shell("/bin/sh", [String::from("-c"), String::from("sleep 60")]);
    let session = Session::open(&options, |_: Report| {}).expect("failed to open PTY session");

    let started = Instant::now();
    drop(session);

    assert!(started.elapsed() < TIMEOUT, "drop blocked on child exit",);
}

#[test]
fn typing_redraws_its_line_while_the_others_stay_drawn() {
    let _alone = alone();
    let mut running = Running::start("printf 'north\\nsouth\\n'; cat");
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);
    running.wait_drawn(&["\"north", "\"south"]);

    for key in [
        Key::Char('w'),
        Key::Char('e'),
        Key::Char('s'),
        Key::Char('t'),
    ] {
        running.chord(key, Modifiers::NONE);
    }

    let drawn = running.wait_drawn(&["\"west"]);
    assert!(
        drawn.contains("\"north") && drawn.contains("\"south"),
        "{drawn}"
    );
}

#[test]
fn resizing_reaches_the_pty_once_per_size() {
    let _alone = alone();
    let mut running = Running::start(SIZES);
    running.wait_for("ready", Modifiers::NONE);
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);
    let first = reply(&mut running, 1);

    running.resize(Size::new(480.0, 300.0));
    for _ in 0..3 {
        running.settle();
    }
    let smaller = reply(&mut running, 2);
    assert_eq!(smaller.winches, first.winches + 1);
    assert_ne!(smaller.size, first.size);

    running.resize(Size::new(640.0, 400.0));
    running.settle();
    let restored = reply(&mut running, 3);
    assert_eq!(restored.winches, first.winches + 2);
    assert_eq!(restored.size, first.size);
}

#[test]
fn focus_reports_follow_focus() {
    let _alone = alone();
    let mut running =
        Running::start(r#"stty raw -echo; printf '\033[?1004h'; printf 'ready\n'; cat -v"#);
    running.wait_for("ready", Modifiers::NONE);
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);

    running.activate(false);
    running.activate(true);
    running.chord(Key::Char('z'), Modifiers::NONE);

    let output = running.wait_for("z", Modifiers::NONE);
    let sent = output
        .split_once("ready")
        .map_or("", |(_, sent)| sent.trim());
    let toggled = sent.strip_prefix("^[[I").unwrap_or(sent);
    assert_eq!(toggled, "^[[O^[[Iz", "{output:?}");
}

#[test]
fn scrolled_screen_draws_like_a_fresh_one() {
    let _alone = alone();
    let mut running = Running::start(PAINTED);
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);
    running.wait_drawn(&["line 79"]);
    running.assert_drawn_like_fresh("filled");

    for letter in ['k', 'm', 'q'] {
        running.chord(Key::Char(letter), Modifiers::NONE);
        running.chord(Key::Enter, Modifiers::NONE);
        running.wait_drawn(&[&format!("{letter} one"), &format!("{letter} two")]);
        running.assert_drawn_like_fresh(&format!("scrolled after {letter}"));
    }

    for (back, step) in [
        (40.0, "scrolled back"),
        (-20.0, "scrolled forward"),
        (700.0, "scrolled far back"),
        (-17.0, "scrolled forward a line"),
        (-560.0, "scrolled far forward"),
    ] {
        running.wheel(back);
        running.assert_drawn_like_fresh(step);
    }
}
