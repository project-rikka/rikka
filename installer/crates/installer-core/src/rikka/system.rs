use rikka_nix_edit::Set;

use crate::answers::Answers;

pub(crate) fn body(answers: &Answers) -> Set {
    let set = Set::new()
        .bind("networking.hostName", &answers.hostname)
        .bind("time.timeZone", &answers.timezone)
        .bind("i18n.defaultLocale", answers.locale.name);
    let set = match answers.english_messages && !answers.locale.english {
        true => set.bind("i18n.extraLocaleSettings.LC_MESSAGES", "en_US.UTF-8"),
        false => set,
    };
    set.bind("console.keyMap", answers.keyboard.keymap)
        .bind("services.xserver.xkb.layout", answers.keyboard.layout)
        .bind("services.xserver.xkb.variant", answers.keyboard.variant)
}
