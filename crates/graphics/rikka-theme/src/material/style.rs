use rikka_material_color::Variant as Scheme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MaterialStyle {
    #[default]
    TonalSpot,
    Vibrant,
    Expressive,
    Neutral,
    Monochrome,
    Fidelity,
    Content,
    Rainbow,
    FruitSalad,
}

impl MaterialStyle {
    pub const ALL: [MaterialStyle; 9] = [
        MaterialStyle::TonalSpot,
        MaterialStyle::Vibrant,
        MaterialStyle::Expressive,
        MaterialStyle::Neutral,
        MaterialStyle::Monochrome,
        MaterialStyle::Fidelity,
        MaterialStyle::Content,
        MaterialStyle::Rainbow,
        MaterialStyle::FruitSalad,
    ];

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            MaterialStyle::TonalSpot => "tonal-spot",
            MaterialStyle::Vibrant => "vibrant",
            MaterialStyle::Expressive => "expressive",
            MaterialStyle::Neutral => "neutral",
            MaterialStyle::Monochrome => "monochrome",
            MaterialStyle::Fidelity => "fidelity",
            MaterialStyle::Content => "content",
            MaterialStyle::Rainbow => "rainbow",
            MaterialStyle::FruitSalad => "fruit-salad",
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<MaterialStyle> {
        MaterialStyle::ALL
            .into_iter()
            .find(|style| style.slug() == slug)
    }

    pub(crate) fn scheme(self) -> Scheme {
        match self {
            MaterialStyle::TonalSpot => Scheme::TonalSpot,
            MaterialStyle::Vibrant => Scheme::Vibrant,
            MaterialStyle::Expressive => Scheme::Expressive,
            MaterialStyle::Neutral => Scheme::Neutral,
            MaterialStyle::Monochrome => Scheme::Monochrome,
            MaterialStyle::Fidelity => Scheme::Fidelity,
            MaterialStyle::Content => Scheme::Content,
            MaterialStyle::Rainbow => Scheme::Rainbow,
            MaterialStyle::FruitSalad => Scheme::FruitSalad,
        }
    }
}
