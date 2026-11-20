use rikka_nix_edit::Set;
use rikka_nixos_layout::Packages;

use crate::{answers::Answers, locale::Locale};

const AUTOSTART: [&str; 5] = [
    "systemd",
    "user",
    "units",
    "app-org.fcitx.Fcitx5@autostart.service",
    "enable",
];

pub(crate) fn body(answers: &Answers) -> Set {
    let set = Set::new()
        .bind("i18n.inputMethod.enable", true)
        .bind("i18n.inputMethod.type", "fcitx5")
        .bind("i18n.inputMethod.fcitx5.waylandFrontend", true);
    let addons = fcitx5_addons(answers.locale);
    let set = match addons.is_empty() {
        true => set,
        false => set.bind("i18n.inputMethod.fcitx5.addons", Packages::new(addons)),
    };
    set.bind(AUTOSTART, false)
}

pub fn fcitx5_addons(locale: &Locale) -> &'static [&'static str] {
    let language = locale.name.split('_').next().unwrap_or(locale.name);
    match language {
        "ja" => &["fcitx5-mozc"],
        "zh" => &["qt6Packages.fcitx5-chinese-addons"],
        "ko" => &["fcitx5-hangul"],
        "vi" => &["qt6Packages.fcitx5-unikey"],
        _ => &[],
    }
}
