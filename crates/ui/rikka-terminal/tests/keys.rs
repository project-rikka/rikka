use rikka_types::{Key, Modifiers, Point};

use crate::support::{Running, alone};

#[test]
fn keys_encode_like_xterm() {
    let _alone = alone();
    let mut running = Running::start("stty raw -echo; printf 'ready\\n'; cat -v");
    running.wait_for("ready", Modifiers::NONE);
    running.click(Point::new(0.5, 0.5), Modifiers::NONE);

    let keys = [
        (Key::Char('q'), Modifiers::NONE, "q"),
        (Key::Char('c'), Modifiers::CONTROL, "^C"),
        (Key::Char('b'), Modifiers::ALT, "^[b"),
        (Key::ArrowUp, Modifiers::NONE, "^[[A"),
        (Key::ArrowUp, Modifiers::ALT, "^[[1;3A"),
        (Key::F(1), Modifiers::NONE, "^[OP"),
        (Key::F(5), Modifiers::NONE, "^[[15~"),
        (Key::Delete, Modifiers::NONE, "^[[3~"),
        (Key::Tab, Modifiers::SHIFT, "^[[Z"),
        (Key::Backspace, Modifiers::NONE, "^?"),
        (Key::Backspace, Modifiers::CONTROL, "^W"),
        (Key::Delete, Modifiers::CONTROL, "^[d"),
        (Key::Enter, Modifiers::NONE, "^M"),
    ];
    for (key, modifiers, _) in keys {
        running.chord(key, modifiers);
    }

    let expected: String = keys.iter().map(|(_, _, sent)| *sent).collect();
    running.wait_for(&expected, Modifiers::NONE);
}
