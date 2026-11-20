use rikka_types::Choice;
use rikka_ui::MaterialSymbol;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SpaceIcon {
    Symbol(Symbol),
    Emoji(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Symbol {
    #[default]
    Star,
    Home,
    Work,
    Code,
    School,
    Favorite,
    Flight,
    Shopping,
    Music,
    Games,
    Pets,
    Palette,
}

impl Choice for Symbol {
    const ALL: &'static [Symbol] = &[
        Symbol::Star,
        Symbol::Home,
        Symbol::Work,
        Symbol::Code,
        Symbol::School,
        Symbol::Favorite,
        Symbol::Flight,
        Symbol::Shopping,
        Symbol::Music,
        Symbol::Games,
        Symbol::Pets,
        Symbol::Palette,
    ];

    fn slug(self) -> &'static str {
        match self {
            Symbol::Star => "star",
            Symbol::Home => "home",
            Symbol::Work => "work",
            Symbol::Code => "code",
            Symbol::School => "school",
            Symbol::Favorite => "favorite",
            Symbol::Flight => "flight",
            Symbol::Shopping => "shopping",
            Symbol::Music => "music",
            Symbol::Games => "games",
            Symbol::Pets => "pets",
            Symbol::Palette => "palette",
        }
    }
}

impl Symbol {
    pub fn glyph(self) -> MaterialSymbol {
        match self {
            Symbol::Star => MaterialSymbol::STAR,
            Symbol::Home => MaterialSymbol::HOME,
            Symbol::Work => MaterialSymbol::WORK,
            Symbol::Code => MaterialSymbol::CODE,
            Symbol::School => MaterialSymbol::SCHOOL,
            Symbol::Favorite => MaterialSymbol::FAVORITE,
            Symbol::Flight => MaterialSymbol::FLIGHT,
            Symbol::Shopping => MaterialSymbol::SHOPPING_CART,
            Symbol::Music => MaterialSymbol::MUSIC_NOTE,
            Symbol::Games => MaterialSymbol::SPORTS_ESPORTS,
            Symbol::Pets => MaterialSymbol::PETS,
            Symbol::Palette => MaterialSymbol::PALETTE,
        }
    }
}

impl SpaceIcon {
    pub fn to_store(&self) -> browser_store::SpaceIcon {
        match self {
            SpaceIcon::Symbol(symbol) => browser_store::SpaceIcon::Symbol(symbol.slug().to_owned()),
            SpaceIcon::Emoji(emoji) => browser_store::SpaceIcon::Emoji(emoji.clone()),
        }
    }

    pub fn from_store(stored: &browser_store::SpaceIcon) -> SpaceIcon {
        match stored {
            browser_store::SpaceIcon::Symbol(slug) => SpaceIcon::Symbol(Symbol::of_slug(slug)),
            browser_store::SpaceIcon::Emoji(emoji) => SpaceIcon::Emoji(emoji.clone()),
        }
    }
}

pub const EMOJI: [&str; 12] = [
    "🏠", "💼", "🎮", "📚", "🎨", "✈️", "🛒", "🎵", "🧪", "🌱", "⚽", "🍳",
];
