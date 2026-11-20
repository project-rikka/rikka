#[derive(Clone, Copy, Debug)]
pub struct EmbeddedFont {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

macro_rules! embedded {
    ($folder:literal, $name:literal) => {
        EmbeddedFont {
            name: $name,
            bytes: include_bytes!(concat!(env!($folder), "/", $name)),
        }
    };
}

pub const UI: [EmbeddedFont; 4] = [
    embedded!("RIKKA_EMBEDDED_UI_FONTS", "InterVariable.ttf"),
    embedded!("RIKKA_EMBEDDED_UI_FONTS", "InterVariable-Italic.ttf"),
    embedded!("RIKKA_EMBEDDED_UI_FONTS", "JetBrainsMono[wght].ttf"),
    embedded!("RIKKA_EMBEDDED_UI_FONTS", "JetBrainsMono-Italic[wght].ttf"),
];

pub const ICONS: [EmbeddedFont; 2] = [
    embedded!("RIKKA_EMBEDDED_ICON_FONTS", "MaterialSymbolsRounded.ttf"),
    embedded!("RIKKA_EMBEDDED_ICON_FONTS", "FontAwesomeBrands.otf"),
];
