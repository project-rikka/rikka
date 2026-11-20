use std::fmt::Write as _;

use rikka_theme::{MaterialStyle, MaterialYou, ThemeSpec};
use rikka_types::Color;

const COLOR_KEYS: [&str; 25] = [
    "crust",
    "mantle",
    "base",
    "surface",
    "overlay",
    "text",
    "subtext",
    "muted",
    "border",
    "cursor",
    "highlight",
    "link",
    "accent",
    "red",
    "orange",
    "yellow",
    "green",
    "cyan",
    "blue",
    "purple",
    "magenta",
    "error",
    "warning",
    "success",
    "info",
];

const SEEDS: [u32; 8] = [
    0x7aa2f7, 0xf44336, 0x4caf50, 0xffc107, 0x9c27b0, 0x000000, 0xffffff, 0x808080,
];

pub(crate) fn theme_toml(slug: &str, accent: &str) -> String {
    let mut toml =
        format!("name = \"Sample\"\nslug = \"{slug}\"\nvariant = \"dark\"\n\n[colors]\n");
    for key in COLOR_KEYS {
        let value = if key == "accent" { accent } else { "#123456" };
        writeln!(toml, "{key} = \"{value}\"").expect("write to String");
    }
    toml
}

pub(crate) fn contrast_ratio(a: Color, b: Color) -> f32 {
    fn linearize(u: f32) -> f32 {
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    }
    let luminance =
        |c: Color| 0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b);
    let (la, lb) = (luminance(a), luminance(b));
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

pub(crate) fn for_each_generated(act: impl Fn(&ThemeSpec)) {
    for seed in SEEDS {
        for style in MaterialStyle::ALL {
            for contrast in [-1.0, 0.0, 1.0] {
                let material = MaterialYou::new(Color::hex(seed))
                    .style(style)
                    .contrast(contrast);
                for spec in material.specs() {
                    act(&spec);
                    act(&spec.clone().glass());
                    act(&spec.liquid());
                }
            }
        }
    }
}
