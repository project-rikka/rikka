use ade_scope::Diagnostic;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::notice::notice;

pub(crate) fn diagnostics_notice(title: Label, diagnostics: &[Diagnostic]) -> Option<Container> {
    if diagnostics.is_empty() {
        return None;
    }

    let palette = palette();
    let rows = column! {
        for diagnostic in diagnostics {
            let (symbol, colour) = match diagnostic.is_loaded() {
                true => (MaterialSymbol::INFO, palette.text_muted),
                false => (MaterialSymbol::ERROR, palette.warning),
            };
            column! {
                notice(symbol, diagnostic.message(), colour);
                text(diagnostic.path().display().to_string())
                    .caption()
                    .muted()
                    .no_wrap();
            }
            .gap(Space::HAIR)
            .fill_width();
        }
    }
    .gap(Space::SNUG)
    .fill_width();

    Some(section(title, rows))
}
