use std::time::Instant;

use rikka_terminal::{ClipboardAccess, Options};
use rikka_types::{Key, Modifiers, Point};
use rikka_ui::clipboard::{self, Selection};

use crate::support::{POLL_INTERVAL, Running, TIMEOUT, alone};

#[test]
fn paste_chords_write_clipboard() {
    let _alone = alone();
    let mut running = Running::start("cat");
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);

    for (modifiers, pasted) in [
        (Modifiers::CONTROL, "pasted with control v"),
        (Modifiers::CONTROL_SHIFT, "pasted with control shift v"),
    ] {
        clipboard::set_text_of(Selection::Clipboard, &format!("{pasted}\n"));
        running.chord(Key::Char('v'), modifiers);
        running.wait_for(pasted, Modifiers::NONE);
    }
}

#[test]
fn control_c_copies_a_selection_and_interrupts_without_one() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "");
    let mut running = Running::start("stty raw -echo; printf 'ready\\n'; cat -v");
    running.wait_for("ready", Modifiers::NONE);

    running.chord(Key::Char('c'), Modifiers::CONTROL);

    let copied = clipboard::text_of(Selection::Clipboard).unwrap_or_default();
    assert!(copied.contains("ready"), "copied {copied:?}");
    assert_eq!(running.session.selected_text(), None);

    running.chord(Key::Char('c'), Modifiers::CONTROL);
    running.chord(Key::Char('q'), Modifiers::NONE);

    let output = running.wait_for("^Cq", Modifiers::NONE);
    assert!(
        !output.contains("^C^C"),
        "the copy reached the program: {output:?}"
    );
}

#[test]
fn pasted_line_endings_arrive_once() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "first\r\nsecond\r\n");
    let mut running = Running::start("stty -echo; printf 'ready\\n'; cat");
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);
    running.wait_for("ready", Modifiers::NONE);

    running.chord(Key::Char('v'), Modifiers::CONTROL_SHIFT);

    let output = running.wait_for("second", Modifiers::NONE);
    assert!(output.contains("first\nsecond"), "saw {output:?}");
}

#[test]
fn osc52_copy_sets_named_selections() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "");
    clipboard::set_text_of(Selection::Primary, "");
    let _running =
        Running::start("printf '\\033]52;cp;b24gYm90aCBzZWxlY3Rpb25z\\033\\\\'; sleep 30");

    let deadline = Instant::now() + TIMEOUT;
    while clipboard::text_of(Selection::Clipboard).as_deref() != Some("on both selections")
        || clipboard::text_of(Selection::Primary).as_deref() != Some("on both selections")
    {
        assert!(Instant::now() < deadline, "no OSC 52 copy received");
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[test]
fn osc52_query_returns_selection() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "read by the program");
    let mut running = Running::with(
        Options::new().clipboard(ClipboardAccess::new().reading(true)),
        "printf '\\033]52;c;?\\007'; sleep 30",
    );

    running.wait_for("^[]52;c;cmVhZCBieSB0aGUgcHJvZ3JhbQ==^G", Modifiers::NONE);
}

#[test]
fn denied_clipboard_access_blocks_osc52() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "kept");
    let shut = ClipboardAccess::new().reading(false).writing(false);
    let mut running = Running::with(
        Options::new().clipboard(shut),
        "printf '\\033]52;c;?\\007\\033]52;c;dGFrZW4=\\007'; sleep 1; printf 'done\\n'; sleep 30",
    );

    let output = running.wait_for("done", Modifiers::NONE);

    assert!(!output.contains("52;c;"), "program answered: {output:?}");
    assert_eq!(
        clipboard::text_of(Selection::Clipboard).as_deref(),
        Some("kept")
    );
}
