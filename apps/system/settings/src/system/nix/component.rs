use rikka_app::prelude::*;
use rikka_nix::{Channel, Module};
use rikka_widgets::column;

use crate::info_row::info_row;
use crate::pending::{Pending, toggled};
use crate::picker::{labelled, value_picker};

const ALLOW_UNFREE: &str = "nixpkgs.config.allowUnfree";
const COLLECT: &str = "nix.gc.automatic";
const COLLECT_WHEN: &str = "nix.gc.dates";
const COLLECT_KEEP: &str = "nix.gc.options";
const OPTIMISE: &str = "nix.optimise.automatic";
const FEATURES: &str = "nix.settings.experimental-features";
const UPGRADE: &str = "system.autoUpgrade.enable";
const UPGRADE_WHEN: &str = "system.autoUpgrade.dates";
const ALLOW_REBOOT: &str = "system.autoUpgrade.allowReboot";

const FEATURES_OFFERED: [(&str, &str); 2] = [
    ("nix-command", "settings.nix.nix-command"),
    ("flakes", "settings.nix.flakes"),
];

const WHEN: [(&str, &str); 4] = [
    ("daily", "settings.nix.when.daily"),
    ("weekly", "settings.nix.when.weekly"),
    ("monthly", "settings.nix.when.monthly"),
    ("04:00", "settings.nix.when.nightly"),
];

const KEEP: [(&str, &str); 4] = [
    ("--delete-older-than 7d", "settings.nix.keep.week"),
    ("--delete-older-than 14d", "settings.nix.keep.fortnight"),
    ("--delete-older-than 30d", "settings.nix.keep.month"),
    ("--delete-older-than 90d", "settings.nix.keep.quarter"),
];

pub(crate) struct Nix {
    channel: Option<Channel>,
}

impl Nix {
    pub(crate) fn new() -> Self {
        Self { channel: None }
    }

    pub(crate) fn opened(&mut self) {
        self.channel = Some(Channel::detect());
    }
}

pub(crate) fn view(nix: &Nix, pending: &Pending) -> impl View {
    column! {
        channel_card(nix);
        packages(pending);
        store(pending);
        upgrades(pending);
    }
    .gap(Space::ROOMY)
}

fn channel_card(nix: &Nix) -> impl View {
    section(
        tr!("settings.nix.nixpkgs"),
        column! {
            if let Some(channel) = &nix.channel {
                info_row(tr!("settings.nix.channel"), channel.name().to_owned());
                info_row(tr!("settings.nix.flake"), channel.flake_url());
            }
        }
        .gap(Space::SNUG),
    )
}

fn packages(pending: &Pending) -> impl View {
    section(
        tr!("settings.nix.packages"),
        flag_field(pending, tr!("settings.nix.allow-unfree"), ALLOW_UNFREE),
    )
}

fn store(pending: &Pending) -> impl View {
    let collecting = pending.get::<bool>(Module::Nix, COLLECT).unwrap_or(false);
    let enabled = features_held(pending);
    section(
        tr!("settings.nix.store"),
        column! {
            flag_field(pending, tr!("settings.nix.collect"), COLLECT);
            if collecting {
                field(
                    tr!("settings.nix.collect-when"),
                    value_picker(
                        tr!("settings.nix.unset"),
                        labelled(&WHEN),
                        pending.get::<String>(Module::Nix, COLLECT_WHEN).as_deref(),
                        |pending: &mut Pending, when: Option<String>| {
                            pending.write(Module::Nix, COLLECT_WHEN, when.as_deref());
                        },
                    ),
                );
                field(
                    tr!("settings.nix.collect-keep"),
                    value_picker(
                        tr!("settings.nix.keep.reachable"),
                        labelled(&KEEP),
                        pending.get::<String>(Module::Nix, COLLECT_KEEP).as_deref(),
                        |pending: &mut Pending, keep: Option<String>| {
                            pending.write(Module::Nix, COLLECT_KEEP, keep.as_deref());
                        },
                    ),
                );
            }
            flag_field(pending, tr!("settings.nix.optimise"), OPTIMISE);
            for (feature, key) in FEATURES_OFFERED {
                field(
                    Label::translated(key),
                    switch(enabled.iter().any(|enabled| enabled == feature)).on_toggle(
                        move |pending: &mut Pending, on: bool| feature_flag(pending, feature, on),
                    ),
                );
            }
        }
        .gap(Space::SNUG),
    )
}

fn upgrades(pending: &Pending) -> impl View {
    let upgrading = pending.get::<bool>(Module::Nix, UPGRADE).unwrap_or(false);
    section(
        tr!("settings.nix.upgrades"),
        column! {
            flag_field(pending, tr!("settings.nix.upgrade"), UPGRADE);
            if upgrading {
                field(
                    tr!("settings.nix.upgrade-when"),
                    value_picker(
                        tr!("settings.nix.unset"),
                        labelled(&WHEN),
                        pending.get::<String>(Module::Nix, UPGRADE_WHEN).as_deref(),
                        |pending: &mut Pending, when: Option<String>| {
                            pending.write(Module::Nix, UPGRADE_WHEN, when.as_deref());
                        },
                    ),
                );
            }
            field(
                tr!("settings.nix.allow-reboot"),
                switch(pending.get::<bool>(Module::Nix, ALLOW_REBOOT).unwrap_or(false))
                    .on_toggle(|pending: &mut Pending, on: bool| flag(pending, ALLOW_REBOOT, on))
                    .enabled(upgrading),
            )
            .dimmed(!upgrading);
        }
        .gap(Space::SNUG),
    )
}

fn flag_field(pending: &Pending, label: Label, path: &'static str) -> impl View {
    field(
        label,
        switch(pending.get::<bool>(Module::Nix, path).unwrap_or(false))
            .on_toggle(move |pending: &mut Pending, on: bool| flag(pending, path, on)),
    )
}

fn feature_flag(pending: &mut Pending, feature: &str, on: bool) {
    let enabled = features_held(pending);
    let features = toggled(&enabled, feature, on);
    match features.is_empty() {
        true => pending.remove(Module::Nix, FEATURES),
        false => pending.set(Module::Nix, FEATURES, features),
    }
}

fn features_held(pending: &Pending) -> Vec<String> {
    pending
        .get::<Vec<String>>(Module::Nix, FEATURES)
        .unwrap_or_default()
}

pub(in crate::system) fn flag(pending: &mut Pending, path: &str, on: bool) {
    match on {
        true => pending.set(Module::Nix, path, true),
        false => pending.remove(Module::Nix, path),
    }
}

impl std::fmt::Debug for Nix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nix")
            .field("channel", &self.channel.as_ref().map(Channel::name))
            .finish_non_exhaustive()
    }
}
