use std::cell::RefCell;

use rikka_appearance::ResolvedAppearance;
use rikka_settings::Schema;
use rikka_settings::session::appearance::{self, DOMAIN};
use shell_kit::Shell;

pub(super) fn schema() -> Schema {
    appearance::schema()
}

pub(super) fn follow(shell: Shell) -> Shell {
    let Some(reader) = shell.reader(DOMAIN) else {
        return shell;
    };
    let resolved = RefCell::new(ResolvedAppearance::from_settings(&reader.borrow()));
    let shell = shell.theme(resolved.borrow().theme());
    let shared_theme = shell.shared_theme();
    shell.follow_domain(DOMAIN, move |_, settings| {
        if let Some(theme) = resolved.borrow_mut().refresh(settings) {
            shared_theme.set(theme);
        }
    })
}
