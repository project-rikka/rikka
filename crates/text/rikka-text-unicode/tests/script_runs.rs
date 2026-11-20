use rikka_text_unicode::{Script, script_runs};

fn runs(text: &str) -> Vec<(&str, Script)> {
    script_runs(text)
        .map(|run| (&text[run.range], run.script))
        .collect()
}

#[test]
fn script_runs_split_text_by_script() {
    assert_eq!(
        runs("Hello \u{043C}\u{0438}\u{0440} \u{4E16}\u{754C}"),
        [
            ("Hello ", Script::Latin),
            ("\u{043C}\u{0438}\u{0440} ", Script::Cyrillic),
            ("\u{4E16}\u{754C}", Script::Han),
        ]
    );
}

#[test]
fn closing_brackets_take_opening_script() {
    assert_eq!(
        runs("\u{05D0} (abc) \u{05D1}"),
        [
            ("\u{05D0} (", Script::Hebrew),
            ("abc", Script::Latin),
            (") \u{05D1}", Script::Hebrew),
        ]
    );
}
