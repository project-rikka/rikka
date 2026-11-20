mod editor;
mod feature;
mod picker;
mod preferences;
mod service;
mod shared;

pub use feature::ColorPicker;

const CAPABILITY: &str = "rikka.color-picker";

const HISTORY: shell_kit::RecentsConfig = shell_kit::RecentsConfig {
    file: "color-picker.json",
    field: "colors",
    capacity: 20,
};
