use music_core::{Mark, Providers};
use rikka_app::prelude::*;

pub(crate) fn mark(mark: Mark) -> IconSource {
    match mark {
        Mark::Brand(codepoint) => BrandSymbol::from_codepoint(codepoint).into(),
        Mark::Symbol(codepoint) => MaterialSymbol::from_codepoint(codepoint).into(),
    }
}

pub(crate) fn mark_of(providers: &Providers, slug: &str) -> IconSource {
    providers.find(slug).map_or_else(
        || MaterialSymbol::LIBRARY_MUSIC.into(),
        |registered| mark(registered.service().mark),
    )
}
