use rikka_nix::{Module, Packages};

use crate::pending::{Pending, toggled};

const PATH: &str = "i18n.inputMethod.fcitx5.addons";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Package {
    pub(crate) attribute: &'static str,
    pub(crate) label: &'static str,
}

const fn package(attribute: &'static str, label: &'static str) -> Package {
    Package { attribute, label }
}

pub(crate) const OFFERED: [Package; 13] = [
    package("fcitx5-mozc", "settings.keyboard.package.mozc"),
    package("fcitx5-anthy", "settings.keyboard.package.anthy"),
    package("fcitx5-skk", "settings.keyboard.package.skk"),
    package(
        "qt6Packages.fcitx5-chinese-addons",
        "settings.keyboard.package.chinese",
    ),
    package("fcitx5-chewing", "settings.keyboard.package.chewing"),
    package("fcitx5-mcbopomofo", "settings.keyboard.package.mcbopomofo"),
    package(
        "fcitx5-table-extra",
        "settings.keyboard.package.table-extra",
    ),
    package(
        "fcitx5-table-other",
        "settings.keyboard.package.table-other",
    ),
    package("fcitx5-rime", "settings.keyboard.package.rime"),
    package("fcitx5-hangul", "settings.keyboard.package.hangul"),
    package(
        "qt6Packages.fcitx5-unikey",
        "settings.keyboard.package.unikey",
    ),
    package("fcitx5-bamboo", "settings.keyboard.package.bamboo"),
    package("fcitx5-m17n", "settings.keyboard.package.m17n"),
];

pub(crate) fn installed(pending: &Pending) -> Vec<String> {
    pending
        .get::<Packages>(Module::Ime, PATH)
        .map(|packages| packages.names().collect())
        .unwrap_or_default()
}

pub(crate) fn installs(installed: &[String], attribute: &str) -> bool {
    installed.iter().any(|package| package == attribute)
}

pub(crate) fn install(pending: &mut Pending, attribute: &'static str, on: bool) {
    let packages = toggled(&installed(pending), attribute, on);
    match packages.is_empty() {
        true => pending.remove(Module::Ime, PATH),
        false => pending.set(Module::Ime, PATH, Packages::new(&packages)),
    }
}
