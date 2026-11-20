use std::sync::OnceLock;

use rikka_text_font::Font;

use crate::icon_font::IconFont;

pub(crate) fn font(icons: IconFont) -> Option<Font<'static>> {
    static MATERIAL_SYMBOLS: OnceLock<Option<Font<'static>>> = OnceLock::new();
    static FONT_AWESOME_BRANDS: OnceLock<Option<Font<'static>>> = OnceLock::new();
    let cell = match icons {
        IconFont::MaterialSymbols => &MATERIAL_SYMBOLS,
        IconFont::FontAwesomeBrands => &FONT_AWESOME_BRANDS,
    };
    *cell.get_or_init(|| open(icons))
}

fn open(icons: IconFont) -> Option<Font<'static>> {
    let file = icons.file();
    let bytes = bytes(file)?;
    match Font::new(bytes) {
        Ok(font) => Some(font),
        Err(error) => {
            tracing::warn!("invalid icon font {file}: {error}");
            None
        }
    }
}

#[cfg(unix)]
fn bytes(file: &str) -> Option<&'static [u8]> {
    use rikka_shipped::Shipped;

    const ICON_FONTS: Shipped = Shipped {
        variable: "RIKKA_ICON_FONT_PATH",
        installed: "share/rikka/fonts",
    };

    let Some(path) = ICON_FONTS.find(file) else {
        tracing::warn!("icon font {file} is not installed");
        return None;
    };
    rikka_text_catalog::mapped_font(&path)
        .inspect_err(|error| tracing::warn!("failed to read {}: {error}", path.display()))
        .ok()
}

#[cfg(windows)]
fn bytes(file: &str) -> Option<&'static [u8]> {
    let embedded = rikka_embedded_fonts::ICONS
        .iter()
        .find(|font| font.name == file)
        .map(|font| font.bytes);
    if embedded.is_none() {
        tracing::warn!("icon font {file} is not embedded");
    }
    embedded
}
