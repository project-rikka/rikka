use rikka_text::{IconFont, IconSpec};

use crate::brand::BrandSymbol;
use crate::material::MaterialSymbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconGlyph {
    font: IconFont,
    codepoint: char,
}

impl IconGlyph {
    pub const fn new(font: IconFont, codepoint: char) -> Self {
        Self { font, codepoint }
    }

    pub const fn font(self) -> IconFont {
        self.font
    }

    pub const fn codepoint(self) -> char {
        self.codepoint
    }

    pub fn spec(self, size: f32) -> IconSpec {
        IconSpec::new(self.codepoint, size).font(self.font)
    }
}

impl From<MaterialSymbol> for IconGlyph {
    fn from(symbol: MaterialSymbol) -> Self {
        Self::new(IconFont::MaterialSymbols, symbol.codepoint())
    }
}

impl From<BrandSymbol> for IconGlyph {
    fn from(symbol: BrandSymbol) -> Self {
        Self::new(IconFont::FontAwesomeBrands, symbol.codepoint())
    }
}
