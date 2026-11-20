use rikka_input::{ActionMap, Chord, Error, parse_modifiers};
use rikka_types::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Quit,
    Close,
    Spawn,
}

fn chord(text: &str) -> Chord {
    text.parse().expect("the test spells its own chords")
}

#[test]
fn modifiers_round_trip_text() {
    assert_eq!(
        parse_modifiers("ctrl+ALT"),
        Ok(Modifiers::CONTROL | Modifiers::ALT)
    );
    assert_eq!(parse_modifiers(""), Ok(Modifiers::NONE));
    assert!(parse_modifiers("Supr").is_err());
    assert!(parse_modifiers("Super+Super").is_err());
    for modifiers in [
        Modifiers::NONE,
        Modifiers::LOGO,
        Modifiers::CONTROL | Modifiers::ALT,
        Modifiers::SHIFT | Modifiers::LOGO,
    ] {
        let printed = Chord::new(modifiers, Key::Char('q')).to_string();
        assert_eq!(
            printed.parse::<Chord>().map(|chord| chord.modifiers()),
            Ok(modifiers)
        );
    }
}

#[test]
fn chord_round_trips_text() {
    for text in [
        "Ctrl+Alt+Backspace",
        "Super+Q",
        "Super+Shift+Q",
        "Super+Return",
        "F5",
        "Ctrl+Shift+F12",
        "Up",
        "Ctrl++",
        "Space",
    ] {
        let parsed = chord(text);
        let printed = parsed.to_string();
        assert_eq!(
            printed.parse::<Chord>(),
            Ok(parsed),
            "{text} printed as {printed}"
        );
    }
    assert_eq!(chord("shift+ctrl+t").to_string(), "Ctrl+Shift+T");
    assert_eq!(chord("arrowup").to_string(), "Up");
    assert_eq!(chord("Win+Q"), chord("Super+q"));
    assert_eq!(chord("Super+Return"), chord("super+enter"));
}

#[test]
fn invalid_chord_reports_reason() {
    assert_eq!("".parse::<Chord>(), Err(Error::EmptyChord));
    assert_eq!(
        "Hyper+Q".parse::<Chord>(),
        Err(Error::UnknownModifier("Hyper".to_owned()))
    );
    assert_eq!(
        "Ctrl+Nonsense".parse::<Chord>(),
        Err(Error::UnknownKey("Nonsense".to_owned()))
    );
    assert_eq!(
        "Ctrl+Ctrl+C".parse::<Chord>(),
        Err(Error::RepeatedModifier("Ctrl".to_owned()))
    );
    assert_eq!(
        "F0".parse::<Chord>(),
        Err(Error::UnknownKey("F0".to_owned()))
    );
}

#[test]
fn chord_ignores_case_matches_modifiers_exactly() {
    let plain = Chord::new(Modifiers::LOGO, Key::Char('q'));
    let shifted = Chord::new(Modifiers::LOGO | Modifiers::SHIFT, Key::Char('q'));
    assert_eq!(Chord::new(Modifiers::LOGO, Key::Char('Q')), plain);
    assert!(plain.matches(Modifiers::LOGO, Key::Char('Q')));
    assert!(!plain.matches(Modifiers::LOGO | Modifiers::SHIFT, Key::Char('Q')));
    assert!(shifted.matches(Modifiers::LOGO | Modifiers::SHIFT, Key::Char('Q')));

    let quit = Chord::new(Modifiers::CONTROL | Modifiers::ALT, Key::Backspace);
    assert!(!quit.matches(Modifiers::CONTROL, Key::Backspace));
    assert!(!quit.matches(
        Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT,
        Key::Backspace
    ));
}

#[test]
fn action_map_uses_last_binding() {
    let map = ActionMap::new()
        .bind(chord("Ctrl+Alt+Backspace"), Action::Quit)
        .bind(chord("Super+Return"), Action::Spawn)
        .bind(chord("Super+Return"), Action::Close);

    assert_eq!(
        map.resolve(Modifiers::CONTROL | Modifiers::ALT, Key::Backspace),
        Some(&Action::Quit)
    );
    assert_eq!(
        map.resolve(Modifiers::LOGO, Key::Enter),
        Some(&Action::Close)
    );
    assert_eq!(map.resolve(Modifiers::NONE, Key::Enter), None);
    assert_eq!(map.chord_for(&Action::Spawn), None);
    assert_eq!(
        map.chord_for(&Action::Close).map(|chord| chord.to_string()),
        Some("Super+Enter".to_owned())
    );
    assert!(map.has_binding_with_modifiers(Modifiers::LOGO));
    assert!(!map.has_binding_with_modifiers(Modifiers::CONTROL));
}
