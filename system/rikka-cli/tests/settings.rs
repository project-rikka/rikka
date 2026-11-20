use crate::support::Session;

#[test]
fn set_persists_and_reset_removes() {
    let session = Session::start();

    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.input", "close-window"])
            .trim(),
        "\"Super+Q\""
    );

    session.expect_success(&["settings", "set", "rikka.input", "close-window", "Super+W"]);
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.input", "close-window"])
            .trim(),
        "\"Super+W\""
    );

    rikka_settings::flush_pending_writes();
    let written = std::fs::read_to_string(session.user_file("rikka.input")).unwrap();
    assert!(written.contains("close-window"), "{written}");
    assert!(written.contains("Super+W"), "{written}");

    session.expect_success(&["settings", "reset", "rikka.input", "close-window"]);
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.input", "close-window"])
            .trim(),
        "\"Super+Q\""
    );
    rikka_settings::flush_pending_writes();
    assert!(!session.user_file("rikka.input").exists());
}

#[test]
fn get_output_round_trips_through_set() {
    let session = Session::start();

    session.expect_success(&["settings", "set", "rikka.input", "repeat-delay", "400"]);
    let read = session.expect_success(&["settings", "get", "rikka.input", "repeat-delay"]);
    assert_eq!(read.trim(), "400");

    session.expect_success(&[
        "settings",
        "set",
        "rikka.input",
        "close-window",
        "\"Ctrl+Alt+Delete\"",
    ]);
    let read = session.expect_success(&["settings", "get", "rikka.input", "close-window"]);
    session.expect_success(&[
        "settings",
        "set",
        "rikka.input",
        "close-window",
        read.trim(),
    ]);
    assert_eq!(
        session
            .expect_success(&["settings", "get", "rikka.input", "close-window"])
            .trim(),
        "\"Ctrl+Alt+Delete\""
    );
}

#[test]
fn set_rejects_invalid_value_with_reason() {
    let session = Session::start();

    let out_of_range =
        session.expect_failure(&["settings", "set", "rikka.input", "repeat-delay", "-5"]);
    assert!(
        out_of_range.contains("between 100 and 2000"),
        "{out_of_range}"
    );

    let not_a_choice =
        session.expect_failure(&["settings", "set", "rikka.input", "cursor-theme", "Breeze"]);
    assert!(not_a_choice.contains("one of"), "{not_a_choice}");

    let not_a_number =
        session.expect_failure(&["settings", "set", "rikka.input", "repeat-delay", "quick"]);
    assert!(not_a_number.contains("whole number"), "{not_a_number}");

    let nothing_there = session.expect_failure(&["settings", "get", "rikka.input", "colour"]);
    assert!(nothing_there.contains("colour"), "{nothing_there}");

    let no_domain = session.expect_failure(&["settings", "list", "com.example.absent"]);
    assert!(no_domain.contains("com.example.absent"), "{no_domain}");

    let not_a_domain = session.expect_failure(&["settings", "list", "../../etc"]);
    assert!(
        not_a_domain.contains("not a valid settings domain"),
        "{not_a_domain}"
    );
}
