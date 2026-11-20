use ade_command::Commands;

use crate::support::{project, write};

#[test]
fn fills_template_arguments() {
    let cases = [
        (
            "Review $ARGUMENTS and say what is wrong.",
            "src/main.rs now",
            "Review src/main.rs now and say what is wrong.",
        ),
        (
            "Move $1 to $2.",
            "src/one.rs src/two.rs",
            "Move src/one.rs to src/two.rs.",
        ),
        ("Review $1.$2", "one", "Review one."),
        ("Review $ARGUMENTS.", "", "Review ."),
        (
            "Review what has changed.",
            "the composer",
            "Review what has changed.\n\nthe composer",
        ),
        ("Review what has changed.", "  ", "Review what has changed."),
        (
            "Costs $0 and $x and $.",
            "one",
            "Costs $0 and $x and $.\n\none",
        ),
        (
            "$ARGUMENT is not one.",
            "one",
            "$ARGUMENT is not one.\n\none",
        ),
        ("$10", "one two", "one0"),
    ];

    for (template, typed, expected) in cases {
        let folder = project();
        write(folder.path(), "t.md", &format!("{template}\n"));
        let commands = Commands::find(Some(folder.path()));

        assert_eq!(
            commands.get("t").unwrap().filled(typed),
            expected,
            "{template:?} with {typed:?}"
        );
    }
}
