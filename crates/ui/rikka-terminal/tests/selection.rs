use rikka_types::{Key, Modifiers};
use rikka_ui::clipboard::{self, Selection};

use crate::support::{Running, alone};

#[test]
fn mark_mode_selects_with_keys_and_copies_on_enter() {
    let _alone = alone();
    clipboard::set_text_of(Selection::Clipboard, "");
    let mut running = Running::start("printf 'alpha beta gamma\\n'; sleep 30");
    running.wait_for("alpha beta gamma", Modifiers::NONE);

    running.chord(Key::Char('m'), Modifiers::CONTROL_SHIFT);
    running.chord(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(running.session.selected_text().as_deref(), Some("a"));

    for (key, modifiers, selected) in [
        (Key::ArrowRight, Modifiers::CONTROL_SHIFT, "alpha"),
        (Key::ArrowRight, Modifiers::CONTROL_SHIFT, "alpha beta"),
        (Key::ArrowLeft, Modifiers::SHIFT, "alpha bet"),
        (Key::Home, Modifiers::SHIFT, "a"),
        (Key::End, Modifiers::SHIFT, "alpha beta gamma"),
    ] {
        running.chord(key, modifiers);
        assert_eq!(
            running.session.selected_text().as_deref(),
            Some(selected),
            "after {key:?} with {modifiers:?}"
        );
    }

    running.chord(Key::Enter, Modifiers::NONE);

    assert_eq!(running.session.selected_text(), None);
    assert_eq!(
        clipboard::text_of(Selection::Clipboard).as_deref(),
        Some("alpha beta gamma")
    );
}

#[test]
fn shift_arrows_reach_the_program_without_a_selection() {
    let _alone = alone();
    let mut running = Running::start("stty raw -echo; printf 'ready\\n'; cat -v");
    running.wait_for("ready", Modifiers::NONE);
    running.chord(Key::Escape, Modifiers::NONE);

    running.chord(Key::ArrowLeft, Modifiers::SHIFT);
    running.chord(Key::Char('q'), Modifiers::NONE);

    let output = running.wait_for("q", Modifiers::NONE);
    assert!(output.contains("^[[1;2Dq"), "saw {output:?}");
}
