#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconFont {
    #[default]
    MaterialSymbols,
    FontAwesomeBrands,
}

impl IconFont {
    pub const fn file(self) -> &'static str {
        match self {
            Self::MaterialSymbols => "MaterialSymbolsRounded.ttf",
            Self::FontAwesomeBrands => "FontAwesomeBrands.otf",
        }
    }

    pub(crate) const fn middle(self) -> f32 {
        match self {
            Self::MaterialSymbols => 0.5,
            Self::FontAwesomeBrands => 0.375,
        }
    }
}
