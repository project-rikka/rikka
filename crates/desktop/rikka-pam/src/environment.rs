use std::process::Command;

const PASSED: &[&str] = &[
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_MESSAGES",
    "LOCALE_ARCHIVE",
];

pub(crate) fn pass_locale(command: &mut Command) {
    command.env_clear();
    for name in PASSED {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}
