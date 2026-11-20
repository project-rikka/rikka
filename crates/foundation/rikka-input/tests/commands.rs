use rikka_input::Command;
use rikka_types::{Key, Modifiers, PointerButton};

#[test]
fn command_matches_exact_chords_and_buttons() {
    let control_shift = Modifiers::CONTROL | Modifiers::SHIFT;
    assert_eq!(
        Command::for_key(Key::Char('c'), Modifiers::CONTROL),
        Some(Command::Copy)
    );
    assert_eq!(
        Command::for_key(Key::Insert, Modifiers::CONTROL),
        Some(Command::Copy)
    );
    assert_eq!(
        Command::for_key(Key::Insert, Modifiers::SHIFT),
        Some(Command::Paste)
    );
    assert_eq!(
        Command::for_key(Key::Char('Z'), control_shift),
        Some(Command::Redo)
    );
    assert_eq!(
        Command::for_key(Key::F(10), Modifiers::SHIFT),
        Some(Command::ContextMenu)
    );
    assert_eq!(
        Command::for_button(PointerButton::Back),
        Some(Command::Back)
    );

    assert_eq!(Command::for_key(Key::Char('c'), control_shift), None);
    assert_eq!(Command::for_key(Key::Char('c'), Modifiers::NONE), None);
    assert_eq!(Command::for_key(Key::ArrowLeft, Modifiers::NONE), None);
    assert_eq!(Command::for_button(PointerButton::Primary), None);
}
