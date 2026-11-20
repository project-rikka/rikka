mod feature;
mod group;
mod picker;
mod service;
mod skin_tone;
mod tab;
mod view;

pub use feature::Emoji;

const CAPABILITY: &str = "rikka.emoji";

const RECENTS: shell_kit::RecentsConfig = shell_kit::RecentsConfig {
    file: "emoji.json",
    field: "emoji",
    capacity: 45,
};
